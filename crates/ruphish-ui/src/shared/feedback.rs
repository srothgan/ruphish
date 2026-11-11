//! Shared feedback keeps message severity and accessible presentation together.
//! Errors are assertive and red with an alert glyph; confirmations are polite and
//! teal with a check. Separate persistent live regions retain their roles.
use crate::{
    shared::components::{ActionButton, ButtonStyle, ControlSize},
    trs,
};
use leptos::leptos_dom::helpers::TimeoutHandle;
use leptos::prelude::*;

const SUCCESS_DURATION: std::time::Duration = std::time::Duration::from_secs(5);

const ERROR_ICON: &str = "<svg viewBox=\"0 0 24 24\" width=\"18\" height=\"18\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><circle cx=\"12\" cy=\"12\" r=\"9\"/><path d=\"M12 8v4\"/><path d=\"M12 16h.01\"/></svg>";
const SUCCESS_ICON: &str = "<svg viewBox=\"0 0 24 24\" width=\"18\" height=\"18\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><circle cx=\"12\" cy=\"12\" r=\"9\"/><path d=\"M8.5 12.5l2.5 2.5 4.5-5\"/></svg>";

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum FeedbackTone {
    #[default]
    Error,
    Warning,
    Success,
}

#[derive(Clone, Copy)]
pub(crate) struct Message(
    RwSignal<(String, bool)>,
    StoredValue<Option<TimeoutHandle>>,
    #[cfg(feature = "demo")] leptos_i18n::I18nContext<crate::localization::Locale>,
);

impl Message {
    pub(crate) fn new(text: String) -> Self {
        let timer = StoredValue::new(None::<TimeoutHandle>);
        on_cleanup(move || {
            timer.update_value(|timer| {
                if let Some(timer) = timer.take() {
                    timer.clear();
                }
            });
        });
        Self(
            RwSignal::new((text, true)),
            timer,
            #[cfg(feature = "demo")]
            crate::localization::use_i18n(),
        )
    }

    pub(crate) fn error(self, text: String) {
        self.publish(text, true);
    }

    pub(crate) fn status(self, text: String) {
        #[cfg(feature = "demo")]
        let text = if text.is_empty() {
            text
        } else {
            leptos_i18n::t_string!(self.2, demo.status).to_string()
        };
        self.publish(text, false);
    }

    pub(crate) fn clear(self) {
        self.publish(String::new(), false);
    }

    /// Editing a new draft invalidates an earlier success, but does not hide a failure.
    pub(crate) fn clear_status(self) {
        if self.0.with_untracked(|value| !value.1) {
            self.clear();
        }
    }

    /// Preserve caller wording for browser-local results and explicit demo notices.
    pub(crate) fn local_status(self, text: String) {
        self.publish(text, false);
    }

    fn publish(self, text: String, error: bool) {
        self.1.update_value(|timer| {
            if let Some(timer) = timer.take() {
                timer.clear();
            }
        });
        let expires = !error && !text.is_empty();
        self.0.set((text, error));
        if expires {
            let timer = set_timeout_with_handle(move || self.clear(), SUCCESS_DURATION).ok();
            self.1.set_value(timer);
        }
    }

    #[cfg(feature = "demo")]
    pub(crate) fn is_empty(self) -> bool {
        self.0.with(|value| value.0.is_empty())
    }

    pub(crate) fn view(self) -> impl IntoView {
        [true, false]
            .into_iter()
            .map(move |error| {
                let text = move || {
                    self.0.with(|value| {
                        if value.1 == error {
                            value.0.clone()
                        } else {
                            String::new()
                        }
                    })
                };
                view! {
                    <FeedbackRegion
                        text=Signal::derive(text)
                        tone=if error { FeedbackTone::Error } else { FeedbackTone::Success }
                        on_dismiss=Callback::new(move |()| self.clear())
                    />
                }
            })
            .collect_view()
    }
}

/// Dismissing feedback changes only its presentation, never the operation's outcome.
/// Typed feature state can retain its failed result while this view hides the notice.
#[component]
pub(crate) fn FeedbackRegion(
    text: Signal<String>,
    #[prop(default = FeedbackTone::Error)] tone: FeedbackTone,
    #[prop(optional)] on_dismiss: Option<Callback<()>>,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    let text = Memo::new(move |_| text.get());
    let dismissed = RwSignal::new(false);
    Effect::new(move |_| {
        text.track();
        dismissed.set(false);
    });
    view! {
        <div
            class="form-message feedback-region"
            class:error=tone == FeedbackTone::Error
            class:warning=tone == FeedbackTone::Warning
            class:success=tone == FeedbackTone::Success
            class:has-content=move || !text.get().is_empty() && !dismissed.get()
        >
            <span
                class="form-message-icon"
                aria-hidden="true"
                inner_html=if tone == FeedbackTone::Success { SUCCESS_ICON } else { ERROR_ICON }
            ></span>
            <div
                class="form-message-text"
                role=if tone == FeedbackTone::Error { "alert" } else { "status" }
                aria-atomic="true"
            >
                {move || if dismissed.get() { String::new() } else { text.get() }}
                {children
                    .map(|children| {
                        view! { <div hidden=move || dismissed.get()>{children()}</div> }
                    })}
            </div>
            <Show when=move || !text.get().is_empty() && !dismissed.get()>
                <ActionButton
                    style=ButtonStyle::Ghost
                    size=ControlSize::Small
                    title=trs!(ui.dismiss_message)
                    aria_label=trs!(ui.dismiss_message)
                    on_click=move |_| {
                        dismissed.set(true);
                        if let Some(on_dismiss) = on_dismiss {
                            on_dismiss.run(());
                        }
                    }
                >
                    <svg
                        aria-hidden="true"
                        viewBox="0 0 24 24"
                        width="18"
                        height="18"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                        stroke-linecap="round"
                    >
                        <path d="m6 6 12 12M6 18 18 6" />
                    </svg>
                </ActionButton>
            </Show>
        </div>
    }
}
