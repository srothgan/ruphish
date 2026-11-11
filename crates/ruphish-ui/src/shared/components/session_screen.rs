//! Full-page progress and recovery; callers own requests and navigation.
use super::{ActionButton, ButtonStyle, Card};
use leptos::prelude::*;

#[component]
pub(crate) fn LoadingSpinner() -> impl IntoView {
    view! { <span class="loading-spinner" aria-hidden="true"></span> }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum SessionScreenState {
    #[default]
    Loading,
    Failed,
}

#[component]
pub(crate) fn SessionScreen(
    #[prop(into)] title: Signal<String>,
    #[prop(into)] subtitle: Signal<String>,
    #[prop(into, optional)] state: Signal<SessionScreenState>,
    #[prop(into)] recovery_label: Signal<String>,
    recover: Callback<()>,
    #[prop(default = false)] recover_when_slow: bool,
) -> impl IntoView {
    let i18n = crate::localization::use_i18n();
    let slow = RwSignal::new(false);
    Effect::new(move |_| {
        slow.set(false);
        if state.get() == SessionScreenState::Loading {
            let timer =
                set_timeout_with_handle(move || slow.set(true), std::time::Duration::from_secs(5))
                    .ok();
            on_cleanup(move || {
                if let Some(timer) = timer {
                    timer.clear();
                }
            });
        }
    });
    let failed = move || state.get() == SessionScreenState::Failed;
    view! {
        <main class="login-screen session-screen">
            <Card class="session-card">
                <Show when=move || !failed()>
                    <LoadingSpinner />
                </Show>
                <div role=move || if failed() { "alert" } else { "status" } aria-atomic="true">
                    <h1>{move || title.get()}</h1>
                    <p class="session-subtitle">
                        {move || {
                            if slow.get() && !failed() {
                                leptos_i18n::t_string!(i18n, ui.session_taking_longer).to_owned()
                            } else {
                                subtitle.get()
                            }
                        }}
                    </p>
                </div>
                <Show when=move || failed() || (recover_when_slow && slow.get())>
                    <ActionButton style=ButtonStyle::Primary on_click=move |_| recover.run(())>
                        {move || recovery_label.get()}
                    </ActionButton>
                </Show>
            </Card>
        </main>
    }
}
