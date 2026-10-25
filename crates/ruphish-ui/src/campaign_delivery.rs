//! Stable recipient rows with an independently loaded, expandable history panel.
use crate::shared::components::{ActionButton, ButtonStyle, StatusBadge, StatusTone};
use crate::{
    account::copy_text,
    campaigns::{array, text},
    client::Api,
    time_display::instant,
};
use leptos::{prelude::*, task::spawn_local_scoped_with_cancellation as spawn_local};
use ruphish_ui::recipient_history::{History, timeline, tone};
use ruphish_ui::tr;
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
            .await
            .map_err(|e| e.message)
            .and_then(|value| {
                let value: ruphish_contracts::campaign_reads::History =
                    serde_json::from_value(value).map_err(|_| api.invalid_read())?;
                if value.result_id != result {
                    return Err(api.invalid_read());
                }
                serde_json::to_value(value).map_err(|_| api.invalid_read())
            });
        history.update(|h| h.finish(request, response));
    });
}

pub(super) fn recipient_row(
    api: Api,
    campaign: StoredValue<String>,
    result: i64,
    data: Signal<Value>,
    controls: Controls,
) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

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
        <tr class="recipient-summary" class:recipient-expanded=move || open.get()>
            <td class="recipient-disclosure">
                <button
                    type="button"
                    class="recipient-toggle"
                    id=button_id.clone()
                    aria-label=move || {
                        leptos_i18n::t_string!(
                            i18n, messages.recipient_history_for_arg0, arg0 = (text(&row.get(),"email")).to_string()
                        )
                    }
                    aria-expanded=move || open.get().to_string()
                    aria-controls=panel_id.clone()
                    on:click=move |_| history.update(History::toggle)
                >
                    <svg
                        viewBox="0 0 20 20"
                        width="18"
                        height="18"
                        aria-hidden="true"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="1.8"
                    >
                        <path d="m7 4 6 6-6 6" />
                    </svg>
                </button>
            </td>
            <th scope="row">{move || text(&row.get(), "email")}</th>
            <td>
                <StatusBadge tone=Signal::derive(move || status_tone(
                    &text(&row.get(), "state"),
                ))>
                    {move || ruphish_ui::localization::delivery_label(
                        i18n,
                        &text(&row.get(), "state"),
                    )}
                </StatusBadge>
                <small class="delivery-code">{move || text(&row.get(), "code")}</small>
            </td>
            <td>{move || instant(&row.get()["due"])}</td>
            <td>
                <span>
                    {move || {
                        let count = row.get()["attempt_count"].as_u64().unwrap_or(0);
                        if count == 1 {
                            leptos_i18n::t_string!(i18n, messages.attempt_one).to_owned()
                        } else {
                            leptos_i18n::t_string!(i18n, messages.attempt_count, count = count)
                        }
                    }}
                </span>
                <div class="recipient-actions">
                    <Show when=move || {
                        controls.can_edit
                            && matches!(text(&row.get(), "state").as_str(), "failed" | "blocked")
                    }>
                        <ActionButton
                            disabled=Signal::derive(move || busy.get())
                            on_click=move |_| {
                                action.set(Some(("retry".into(), row.get_untracked())))
                            }
                        >
                            {tr!(ui.retry_after_correction)}
                        </ActionButton>
                    </Show>
                    <Show when=move || controls.can_edit && text(&row.get(), "state") == "unknown">
                        <ActionButton
                            disabled=Signal::derive(move || busy.get())
                            on_click=move |_| {
                                action.set(Some(("confirm_accepted".into(), row.get_untracked())))
                            }
                        >
                            {tr!(ui.confirm_accepted)}
                        </ActionButton>
                        <ActionButton
                            style=ButtonStyle::Ghost
                            disabled=Signal::derive(move || busy.get())
                            on_click=move |_| {
                                action.set(Some(("resend_unknown".into(), row.get_untracked())))
                            }
                        >
                            {tr!(ui.resend_despite_uncertainty)}
                        </ActionButton>
                    </Show>
                </div>
            </td>
        </tr>
        <tr class="recipient-history-row" hidden=move || !open.get()>
            <td colspan="5">
                <section
                    id=panel_id
                    aria-labelledby=button_id
                    class="recipient-history"
                    aria-busy=move || history.with(History::loading).to_string()
                >
                    <div class="recipient-history-heading">
                        <h3>{tr!(ui.recipient_history)}</h3>
                        <span>
                            {move || {
                                leptos_i18n::t_string!(
                                    i18n, messages.times_shown_in_arg0, arg0 = (crate::time_display::zone_name()).to_string()
                                )
                            }}
                        </span>
                    </div>
                    <Show when=move || history.with(History::loading)>
                        <p role="status">{tr!(ui.loading_history)}</p>
                    </Show>
                    {move || {
                        history
                            .get()
                            .error
                            .map(|error| {
                                view! {
                                    <div class="form-message error" role="alert">
                                        <p>{error}</p>
                                        <ActionButton
                                            kind="button"
                                            on_click=move |_| fetch(
                                                tasks,
                                                api,
                                                campaign,
                                                result,
                                                history,
                                                history.get_untracked().retry_older,
                                            )
                                        >
                                            {tr!(ui.retry_history)}
                                        </ActionButton>
                                    </div>
                                }
                            })
                    }}
                    <div class="recipient-history-columns">
                        <section class="recipient-timeline">
                            <h4>{tr!(ui.timeline)}</h4>
                            <ol class="delivery-timeline">
                                {move || {
                                    entries
                                        .get()
                                        .into_iter()
                                        .map(|entry| {
                                            view! {
                                                <li class=format!("timeline-event {}", entry.tone)>
                                                    <span class="timeline-marker" aria-hidden="true"></span>
                                                    <div>
                                                        <strong>
                                                            {ruphish_ui::localization::timeline_title(
                                                                i18n,
                                                                &entry.title,
                                                            )}
                                                        </strong>
                                                        <time>
                                                            {entry
                                                                .at
                                                                .map(|at| instant(&Value::from(at)))
                                                                .unwrap_or_else(|| {
                                                                    leptos_i18n::t_string!(i18n, ui.time_not_recorded).into()
                                                                })}
                                                        </time>
                                                    </div>
                                                </li>
                                            }
                                        })
                                        .collect_view()
                                }}
                                <Show when=move || {
                                    matches!(text(&row.get(), "state").as_str(), "ready" | "retry")
                                }>
                                    <li class="timeline-event neutral">
                                        <span
                                            class="timeline-marker pending"
                                            aria-hidden="true"
                                        ></span>
                                        <div>
                                            <strong>
                                                {move || {
                                                    if text(&row.get(), "state") == "retry" {
                                                        leptos_i18n::t_string!(i18n, ui.next_retry_scheduled)
                                                    } else {
                                                        leptos_i18n::t_string!(i18n, ui.scheduled_to_send)
                                                    }
                                                }}
                                            </strong>
                                            <time>{move || instant(&row.get()["due"])}</time>
                                        </div>
                                    </li>
                                </Show>
                            </ol>
                            <Show when=move || {
                                !history.with(History::loading)
                                    && history
                                        .with(|h| {
                                            h.error.is_none() && h.events.is_empty()
                                                && h.attempts.is_empty()
                                        })
                            }>
                                <p>{tr!(ui.no_attempts_or_recipient_events_recorded_yet)}</p>
                            </Show>
                        </section>
                        <section class="recipient-attempts">
                            <h4>{tr!(ui.delivery_attempts)}</h4>
                            {move || {
                                attempts
                                    .get()
                                    .into_values()
                                    .rev()
                                    .map(|attempt| attempt_details(attempt, copy_message))
                                    .collect_view()
                            }}
                            <p class="copy-status" role="status">
                                {move || copy_message.get()}
                            </p>
                        </section>
                    </div>
                    <Show when=move || {
                        history.with(|h| h.next_attempt.is_some() || h.next_event.is_some())
                    }>
                        <ActionButton
                            kind="button"
                            disabled=Signal::derive(move || history.with(History::loading))
                            on_click=move |_| fetch(tasks, api, campaign, result, history, true)
                        >
                            {tr!(ui.load_earlier_history)}
                        </ActionButton>
                    </Show>
                </section>
            </td>
        </tr>
    }
}

fn attempt_details(a: Value, copied: RwSignal<String>) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let message_id = text(&a, "message_id");
    let copy_id = message_id.clone();
    let state = text(&a, "state");
    let stage = text(&a, "stage");
    view! {
        <article class="delivery-attempt">
            <h5>
                {leptos_i18n::t!(i18n, messages.attempt_arg0, arg0 = (a["generation"]).to_string())}
            </h5>
            <StatusBadge tone=status_tone(
                &state,
            )>{move || ruphish_ui::localization::delivery_label(i18n, &state)}</StatusBadge>
            <dl class="attempt-facts">
                <div>
                    <dt>{tr!(ui.started)}</dt>
                    <dd class="data-value">{instant(&a["started"])}</dd>
                </div>
                {a["finished"]
                    .as_i64()
                    .map(|at| {
                        view! {
                            <div>
                                <dt>{tr!(ui.finished)}</dt>
                                <dd class="data-value">{instant(&Value::from(at))}</dd>
                            </div>
                        }
                    })}
                {(!stage.is_empty())
                    .then(|| {
                        view! {
                            <div>
                                <dt>{tr!(ui.protocol_stage)}</dt>
                                <dd>{stage}</dd>
                            </div>
                        }
                    })}
                {a["smtp_code"]
                    .as_u64()
                    .map(|code| {
                        view! {
                            <div>
                                <dt>{tr!(ui.smtp_response)}</dt>
                                <dd>{code}</dd>
                            </div>
                        }
                    })}
                {(!message_id.is_empty())
                    .then(|| {
                        view! {
                            <div class="attempt-message-id">
                                <dt>{tr!(ui.message_id)}</dt>
                                <dd>
                                    <code>{message_id}</code>
                                    <ActionButton
                                        kind="button"
                                        on_click=move |_| {
                                            let value = copy_id.clone();
                                            spawn_local(async move {
                                                copied
                                                    .set(
                                                        match copy_text(&value).await {
                                                            Ok(()) => {
                                                                leptos_i18n::t_string!(i18n, ui.message_id_copied).into()
                                                            }
                                                            Err(_) => {
                                                                leptos_i18n::t_string!(
                                                                    i18n, ui.copy_unavailable_select_the_message_id_and_copy_it_manually
                                                                )
                                                                    .into()
                                                            }
                                                        },
                                                    );
                                            });
                                        }
                                    >
                                        {tr!(ui.copy)}
                                    </ActionButton>
                                </dd>
                            </div>
                        }
                    })}
            </dl>
        </article>
    }
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
