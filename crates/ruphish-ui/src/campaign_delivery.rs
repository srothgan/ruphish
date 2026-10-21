//! Stable recipient rows with an independently loaded, expandable history panel.
use crate::shared::components::{ActionButton, ButtonStyle, StatusBadge, StatusTone};
use crate::{
    account::{Api, copy_text},
    campaigns::{array, text},
    time_display::instant,
};
use leptos::{prelude::*, task::spawn_local_scoped_with_cancellation as spawn_local};
use ruphish_ui::recipient_history::{History, outcome, timeline, tone};
use serde_json::Value;

#[derive(Clone, Copy)]
pub(super) struct Controls {
    pub busy: RwSignal<bool>,
    pub can_edit: bool,
    pub action: RwSignal<Option<(String, Value)>>,
}

fn fetch(
    tasks: ruphish_ui::view_tasks::ViewTasks,
    api: Api,
    campaign: StoredValue<String>,
    result: i64,
    history: RwSignal<History>,
    older: bool,
) {
    let mut request = None;
    history.update(|h| request = h.begin(older));
    let Some((request, query)) = request else {
        return;
    };
    tasks.spawn(async move {
        let response = api
            .get(&format!(
                "/api/campaigns/{}/delivery/{result}/history{query}",
                campaign.get_value()
            ))
            .await;
        history.update(|h| h.finish(request, response));
    });
}

pub(super) fn recipient_row(
    api: Api,
    campaign: StoredValue<String>,
    result: i64,
    data: RwSignal<Value>,
    controls: Controls,
) -> impl IntoView {
    let row = Memo::new(move |_| {
        array(&data.get(), "items")
            .into_iter()
            .find(|r| r["id"] == result)
            .unwrap_or(Value::Null)
    });
    let tasks = ruphish_ui::view_tasks::ViewTasks::default();
    let history = RwSignal::new(History::default());
    let open = Memo::new(move |_| history.with(|h| h.open));
    let entries = Memo::new(move |_| history.with(timeline));
    let attempts = Memo::new(move |_| history.with(|h| h.attempts.clone()));
    let copy_message = RwSignal::new(String::new());
    Effect::new(move |_| {
        data.track();
        if open.get() && history.with_untracked(|h| h.error.is_none()) {
            fetch(tasks, api, campaign, result, history, false);
        }
    });
    let panel_id = format!("recipient-history-{result}");
    let button_id = format!("recipient-toggle-{result}");
    let busy = controls.busy;
    let action = controls.action;
    view! {
        <tr class="recipient-summary" class:recipient-expanded=move||open.get()>
            <td class="recipient-disclosure"><button type="button" class="recipient-toggle" id=button_id.clone() aria-label=move||format!("Recipient history for {}",text(&row.get(),"email")) aria-expanded=move||open.get().to_string() aria-controls=panel_id.clone() on:click=move |_|history.update(History::toggle)>
                <svg viewBox="0 0 20 20" width="18" height="18" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.8"><path d="m7 4 6 6-6 6"/></svg>
            </button></td>
            <th scope="row">{move||text(&row.get(),"email")}</th>
            <td><StatusBadge tone=Signal::derive(move||status_tone(&text(&row.get(),"state")))>{move||outcome(&text(&row.get(),"state"))}</StatusBadge><small class="delivery-code">{move||text(&row.get(),"code")}</small></td>
            <td>{move||instant(&row.get()["due"])}</td>
            <td><span>{move||{let count=row.get()["attempt_count"].as_u64().unwrap_or(0);format!("{count} recorded {}",if count==1{"attempt"}else{"attempts"})}}</span>
                <div class="recipient-actions">
                <Show when=move||controls.can_edit&&matches!(text(&row.get(),"state").as_str(),"failed"|"blocked")><ActionButton disabled=Signal::derive(move||busy.get()) on_click=move |_|action.set(Some(("retry".into(),row.get_untracked())))>"Retry after correction"</ActionButton></Show>
                <Show when=move||controls.can_edit&&text(&row.get(),"state")=="unknown"><ActionButton disabled=Signal::derive(move||busy.get()) on_click=move |_|action.set(Some(("confirm_accepted".into(),row.get_untracked())))>"Confirm accepted"</ActionButton><ActionButton style=ButtonStyle::Ghost disabled=Signal::derive(move||busy.get()) on_click=move |_|action.set(Some(("resend_unknown".into(),row.get_untracked())))>"Resend despite uncertainty"</ActionButton></Show>
                </div>
            </td>
        </tr>
        <tr class="recipient-history-row" hidden=move||!open.get()><td colspan="5">
            <section id=panel_id aria-labelledby=button_id class="recipient-history" aria-busy=move||history.with(History::loading).to_string()>
                <div class="recipient-history-heading"><h3>"Recipient history"</h3><span>{move || format!("Times shown in {}", crate::time_display::zone_name())}</span></div>
                <Show when=move||history.with(History::loading)><p role="status">"Loading history…"</p></Show>
                {move||history.get().error.map(|error|view!{<div class="form-message error" role="alert"><p>{error}</p><ActionButton kind="button" on_click=move |_|fetch(tasks,api,campaign,result,history,history.get_untracked().retry_older)>"Retry history"</ActionButton></div>})}
                <div class="recipient-history-columns">
                    <section class="recipient-timeline"><h4>"Timeline"</h4><ol class="delivery-timeline">
                        {move||entries.get().into_iter().map(|entry|view!{<li class=format!("timeline-event {}",entry.tone)><span class="timeline-marker" aria-hidden="true"></span><div><strong>{entry.title}</strong><time>{entry.at.map(|at|instant(&Value::from(at))).unwrap_or_else(||"Time not recorded".into())}</time></div></li>}).collect_view()}
                        <Show when=move||matches!(text(&row.get(),"state").as_str(),"ready"|"retry")><li class="timeline-event neutral"><span class="timeline-marker pending" aria-hidden="true"></span><div><strong>{move||if text(&row.get(),"state")=="retry"{"Next retry scheduled"}else{"Scheduled to send"}}</strong><time>{move||instant(&row.get()["due"])}</time></div></li></Show>
                    </ol><Show when=move||!history.with(History::loading)&&history.with(|h|h.events.is_empty()&&h.attempts.is_empty())><p>"No attempts or recipient events recorded yet."</p></Show></section>
                    <section class="recipient-attempts"><h4>"Delivery attempts"</h4>
                        {move||attempts.get().into_values().rev().map(|attempt|attempt_details(attempt,copy_message)).collect_view()}
                        <p class="copy-status" role="status">{move||copy_message.get()}</p>
                    </section>
                </div>
                <Show when=move||history.with(|h|h.next_attempt.is_some()||h.next_event.is_some())><ActionButton kind="button" disabled=Signal::derive(move||history.with(History::loading)) on_click=move |_|fetch(tasks,api,campaign,result,history,true)>"Load earlier history"</ActionButton></Show>
            </section>
        </td></tr>
    }
}

fn attempt_details(a: Value, copied: RwSignal<String>) -> impl IntoView {
    let message_id = text(&a, "message_id");
    let copy_id = message_id.clone();
    let state = text(&a, "state");
    let stage = text(&a, "stage");
    view! {<article class="delivery-attempt"><h5>{format!("Attempt {}",a["generation"])}</h5>
        <StatusBadge tone=status_tone(&state)>{outcome(&state)}</StatusBadge>
        <dl class="attempt-facts"><div><dt>"Started"</dt><dd class="data-value">{instant(&a["started"])}</dd></div>
            {a["finished"].as_i64().map(|at|view!{<div><dt>"Finished"</dt><dd class="data-value">{instant(&Value::from(at))}</dd></div>})}
            {(!stage.is_empty()).then(||view!{<div><dt>"Protocol stage"</dt><dd>{stage}</dd></div>})}
            {a["smtp_code"].as_u64().map(|code|view!{<div><dt>"SMTP response"</dt><dd>{code}</dd></div>})}
            {(!message_id.is_empty()).then(||view!{<div class="attempt-message-id"><dt>"Message-ID"</dt><dd><code>{message_id}</code><ActionButton kind="button" on_click=move |_|{let value=copy_id.clone();spawn_local(async move {copied.set(match copy_text(&value).await {Ok(())=>"Message-ID copied.".into(),Err(_)=>"Copy unavailable. Select the Message-ID and copy it manually.".into()});});}>"Copy"</ActionButton></dd></div>})}
        </dl>
    </article>}
}

fn status_tone(state: &str) -> StatusTone {
    match tone(state) {
        "success" => StatusTone::Positive,
        "failure" => StatusTone::Negative,
        "warning" => StatusTone::Warning,
        "active" => StatusTone::Active,
        _ => StatusTone::Neutral,
    }
}
