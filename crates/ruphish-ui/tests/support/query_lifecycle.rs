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
    until(&owner, move || view.data.get().is_some()).await;
    query.invalidate(1);
    started.wait().await;
    query.forget(1);
    assert!(view.data.get_untracked().is_none());
    release.release();
    until(&owner, move || {
        !view.fetching.get() && view.error.get().is_some()
    })
    .await;
    assert!(view.data.get_untracked().is_none());
    let revisited = owner.with(|| query.observe(|| 1));
    assert!(revisited.data.get_untracked().is_none());
    owner.cleanup();
}

#[wasm_bindgen_test]
async fn changed_keys_and_separate_session_clients_never_share_confirmed_values() {
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
    until(&owner, move || {
        view.data.get() == Some(vec![1]) && other.data.get() == Some(vec![1])
    })
    .await;
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
    selection.set(3);
    until(&owner, move || view.data.get() == Some(vec![3])).await;
    slow.release();
    let old = owner.with(|| query.observe(|| 2));
    until(&owner, move || {
        old.data.get() == Some(vec![2]) && !old.fetching.get()
    })
    .await;
    assert_eq!(view.data.get_untracked(), Some(vec![3]));
    assert_eq!(other.data.get_untracked(), Some(vec![1]));
    owner.cleanup();
}

#[wasm_bindgen_test]
async fn app_client_outlives_removed_consumer_during_a_cached_refresh() {
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
