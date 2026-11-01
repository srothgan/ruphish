//! Read-only attribution; resource editors never own these values.
use leptos::prelude::*;
use ruphish_contracts::attribution::{Attribution, UserReference};
use ruphish_ui::tr;

#[component]
pub(crate) fn AttributionInfo(#[prop(into)] value: Signal<Attribution>) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    let name = move |user: Option<UserReference>| -> String {
        let Some(user) = user else { return "-".into() };
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
    view! {
        <dl class="resource-facts">
            <div>
                <dt>{tr!(ui.attribution_creator)}</dt>
                <dd>{move || name(value.get().created_by)}</dd>
            </div>
            <div>
                <dt>{tr!(ui.attribution_editor)}</dt>
                <dd>{move || name(value.get().edited_by)}</dd>
            </div>
            <div>
                <dt>{tr!(ui.attribution_edited_at)}</dt>
                <dd>
                    {move || {
                        value
                            .get()
                            .edited_at
                            .map_or_else(
                                || "-".to_owned(),
                                |at| crate::time_display::instant(&serde_json::json!(at)),
                            )
                    }}
                </dd>
            </div>
        </dl>
    }
}
