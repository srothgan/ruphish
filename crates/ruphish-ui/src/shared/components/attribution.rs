//! Read-only attribution; resource editors never own these values.
use super::{DetailField, DetailGrid, DetailSection};
use leptos::prelude::*;
use ruphish_contracts::attribution::{Attribution, UserReference};
use ruphish_ui::trs;

#[component]
pub(crate) fn AttributionInfo(#[prop(into)] value: Signal<Attribution>) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    let name = move |user: Option<UserReference>| -> String {
        let Some(user) = user else {
            return leptos_i18n::t_string!(i18n, ui.unknown).into();
        };
        let username = user.username.unwrap_or_else(|| format!("#{}", user.id));
        if user.deleted {
            format!(
                "{} ({})",
                username,
                leptos_i18n::t_string!(i18n, ui.attribution_deleted)
            )
        } else {
            username
        }
    };
    let timestamp = move |at: Option<String>| {
        at.map_or_else(
            || leptos_i18n::t_string!(i18n, ui.unknown).to_owned(),
            |at| crate::time_display::instant(&serde_json::json!(at)),
        )
    };
    view! {
        <DetailSection title=trs!(ui.record_information)>
            <DetailGrid>
                <DetailField label=trs!(
                    ui.attribution_created_at
                )>{move || timestamp(value.get().created_at)}</DetailField>
                <DetailField label=trs!(
                    ui.attribution_creator
                )>{move || name(value.get().created_by)}</DetailField>
                <DetailField label=trs!(
                    ui.attribution_edited_at
                )>{move || timestamp(value.get().edited_at)}</DetailField>
                <DetailField label=trs!(
                    ui.attribution_editor
                )>{move || name(value.get().edited_by)}</DetailField>
            </DetailGrid>
        </DetailSection>
    }
}
