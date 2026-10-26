//! Route-independent demo feedback. Payloads contain message kinds only.
use super::{ActionButton, ButtonStyle, ControlSize};
use leptos::prelude::*;
use ruphish_ui::{tr, trs};

#[component]
pub(crate) fn DemoBanner() -> impl IntoView {
    let visible = RwSignal::new(true);
    view! {
        <Show when=move || visible.get()>
            <aside class="demo-notice" role="note">
                <div class="demo-notice-content">
                    <span>{tr!(demo.notice)}</span>
                    <a href="https://www.ruphish.com/" rel="external">
                        {tr!(demo.website)}
                    </a>
                </div>
                <ActionButton
                    style=ButtonStyle::Ghost
                    size=ControlSize::Small
                    title=trs!(demo.dismiss)
                    aria_label=trs!(demo.dismiss)
                    on_click=move |_| visible.set(false)
                >
                    <span aria-hidden="true">"×"</span>
                </ActionButton>
            </aside>
        </Show>
    }
}

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
