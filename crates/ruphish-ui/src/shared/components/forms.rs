use leptos::prelude::*;

#[component]
pub(crate) fn FormFields(
    #[prop(into)] disabled: Signal<bool>,
    children: Children,
) -> impl IntoView {
    view! { <fieldset class="management-form" disabled=move || disabled.get()>{children()}</fieldset> }
}

#[component]
pub(crate) fn Field(
    #[prop(into)] label: String,
    #[prop(default = false)] checkbox: bool,
    children: Children,
) -> impl IntoView {
    view! { <label class="form-field" class:checkbox-label=checkbox>{label}{children()}</label> }
}

#[component]
pub(crate) fn SecretField(
    label: &'static str,
    saved_hint: &'static str,
    #[prop(into)] has_saved_value: Signal<bool>,
    #[prop(into)] value: Signal<String>,
    on_input: Callback<String>,
) -> impl IntoView {
    view! {
        <Field label=label>
            <input type="password" autocomplete="new-password"
                required=move || !has_saved_value.get()
                prop:value=move || value.get()
                on:input=move |event| on_input.run(event_target_value(&event))/>
            <Show when=move || has_saved_value.get()><small>{saved_hint}</small></Show>
        </Field>
    }
}
