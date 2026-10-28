//! Production leptos-fetch adapter under controlled request completion order.
use leptos::prelude::*;
use leptos_fetch::{QueryClient, QueryOptions};
use ruphish_ui::api_error::ApiError;
use ruphish_ui::query::Query;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen::prelude::wasm_bindgen(
    inline_js = "export function bounded(p) { return new Promise((resolve, reject) => { const timer = setTimeout(() => reject(new Error('Query condition did not complete')), 5000); p.then(v => { clearTimeout(timer); resolve(v); }, e => { clearTimeout(timer); reject(e); }); }); }"
)]
extern "C" {
    fn bounded(p: &js_sys::Promise) -> js_sys::Promise;
}

#[derive(Clone)]
struct Gate {
    promise: js_sys::Promise,
    resolve: js_sys::Function,
}
impl Gate {
    fn new() -> Self {
        let mut resolve = None;
        let promise = js_sys::Promise::new(&mut |yes, _| resolve = Some(yes));
        Self {
            promise,
            resolve: resolve.unwrap(),
        }
    }
    fn release(&self) {
        self.resolve.call0(&JsValue::NULL).unwrap();
    }
    async fn wait(&self) {
        JsFuture::from(bounded(&self.promise)).await.unwrap();
    }
}
async fn until(owner: &Owner, condition: impl Fn() -> bool + 'static) {
    let gate = Gate::new();
    let wake = gate.clone();
    let effect = owner.with(|| {
        Effect::new(move |_| {
            if condition() {
                wake.release();
            }
        })
    });
    gate.wait().await;
    effect.stop();
}
fn client() -> QueryClient {
    QueryClient::new().with_options(QueryOptions::new().with_gc_time(Duration::MAX))
}

#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
let original, now, intervals, serial;
export function startPollClock() {
  globalThis.Window ??= class { static [Symbol.hasInstance](value) { return value === globalThis; } };
  original = { now: Date.now, set: globalThis.setInterval, clear: globalThis.clearInterval };
  now = 1000000; intervals = new Map(); serial = 0;
  Date.now = () => now;
  globalThis.setInterval = (callback, delay) => { if (delay !== 1000) throw new Error('Unexpected polling interval'); intervals.set(++serial, callback); return serial; };
  globalThis.clearInterval = id => intervals.delete(id);
}
export function advancePollClock(ms) { now += ms; for (const callback of [...intervals.values()]) callback(); }
export function pollTimerCount() { return intervals.size; }
export function restorePollClock() { Date.now = original.now; globalThis.setInterval = original.set; globalThis.clearInterval = original.clear; }
"#)]
extern "C" {
    fn startPollClock();
    fn advancePollClock(ms: u32);
    fn pollTimerCount() -> usize;
    fn restorePollClock();
}
struct PollClock;
impl Drop for PollClock {
    fn drop(&mut self) {
        restorePollClock();
    }
}

#[wasm_bindgen_test]
async fn periodic_reads_share_work_back_off_preserve_completed_data_and_release_navigation_timers()
{
    super::locale_window::install();
    let _ = any_spawner::Executor::init_wasm_bindgen();
    startPollClock();
    let _clock = PollClock;
    let app = Owner::new();
    let first_page = app.child();
    let second_page = app.child();
    let calls = Rc::new(Cell::new(0));
    let status = Rc::new(Cell::new(200));
    let release = Gate::new();
    let started = Gate::new();
    let hold = Rc::new(Cell::new(false));
    let (query, visible) = app.with(|| {
        let (calls, status, hold, release, started) = (calls.clone(), status.clone(), hold.clone(), release.clone(), started.clone());
        (Query::new(client(), move |key: (u8, String)| {
            calls.set(calls.get() + 1);
            let (count, status, hold, release, started) = (calls.get(), status.get(), hold.get(), release.clone(), started.clone());
            async move {
                if hold { started.release(); release.wait().await; }
                if status != 200 { return Err(ApiError::http(status, "TEST_FAILURE".into(), "Temporary".into())); }
                Ok(serde_json::json!({"id":key.0,"filter":key.1,"status":"Completed","revision":count}))
            }
        }), RwSignal::new(true))
    });
    let first = first_page.with(|| {
        query.observe_polling(
            || (7, "sort=opened&order=desc&offset=50&recipients=9".into()),
            visible.into(),
        )
    });
    let second = second_page.with(|| {
        query.observe_polling(
            || (7, "sort=opened&order=desc&offset=50&recipients=9".into()),
            visible.into(),
        )
    });
    until(&app, move || {
        first.data.get().is_some() && second.data.get().is_some() && !first.fetching.get()
    })
    .await;
    assert_eq!(calls.get(), 1);
    advancePollClock(4999);
    any_spawner::Executor::tick().await;
    assert_eq!(calls.get(), 1);
    advancePollClock(1);
    until(&app, move || {
        first.data.get().is_some_and(|data| data["revision"] == 2) && !first.fetching.get()
    })
    .await;
    assert_eq!(calls.get(), 2, "two observers produce one periodic read");
    assert_eq!(
        second.data.get_untracked().unwrap()["filter"],
        "sort=opened&order=desc&offset=50&recipients=9"
    );
    visible.set(false);
    advancePollClock(5000);
    any_spawner::Executor::tick().await;
    assert_eq!(calls.get(), 2, "hidden views do not poll");
    visible.set(true);
    hold.set(true);
    advancePollClock(1);
    started.wait().await;
    advancePollClock(160000);
    any_spawner::Executor::tick().await;
    assert_eq!(
        calls.get(),
        3,
        "a slow read is neither restarted nor duplicated"
    );
    release.release();
    hold.set(false);
    until(&app, move || {
        !first.fetching.get() && first.data.get().is_some_and(|data| data["revision"] == 3)
    })
    .await;
    status.set(503);
    advancePollClock(5000);
    until(&app, move || {
        !first.fetching.get() && first.failure.get().is_some()
    })
    .await;
    assert_eq!(first.data.get_untracked().unwrap()["revision"], 3);
    for delay in [10000, 20000, 40000, 80000, 160000, 160000] {
        let previous = calls.get();
        advancePollClock(delay - 1);
        any_spawner::Executor::tick().await;
        assert_eq!(calls.get(), previous, "backoff must not fire early");
        advancePollClock(1);
        let watched = calls.clone();
        until(&app, move || {
            !first.fetching.get() && watched.get() == previous + 1
        })
        .await;
        assert_eq!(first.data.get_untracked().unwrap()["revision"], 3);
    }
    status.set(200);
    advancePollClock(160000);
    until(&app, move || {
        !first.fetching.get() && first.failure.get().is_none()
    })
    .await;
    let recovered = calls.get();
    advancePollClock(5000);
    until(&app, move || {
        !first.fetching.get()
            && first
                .data
                .get()
                .is_some_and(|data| data["revision"] == recovered + 1)
    })
    .await;
    status.set(403);
    advancePollClock(5000);
    until(&app, move || {
        !first.fetching.get() && first.failure.get().is_some_and(|error| error.status == 403)
    })
    .await;
    let forbidden = calls.get();
    assert!(first.data.get_untracked().is_none());
    advancePollClock(1000000);
    any_spawner::Executor::tick().await;
    assert_eq!(
        calls.get(),
        forbidden,
        "terminal errors stop periodic reads"
    );
    first_page.cleanup();
    assert_eq!(pollTimerCount(), 1);
    second_page.cleanup();
    assert_eq!(pollTimerCount(), 0);
    advancePollClock(1000000);
    assert_eq!(calls.get(), forbidden);
    app.cleanup();
}

#[wasm_bindgen_test]
async fn polling_navigation_keeps_selection_and_shares_pending_selector_reads() {
    super::locale_window::install();
    let _ = any_spawner::Executor::init_wasm_bindgen();
    startPollClock();
    let _clock = PollClock;
    let app = Owner::new();
    let page = app.child();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let release = Gate::new();
    let started = Gate::new();
    let (query, selected) = app.with(|| {
        let (calls, release, started) = (calls.clone(), release.clone(), started.clone());
        (
            Query::new(client(), move |key: u8| {
                calls.borrow_mut().push(key);
                let release = release.clone();
                let started = started.clone();
                async move {
                    if key == 1 {
                        started.release();
                        release.wait().await;
                    }
                    Ok(key)
                }
            }),
            RwSignal::new(1u8),
        )
    });
    let view = page.with(|| query.observe_polling(move || selected.get(), Signal::derive(|| true)));
    started.wait().await;
    let selector_done = Gate::new();
    let done = selector_done.clone();
    app.with(|| {
        leptos::task::spawn_local(async move {
            let result = query.fetch(1).await;
            assert_eq!(result.data, Some(1));
            assert!(result.error.is_none());
            done.release();
        })
    });
    any_spawner::Executor::tick().await;
    assert_eq!(
        *calls.borrow(),
        [1],
        "selector shares the pending analysis read"
    );
    selected.set(2);
    until(&app, move || {
        view.data.get() == Some(2) && !view.fetching.get()
    })
    .await;
    assert_eq!(pollTimerCount(), 1, "old selection releases its timer");
    release.release();
    selector_done.wait().await;
    assert_eq!(
        view.data.get_untracked(),
        Some(2),
        "old response cannot replace the current selection"
    );
    advancePollClock(5000);
    let observed = calls.clone();
    until(&app, move || {
        !view.fetching.get() && observed.borrow().as_slice() == [1, 2, 2]
    })
    .await;
    page.cleanup();
    assert_eq!(pollTimerCount(), 0);
    advancePollClock(1000000);
    assert_eq!(*calls.borrow(), [1, 2, 2]);
    app.cleanup();
}

#[wasm_bindgen_test]
async fn missing_and_forbidden_refreshes_remove_stale_records_and_recover_on_success() {
    super::locale_window::install();
    let _ = any_spawner::Executor::init_wasm_bindgen();
    let owner = Owner::new();
    let status = Rc::new(Cell::new(200u16));
    let (query, view, other) = owner.with(|| {
        let status = status.clone();
        let query = Query::new(client(), move |key: u8| {
            let status = if key == 1 { status.get() } else { 200 };
            async move {
                ruphish_ui::api_error::response(
                    status,
                    if status == 200 {
                        Ok(serde_json::json!({"id":key,"name":"saved"}))
                    } else {
                        Err("non-JSON proxy rejection".into())
                    },
                    str::to_owned,
                )
            }
        });
        (query, query.observe(|| 1), query.observe(|| 2))
    });
    until(&owner, move || {
        view.data.get().is_some() && other.data.get().is_some()
    })
    .await;
    for rejection in [404, 403, 401] {
        // A retryable failure must preserve the saved record and allow recovery.
        status.set(503);
        query.invalidate(1);
        until(&owner, move || {
            !view.fetching.get()
                && view
                    .failure
                    .with(|e| e.as_ref().is_some_and(|e| e.status == 503))
        })
        .await;
        assert_eq!(view.data.get_untracked().unwrap()["name"], "saved");
        status.set(rejection);
        view.refresh.run(());
        until(&owner, move || {
            !view.fetching.get()
                && view
                    .failure
                    .with(|e| e.as_ref().is_some_and(|e| e.status == rejection))
        })
        .await;
        assert!(
            view.data.get_untracked().is_none(),
            "a terminal rejection invalidates the old record"
        );
        assert!(
            other.data.get_untracked().is_some(),
            "other records remain usable"
        );
        assert!(other.failure.get_untracked().is_none());
        let revisited = owner.with(|| query.observe(|| 1));
        assert!(
            revisited.data.get_untracked().is_none(),
            "revisiting cannot resurrect cached data"
        );
        // Going offline after deletion/permission loss cannot resurrect it either.
        status.set(503);
        query.invalidate(1);
        until(&owner, move || {
            !view.fetching.get()
                && view
                    .failure
                    .with(|e| e.as_ref().is_some_and(|e| e.status == 503))
        })
        .await;
        assert!(view.data.get_untracked().is_none());
        status.set(200);
        view.refresh.run(());
        until(&owner, move || {
            !view.fetching.get() && view.data.get().is_some() && view.failure.get().is_none()
        })
        .await;
        assert_eq!(view.data.get_untracked().unwrap()["id"], 1);
    }
    owner.cleanup();
}

#[wasm_bindgen_test]
async fn initial_missing_record_does_not_contaminate_navigation_to_a_valid_record() {
    super::locale_window::install();
    let _ = any_spawner::Executor::init_wasm_bindgen();
    let owner = Owner::new();
    let (selection, view) = owner.with(|| {
        let query = Query::new(client(), |id: u8| async move {
            if id == 1 {
                Err(ApiError::http(
                    404,
                    "RESOURCE_NOT_FOUND".into(),
                    "Missing".into(),
                ))
            } else {
                Ok(id)
            }
        });
        let selection = RwSignal::new(1);
        (selection, query.observe(move || selection.get()))
    });
    until(&owner, move || {
        view.failure.get().is_some() && !view.fetching.get()
    })
    .await;
    assert!(view.data.get_untracked().is_none());
    assert!(view.failure.get_untracked().unwrap().is_not_found());
    selection.set(2);
    until(&owner, move || {
        view.data.get() == Some(2) && !view.fetching.get()
    })
    .await;
    assert!(view.failure.get_untracked().is_none());
    selection.set(1);
    until(&owner, move || {
        view.failure.get().is_some() && !view.fetching.get()
    })
    .await;
    assert!(view.data.get_untracked().is_none());
    owner.cleanup();
}

#[wasm_bindgen_test]
async fn query_consumers_share_reads_and_recover_without_losing_confirmed_data() {
    super::locale_window::install();
    let _ = any_spawner::Executor::init_wasm_bindgen();
    let owner = Owner::new();
    let started = Gate::new();
    let release = Gate::new();
    let calls = Rc::new(Cell::new(0));
    let fail = Rc::new(Cell::new(false));
    let (query, a, b) = owner.with(|| {
        let (started, release, calls, fail) = (
            started.clone(),
            release.clone(),
            calls.clone(),
            fail.clone(),
        );
        let query = Query::new(client(), move |_: u8| {
            calls.set(calls.get() + 1);
            let (started, release, fail) = (started.clone(), release.clone(), fail.clone());
            async move {
                started.release();
                release.wait().await;
                if fail.get() {
                    Err(ApiError::local("offline".into()))
                } else {
                    Ok(vec![41, 42])
                }
            }
        });
        (query, query.observe(|| 1), query.observe(|| 1))
    });
    started.wait().await;
    assert_eq!(
        calls.get(),
        1,
        "same query/key must share one in-flight read"
    );
    assert!(a.data.get_untracked().is_none());
    assert!(b.data.get_untracked().is_none());
    release.release();
    until(&owner, move || {
        a.data.get() == Some(vec![41, 42]) && b.data.get() == Some(vec![41, 42])
    })
    .await;
    fail.set(true);
    query.invalidate(1);
    until(&owner, move || a.error.get().is_some() && !a.fetching.get()).await;
    assert_eq!(a.data.get_untracked(), Some(vec![41, 42]));
    assert_eq!(b.error.get_untracked(), Some("offline".into()));
    fail.set(false);
    b.refresh.run(());
    until(&owner, move || a.error.get().is_none() && !a.fetching.get()).await;
    assert_eq!(calls.get(), 3);
    assert_eq!(b.data.get_untracked(), Some(vec![41, 42]));
    owner.cleanup();
}

#[wasm_bindgen_test]
async fn confirmed_delete_cancels_old_read_and_failed_reconciliation_keeps_row_absent() {
    super::locale_window::install();
    let _ = any_spawner::Executor::init_wasm_bindgen();
    let owner = Owner::new();
    let old_started = Gate::new();
    let old_release = Gate::new();
    let new_started = Gate::new();
    let new_release = Gate::new();
    let calls = Rc::new(Cell::new(0));
    let (query, view) = owner.with(|| {
        let (old_started, old_release, new_started, new_release, calls) = (
            old_started.clone(),
            old_release.clone(),
            new_started.clone(),
            new_release.clone(),
            calls.clone(),
        );
        let query = Query::new(client(), move |_: u8| {
            let call = calls.get();
            calls.set(call + 1);
            let (old_started, old_release, new_started, new_release) = (
                old_started.clone(),
                old_release.clone(),
                new_started.clone(),
                new_release.clone(),
            );
            async move {
                match call {
                    0 => Ok(vec![1, 2]),
                    1 => {
                        old_started.release();
                        old_release.wait().await;
                        Ok(vec![1, 2])
                    }
                    _ => {
                        new_started.release();
                        new_release.wait().await;
                        Err(ApiError::local("reconciliation failed".into()))
                    }
                }
            }
        });
        (query, query.observe(|| 1))
    });
    until(&owner, move || view.data.get() == Some(vec![1, 2])).await;
    query.invalidate(1);
    old_started.wait().await;
    query.confirmed(1, |rows| rows.retain(|id| *id != 2));
    assert_eq!(view.data.get_untracked(), Some(vec![1]));
    new_started.wait().await;
    old_release.release();
    new_release.release();
    until(&owner, move || {
        view.error.get().is_some() && !view.fetching.get()
    })
    .await;
    assert_eq!(view.data.get_untracked(), Some(vec![1]));
    owner.cleanup();
}

#[wasm_bindgen_test]
async fn deleted_detail_is_not_reused_when_reconciliation_fails() {
    super::locale_window::install();
    let _ = any_spawner::Executor::init_wasm_bindgen();
    let owner = Owner::new();
    let started = Gate::new();
    let release = Gate::new();
    let calls = Rc::new(Cell::new(0));
    let (query, view) = owner.with(|| {
        let (started, release, calls) = (started.clone(), release.clone(), calls.clone());
        let query = Query::new(client(), move |_: u8| {
            let call = calls.get();
            calls.set(call + 1);
            let (started, release) = (started.clone(), release.clone());
            async move {
                match call {
                    0 => Ok("deleted detail".to_owned()),
                    1 => {
                        started.release();
                        release.wait().await;
                        Ok("obsolete detail".to_owned())
                    }
                    _ => Err(ApiError::local("not found".into())),
                }
            }
        });
        (query, query.observe(|| 1))
    });
    let presentation = owner.with(|| view.presentation(|| 1));
    until(&owner, move || presentation.data.get().is_some()).await;
    query.invalidate(1);
    started.wait().await;
    query.forget(1);
    assert!(view.data.get_untracked().is_none());
    // Do not force a presentation read between deletion and failed reconciliation.
    // Even a fast failure must not resurrect the previously displayed response.
    release.release();
    until(&owner, move || {
        !view.fetching.get() && view.error.get().is_some()
    })
    .await;
    assert!(view.data.get_untracked().is_none());
    assert!(presentation.data.get_untracked().is_none());
    let revisited = owner.with(|| query.observe(|| 1));
    assert!(revisited.data.get_untracked().is_none());
    owner.cleanup();
}

#[wasm_bindgen_test]
async fn changed_keys_and_separate_session_clients_never_share_confirmed_values() {
    super::locale_window::install();
    let _ = any_spawner::Executor::init_wasm_bindgen();
    let owner = Owner::new();
    let slow = Gate::new();
    let started = Gate::new();
    let (query, selection, view, other_query, other) = owner.with(|| {
        let (slow, started) = (slow.clone(), started.clone());
        let fetch = move |key: u8| {
            let (slow, started) = (slow.clone(), started.clone());
            async move {
                if key == 2 {
                    started.release();
                    slow.wait().await;
                }
                Ok(vec![key])
            }
        };
        let query = Query::new(client(), fetch.clone());
        let selection = RwSignal::new(1u8);
        let view = query.observe(move || selection.get());
        let other_query = Query::new(client(), fetch);
        let other = other_query.observe(|| 1);
        (query, selection, view, other_query, other)
    });
    let presentation = owner.with(|| view.presentation(move || selection.get()));
    until(&owner, move || {
        view.data.get() == Some(vec![1]) && other.data.get() == Some(vec![1])
    })
    .await;
    assert_eq!(presentation.data.get_untracked(), Some(vec![1]));
    other_query.confirmed(1, |data| data.push(99));
    assert_eq!(view.data.get_untracked(), Some(vec![1]));
    assert_eq!(other.data.get_untracked(), Some(vec![1, 99]));
    selection.set(2);
    started.wait().await;
    assert_eq!(
        view.data.get_untracked(),
        None,
        "another selection's data is not a placeholder"
    );
    assert_eq!(presentation.data.get_untracked(), Some(vec![1]));
    assert!(presentation.pending.get_untracked());
    assert!(presentation.is_previous_data.get_untracked());
    selection.set(3);
    until(&owner, move || view.data.get() == Some(vec![3])).await;
    slow.release();
    let old = owner.with(|| query.observe(|| 2));
    until(&owner, move || {
        old.data.get() == Some(vec![2]) && !old.fetching.get()
    })
    .await;
    assert_eq!(view.data.get_untracked(), Some(vec![3]));
    assert_eq!(presentation.data.get_untracked(), Some(vec![3]));
    assert!(!presentation.pending.get_untracked());
    assert!(!presentation.is_previous_data.get_untracked());
    selection.set(1);
    assert_eq!(presentation.data.get_untracked(), Some(vec![1]));
    assert!(
        !presentation.pending.get_untracked(),
        "cached selection needs no loading panel"
    );
    assert_eq!(other.data.get_untracked(), Some(vec![1]));
    owner.cleanup();
}

#[wasm_bindgen_test]
async fn app_client_outlives_removed_consumer_during_a_cached_refresh() {
    super::locale_window::install();
    let _ = any_spawner::Executor::init_wasm_bindgen();
    let app = Owner::new();
    let started = Gate::new();
    let release = Gate::new();
    let calls = Rc::new(Cell::new(0));
    let query = app.with(|| {
        let client = client();
        let (started, release, calls) = (started.clone(), release.clone(), calls.clone());
        Query::new(client, move |_: u8| {
            calls.set(calls.get() + 1);
            let first = calls.get() == 1;
            let (started, release) = (started.clone(), release.clone());
            async move {
                if first {
                    Ok(vec![10])
                } else {
                    started.release();
                    release.wait().await;
                    Ok(vec![20])
                }
            }
        })
    });
    let old_page = app.child();
    let old_view = old_page.with(|| query.observe(|| 1));
    until(&app, move || old_view.data.get() == Some(vec![10])).await;
    query.invalidate(1);
    started.wait().await;
    old_page.cleanup();
    release.release();
    let new_page = app.child();
    let view = new_page.with(|| query.observe(|| 1));
    assert_eq!(view.data.get_untracked(), Some(vec![10]));
    until(&app, move || {
        view.data.get() == Some(vec![20]) && !view.fetching.get()
    })
    .await;
    assert_eq!(view.error.get_untracked(), None);
    assert_eq!(calls.get(), 2);
    new_page.cleanup();
    any_spawner::Executor::tick().await;
    app.cleanup();
}

#[wasm_bindgen_test]
async fn deletion_reconciles_inactive_filters_and_pending_reads_without_touching_other_campaigns() {
    super::locale_window::install();
    let _ = any_spawner::Executor::init_wasm_bindgen();
    let owner = Owner::new();
    let inactive = owner.child();
    let started = Gate::new();
    let release = Gate::new();
    let phase = Rc::new(Cell::new(0));
    let query = owner.with(|| {
        let (phase, started, release) = (phase.clone(), started.clone(), release.clone());
        Query::new(client(), move |_: (u8, u8)| {
            let phase = phase.get();
            let (started, release) = (started.clone(), release.clone());
            async move {
                match phase {
                    0 => Ok(vec![1, 2]),
                    1 => {
                        started.release();
                        release.wait().await;
                        Ok(vec![1, 2])
                    }
                    _ => Err(ApiError::local("reconciliation unavailable".into())),
                }
            }
        })
    });
    let first = owner.with(|| query.observe(|| (7, 0)));
    let second = inactive.with(|| query.observe(|| (7, 1)));
    let unrelated = owner.with(|| query.observe(|| (8, 0)));
    until(&owner, move || {
        first.data.get().is_some() && second.data.get().is_some() && unrelated.data.get().is_some()
    })
    .await;
    inactive.cleanup();
    phase.set(1);
    query.invalidate((7, 0));
    started.wait().await;
    phase.set(2);
    query.confirmed_where(
        |(campaign, _)| *campaign == 7,
        |rows| rows.retain(|id| *id != 2),
    );
    assert_eq!(first.data.get_untracked(), Some(vec![1]));
    release.release();
    until(&owner, move || {
        !first.fetching.get() && first.error.get().is_some()
    })
    .await;
    let revisited = owner.with(|| query.observe(|| (7, 1)));
    until(&owner, move || {
        !revisited.fetching.get() && revisited.error.get().is_some()
    })
    .await;
    assert_eq!(revisited.data.get_untracked(), Some(vec![1]));
    assert_eq!(first.data.get_untracked(), Some(vec![1]));
    assert_eq!(unrelated.data.get_untracked(), Some(vec![1, 2]));
    assert!(unrelated.error.get_untracked().is_none());
    query.forget_where(|(campaign, _)| *campaign == 7);
    assert!(first.data.get_untracked().is_none());
    assert!(revisited.data.get_untracked().is_none());
    until(&owner, move || {
        !first.fetching.get()
            && first.error.get().is_some()
            && !revisited.fetching.get()
            && revisited.error.get().is_some()
    })
    .await;
    assert!(first.data.get_untracked().is_none());
    assert!(revisited.data.get_untracked().is_none());
    assert_eq!(unrelated.data.get_untracked(), Some(vec![1, 2]));
    owner.cleanup();
}

#[wasm_bindgen_test]
async fn completed_snapshots_stop_automatic_polling_but_allow_manual_reload() {
    super::locale_window::install();
    let _ = any_spawner::Executor::init_wasm_bindgen();
    startPollClock();
    let _clock = PollClock;
    let owner = Owner::new();
    let calls = Rc::new(Cell::new(0));
    let completed = Rc::new(Cell::new(false));
    let query = owner.with(|| {
        let calls = calls.clone();
        let completed = completed.clone();
        Query::new(client(), move |_: u8| {
            calls.set(calls.get() + 1);
            let value = completed.get();
            async move { Ok(value) }
        })
        .with_automatic_refresh(|completed| !completed)
    });
    let view = owner.with(|| query.observe_polling(|| 1, true.into()));
    until(&owner, move || view.data.get() == Some(false)).await;
    completed.set(true);
    advancePollClock(6000);
    until(&owner, move || view.data.get() == Some(true)).await;
    assert_eq!(calls.get(), 2);
    super::locale_window::focus();
    advancePollClock(60000);
    any_spawner::Executor::tick().await;
    assert_eq!(calls.get(), 2);
    view.refresh.run(());
    let count = calls.clone();
    until(&owner, move || {
        view.fetching.track();
        count.get() == 3 && !view.fetching.get()
    })
    .await;
    owner.cleanup();
    assert_eq!(pollTimerCount(), 0);
}

#[wasm_bindgen_test]
async fn refresh_indicator_tracks_every_held_request_without_clearing_confirmed_data() {
    super::locale_window::install();
    let _ = any_spawner::Executor::init_wasm_bindgen();
    let owner = Owner::new();
    let gates = Rc::new(
        (0..4)
            .map(|_| (Gate::new(), Gate::new()))
            .collect::<Vec<_>>(),
    );
    let calls = Rc::new(Cell::new(0usize));
    let (query, view, indicator) = owner.with(|| {
        let requests = gates.clone();
        let calls = calls.clone();
        let query = Query::new(client(), move |_: u8| {
            let n = calls.get();
            calls.set(n + 1);
            let (started, release) = requests[n].clone();
            async move {
                started.release();
                release.wait().await;
                Ok(n)
            }
        });
        let view = query.observe(|| 1);
        let group = ruphish_ui::query::RefreshState::provide();
        ruphish_ui::query::RefreshState::register(view.fetching);
        let fetching = group.fetching();
        let indicator = RwSignal::new(false);
        Effect::new(move |_| indicator.set(fetching.get()));
        (query, view, indicator)
    });
    for n in 0..4 {
        if n == 1 {
            view.refresh.run(());
        }
        if n == 2 {
            query.invalidate_where(|_| true);
        }
        if n == 3 {
            super::locale_window::focus();
        }
        gates[n].0.wait().await;
        any_spawner::Executor::tick().await;
        assert!(
            indicator.get_untracked(),
            "request {n} must reach the reactive loading indicator"
        );
        assert_eq!(view.data.get_untracked(), n.checked_sub(1));
        gates[n].1.release();
        until(&owner, move || {
            view.data.get() == Some(n) && !view.fetching.get()
        })
        .await;
        any_spawner::Executor::tick().await;
        assert!(!indicator.get_untracked());
    }
    owner.cleanup();
}

#[wasm_bindgen_test]
async fn presentation_and_export_readiness_follow_current_filter_without_discarding_refresh_data() {
    use ruphish_ui::{query::RefreshState, read_state::ReadState};
    super::locale_window::install();
    let _ = any_spawner::Executor::init_wasm_bindgen();
    let owner = Owner::new();
    let gates = Rc::new(
        (0..6)
            .map(|_| (Gate::new(), Gate::new()))
            .collect::<Vec<_>>(),
    );
    let calls = Rc::new(Cell::new(0usize));
    let (selection, view, displayed, state, ready) = owner.with(|| {
        let selection = RwSignal::new(1u8);
        let requests = gates.clone();
        let calls = calls.clone();
        let query = Query::new(client(), move |key: u8| {
            let n = calls.get();
            calls.set(n + 1);
            let (started, release) = requests[n].clone();
            async move {
                started.release();
                release.wait().await;
                if n == 2 || n == 4 {
                    Err(ApiError::http(503, "UNAVAILABLE".into(), "offline".into()))
                } else if n == 5 {
                    Err(ApiError::http(
                        403,
                        "FORBIDDEN".into(),
                        "access lost".into(),
                    ))
                } else {
                    Ok(if key == 1 { vec![1u8] } else { Vec::new() })
                }
            }
        });
        let group = RefreshState::provide();
        let view = query.observe(move || selection.get());
        let displayed = view.presentation(move || selection.get()).data;
        RefreshState::register_read(
            view.fetching,
            Signal::derive(move || view.data.with(Option::is_some)),
        );
        let state = Signal::derive(move || {
            ReadState::from_query(
                displayed.with(Option::is_some),
                view.fetching.get(),
                view.failure.get().is_some(),
            )
        });
        (selection, view, displayed, state, group.ready())
    });
    assert_eq!(state.get_untracked(), ReadState::Loading);
    assert!(!ready.get_untracked());
    gates[0].0.wait().await;
    gates[0].1.release();
    until(&owner, move || {
        state.get() == ReadState::Ready && ready.get()
    })
    .await;
    for n in [1, 2] {
        view.refresh.run(());
        gates[n].0.wait().await;
        assert!(state.get_untracked().shows_content());
        assert_eq!(view.data.get_untracked(), Some(vec![1]));
        assert!(ready.get_untracked());
        gates[n].1.release();
        until(&owner, move || !view.fetching.get()).await;
    }
    assert_eq!(state.get_untracked(), ReadState::Stale);
    selection.set(2);
    gates[3].0.wait().await;
    assert_eq!(state.get_untracked(), ReadState::Refreshing);
    assert_eq!(displayed.get_untracked(), Some(vec![1]));
    assert!(!ready.get_untracked());
    assert!(view.data.get_untracked().is_none());
    gates[3].1.release();
    until(&owner, move || {
        state.get() == ReadState::Ready && ready.get()
    })
    .await;
    assert_eq!(
        view.data.get_untracked(),
        Some(vec![]),
        "successful empty results are ready, never loading"
    );
    assert_eq!(displayed.get_untracked(), Some(vec![]));
    selection.set(3);
    gates[4].0.wait().await;
    assert!(state.get_untracked().shows_content());
    gates[4].1.release();
    until(&owner, move || state.get() == ReadState::Stale).await;
    assert_eq!(displayed.get_untracked(), Some(vec![]));
    assert!(!ready.get_untracked());
    selection.set(4);
    gates[5].0.wait().await;
    gates[5].1.release();
    until(&owner, move || state.get() == ReadState::Failed).await;
    assert!(
        displayed.get_untracked().is_none(),
        "access loss must discard retained presentation"
    );
    assert!(!ready.get_untracked());
    owner.cleanup();
}

#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
let originalCacheTimers, cacheTimers, cacheSerial;
export function startCacheClock() {
  originalCacheTimers = {set: globalThis.setTimeout, clear: globalThis.clearTimeout};
  cacheTimers = new Map(); cacheSerial = -1000;
  globalThis.setTimeout = (callback, delay, ...args) => {
    if (delay !== 1234) return originalCacheTimers.set(callback, delay, ...args);
    const id = cacheSerial--; cacheTimers.set(id, callback); return id;
  };
  globalThis.clearTimeout = id => {
    if (cacheTimers.delete(id)) return;
    originalCacheTimers.clear(id);
  };
}
export function expireCachedReads() {
  for (const [id, callback] of [...cacheTimers]) {
    if (cacheTimers.delete(id)) callback();
  }
}
export function restoreCacheClock() {
  globalThis.setTimeout = originalCacheTimers.set;
  globalThis.clearTimeout = originalCacheTimers.clear;
}
"#)]
extern "C" {
    fn startCacheClock();
    fn expireCachedReads();
    fn restoreCacheClock();
}
struct CacheClock;
impl Drop for CacheClock {
    fn drop(&mut self) {
        restoreCacheClock();
    }
}

#[wasm_bindgen_test]
async fn every_source_filter_retains_visible_results_when_completed_campaign_cache_entries_expire()
{
    super::locale_window::install();
    let _ = any_spawner::Executor::init_wasm_bindgen();
    startCacheClock();
    let _clock = CacheClock;
    for completed in [true, false] {
        let owner = Owner::new();
        let gates = Rc::new(
            (0..5)
                .map(|_| (Gate::new(), Gate::new()))
                .collect::<Vec<_>>(),
        );
        let calls = Rc::new(Cell::new(0usize));
        let (selection, view, displayed) = owner.with(|| {
            let selection = RwSignal::new("");
            let requests = gates.clone();
            let calls = calls.clone();
            let client = QueryClient::new()
                .with_options(QueryOptions::new().with_gc_time(Duration::from_millis(1234)));
            let query = Query::new(client, move |source: String| {
                let n = calls.get();
                calls.set(n + 1);
                let (started, release) = requests[n].clone();
                async move {
                    started.release();
                    release.wait().await;
                    Ok((completed, source))
                }
            })
            .with_automatic_refresh(|value| !value.0);
            let view = query.observe_polling(move || selection.get().to_owned(), true.into());
            let displayed = view.presentation(move || selection.get().to_owned()).data;
            (selection, view, displayed)
        });
        assert!(displayed.get_untracked().is_none());
        for (n, source) in ["", "public", "operator", "mailbox", "public"]
            .into_iter()
            .enumerate()
        {
            selection.set(source);
            gates[n].0.wait().await;
            if n > 0 {
                let previous = displayed.get_untracked();
                assert!(previous.is_some());
                expireCachedReads();
                any_spawner::Executor::tick().await;
                assert_eq!(
                    displayed.get_untracked(),
                    previous,
                    "cache expiry must not turn a filter transition into initial loading: completed={completed}, source={source}"
                );
                assert!(view.data.get_untracked().is_none());
            }
            gates[n].1.release();
            until(&owner, move || {
                displayed.get().is_some_and(|v| v.1 == source) && !view.fetching.get()
            })
            .await;
        }
        owner.cleanup();
        expireCachedReads();
    }
}

#[wasm_bindgen_test]
async fn unknown_campaign_state_waits_for_explicit_retry_and_completed_state_never_polls() {
    super::locale_window::install();
    let _ = any_spawner::Executor::init_wasm_bindgen();
    startPollClock();
    let _clock = PollClock;
    let owner = Owner::new();
    let calls = Rc::new(Cell::new(0usize));
    let view = owner.with(|| {
        let calls = calls.clone();
        let query = Query::new(client(), move |_: u8| {
            let n = calls.get();
            calls.set(n + 1);
            async move {
                if n == 0 {
                    Err(ApiError::http(503, "UNAVAILABLE".into(), "offline".into()))
                } else {
                    Ok(true)
                }
            }
        })
        .with_automatic_refresh(|completed| !completed);
        query.observe_polling(|| 1, true.into())
    });
    until(&owner, move || {
        view.error.get().is_some() && !view.fetching.get()
    })
    .await;
    advancePollClock(60000);
    super::locale_window::focus();
    any_spawner::Executor::tick().await;
    assert_eq!(
        calls.get(),
        1,
        "unknown campaign state does not enable automatic reads"
    );
    view.refresh.run(());
    until(&owner, move || {
        view.data.get() == Some(true) && !view.fetching.get()
    })
    .await;
    advancePollClock(60000);
    super::locale_window::focus();
    any_spawner::Executor::tick().await;
    assert_eq!(
        calls.get(),
        2,
        "completed campaign still permits manual retry but never polls"
    );
    owner.cleanup();
    assert_eq!(pollTimerCount(), 0);
}
