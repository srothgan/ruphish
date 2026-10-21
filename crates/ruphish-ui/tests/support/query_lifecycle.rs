//! Production leptos-fetch adapter under controlled request completion order.
use leptos::prelude::*;
use leptos_fetch::{QueryClient, QueryOptions};
use ruphish_ui::query::Query;
use std::{cell::Cell, rc::Rc, time::Duration};
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
                    Err("offline".into())
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
                        Err("reconciliation failed".into())
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
                    _ => Err("not found".into()),
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
                    _ => Err("reconciliation unavailable".into()),
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
