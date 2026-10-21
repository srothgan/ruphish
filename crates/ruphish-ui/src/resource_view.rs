//! Read-only resource pages, distinct from the create/edit draft lifecycle.
use super::{Api, Family, ResourceAccess, download};
use crate::feedback::Message;
use crate::shared::components::*;
use leptos::{prelude::*, task::spawn_local_scoped_with_cancellation as spawn_local};
use ruphish_ui::preview_document;
use ruphish_ui::{tr, trs};
use serde_json::Value;

pub(super) fn show(api: Api, token: String, family: Family, id: String) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let data = RwSignal::new(None::<Value>);
    let message = Message::new(String::new());
    let loading = RwSignal::new(true);
    let can_edit = use_context::<ResourceAccess>().is_some_and(|access| access.0);
    let reload = RwSignal::new(0u64);
    let deletion = ResourceDeletion::new();
    let revision = RwSignal::new(None::<String>);
    let token = StoredValue::new(token);
    let navigate = leptos_router::hooks::use_navigate();
    let deleted =
        Callback::new(move |_| navigate(&format!("/{}", family.route()), Default::default()));
    Effect::new(move |_| {
        reload.get();
        loading.set(true);
        message.clear();
        let id = id.clone();
        let token = token.get_value();
        spawn_local(async move {
            match api
                .resource(
                    "GET",
                    &format!("/api/{}/{id}", family.route()),
                    &token,
                    None,
                    None,
                )
                .await
            {
                Ok((value, etag)) => {
                    revision.set(etag);
                    data.set(Some(value));
                }
                Err(error) => message.error(error),
            }
            loading.set(false);
        });
    });
    view! {<ManagementPage>
        <BackLink href=format!("/{}",family.route())>{move || family.back_label()}</BackLink>
        {message.view()}
        <ResourceDeleteDialog deletion=deletion api=api token=token.get_value() on_deleted=deleted/>
        <Show when=move||!loading.get()&&data.with(Option::is_none)><ActionButton on_click=move |_|reload.update(|n|*n+=1)>{tr!(ui.retry_loading_resource)}</ActionButton></Show>
        <Show when=move||loading.get()><p role="status">{tr!(ui.loading_resource)}</p></Show>
        {move||data.get().map(|value| {
            let id=value["id"].as_i64().unwrap_or(0);
            let name=text(&value,"name");
            let export=value.clone();
            let export_name=name.clone();
            let delete_name=name.clone();
            view! {<PageHeader title=name subtitle=leptos_i18n::t_string!(i18n, messages.last_updated_arg0, arg0 = (crate::time_display::instant(&value["modified_date"])).to_string())>
                <ResourceActions edit_href=format!("/{}/{id}/edit",family.route()) can_edit=can_edit disabled=deletion.busy on_delete=Callback::new(move |()| deletion.select(DeleteTarget {id,name:delete_name.clone(),path:format!("/api/{}/{id}",family.route()),revision:revision.get_untracked(),description:leptos_i18n::t_string!(i18n, ui.this_permanently_removes_the_saved_resource_resources_used_by_unfinished_campaig)}))>
                    <ActionLink hidden=!can_edit disabled=deletion.busy href=format!("/{}/new?duplicate={id}",family.route())>{tr!(ui.duplicate)}</ActionLink>
                    <ActionButton on_click=move |_|download(&export,&export_name,message)>{tr!(ui.export_json)}</ActionButton>
                </ResourceActions>
            </PageHeader><Card class="resource-read-panel">{body(value,family)}</Card>}
        })}
    </ManagementPage>}
}

fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or("").to_owned()
}
fn field(label: Signal<String>, value: String) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    view! {<div><dt>{move || label.get()}</dt><dd>{move || if value.is_empty(){leptos_i18n::t_string!(i18n, ui.not_set).to_owned()}else{value.clone()}}</dd></div>}
}
fn content(value: &Value) -> impl IntoView + use<> {
    let html = text(value, "html");
    let plain = text(value, "text");
    view! {<section class="resource-section"><h2>{tr!(ui.content)}</h2>
        {(!html.is_empty()).then(||view!{<p>{tr!(ui.saved_html_template_expressions_are_shown_as_authored_scripts_and_external_resou)}</p><iframe class="saved-content-preview" title=trs!(ui.saved_content_preview) sandbox="" referrerpolicy="no-referrer" srcdoc=preview_document(&html)></iframe><details><summary>{tr!(ui.html_source)}</summary><pre>{html}</pre></details>})}
        {(!plain.is_empty()).then(||view!{<h3>{tr!(ui.plain_text)}</h3><pre>{plain}</pre>})}
    </section>}
}
fn body(value: Value, family: Family) -> AnyView {
    let i18n = ruphish_ui::localization::use_i18n();

    match family {
        Family::Groups => {
            let rows=value["targets"].as_array().cloned().unwrap_or_default();
            view!{<section><h2>{leptos_i18n::t!(i18n, messages.recipients_arg0, arg0 = (rows.len()).to_string())}</h2><DataTable label=trs!(ui.recipients) headers=vec![trs!(ui.email),trs!(ui.first_name),trs!(ui.last_name),trs!(ui.position)]>
            {rows.into_iter().map(|row|view!{<tr><th scope="row">{text(&row,"email")}</th><td>{text(&row,"first_name")}</td><td>{text(&row,"last_name")}</td><td>{text(&row,"position")}</td></tr>}).collect_view()}
            </DataTable></section>}.into_any()
        }
        Family::Templates => {
            let attachments=value["attachments"].as_array().cloned().unwrap_or_default();
            view!{<dl class="resource-facts">{field(trs!(ui.subject), text(&value,"subject"))}{field(trs!(ui.visible_from), if text(&value,"envelope_sender").is_empty(){leptos_i18n::t_string!(i18n, ui.sending_profile_address).into()}else{text(&value,"envelope_sender")})}</dl>
            {content(&value)}<section class="resource-section"><h2>{leptos_i18n::t!(i18n, messages.attachments_arg0, arg0 = (attachments.len()).to_string())}</h2><ul>{attachments.into_iter().map(|a|view!{<li>{text(&a,"name")}" · "{text(&a,"type")}</li>}).collect_view()}</ul></section>}.into_any()
        }
        Family::Pages => view!{<section class="resource-section"><h2>{tr!(ui.optional_campaign_defaults)}</h2><p>{tr!(ui.suggested_submission_settings_for_campaigns_using_this_page)}</p><dl class="resource-facts">{field(trs!(ui.record_form_submission), yes_no(&value,"capture_credentials"))}{field(trs!(ui.record_password_presence), yes_no(&value,"capture_passwords"))}{field(trs!(ui.redirect_after_submission), text(&value,"redirect_url"))}</dl></section>{content(&value)}}.into_any(),
        Family::Profiles => {
            let headers=value["headers"].as_array().cloned().unwrap_or_default();
            view!{<dl class="resource-facts">{field(trs!(ui.smtp_host), text(&value,"host"))}{field(trs!(ui.sender_address), text(&value,"from_address"))}{field(trs!(ui.username), text(&value,"username"))}{field(trs!(ui.password), if text(&value,"password").is_empty(){leptos_i18n::t_string!(i18n, ui.not_configured).into()}else{leptos_i18n::t_string!(i18n, ui.configured).into()})}{field(trs!(ui.ignore_certificate_errors), yes_no(&value,"ignore_cert_errors"))}</dl><section class="resource-section"><h2>{tr!(ui.additional_headers)}</h2><dl class="resource-facts">{headers.into_iter().map(|row|view!{<div><dt>{text(&row,"key")}</dt><dd>{text(&row,"value")}</dd></div>}).collect_view()}</dl></section>}.into_any()
        }
    }
}
fn yes_no(value: &Value, key: &str) -> String {
    let i18n = ruphish_ui::localization::use_i18n();

    if value[key].as_bool().unwrap_or(false) {
        leptos_i18n::t_string!(i18n, ui.yes)
    } else {
        leptos_i18n::t_string!(i18n, ui.no)
    }
    .into()
}
