//! Shared read policy over leptos-fetch. The query cache is the only data owner.
use leptos::prelude::*;
use leptos_fetch::{QueryClient, QueryOptions, QueryScopeLocal};
use std::{fmt::Debug, hash::Hash, time::Duration};

/// Install at the app root: reuse reads for 30 seconds and revalidate on focus.
/// Session termination hides the shell and navigates to a new document/cache.
/// The client outlives route/authenticated-shell consumers and pending reads.
/// Do not clear during refresh: leptos-fetch 0.4.10 can panic when the old
/// cached query completes after clear. Identity changes already replace the document.
pub fn provide_client() -> QueryClient {
    let client = QueryClient::new()
        .with_options(QueryOptions::new().with_stale_time(Duration::from_secs(30)))
        .provide();
    let focus = window_event_listener(leptos::ev::focus, move |_| client.invalidate_all_queries());
    let visible = window_event_listener(leptos::ev::visibilitychange, move |_| {
        if web_sys::window()
            .and_then(|w| w.document())
            .is_some_and(|d| !d.hidden())
        {
            client.invalidate_all_queries();
        }
    });
    on_cleanup(move || {
        focus.remove();
        visible.remove();
    });
    client
}

/// One cached result: a failed refresh retains the last confirmed value.
#[derive(Clone, Debug)]
pub struct Snapshot<T> {
    /// Absent until a response has been validated successfully.
    pub data: Option<T>,
    /// The latest read failure, cleared by the next successful read.
    pub error: Option<String>,
}

fn scope<K, T, F, Fut>(client: QueryClient, fetch: F) -> QueryScopeLocal<K, Snapshot<T>>
where
    K: Clone + Hash + Debug + 'static,
    T: Clone + Debug + 'static,
    F: Fn(K) -> Fut + Clone + 'static,
    Fut: Future<Output = Result<T, String>> + 'static,
{
    QueryScopeLocal::new(move |key: K| {
        let fetch = fetch.clone();
        async move {
            match fetch(key.clone()).await {
                Ok(data) => Snapshot {
                    data: Some(data),
                    error: None,
                },
                Err(error) => Snapshot {
                    // Read at completion so confirmed mutation updates are retained.
                    // Reconstructing this scope preserves the fetcher's type identity.
                    data: client
                        .get_cached_query(scope(client, fetch), key)
                        .and_then(|s| s.data),
                    error: Some(error),
                },
            }
        }
    })
}

/// Define once per authenticated shell; share across all consumers and mutations.
pub struct Query<K: 'static, T: 'static> {
    client: QueryClient,
    scope: StoredValue<QueryScopeLocal<K, Snapshot<T>>, LocalStorage>,
}
impl<K, T> Copy for Query<K, T> {}
impl<K, T> Clone for Query<K, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K, T> Query<K, T>
where
    K: Clone + Hash + Debug + Send + Sync + 'static,
    T: Clone + Debug + Send + Sync + 'static,
{
    /// Retain typed keys and fetcher identity; never construct keys in views.
    pub fn new<F, Fut>(client: QueryClient, fetch: F) -> Self
    where
        F: Fn(K) -> Fut + Clone + 'static,
        Fut: Future<Output = Result<T, String>> + 'static,
    {
        Self {
            client,
            scope: StoredValue::new_local(scope(client, fetch)),
        }
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
            error: Signal::derive(move || value.get().and_then(|s| s.error)),
            fetching,
            refresh: Callback::new(move |()| self.invalidate(key())),
        }
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
    /// Latest read error, distinct from mutation feedback.
    pub error: Signal<Option<String>>,
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
