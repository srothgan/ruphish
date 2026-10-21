//! Real Leptos owners/effects; no browser automation or arbitrary sleeps.
use leptos::prelude::*;
use ruphish_ui::{recipient_history::History, view_tasks::ViewTasks};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::wasm_bindgen_test;

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
        JsFuture::from(self.promise.clone()).await.unwrap();
    }
}

#[wasm_bindgen_test]
async fn effect_rerun_during_an_active_view_task_cannot_strand_loading() {
    let _ = any_spawner::Executor::init_wasm_bindgen();
    let owner = Owner::new();
    let started = Gate::new();
    let response = Gate::new();
    let rerun = Gate::new();
    let done = Gate::new();
    let (refresh, loading, error) = owner.with(|| {
        let tasks = ViewTasks::default();
        let refresh = RwSignal::new(0);
        let loading = RwSignal::new(false);
        let error = RwSignal::new(None::<String>);
        let (started, response, rerun, done) = (
            started.clone(),
            response.clone(),
            rerun.clone(),
            done.clone(),
        );
        Effect::new(move |_| {
            let generation = refresh.get();
            if generation > 0 {
                rerun.release();
            }
            if loading.get_untracked() {
                return;
            }
            loading.set(true);
            let (started, response, done) = (started.clone(), response.clone(), done.clone());
            tasks.spawn(async move {
                started.release();
                response.wait().await;
                error.set(Some("controlled failure".into()));
                loading.set(false);
                done.release();
            });
        });
        (refresh, loading, error)
    });
    started.wait().await;
    refresh.set(1); // focus/poll/mutation reruns and cleans the effect while Fetch is pending
    rerun.wait().await;
    response.release();
    done.wait().await;
    assert!(!loading.get_untracked());
    assert!(error.get_untracked().is_some());
    owner.cleanup();
}

#[wasm_bindgen_test]
async fn closing_a_dialog_does_not_cancel_its_user_list_reload_or_history_retry() {
    let _ = any_spawner::Executor::init_wasm_bindgen();
    let owner = Owner::new();
    let gate = Gate::new();
    let started = Gate::new();
    let done = Gate::new();
    let (tasks, history, rows) = owner.with(|| {
        (
            ViewTasks::default(),
            RwSignal::new(History::default()),
            RwSignal::new(vec![1, 2]),
        )
    });
    history.update(History::toggle);
    let ticket = history.try_update(|h| h.begin(false)).flatten().unwrap().0;
    let dialog = owner.child();
    dialog.with(|| {
        let (gate, started, done) = (gate.clone(), started.clone(), done.clone());
        tasks.spawn(async move {
            started.release();
            gate.wait().await;
            rows.set(vec![1]);
            history.update(|h| h.finish(ticket, Ok(serde_json::json!({"events":[{"id":9,"message":"Submitted Data"}],"attempts":[]}))));
            done.release();
        });
    });
    started.wait().await;
    dialog.cleanup(); // delete confirmation or history error panel disappears
    gate.release();
    done.wait().await;
    assert_eq!(rows.get_untracked(), vec![1]);
    assert!(history.with_untracked(|h| !h.loading() && h.events.contains_key(&9)));
    owner.cleanup();
}

#[wasm_bindgen_test]
async fn leaving_the_page_cancels_pending_work_and_prevents_late_state_writes() {
    let _ = any_spawner::Executor::init_wasm_bindgen();
    let owner = Owner::new();
    let gate = Gate::new();
    let started = Gate::new();
    let dropped = Gate::new();
    let writes = Rc::new(Cell::new(0));
    struct Dropped(Gate);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.release();
        }
    }
    owner.with(|| {
        let tasks = ViewTasks::default();
        let (gate, started, dropped, writes) = (
            gate.clone(),
            started.clone(),
            dropped.clone(),
            writes.clone(),
        );
        tasks.spawn(async move {
            let _drop = Dropped(dropped);
            started.release();
            gate.wait().await;
            writes.set(writes.get() + 1);
        });
    });
    started.wait().await;
    owner.cleanup();
    dropped.wait().await;
    gate.release();
    leptos::task::tick().await;
    assert_eq!(writes.get(), 0);
}

#[wasm_bindgen_test]
fn successful_http_status_does_not_hide_broken_json() {
    use ruphish_ui::browser_http::decode;
    for body in ["", "{", "<html>upstream error</html>"] {
        assert!(decode(200, body).is_err());
    }
    assert_eq!(decode(204, "").unwrap(), serde_json::Value::Null);
    assert_eq!(decode(201, "{\"id\":7}").unwrap()["id"], 7);
}

#[wasm_bindgen::prelude::wasm_bindgen(
    inline_js = "export function transportFixture(fail) { const original = globalThis.fetch; globalThis.fetch = async () => { if(fail) throw new Error('private transport diagnostic'); return new Response('{\"id\":7}', {status:201, headers:{etag:'revision-7'}}); }; return () => {globalThis.fetch = original;}; }"
)]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = transportFixture)]
    fn transport_fixture(fail: bool) -> js_sys::Function;
}

#[wasm_bindgen_test]
async fn production_transport_crosses_wasm_boundary_and_marks_failed_writes_uncertain() {
    use ruphish_ui::browser_http::{resource_request, send};
    struct Restore(js_sys::Function);
    impl Drop for Restore {
        fn drop(&mut self) {
            self.0.call0(&JsValue::NULL).unwrap();
        }
    }
    {
        let _restore = Restore(transport_fixture(false));
        let observed = Rc::new(Cell::new(0));
        let status = observed.clone();
        let request =
            resource_request("GET", "https://admin.test/api/groups/7", "", None, None).unwrap();
        let response = send(request, move |code| status.set(code)).await.unwrap();
        assert_eq!(observed.get(), 201);
        assert_eq!(response.status, 201);
        assert_eq!(response.etag.as_deref(), Some("revision-7"));
        assert_eq!(response.body, "{\"id\":7}");
    }
    let _restore = Restore(transport_fixture(true));
    let request =
        resource_request("DELETE", "https://admin.test/api/groups/7", "", None, None).unwrap();
    let error = send(request, |_| {}).await.err().unwrap();
    assert!(error.contains("outcome is unconfirmed"));
    assert!(!error.contains("private transport diagnostic"));
}
