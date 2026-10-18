//! Shared feedback keeps message severity and accessible presentation together.
use leptos::prelude::*;

#[derive(Clone, Copy)]
pub(crate) struct Message(RwSignal<(String, bool)>);

impl Message {
    pub(crate) fn new(text: String) -> Self {
        Self(RwSignal::new((text, true)))
    }

    pub(crate) fn error(self, text: String) {
        self.0.set((text, true));
    }

    pub(crate) fn status(self, text: String) {
        self.0.set((text, false));
    }

    pub(crate) fn clear(self) {
        self.status(String::new());
    }

    pub(crate) fn view(self) -> impl IntoView {
        view! {
            <p class="form-message" class:error=move || self.0.with(|v| v.1)
                role=move || if self.0.with(|v| v.1) { "alert" } else { "status" }
                tabindex="-1">{move || self.0.with(|v| v.0.clone())}</p>
        }
    }
}
