use super::TextInput;
use leptos::prelude::*;

#[component]
pub(crate) fn FormFields(
    #[prop(into)] disabled: Signal<bool>,
    children: Children,
) -> impl IntoView {
    view! {
        <fieldset class="management-form" disabled=move || disabled.get()>
            {children()}
        </fieldset>
    }
}

#[component]
pub(crate) fn Field(
    #[prop(into)] label: Signal<String>,
    #[prop(default = false)] checkbox: bool,
    children: Children,
) -> impl IntoView {
    view! {
        <label class="form-field" class:checkbox-label=checkbox>
            {move || label.get()}
            {children()}
        </label>
    }
}

#[component]
pub(crate) fn SecretField(
    #[prop(default = true)] required: bool,
    #[prop(into, optional)] validation_field: String,
    #[prop(into)] label: Signal<String>,
    #[prop(into)] saved_hint: Signal<String>,
    #[prop(into)] has_saved_value: Signal<bool>,
    #[prop(into)] value: Signal<String>,
    on_input: Callback<String>,
) -> impl IntoView {
    view! {
        <Field label=label>
            <TextInput
                validation_field=validation_field
                input_type="password"
                autocomplete="new-password"
                required=Signal::derive(move || required && !has_saved_value.get())
                value=Signal::derive(move || value.get())
                on_input=move |event| on_input.run(event_target_value(&event))
            />
            <Show when=move || has_saved_value.get()>
                <small>{move || saved_hint.get()}</small>
            </Show>
        </Field>
    }
}
