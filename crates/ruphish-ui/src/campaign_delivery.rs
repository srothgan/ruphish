//! Stable recipient rows with an independently loaded, expandable history panel.
use crate::shared::components::{
    ActionButton, ButtonStyle, Dialog, QueryBoundary, StatusBadge, StatusTone,
};
use crate::{account::copy_text, campaigns::text, client::Api, time_display::instant};
use leptos::{prelude::*, task::spawn_local_scoped_with_cancellation as spawn_local};
use ruphish_contracts::campaign_reads::DeliveryRow;
use ruphish_domain::campaign::DeliveryState;
use ruphish_ui::recipient_history::{History, tone};
use ruphish_ui::{tr, trs};
use serde_json::{Value, json};

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
    let previous_newest = history.with_untracked(|h| h.events.last_key_value().map(|(id, _)| *id));
    let cursors = if older {
        Vec::new()
    } else {
        history.with_untracked(|h| {
            h.events
                .keys()
                .rev()
                .copied()
                .collect::<Vec<_>>()
                .chunks(100)
                .filter_map(|page| page.first().and_then(|id| id.checked_add(1)))
                .collect::<Vec<_>>()
        })
    };
    tasks.spawn(async move {
        let read = async {
            let path = format!(
                "/api/campaigns/{}/recipients/{result}/history",
                campaign.get_value()
            );
            let mut value: ruphish_contracts::campaign_reads::History =
                api.read(&format!("{path}{query}")).await?;
            if value.result_id != result {
                return Err(api.invalid_read_err());
            }
            let gap = value
                .events
                .iter()
                .map(|e| e.id)
                .min()
                .zip(previous_newest)
                .is_some_and(|(oldest, previous)| oldest > previous);
            // Revalidate already loaded pages by their saved IDs. A burst of new
            // observations cannot turn a refresh into an unbounded history walk.
            for cursor in cursors {
                let page: ruphish_contracts::campaign_reads::History = api
                    .read(&format!("{path}?before_attempt=0&before_event={cursor}"))
                    .await?;
                if page.result_id != result {
                    return Err(api.invalid_read_err());
                }
                value.events.extend(page.events);
            }
            let mut value = serde_json::to_value(value).map_err(|_| api.invalid_read_err())?;
            value["refresh_event_gap"] = gap.into();
            Ok(value)
        }
        .await;
        history.update(|h| h.finish(request, read));
    });
}

pub(super) fn recipient_row(
    api: Api,
    campaign: StoredValue<String>,
    result: i64,
    row: Signal<Option<DeliveryRow>>,
    controls: Controls,
) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let query = leptos_router::hooks::use_query_map();
    let mut initial = History::default();
    if query
        .with(|q| q.get("recipient"))
        .and_then(|v| v.parse::<i64>().ok())
        == Some(result)
    {
        initial.toggle();
    }
    let history = RwSignal::new(initial);
    let open = Memo::new(move |_| history.with(|h| h.open));
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
                            i18n, messages.recipient_history_for_arg0, arg0 = (row.with(|row| row.as_ref().map(|row| row.email.clone()).unwrap_or_default())).to_string()
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
            <th scope="row">
                {move || {
                    row.with(|row| row.as_ref().map(|row| row.email.clone()).unwrap_or_default())
                }}
            </th>
            <td>
                <StatusBadge tone=Signal::derive(move || status_tone(
                    row.with(|row| row.as_ref().map(|row| row.state.as_str()).unwrap_or_default()),
                ))>
                    {move || {
                        row.with(|row| {
                            row.as_ref()
                                .map(|value| {
                                    if value.state == DeliveryState::Ready
                                        && value.due > (js_sys::Date::now() / 1000.0) as i64
                                    {
                                        leptos_i18n::t_string!(i18n, ui.scheduled).to_string()
                                    } else {
                                        ruphish_ui::localization::delivery_label(
                                            i18n,
                                            value.state.as_str(),
                                        )
                                    }
                                })
                                .unwrap_or_default()
                        })
                    }}
                </StatusBadge>
                <small class="delivery-code">
                    {move || ruphish_ui::localization::delivery_failure_label(
                        i18n,
                        &row
                            .with(|row| {
                                row.as_ref().map(|row| row.code.clone()).unwrap_or_default()
                            }),
                    )}
                </small>
            </td>
            <td>
                <small>
                    {move || {
                        if row
                            .with(|row| {
                                row.as_ref()
                                    .is_some_and(|row| {
                                        matches!(
                                            row.state,
                                            DeliveryState::Ready | DeliveryState::Retry
                                        )
                                    })
                            })
                        {
                            leptos_i18n::t_string!(i18n, ui.next_attempt).to_string()
                        } else {
                            leptos_i18n::t_string!(i18n, ui.last_scheduled_attempt).to_string()
                        }
                    }}
                </small>
                {move || row.with(|row| row.as_ref().map(|row| instant(&row.due.into())))}
            </td>
            <td>
                <span>
                    {move || {
                        let count = row
                            .with(|row| row.as_ref().map(|row| row.attempt_count).unwrap_or(0));
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
                            && row
                                .with(|row| {
                                    row.as_ref()
                                        .is_some_and(|row| {
                                            matches!(
                                                row.state,
                                                DeliveryState::Failed | DeliveryState::Blocked
                                            )
                                        })
                                })
                    }>
                        <ActionButton
                            disabled=Signal::derive(move || busy.get())
                            on_click=move |_| {
                                if let Some(row) = row.get_untracked() {
                                    action
                                        .set(
                                            Some((
                                                "retry".into(),
                                                json!({"id":row.id,"generation":row.generation}),
                                            )),
                                        )
                                }
                            }
                        >
                            {tr!(ui.retry_after_correction)}
                        </ActionButton>
                    </Show>
                    <Show when=move || {
                        controls.can_edit
                            && row
                                .with(|row| {
                                    row.as_ref()
                                        .is_some_and(|row| row.state == DeliveryState::Unknown)
                                })
                    }>
                        <ActionButton
                            disabled=Signal::derive(move || busy.get())
                            on_click=move |_| {
                                if let Some(row) = row.get_untracked() {
                                    action
                                        .set(
                                            Some((
                                                "confirm_accepted".into(),
                                                json!({"id":row.id,"generation":row.generation}),
                                            )),
                                        )
                                }
                            }
                        >
                            {tr!(ui.confirm_accepted)}
                        </ActionButton>
                        <ActionButton
                            style=ButtonStyle::Ghost
                            disabled=Signal::derive(move || busy.get())
                            on_click=move |_| {
                                if let Some(row) = row.get_untracked() {
                                    action
                                        .set(
                                            Some((
                                                "resend_unknown".into(),
                                                json!({"id":row.id,"generation":row.generation}),
                                            )),
                                        )
                                }
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
                <div id=panel_id aria-labelledby=button_id>
                    <Show when=move || {
                        open.get()
                    }>{recipient_detail(api, campaign, result, history, row)}</Show>
                </div>
            </td>
        </tr>
    }
}

pub(super) fn recipient_detail(
    api: Api,
    campaign: StoredValue<String>,
    result: i64,
    history: RwSignal<History>,
    row: Signal<Option<DeliveryRow>>,
) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    let editing = RwSignal::new(false);
    let editor = RwSignal::new(ruphish_ui::event_editor::Editor::default());
    let locked = Signal::derive(move || editor.with(|s| s.busy || s.pending.is_some()));
    crate::campaign_queries::RefreshState::register(Signal::derive(move || {
        history.with(History::loading)
    }));
    let tasks = ruphish_ui::view_tasks::ViewTasks::default();
    let can_leave = crate::navigation::confirm_leave_action();
    let close = Callback::new(move |()| {
        if locked.get_untracked() {
            return;
        }
        tasks.spawn(async move {
            if can_leave().await && !locked.get_untracked() {
                editor.update(ruphish_ui::event_editor::Editor::cancel);
                editing.set(false);
            }
        });
    });
    let attempts = Memo::new(move |_| history.with(|h| h.attempts.clone()));
    let copy_message = RwSignal::new(String::new());
    let open = Memo::new(move |_| history.with(|h| h.open));
    Effect::new(move |_| {
        row.track();
        if open.get() && history.with_untracked(|h| h.error.is_none()) {
            fetch(tasks, api, campaign, result, history, false);
        }
    });
    view! {
        <section
            class="recipient-history"
            aria-busy=move || history.with(History::loading).to_string()
        >
            <div class="recipient-history-heading">
                <span>
                    {move || {
                        leptos_i18n::t_string!(
                            i18n, messages.times_shown_in_arg0, arg0 = (crate::time_display::zone_name()).to_string()
                        )
                    }}
                </span>
                <Show when=crate::campaigns::editable>
                    <ActionButton
                        disabled=Signal::derive(move || !history.with(History::has_loaded))
                        on_click=move |_| { editing.set(true) }
                    >
                        {tr!(ui.edit_timeline)}
                    </ActionButton>
                </Show>
            </div>
            <QueryBoundary
                fetching=Signal::derive(move || history.with(History::loading))
                has_data=Signal::derive(move || history.with(History::has_loaded))
                error=Signal::derive(move || history.with(|h| h.error.clone()))
                noun=trs!(ui.recipient_history)
                refresh=Callback::new(move |_| fetch(
                    tasks,
                    api,
                    campaign,
                    result,
                    history,
                    history.get_untracked().retry_older,
                ))
            >
                <div class="recipient-history-columns">
                    <section>
                        <h4>{tr!(ui.result_history)}</h4>
                        {crate::event_table::timeline(history)}
                    </section>
                    <section class="recipient-attempts">
                        <h4>{tr!(ui.delivery_attempts)}</h4>
                        <Show when=move || {
                            history.with(|h| h.has_loaded() && h.attempts.is_empty())
                        }>
                            <p>{tr!(ui.no_delivery_attempts)}</p>
                        </Show>
                        {move || {
                            attempts
                                .get()
                                .into_values()
                                .rev()
                                .map(|attempt| attempt_details(attempt, copy_message))
                                .collect_view()
                        }}
                        <span class="copy-status" role="status">
                            {move || copy_message.get()}
                        </span>
                    </section>
                </div>
                <Show when=move || editing.get()>
                    <Dialog
                        title=trs!(ui.edit_timeline)
                        class="event-edit-dialog"
                        busy=locked
                        on_cancel=close
                    >
                        <div class="recipient-history-heading">
                            <div>
                                <h2>{tr!(ui.edit_timeline)}</h2>
                                <p>
                                    {move || {
                                        row.with(|row| row.as_ref().map(|row| row.email.clone()))
                                    }}
                                </p>
                            </div>
                            <ActionButton disabled=locked on_click=move |_| close.run(())>
                                {tr!(ui.close)}
                            </ActionButton>
                        </div>
                        {crate::event_table::table(api, campaign, result, history, editor)}
                        <Show when=move || {
                            history.with(|h| h.next_event.is_some() || h.next_attempt.is_some())
                        }>
                            <ActionButton
                                disabled=Signal::derive(move || {
                                    locked.get() || editor.with(|s| s.draft.is_some())
                                        || history.with(History::loading)
                                })
                                on_click=move |_| fetch(tasks, api, campaign, result, history, true)
                            >
                                {tr!(ui.load_earlier_history)}
                            </ActionButton>
                        </Show>
                    </Dialog>
                </Show>
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
            </QueryBoundary>
        </section>
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
