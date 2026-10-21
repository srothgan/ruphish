//! Native text controls. Feature code owns values and validation; these controls
//! preserve native form submission, autofill and constraint validation.
use leptos::prelude::*;

#[component]
pub(crate) fn TextInput(
    #[prop(into)] value: Signal<String>,
    #[prop(into, optional)] on_input: Option<Callback<leptos::ev::Event>>,
    #[prop(into, optional)] on_focus: Option<Callback<leptos::ev::FocusEvent>>,
    #[prop(default = "text")] input_type: &'static str,
    #[prop(default = "")] name: &'static str,
    #[prop(default = "")] autocomplete: &'static str,
    #[prop(into, optional)] id: String,
    #[prop(into, optional)] placeholder: Signal<String>,
    #[prop(into, optional)] aria_label: Signal<String>,
    #[prop(default = "")] class: &'static str,
    #[prop(default = "")] step: &'static str,
    #[prop(optional)] maxlength: Option<u32>,
    #[prop(into, optional)] required: Signal<bool>,
    #[prop(into, optional)] disabled: Signal<bool>,
    #[prop(default = false)] readonly: bool,
) -> impl IntoView {
    view! {
        <input
            class=format!("text-input {class}")
            type=input_type
            name=(!name.is_empty()).then_some(name)
            autocomplete=(!autocomplete.is_empty()).then_some(autocomplete)
            id=(!id.is_empty()).then_some(id)
            aria-label=move || (!aria_label.get().is_empty()).then(|| aria_label.get())
            placeholder=move || placeholder.get()
            step=(!step.is_empty()).then_some(step)
            maxlength=maxlength
            prop:value=move || value.get()
            required=move || required.get()
            disabled=move || disabled.get()
            readonly=readonly
            on:input=move |event| {
                if let Some(callback) = on_input {
                    callback.run(event);
                }
            }
            on:focus=move |event| {
                if let Some(callback) = on_focus {
                    callback.run(event);
                }
            }
        />
    }
}

#[component]
pub(crate) fn Textarea(
    #[prop(into)] value: Signal<String>,
    #[prop(into)] on_input: Callback<leptos::ev::Event>,
    #[prop(default = 6)] rows: u32,
    #[prop(into, optional)] disabled: Signal<bool>,
    #[prop(into, optional)] required: Signal<bool>,
) -> impl IntoView {
    view! {
        <textarea
            class="text-input"
            rows=rows
            prop:value=move || value.get()
            disabled=move || disabled.get()
            required=move || required.get()
            on:input=move |event| on_input.run(event)
        ></textarea>
    }
}
