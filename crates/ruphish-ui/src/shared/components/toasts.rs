//! Route-independent demo feedback. Payloads contain message kinds only.
use super::{ActionButton, ButtonStyle, ControlSize};
use leptos::prelude::*;
use ruphish_ui::{tr, trs};

#[derive(Clone, Copy)]
pub(crate) enum DemoNotice {
    Created,
    Saved,
    Deleted,
    External,
    Launched,
}

#[derive(Clone, Copy)]
pub(crate) struct DemoNotifications(RwSignal<Option<DemoNotice>>);

impl DemoNotifications {
    pub fn new() -> Self {
        Self(RwSignal::new(None))
    }

    pub fn show(self, notice: DemoNotice) {
        self.0.set(Some(notice));
    }
}

#[component]
pub(crate) fn DemoToastHost(notifications: DemoNotifications) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    let notice = notifications.0;
    let text = move || {
        notice.get().map(|value| {
            match value {
                DemoNotice::Created => leptos_i18n::t_string!(i18n, demo.created),
                DemoNotice::Saved => leptos_i18n::t_string!(i18n, demo.saved),
                DemoNotice::Deleted => leptos_i18n::t_string!(i18n, demo.deleted),
                DemoNotice::External => leptos_i18n::t_string!(i18n, demo.external),
                DemoNotice::Launched => leptos_i18n::t_string!(i18n, demo.launched),
            }
            .to_string()
        })
    };
    Effect::new(move |_| {
        if notice.get().is_some() {
            let timer = set_timeout_with_handle(
                move || notice.set(None),
                std::time::Duration::from_secs(10),
            )
            .ok();
            on_cleanup(move || {
                if let Some(timer) = timer {
                    timer.clear();
                }
            });
        }
    });
    view! {
        <aside class="demo-toast" class:has-content=move || notice.get().is_some()>
            <p role="status" aria-live="polite" aria-atomic="true">
                {text}
            </p>
            <Show when=move || notice.get().is_some()>
                <ActionButton
                    style=ButtonStyle::Ghost
                    size=ControlSize::Small
                    title=trs!(demo.dismiss)
                    on_click=move |_| notice.set(None)
                >
                    {tr!(demo.dismiss)}
                </ActionButton>
            </Show>
        </aside>
    }
}
