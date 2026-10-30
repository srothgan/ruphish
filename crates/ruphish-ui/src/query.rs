//! Shared server reads over leptos-fetch, with observer-local transition snapshots.
use crate::api_error::ApiError;
use leptos::prelude::*;
use leptos_fetch::{QueryClient, QueryOptions, QueryScopeLocal};
use std::{cell::RefCell, collections::HashMap, fmt::Debug, hash::Hash, rc::Rc, time::Duration};

/// Install at the app root: reuse reads for 30 seconds.
/// Session termination hides the shell and navigates to a new document/cache.
/// The client outlives route/authenticated-shell consumers and pending reads.
/// Do not clear during refresh: leptos-fetch 0.4.10 can panic when the old
/// cached query completes after clear. Identity changes already replace the document.
pub fn provide_client() -> QueryClient {
    QueryClient::new()
        .with_options(QueryOptions::new().with_stale_time(Duration::from_secs(30)))
        .provide()
}

/// One cached result: only a retryable refresh failure retains confirmed data.
#[derive(Clone, Debug)]
pub struct Snapshot<T> {
    /// Absent until a response has been validated successfully.
    pub data: Option<T>,
    /// The latest structured failure, cleared by a successful read or confirmed change.
    pub error: Option<ApiError>,
}

fn scope<K, T, F, Fut>(
    client: QueryClient,
    fetch: F,
    polls: Polls<K>,
) -> QueryScopeLocal<K, Snapshot<T>>
where
    K: Clone + Eq + Hash + Debug + 'static,
    T: Clone + Debug + 'static,
    F: Fn(K) -> Fut + Clone + 'static,
    Fut: Future<Output = Result<T, ApiError>> + 'static,
{
    QueryScopeLocal::new(move |key: K| {
        let fetch = fetch.clone();
        let polls = polls.clone();
        async move {
            let result = fetch(key.clone()).await;
            if let Some(poll) = polls.borrow_mut().get_mut(&key) {
                poll.failures = if result.is_err() {
                    (poll.failures + 1).min(5)
                } else {
                    0
                };
                poll.next = js_sys::Date::now() + 5000.0 * 2f64.powi(poll.failures as i32);
            }
            match result {
                Ok(data) => Snapshot {
                    data: Some(data),
                    error: None,
                },
                Err(error) => Snapshot {
                    // Read at completion so confirmed mutation updates are retained.
                    // Reconstructing this scope preserves the fetcher's type identity.
                    data: client
                        .get_cached_query(scope(client, fetch, polls), key)
                        .and_then(|s| s.data)
                        .filter(|_| !error.is_terminal()),
                    error: Some(error),
                },
            }
        }
    })
}

/// Define once per authenticated shell; share across all consumers and mutations.
pub struct Query<K: 'static, T: 'static> {
    client: QueryClient,
    automatic: fn(&T) -> bool,
    scope: StoredValue<QueryScopeLocal<K, Snapshot<T>>, LocalStorage>,
    polls: StoredValue<Polls<K>, LocalStorage>,
}
impl<K, T> Copy for Query<K, T> {}
impl<K, T> Clone for Query<K, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K, T> Query<K, T>
where
    K: Clone + Eq + Hash + Debug + Send + Sync + 'static,
    T: Clone + Debug + Send + Sync + 'static,
{
    /// Retain typed keys and fetcher identity; never construct keys in views.
    pub fn new<F, Fut>(client: QueryClient, fetch: F) -> Self
    where
        F: Fn(K) -> Fut + Clone + 'static,
        Fut: Future<Output = Result<T, ApiError>> + 'static,
    {
        let polls = Rc::new(RefCell::new(HashMap::new()));
        Self {
            client,
            automatic: |_| true,
            scope: StoredValue::new_local(scope(client, fetch, polls.clone())),
            polls: StoredValue::new_local(polls),
        }
    }

    /// Restrict automatic interval refreshes using the confirmed snapshot.
    /// Initial loads, explicit reloads and mutation invalidations remain available.
    pub fn with_automatic_refresh(mut self, predicate: fn(&T) -> bool) -> Self {
        self.automatic = predicate;
        self
    }

    /// Subscribe using the current key, without showing a previous key's data.
    pub fn observe(self, key: impl Fn() -> K + Clone + Send + Sync + 'static) -> QueryView<T> {
        let scope = self.scope.get_value();
        // A resource belongs to one selection. LocalResource serializes its
        // pending future, so reusing it across keys would delay a new selection
        // behind the previous request. Effect cleanup releases the old consumer;
        // leptos-fetch still owns shared reads and their cached results.
        let fetch_key = key.clone();
        let fetch_scope = scope.clone();
        Effect::new(move |_| {
            let current = fetch_key();
            self.client
                .local_resource(fetch_scope.clone(), move || current.clone());
        });
        let value = self
            .client
            .subscribe_value_local(scope.clone(), key.clone());
        let fetching = self.client.subscribe_is_fetching_local(scope, key.clone());
        QueryView {
            data: Signal::derive(move || value.get().and_then(|s| s.data)),
            error: Signal::derive(move || value.get().and_then(|s| s.error.map(|e| e.message))),
            failure: Signal::derive(move || value.get().and_then(|s| s.error)),
            fetching,
            refresh: Callback::new(move |()| self.invalidate(key())),
        }
    }

    /// Poll the visible selection without cancelling an in-flight read. Scheduling
    /// belongs to the shared query key, so multiple consumers cannot multiply reads.
    /// Each consumer releases its timer and lease when its route/key is disposed.
    pub fn observe_polling(
        self,
        key: impl Fn() -> K + Clone + Send + Sync + 'static,
        enabled: Signal<bool>,
    ) -> QueryView<T> {
        let view = self.observe(key.clone());
        Effect::new(move |_| {
            let current = key();
            let polls = self.polls.get_value();
            polls
                .borrow_mut()
                .entry(current.clone())
                .or_insert_with(|| PollSchedule {
                    consumers: 0,
                    failures: 0,
                    next: js_sys::Date::now() + 5000.0,
                })
                .consumers += 1;
            let release = current.clone();
            on_cleanup(move || {
                let polls = self.polls.get_value();
                let mut polls = polls.borrow_mut();
                if let Some(poll) = polls.get_mut(&release) {
                    poll.consumers -= 1;
                    if poll.consumers == 0 {
                        polls.remove(&release);
                    }
                }
            });
            if let Ok(timer) = set_interval_with_handle(
                move || {
                    if !enabled.get_untracked()
                        || !view
                            .data
                            .with_untracked(|v| v.as_ref().is_some_and(self.automatic))
                        || view.fetching.get_untracked()
                        || view
                            .failure
                            .with_untracked(|e| e.as_ref().is_some_and(ApiError::is_terminal))
                    {
                        return;
                    }
                    let polls = self.polls.get_value();
                    let due = {
                        let mut polls = polls.borrow_mut();
                        let Some(poll) = polls.get_mut(&current) else {
                            return;
                        };
                        let now = js_sys::Date::now();
                        if now < poll.next {
                            false
                        } else {
                            // Claim before invalidation: a second subscriber's tick cannot
                            // race the reactive fetching notification.
                            poll.next = now + 5000.0 * 2f64.powi(poll.failures as i32);
                            true
                        }
                    };
                    if due {
                        self.invalidate(current.clone());
                    }
                },
                Duration::from_secs(1),
            ) {
                on_cleanup(move || timer.clear());
            }
        });
        view
    }

    /// Read through the same cache and in-flight request as subscribed consumers.
    pub async fn fetch(self, key: K) -> Snapshot<T> {
        self.client
            .fetch_query_local(self.scope.get_value(), key)
            .await
    }

    /// Invalidation cancels obsolete in-flight reads and refetches active consumers.
    pub fn invalidate(self, key: K) {
        self.client.invalidate_query(self.scope.get_value(), key);
    }

    /// Invalidate all cached selections affected by a mutation, including inactive pages.
    pub fn invalidate_where(self, predicate: impl Fn(&K) -> bool) {
        self.client
            .invalidate_queries_with_predicate(self.scope.get_value(), predicate);
    }

    fn reconcile_where(self, predicate: impl Fn(&K) -> bool, update: impl Fn(&mut Snapshot<T>)) {
        // The crate enumerates existing keys while cancelling obsolete reads. Apply
        // updates after its cache lock is released; no parallel key registry is needed.
        let keys = std::cell::RefCell::new(Vec::new());
        self.client
            .invalidate_queries_with_predicate(self.scope.get_value(), |key| {
                if predicate(key) {
                    keys.borrow_mut().push(key.clone());
                    true
                } else {
                    false
                }
            });
        for key in keys.into_inner() {
            self.client
                .update_query(self.scope.get_value(), &key, |snapshot| {
                    if let Some(snapshot) = snapshot {
                        update(snapshot);
                    }
                });
        }
    }

    /// Reconcile a confirmed mutation across cached filters/pages without clearing entries.
    pub fn confirmed_where(self, predicate: impl Fn(&K) -> bool, update: impl Fn(&mut T)) {
        self.reconcile_where(predicate, |snapshot| {
            if let Some(data) = &mut snapshot.data {
                update(data);
            }
            snapshot.error = None;
        });
    }

    /// Make every affected detail/filter snapshot unavailable after confirmed deletion.
    pub fn forget_where(self, predicate: impl Fn(&K) -> bool) {
        self.reconcile_where(predicate, |snapshot| {
            snapshot.data = None;
            snapshot.error = None;
        });
    }

    /// A confirmed deletion makes the previous detail unusable, even if rereading fails.
    /// Keep the cache entry itself alive; clearing it during a refresh is unsafe in 0.4.10.
    pub fn forget(self, key: K) {
        self.client
            .update_query(self.scope.get_value(), &key, |snapshot| {
                if let Some(snapshot) = snapshot {
                    snapshot.data = None;
                    snapshot.error = None;
                }
            });
        self.invalidate(key);
    }

    /// Apply only a confirmed mutation, then reconcile with the server.
    pub fn confirmed(self, key: K, update: impl FnOnce(&mut T)) {
        let scope = self.scope.get_value();
        self.client.update_query(scope.clone(), &key, |snapshot| {
            if let Some(snapshot) = snapshot {
                if let Some(data) = &mut snapshot.data {
                    update(data);
                }
                snapshot.error = None;
            }
        });
        self.client.invalidate_query(scope, key);
    }
}

/// Reactive presentation inputs derived from the shared cache, without mirrored state.
pub struct QueryView<T: Send + Sync + 'static> {
    /// Last confirmed data for the current key only.
    pub data: Signal<Option<T>>,
    /// Latest read error message, distinct from mutation feedback.
    pub error: Signal<Option<String>>,
    /// Structured failure for page-level missing/access/retry presentation.
    pub failure: Signal<Option<ApiError>>,
    /// Initial or background read is in flight.
    pub fetching: Signal<bool>,
    /// Explicit retry/refresh uses the same query definition.
    pub refresh: Callback<()>,
}
impl<T: Send + Sync> Copy for QueryView<T> {}
impl<T: Send + Sync> Clone for QueryView<T> {
    fn clone(&self) -> Self {
        *self
    }
}

/// Observer-local presentation; the query cache still owns exact-selection data.
/// Previous data is never written into a new selection's cache or used for exports.
pub struct QueryPresentation<T: Send + Sync + 'static> {
    /// Last confirmed response for this view, including a successful empty response.
    pub data: Signal<Option<T>>,
    /// A new selection has no response yet, including before its request starts.
    pub pending: Signal<bool>,
    /// The visible response belongs to the preceding selection.
    pub is_previous_data: Signal<bool>,
}
impl<T: Send + Sync> Copy for QueryPresentation<T> {}
impl<T: Send + Sync> Clone for QueryPresentation<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Clone + Send + Sync + 'static> QueryView<T> {
    /// Retain the response, not a cache entry that may expire during a transition.
    /// Mount one observer per entity; changing filters/pages keeps that observer alive.
    /// Terminal errors and confirmed deletion of the displayed selection clear it.
    pub fn presentation<K>(
        self,
        key: impl Fn() -> K + Send + Sync + 'static,
    ) -> QueryPresentation<T>
    where
        K: Clone + Eq + Send + Sync + 'static,
    {
        let retained = Memo::new_owning(move |previous: Option<Option<(K, T)>>| {
            let current = key();
            let response = self.data.get();
            let failure = self.failure.get();
            if failure.as_ref().is_some_and(ApiError::is_terminal) {
                return (None, true);
            }
            if let Some(response) = response {
                return (Some((current, response)), true);
            }
            let previous = previous.flatten();
            if previous
                .as_ref()
                .is_some_and(|(shown, _)| *shown == current)
            {
                // Exact-selection data is cleared by a confirmed deletion, not a refresh.
                return (None, true);
            }
            (previous, false)
        });
        let data = Signal::derive(move || {
            retained.with(|value| value.as_ref().map(|(_, response)| response.clone()))
        });
        QueryPresentation {
            data,
            pending: Signal::derive(move || {
                self.data.with(Option::is_none)
                    && (self.fetching.get() || self.failure.with(Option::is_none))
            }),
            is_previous_data: Signal::derive(move || {
                self.data.with(Option::is_none) && data.with(Option::is_some)
            }),
        }
    }
}

#[derive(Default)]
struct PollSchedule {
    consumers: usize,
    failures: u32,
    next: f64,
}
type Polls<K> = Rc<RefCell<HashMap<K, PollSchedule>>>;

/// Sample document visibility whenever the periodic scheduler checks eligibility.
pub fn document_visible() -> Signal<bool> {
    Signal::derive(|| {
        web_sys::window()
            .and_then(|w| w.document())
            .is_some_and(|d| !d.hidden())
    })
}

struct RegisteredRead {
    id: u64,
    fetching: Signal<bool>,
    ready: Signal<bool>,
}

/// Page-level refresh feedback aggregates reads without changing content layout.
#[derive(Clone, Copy)]
pub struct RefreshState {
    reads: RwSignal<Vec<RegisteredRead>>,
    next: RwSignal<u64>,
}
impl RefreshState {
    /// Install a toolbar loading group for this page.
    pub fn provide() -> Self {
        let state = Self {
            reads: RwSignal::new(Vec::new()),
            next: RwSignal::new(0),
        };
        provide_context(state);
        state
    }
    /// Include a child read until its owner is disposed.
    pub fn register(fetching: Signal<bool>) {
        Self::register_read(fetching, true.into());
    }
    /// Register whether the current selection has a confirmed snapshot for dependent actions.
    pub fn register_read(fetching: Signal<bool>, ready: Signal<bool>) {
        if let Some(state) = use_context::<Self>() {
            let id = state.next.get_untracked();
            state.next.set(id + 1);
            state.reads.update(|reads| {
                reads.push(RegisteredRead {
                    id,
                    fetching,
                    ready,
                })
            });
            on_cleanup(move || {
                if !state.reads.is_disposed() {
                    state
                        .reads
                        .update(|reads| reads.retain(|read| read.id != id));
                }
            });
        }
    }
    /// Whether all required current selections have confirmed data.
    pub fn ready(self) -> Signal<bool> {
        Signal::derive(move || {
            self.reads
                .with(|reads| !reads.is_empty() && reads.iter().all(|read| read.ready.get()))
        })
    }
    /// Derive activity from all currently registered requests.
    pub fn fetching(self) -> Signal<bool> {
        Signal::derive(move || {
            self.reads
                .with(|reads| reads.iter().any(|read| read.fetching.get()))
        })
    }
}
