//! Shared feedback keeps message severity and accessible presentation together.
//! Errors are assertive and red with an alert glyph; confirmations are polite and
//! teal with a check. Separate persistent live regions retain their roles.
use leptos::prelude::*;

const ERROR_ICON: &str = "<svg viewBox=\"0 0 24 24\" width=\"18\" height=\"18\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><circle cx=\"12\" cy=\"12\" r=\"9\"/><path d=\"M12 8v4\"/><path d=\"M12 16h.01\"/></svg>";
const SUCCESS_ICON: &str = "<svg viewBox=\"0 0 24 24\" width=\"18\" height=\"18\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><circle cx=\"12\" cy=\"12\" r=\"9\"/><path d=\"M8.5 12.5l2.5 2.5 4.5-5\"/></svg>";

#[derive(Clone, Copy)]
pub(crate) struct Message(
    RwSignal<(String, bool)>,
    #[cfg(feature = "demo")] leptos_i18n::I18nContext<ruphish_ui::localization::Locale>,
);

impl Message {
    pub(crate) fn new(text: String) -> Self {
        Self(
            RwSignal::new((text, true)),
            #[cfg(feature = "demo")]
            ruphish_ui::localization::use_i18n(),
        )
    }

    pub(crate) fn error(self, text: String) {
        self.0.set((text, true));
    }

    pub(crate) fn status(self, text: String) {
        #[cfg(feature = "demo")]
        let text = if text.is_empty() {
            text
        } else {
            leptos_i18n::t_string!(self.1, demo.status).to_string()
        };
        self.0.set((text, false));
    }

    pub(crate) fn clear(self) {
        self.status(String::new());
    }

    /// A browser action such as copying or downloading really occurs in either build mode.
    pub(crate) fn local_status(self, text: String) {
        self.0.set((text, false));
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
                    <p
                        class="form-message feedback-region"
                        class:error=error
                        class:success=!error
                        class:has-content=move || !text().is_empty()
                        role=if error { "alert" } else { "status" }
                        aria-atomic="true"
                    >
                        <span
                            class="form-message-icon"
                            aria-hidden="true"
                            inner_html=if error { ERROR_ICON } else { SUCCESS_ICON }
                        ></span>
                        <span class="form-message-text">{text}</span>
                    </p>
                }
            })
            .collect_view()
    }
}
