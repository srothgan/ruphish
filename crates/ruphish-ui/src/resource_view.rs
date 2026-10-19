//! Read-only resource pages, distinct from the create/edit draft lifecycle.
use super::{Api, Family, ResourceAccess, download};
use crate::components::{DataTable, ManagementPage, PageHeader};
use crate::feedback::Message;
use leptos::{prelude::*, task::spawn_local_scoped_with_cancellation as spawn_local};
use ruphish_ui::preview_document;
use serde_json::Value;

pub(super) fn show(api: Api, token: String, family: Family, id: String) -> impl IntoView {
    let data = RwSignal::new(None::<Value>);
    let message = Message::new(String::new());
    let loading = RwSignal::new(true);
    let can_edit = use_context::<ResourceAccess>().is_some_and(|access| access.0);
    let reload = RwSignal::new(0u64);
    Effect::new(move |_| {
        reload.get();
        loading.set(true);
        message.clear();
        let id = id.clone();
        let token = token.clone();
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
                Ok((value, _)) => data.set(Some(value)),
                Err(error) => message.error(error),
            }
            loading.set(false);
        });
    });
    view! {<ManagementPage>
        <a class="breadcrumb" href=format!("/{}",family.route())>{format!("Back to {}",family.title().to_lowercase())}</a>
        {message.view()}
        <Show when=move||!loading.get()&&data.with(Option::is_none)><button on:click=move |_|reload.update(|n|*n+=1)>"Retry loading resource"</button></Show>
        <Show when=move||loading.get()><p role="status">"Loading resource..."</p></Show>
        {move||data.get().map(|value| {
            let id=value["id"].as_i64().unwrap_or(0);
            let name=text(&value,"name");
            let export=value.clone();
            let export_name=name.clone();
            view! {<PageHeader title=name subtitle=format!("Last updated: {}",text(&value,"modified_date"))>
                <a hidden=!can_edit class="button primary" href=format!("/{}/{id}/edit",family.route())>"Edit"</a>
                <a hidden=!can_edit class="button" href=format!("/{}/new?duplicate={id}",family.route())>"Duplicate"</a>
                <button on:click=move |_|download(&export,&export_name,message)>"Export JSON"</button>
            </PageHeader><div class="resource-read-panel">{body(value,family)}</div>}
        })}
    </ManagementPage>}
}

fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or("").to_owned()
}
fn field(label: &'static str, value: String) -> impl IntoView {
    view! {<div><dt>{label}</dt><dd>{if value.is_empty(){"Not set".to_owned()}else{value}}</dd></div>}
}
fn content(value: &Value) -> impl IntoView + use<> {
    let html = text(value, "html");
    let plain = text(value, "text");
    view! {<section class="resource-section"><h2>"Content"</h2>
        {(!html.is_empty()).then(||view!{<p>"Saved HTML. Template expressions are shown as authored; scripts and external resources are blocked."</p><iframe class="saved-content-preview" title="Saved content preview" sandbox="" referrerpolicy="no-referrer" srcdoc=preview_document(&html)></iframe><details><summary>"HTML source"</summary><pre>{html}</pre></details>})}
        {(!plain.is_empty()).then(||view!{<h3>"Plain text"</h3><pre>{plain}</pre>})}
    </section>}
}
fn body(value: Value, family: Family) -> AnyView {
    match family {
        Family::Groups => {
            let rows=value["targets"].as_array().cloned().unwrap_or_default();
            view!{<section><h2>{format!("Recipients ({})",rows.len())}</h2><DataTable label="Recipients" headers=vec!["Email","First name","Last name","Position"]>
            {rows.into_iter().map(|row|view!{<tr><th scope="row">{text(&row,"email")}</th><td>{text(&row,"first_name")}</td><td>{text(&row,"last_name")}</td><td>{text(&row,"position")}</td></tr>}).collect_view()}
            </DataTable></section>}.into_any()
        }
        Family::Templates => {
            let attachments=value["attachments"].as_array().cloned().unwrap_or_default();
            view!{<dl class="resource-facts">{field("Subject",text(&value,"subject"))}{field("Visible From",if text(&value,"envelope_sender").is_empty(){"Sending-profile address".into()}else{text(&value,"envelope_sender")})}</dl>
            {content(&value)}<section class="resource-section"><h2>{format!("Attachments ({})",attachments.len())}</h2><ul>{attachments.into_iter().map(|a|view!{<li>{text(&a,"name")}" · "{text(&a,"type")}</li>}).collect_view()}</ul></section>}.into_any()
        }
        Family::Pages => view!{<section class="resource-section"><h2>"Optional campaign defaults"</h2><p>"Suggested submission settings for campaigns using this page."</p><dl class="resource-facts">{field("Record form submission",yes_no(&value,"capture_credentials"))}{field("Record password presence",yes_no(&value,"capture_passwords"))}{field("Redirect after submission",text(&value,"redirect_url"))}</dl></section>{content(&value)}}.into_any(),
        Family::Profiles => {
            let headers=value["headers"].as_array().cloned().unwrap_or_default();
            view!{<dl class="resource-facts">{field("SMTP host",text(&value,"host"))}{field("Sender address",text(&value,"from_address"))}{field("Username",text(&value,"username"))}{field("Password",if text(&value,"password").is_empty(){"Not configured".into()}else{"Configured".into()})}{field("Ignore certificate errors",yes_no(&value,"ignore_cert_errors"))}</dl><section class="resource-section"><h2>"Additional headers"</h2><dl class="resource-facts">{headers.into_iter().map(|row|view!{<div><dt>{text(&row,"key")}</dt><dd>{text(&row,"value")}</dd></div>}).collect_view()}</dl></section>}.into_any()
        }
    }
}
fn yes_no(value: &Value, key: &str) -> String {
    if value[key].as_bool().unwrap_or(false) {
        "Yes"
    } else {
        "No"
    }
    .into()
}
