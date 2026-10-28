//! One event table for expanded and directly addressed campaign recipients.
use crate::{client::Api, shared::components::*, time_display::instant};
use leptos::prelude::*;
use ruphish_domain::{
    event_edit::{Action, Change, Outcome, Presence, Record, Value as EventValue},
    events::{Kind, RecordedKind},
};
use ruphish_ui::{event_editor::Editor, recipient_history::History, tr, trs};
use serde_json::{Value, json};

#[derive(Clone, Copy)]
pub(crate) struct Csrf(pub StoredValue<String>);
fn record(row: &Value) -> Option<Record> {
    serde_json::from_value(row["record"].clone()).ok()
}
fn patch(history: RwSignal<History>, record: &Record) {
    history.update(|history| {
        if history.events.get(&record.id).and_then(self::record).is_some_and(|old|old.revision>record.revision) {return;}
        let at=ruphish_domain::datetime::parse_stored(&record.current.time).ok().map(|v|v.as_second());
        let details=json!({"username_present":record.current.presence.username_present,"password_present":record.current.presence.password_present}).to_string();
        history.events.insert(record.id,json!({"id":record.id,"time":record.current.time,"at":at,"message":record.current.kind.label(),"details":details,"record":record}));
    });
}
fn presence(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}
fn presence_value(value: Option<bool>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}

pub(super) fn table(
    api: Api,
    campaign: StoredValue<String>,
    recipient: i64,
    history: RwSignal<History>,
    editor: RwSignal<Editor>,
) -> impl IntoView {
    let i18n = api.i18n;
    crate::navigation::protect_draft(move || {
        editor.with(|s| s.draft.is_some() || s.pending.is_some() || s.busy)
    });
    let csrf = expect_context::<Csrf>().0;
    let tasks = ruphish_ui::view_tasks::ViewTasks::default();
    let input = RwSignal::new(String::new());
    let original_input = StoredValue::new(String::new());
    let zone = StoredValue::new(crate::time_display::zone());
    let sorting = TableSorting::new(
        "recipient-history-events",
        "time",
        ruphish_domain::reporting::SortDirection::Desc,
    );
    let queries = expect_context::<crate::campaign_queries::CampaignQueries>();
    let open = Callback::new(move |(action, target): (Action, Option<Record>)| {
        if editor.with_untracked(|s| s.draft.is_some() || s.pending.is_some() || s.busy) {
            return;
        }
        let Ok(key) = crate::campaigns::random_key() else {
            editor.update(|s| s.error = leptos_i18n::t_string!(i18n, ui.event_key_failed).into());
            return;
        };
        let current = target.as_ref().map(|r| r.current.clone());
        let time = current
            .as_ref()
            .map(|f| f.time.clone())
            .unwrap_or_else(|| ruphish_domain::datetime::now().to_string());
        let time = ruphish_domain::datetime::parse_stored(&time)
            .map(ruphish_domain::datetime::utc)
            .unwrap_or(time);
        let kind = current
            .as_ref()
            .and_then(|f| match f.kind {
                RecordedKind::Known(k) => Some(k),
                _ => None,
            })
            .unwrap_or(Kind::EmailOpened);
        let value = matches!(action, Action::Add | Action::Edit).then(|| EventValue {
            time: time.clone(),
            kind,
            presence: current.map(|f| f.presence).unwrap_or_default(),
        });
        let active_zone = crate::time_display::zone();
        let local = ruphish_domain::datetime::parse_stored(&time)
            .ok()
            .map(|t| ruphish_domain::datetime::local_input(t, &active_zone))
            .unwrap_or_default();
        zone.set_value(active_zone);
        original_input.set_value(local.clone());
        input.set(local);
        editor.update(|s| {
            s.open(Change {
                request_id: key,
                event_id: target.as_ref().map(|r| r.id),
                revision: target.map_or(0, |r| r.revision),
                action,
                value,
            })
        });
    });
    let save = Callback::new(move |()| {
        if editor.with_untracked(|s| s.busy) {
            return;
        }
        if editor.with_untracked(|s| s.pending.is_none()) {
            let Some(mut change) = editor.with_untracked(|s| s.draft.clone()) else {
                return;
            };
            if let Some(value) = &mut change.value
                && input.get_untracked() != original_input.get_value()
            {
                let parsed = zone.with_value(|z| {
                    ruphish_ui::datetime::input_seconds(&input.get_untracked(), None, z)
                });
                let Ok(Some(seconds)) = parsed else {
                    editor.update(|s| {
                        s.error = leptos_i18n::t_string!(i18n, ui.event_invalid_time).into()
                    });
                    return;
                };
                let Ok(time) = ruphish_domain::datetime::utc_seconds(seconds) else {
                    return;
                };
                value.time = time;
            }
            if ruphish_domain::event_edit::validate(
                &change,
                None,
                None,
                ruphish_domain::datetime::now().as_second(),
            )
            .is_err()
            {
                editor.update(|s| {
                    s.error = leptos_i18n::t_string!(i18n, ui.event_invalid_time).into()
                });
                return;
            }
            editor.update(|s| s.draft = Some(change));
        }
        let Some(request) = editor.try_update(Editor::begin).flatten() else {
            return;
        };
        tasks.spawn(async move {
            let path = format!(
                "/api/campaigns/{}/recipients/{recipient}/events",
                campaign.get_value()
            );
            match api
                .resource_response(
                    "POST",
                    &path,
                    &csrf.get_value(),
                    Some(&json!(request)),
                    None,
                )
                .await
            {
                Ok((value, _)) => match serde_json::from_value::<Outcome>(value) {
                    Ok(record) => {
                        if editor.try_update(|s| s.saved(&record)).unwrap_or(false) {
                            history.update(History::invalidate_reads);
                            if let Some(event) = &record.event {
                                patch(history, event);
                            } else {
                                history.update(|h| {
                                    h.events.remove(&record.id);
                                });
                            }
                            queries.changed(&campaign.get_value());
                        } else {
                            editor.update(|s| {
                                s.failed(
                                    leptos_i18n::t_string!(i18n, ui.event_retry_unknown).into(),
                                    false,
                                )
                            });
                        }
                    }
                    Err(_) => editor.update(|s| {
                        s.failed(
                            leptos_i18n::t_string!(i18n, ui.event_retry_unknown).into(),
                            false,
                        )
                    }),
                },
                Err(error) => {
                    let definite = (400..500).contains(&error.status);
                    editor.update(|s| s.failed(error.message, definite));
                }
            }
        });
    });
    let locked = Signal::derive(move || editor.with(|s| s.busy || s.pending.is_some()));
    let editing = Signal::derive(move || editor.with(|s| s.draft.is_some()));
    let form = move || {
        view! {
            <form
                class="event-correction"
                on:submit=move |event| {
                    event.prevent_default();
                    save.run(());
                }
                aria-busy=move || editor.with(|s| s.busy).to_string()
            >
                <header class="event-correction-heading">
                    <h3>
                        {move || editor.with(|s| s.draft.as_ref().map(|d| action_label(d.action)))}
                    </h3>
                </header>
                <Show when=move || {
                    editor.with(|s| s.draft.as_ref().is_some_and(|d| d.value.is_some()))
                }>
                    <div class="event-correction-fields">
                        <Field label=trs!(ui.event_occurrence)>
                            <TextInput
                                input_type="datetime-local"
                                step="1"
                                required=true
                                value=Signal::derive(move || input.get())
                                disabled=locked
                                on_input=move |event| input.set(event_target_value(&event))
                            />
                            <small>
                                {move || {
                                    zone.with_value(|z| {
                                        ruphish_domain::datetime::zone_name(z).to_owned()
                                    })
                                }}
                            </small>
                        </Field>
                        <Select
                            label=trs!(ui.event)
                            disabled=locked
                            value=Signal::derive(move || {
                                editor
                                    .with(|s| {
                                        s.draft
                                            .as_ref()
                                            .and_then(|d| d.value.as_ref())
                                            .map(|v| v.kind.key().to_owned())
                                            .unwrap_or_default()
                                    })
                            })
                            on_change=Callback::new(move |key: String| {
                                if let Some(kind) = Kind::from_key(&key) {
                                    editor
                                        .update(|s| {
                                            if let Some(value) = s
                                                .draft
                                                .as_mut()
                                                .and_then(|d| d.value.as_mut())
                                            {
                                                value.kind = kind;
                                                if kind != Kind::FormSubmitted {
                                                    value.presence = Presence::default();
                                                }
                                            }
                                        });
                                }
                            })
                        >
                            {[
                                Kind::EmailOpened,
                                Kind::LinkClicked,
                                Kind::FormSubmitted,
                                Kind::EmailReported,
                            ]
                                .into_iter()
                                .map(|kind| {
                                    view! {
                                        <option value=kind
                                            .key()>
                                            {ruphish_ui::localization::event_label(i18n, kind.label())}
                                        </option>
                                    }
                                })
                                .collect_view()}
                        </Select>
                    </div>
                    <Show when=move || {
                        editor
                            .with(|s| {
                                s.draft
                                    .as_ref()
                                    .and_then(|d| d.value.as_ref())
                                    .is_some_and(|v| v.kind == Kind::FormSubmitted)
                            })
                    }>
                        <fieldset class="event-correction-presence">
                            <legend>{tr!(ui.event_form_fields)}</legend>
                            <div class="event-correction-fields event-correction-fields--presence">
                                {presence_select(editor, locked, false)}
                                {presence_select(editor, locked, true)}
                            </div>
                        </fieldset>
                    </Show>
                </Show>
                <Show when=move || {
                    editor.with(|s| s.draft.as_ref().is_some_and(|d| d.action == Action::Delete))
                }>
                    <p>{tr!(ui.event_delete_explanation)}</p>
                </Show>
                <Show when=move || editor.with(|s| !s.error.is_empty())>
                    <p role="alert" class="form-message error has-content">
                        {move || editor.with(|s| s.error.clone())}
                    </p>
                </Show>
                <Show when=move || editor.with(|s| s.pending.is_some() && !s.busy)>
                    <p>{tr!(ui.event_retry_unknown)}</p>
                </Show>
                <footer class="event-correction-actions">
                    <ActionButton
                        kind="submit"
                        style=ButtonStyle::Primary
                        disabled=Signal::derive(move || editor.with(|s| s.busy))
                    >
                        {move || {
                            editor
                                .with(|s| {
                                    if s.pending.is_some() {
                                        leptos_i18n::t_string!(i18n, ui.retry).into_owned()
                                    } else if s
                                        .draft
                                        .as_ref()
                                        .is_some_and(|d| d.action == Action::Edit)
                                    {
                                        leptos_i18n::t_string!(i18n, ui.save_changes).into_owned()
                                    } else {
                                        s.draft
                                            .as_ref()
                                            .map(|d| action_label(d.action))
                                            .unwrap_or_default()
                                    }
                                })
                        }}
                    </ActionButton>
                    <ActionButton
                        kind="button"
                        disabled=locked
                        on_click=move |_| editor.update(Editor::cancel)
                    >
                        {tr!(ui.cancel)}
                    </ActionButton>
                </footer>
            </form>
        }
    };
    view! {
        <Show when=move || editing.get()>{form()}</Show>
        <div class="event-review" hidden=move || editing.get()>
            <p>{tr!(ui.event_effective_explanation)}</p>
            <Show when=crate::campaigns::editable>
                <ActionButton
                    kind="button"
                    disabled=Signal::derive(move || editor.with(|s| s.draft.is_some()))
                    on_click=move |_| open.run((Action::Add, None))
                >
                    {tr!(ui.event_add)}
                </ActionButton>
            </Show>
            <Show when=move || editor.with(|s| !s.error.is_empty())>
                <p role="alert" class="form-message error has-content">
                    {move || editor.with(|s| s.error.clone())}
                </p>
            </Show>
            <DataTable
                label=trs!(ui.recipient_events)
                sorting=sorting
                columns=vec!["time", "message", "", "", ""]
                headers=vec![
                    trs!(ui.time),
                    trs!(ui.event),
                    trs!(ui.event_origin),
                    trs!(ui.status),
                    trs!(ui.actions),
                ]
            >
                <For
                    each=move || {
                        sorting
                            .json(
                                history
                                    .with(|h| {
                                        h.events
                                            .values()
                                            .cloned()
                                            .map(|mut row| {
                                                if let Some(r) = record(&row) {
                                                    row["time"] = json!(r.current.time);
                                                    row["message"] = json!(r.current.kind.label());
                                                }
                                                row
                                            })
                                            .collect()
                                    }),
                                &["time"],
                            )
                            .into_iter()
                            .filter_map(|r| r["id"].as_i64())
                            .collect::<Vec<_>>()
                    }
                    key=|id| *id
                    children=move |id| {
                        let row = Signal::derive(move || {
                            history.with(|h| h.events.get(&id).cloned().unwrap_or_default())
                        });
                        let current = Signal::derive(move || record(&row.get()));
                        view! {
                            <tr>
                                <td>{move || instant(&row.get()["time"])}</td>
                                <td>
                                    {move || event_title(i18n, &row.get())}
                                    <small>
                                        {move || {
                                            current
                                                .get()
                                                .and_then(|r| r.current.presence.password_present)
                                                .map(|present| {
                                                    if present {
                                                        leptos_i18n::t_string!(i18n,ui.password_field_was_nonempty)
                                                    } else {
                                                        leptos_i18n::t_string!(i18n,ui.password_field_was_empty)
                                                    }
                                                })
                                        }}
                                    </small>
                                </td>
                                <td>
                                    {move || {
                                        if current.get().is_some_and(|r| r.source == "operator") {
                                            leptos_i18n::t_string!(i18n,ui.event_operator)
                                        } else {
                                            leptos_i18n::t_string!(i18n,ui.event_system)
                                        }
                                    }}
                                </td>
                                <td>
                                    <Show when=move || current.get().is_some_and(|r| r.edited)>
                                        <StatusBadge tone=StatusTone::Warning>
                                            {tr!(ui.event_edited)}
                                        </StatusBadge>
                                    </Show>
                                </td>
                                <td>
                                    <div class="table-actions">
                                        <Show when=move || {
                                            crate::campaigns::editable()
                                                && current.get().is_some_and(|r| r.editable())
                                        }>
                                            <ActionButton
                                                kind="button"
                                                disabled=editing
                                                on_click=move |_| {
                                                    open.run((Action::Edit, current.get_untracked()))
                                                }
                                            >
                                                {tr!(ui.event_edit)}
                                            </ActionButton>
                                            <ActionButton
                                                kind="button"
                                                disabled=editing
                                                on_click=move |_| {
                                                    open.run((Action::Delete, current.get_untracked()))
                                                }
                                            >
                                                {tr!(ui.delete)}
                                            </ActionButton>
                                        </Show>
                                    </div>
                                </td>
                            </tr>
                        }
                    }
                />
            </DataTable>
        </div>
    }
}
fn presence_select(
    editor: RwSignal<Editor>,
    locked: Signal<bool>,
    username: bool,
) -> impl IntoView {
    view! {
        <Select
            label=if username {
                trs!(ui.event_username_presence)
            } else {
                trs!(ui.event_password_presence)
            }
            disabled=locked
            value=Signal::derive(move || {
                editor
                    .with(|s| presence_value(
                        s
                            .draft
                            .as_ref()
                            .and_then(|d| d.value.as_ref())
                            .and_then(|v| {
                                if username {
                                    v.presence.username_present
                                } else {
                                    v.presence.password_present
                                }
                            }),
                    ))
            })
            on_change=Callback::new(move |text: String| {
                editor
                    .update(|s| {
                        if let Some(v) = s.draft.as_mut().and_then(|d| d.value.as_mut()) {
                            if username {
                                v.presence.username_present = presence(&text)
                            } else {
                                v.presence.password_present = presence(&text)
                            }
                        }
                    })
            })
        >
            <option value="">{tr!(ui.event_not_recorded)}</option>
            <option value="true">{tr!(ui.event_present)}</option>
            <option value="false">{tr!(ui.event_absent)}</option>
        </Select>
    }
}

fn action_label(action: Action) -> String {
    let i18n = ruphish_ui::localization::use_i18n();
    match action {
        Action::Add => leptos_i18n::t_string!(i18n, ui.event_add).into(),
        Action::Edit => leptos_i18n::t_string!(i18n, ui.event_edit).into(),
        Action::Delete => leptos_i18n::t_string!(i18n, ui.delete).into(),
    }
}
/// Read-only chronological history; correction controls live exclusively in the dialog.
pub(super) fn timeline(history: RwSignal<History>) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    view! {
        <ol class="delivery-timeline">
            <For
                each=move || {
                    let mut rows = history.with(|h| h.events.values().cloned().collect::<Vec<_>>());
                    rows.sort_by_key(|row| (row["at"].as_i64(), row["id"].as_i64()));
                    rows
                }
                key=|row| (row["id"].as_i64(), row["record"]["revision"].as_i64())
                children=move |row| {
                    let record = record(&row);
                    let edited = record.as_ref().is_some_and(|r| r.edited);
                    let operator = record.as_ref().is_some_and(|r| r.source == "operator");
                    let label = event_title(i18n, &row);
                    let tone = match row["message"].as_str() {
                        Some("Email Sent") => "success",
                        Some("Error Sending Email") => "failure",
                        Some("Delivery reconciled") => "warning",
                        _ => "neutral",
                    };
                    view! {
                        <li class=format!("timeline-event {tone}")>
                            <span class="timeline-marker" aria-hidden="true"></span>
                            <div>
                                <strong>{label}</strong>
                                <time>{instant(&row["time"])}</time>
                                <div class="timeline-labels">
                                    {operator
                                        .then(|| view! { <span>{tr!(ui.event_operator)}</span> })}
                                    {edited.then(|| view! { <span>{tr!(ui.event_edited)}</span> })}
                                </div>
                            </div>
                        </li>
                    }
                }
            />
        </ol>
        <Show when=move || {
            history.with(|h| h.has_loaded() && h.error.is_none() && h.events.is_empty())
        }>
            <p>{tr!(ui.no_attempts_or_recipient_events_recorded_yet)}</p>
        </Show>
    }
}

fn event_title(
    i18n: leptos_i18n::I18nContext<ruphish_ui::localization::Locale>,
    row: &Value,
) -> String {
    let message = row["message"].as_str().unwrap_or_default();
    if message == "Delivery reconciled" {
        let metadata: Value =
            serde_json::from_str(row["details"].as_str().unwrap_or_default()).unwrap_or_default();
        return match metadata["action"].as_str() {
            Some("retry") => leptos_i18n::t_string!(i18n, ui.retry_requested).into(),
            Some("resend_unknown") => leptos_i18n::t_string!(i18n, ui.resend_requested).into(),
            Some("confirm_accepted") => {
                leptos_i18n::t_string!(i18n, ui.acceptance_confirmed).into()
            }
            _ => leptos_i18n::t_string!(i18n, ui.delivery_resolution).into(),
        };
    }
    ruphish_ui::localization::event_label(i18n, message)
}
