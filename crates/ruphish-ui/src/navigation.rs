//! Shared navigation with stable sections, visible labels and decorative inline SVGs.
use crate::shared::components::Dialog;
use leptos::prelude::*;
use ruphish_ui::{tr, trs};
use wasm_bindgen::{JsCast, closure::Closure, prelude::*};

#[wasm_bindgen(module = "/web/navigation.mjs")]
extern "C" {
    #[wasm_bindgen(js_name = installNavigationGuard)]
    fn install_navigation_guard(
        dirty: &js_sys::Function,
        dialog: &web_sys::HtmlDialogElement,
    ) -> js_sys::Function;
    #[wasm_bindgen(catch, js_name = confirmDiscard)]
    async fn confirm_discard(dirty: bool) -> Result<bool, JsValue>;
}

#[derive(Clone, Copy)]
struct DraftGuard(RwSignal<Option<Memo<bool>>>);

/// Capture the shared guard while mounted, for actions such as direct sign-out.
pub(super) fn confirm_leave_action() -> impl AsyncFn() -> bool + Copy {
    let guard = use_context::<DraftGuard>();
    async move || {
        let dirty = guard
            .and_then(|g| g.0.get_untracked())
            .is_some_and(|d| d.get_untracked());
        confirm_discard(dirty).await.unwrap_or(false)
    }
}

/// One browser adapter guards links, history traversal and document unloads.
/// A terminal session invalidation bypasses it so stale authenticated content clears.
pub(super) fn install_guard(active: impl Fn() -> bool + 'static) -> impl IntoView {
    let guard = DraftGuard(RwSignal::new(None));
    provide_context(guard);
    let dirty = Closure::<dyn Fn() -> bool>::new(move || {
        active()
            && guard
                .0
                .get_untracked()
                .is_some_and(|dirty| dirty.get_untracked())
    });
    let dialog = NodeRef::<leptos::html::Dialog>::new();
    let callbacks = StoredValue::new_local((dirty, None::<js_sys::Function>));
    Effect::new(move |_| {
        if let Some(dialog) = dialog.get() {
            callbacks.update_value(|(dirty, remove)| {
                *remove = Some(install_navigation_guard(
                    dirty.as_ref().unchecked_ref(),
                    &dialog,
                ));
            });
        }
    });
    on_cleanup(move || {
        callbacks.with_value(|(_, remove)| {
            if let Some(remove) = remove {
                let _ = remove.call0(&JsValue::NULL);
            }
        })
    });
    view! { <Dialog node=dialog title=trs!(ui.discard_unsaved_changes) compact=true open_on_mount=false>
        <h2 id="discard-heading">{tr!(ui.discard_unsaved_changes)}</h2>
        <p id="discard-description">{tr!(ui.your_changes_have_not_been_saved_leaving_this_page_will_discard_them)}</p>
        <div class="confirmation-actions">
            <button type="button" data-discard="cancel" autofocus>{tr!(ui.keep_editing)}</button>
            <button type="button" class="danger" data-discard="confirm">{tr!(ui.discard_and_continue)}</button>
        </div>
    </Dialog> }
}

pub(super) fn protect_draft(dirty: impl Fn() -> bool + Send + Sync + 'static) {
    if let Some(guard) = use_context::<DraftGuard>() {
        let dirty = Memo::new(move |_| dirty());
        guard.0.set(Some(dirty));
        on_cleanup(move || {
            guard.0.try_update(|current| {
                if *current == Some(dirty) {
                    *current = None;
                }
            });
        });
    }
}

fn icon(path: &'static str) -> impl IntoView + use<> {
    view! { <svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false"><path d=path/></svg> }
}
fn link(
    path: Memo<String>,
    href: &'static str,
    label: Signal<String>,
    drawing: &'static str,
) -> impl IntoView + use<> {
    let active = move || {
        let path = path.get();
        path == href
            || path
                .strip_prefix(href)
                .is_some_and(|tail| tail.starts_with('/'))
    };
    view! { <a href=href aria-current=move || active().then_some("page")>{icon(drawing)}<span>{move || label.get()}</span></a> }
}
pub(super) fn sections(path: Memo<String>, admin: bool) -> impl IntoView + use<> {
    view! {
        {link(path, "/", trs!(ui.dashboard), "M3 3h7v7H3z M14 3h7v7h-7z M3 14h7v7H3z M14 14h7v7h-7z")}
        <section class="nav-section" aria-labelledby="nav-campaigns"><h2 id="nav-campaigns">{tr!(ui.campaigns)}</h2>
        {link(path, "/analysis", trs!(ui.analysis), "M3 3v18h18 M7 17v-5 M12 17V7 M17 17v-9")}
        {link(path, "/campaigns", trs!(ui.campaigns), "M4 21V3 M4 4c5-4 10 4 16 0v10c-6 4-11-4-16 0")}
        </section>
        <section class="nav-section" aria-labelledby="nav-resources"><h2 id="nav-resources">{tr!(ui.resources)}</h2>
        {link(path, "/domains", trs!(ui.domains), "M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0 M3 12h18 M12 3c5 5 5 13 0 18-5-5-5-13 0-18")}
        {link(path, "/templates", trs!(ui.email_templates), "M3 5h18v14H3z M3 5l9 7 9-7")}
        {link(path, "/pages", trs!(ui.landing_pages), "M3 3h18v18H3z M3 8h18 M7 5.5h.01 M11 5.5h.01 M7 12h10 M7 16h6")}
        {link(path, "/groups", trs!(ui.recipient_groups), "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2 M22 21v-2a4 4 0 0 0-3-3.87 M16 3.13a4 4 0 0 1 0 7.75 M13 7a4 4 0 1 1-8 0 4 4 0 0 1 8 0")}
        {link(path, "/reporting-mailboxes", trs!(ui.reporting_mailboxes), "M3 5h18v14H3z M3 5l9 7 9-7 M9 16h6")}
        {link(path, "/smtp", trs!(ui.sending_profiles), "M22 2 9 15 M22 2l-7 20-4-9-9-4 20-7")}
        </section>
        {admin.then(||view!{<section class="nav-section" aria-labelledby="nav-admin"><h2 id="nav-admin">{tr!(ui.administration)}</h2>
        {link(path, "/users", trs!(ui.users), "M12 3 3 7v5c0 5 9 9 9 9s9-4 9-9V7l-9-4 M9 12l2 2 4-4")}
        {link(path, "/webhooks", trs!(ui.webhooks), "M7 12a4 4 0 1 1 0-8 M17 12a4 4 0 1 1 0 8 M7 8l10 8 M7 16a4 4 0 1 1 0 4 M7 16h10")}
        </section>})}
    }
}
