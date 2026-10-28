//! One event table for expanded and directly addressed campaign recipients.
use crate::{client::Api, shared::components::*, time_display::instant};
use leptos::prelude::*;
use ruphish_domain::{
    event_edit::{Action, Change, Inspection, Presence, Record, Value as EventValue},
    events::{Kind, RecordedKind},
};
use ruphish_ui::{event_editor::Editor, recipient_history::History, tr, trs};
use serde_json::{Value, json};

#[derive(Clone, Copy)]
pub(crate) struct Csrf(pub StoredValue<String>);
fn record(row: &Value) -> Option<Record> {
    serde_json::from_value(row["correction"].clone()).ok()
}
fn patch(history: RwSignal<History>, record: &Record) {
    history.update(|history| {
        if history.events.get(&record.id).and_then(self::record).is_some_and(|old|old.revision>record.revision) {return;}
        let at=ruphish_domain::datetime::parse_stored(&record.current.time).ok().map(|v|v.as_second());
        let details=json!({"username_present":record.current.presence.username_present,"password_present":record.current.presence.password_present}).to_string();
        history.events.insert(record.id,json!({"id":record.id,"time":record.current.time,"at":at,"message":record.current.kind.label(),"details":details,"correction":record}));
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
) -> impl IntoView {
    let i18n = api.i18n;
    let csrf = expect_context::<Csrf>().0;
    let tasks = ruphish_ui::view_tasks::ViewTasks::default();
    let editor = RwSignal::new(Editor::default());
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
        let time = current.as_ref().map(|f| f.time.clone()).unwrap_or_else(|| {
            history
                .with_untracked(|h| {
                    h.events
                        .values()
                        .rev()
                        .find_map(record)
                        .map(|r| r.current.time)
                })
                .unwrap_or_else(|| ruphish_domain::datetime::now().to_string())
        });
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
                Ok((value, _)) => match serde_json::from_value::<Record>(value) {
                    Ok(record) => {
                        if editor.try_update(|s| s.saved(&record)).unwrap_or(false) {
                            patch(history, &record);
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
    let form = move || {
        view! {
            <div class="event-editor">
                <Show when=move || {
                    editor.with(|s| s.draft.as_ref().is_some_and(|d| d.value.is_some()))
                }>
                    <div class="event-edit-fields">
                        <Field label=trs!(ui.event_occurrence)>
                            <TextInput
                                input_type="datetime-local"
                                step="1"
                                value=Signal::derive(move || input.get())
                                disabled=locked
                                on_input=move |e| input.set(event_target_value(&e))
                            />
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
                        <Show when=move || {
                            editor
                                .with(|s| {
                                    s.draft
                                        .as_ref()
                                        .and_then(|d| d.value.as_ref())
                                        .is_some_and(|v| v.kind == Kind::FormSubmitted)
                                })
                        }>
                            {presence_select(editor, locked, false)}
                            {presence_select(editor, locked, true)}
                        </Show>
                    </div>
                    <p>
                        {move || {
                            zone.with_value(|z| ruphish_domain::datetime::zone_name(z).to_owned())
                        }}
                    </p>
                </Show>
                <Show when=move || {
                    editor.with(|s| s.draft.as_ref().is_some_and(|d| d.action == Action::Exclude))
                }>
                    <p>{tr!(ui.event_exclude_explanation)}</p>
                </Show>
                <Show when=move || {
                    editor.with(|s| s.draft.as_ref().is_some_and(|d| d.action == Action::Restore))
                }>
                    <p>{tr!(ui.event_restore_explanation)}</p>
                </Show>
                <div class="form-actions">
                    <ActionButton
                        kind="button"
                        style=ButtonStyle::Primary
                        disabled=Signal::derive(move || editor.with(|s| s.busy))
                        on_click=move |_| save.run(())
                    >
                        {move || {
                            if editor.with(|s| s.pending.is_some()) {
                                leptos_i18n::t_string!(i18n,ui.retry)
                            } else {
                                leptos_i18n::t_string!(i18n,ui.save_changes)
                            }
                        }}
                    </ActionButton>
                    <ActionButton
                        kind="button"
                        disabled=locked
                        on_click=move |_| editor.update(Editor::cancel)
                    >
                        {tr!(ui.cancel)}
                    </ActionButton>
                </div>
                <Show when=move || editor.with(|s| s.pending.is_some() && !s.busy)>
                    <p>{tr!(ui.event_retry_unknown)}</p>
                </Show>
            </div>
        }
    };
    view! {
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
        <p role="alert" class="form-message error">
            {move || editor.with(|s| s.error.clone())}
        </p>
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
            <Show when=move || {
                editor.with(|s| s.draft.as_ref().is_some_and(|d| d.event_id.is_none()))
            }>
                <tr>
                    <td colspan="5">{form()}</td>
                </tr>
            </Show>
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
                    let inspection = RwSignal::new(None::<Inspection>);
                    let audit_error = RwSignal::new(String::new());
                    let audit_busy = RwSignal::new(false);
                    let inspect = Callback::new(move |older: bool| {
                        if audit_busy.get_untracked() {
                            return;
                        }
                        audit_busy.set(true);
                        audit_error.set(String::new());
                        let before = if older {
                            inspection.with_untracked(|v| v.as_ref().and_then(|v| v.next_before))
                        } else {
                            None
                        };
                        tasks
                            .spawn(async move {
                                let suffix = before
                                    .map(|v| format!("?before={v}"))
                                    .unwrap_or_default();
                                match api
                                    .read::<
                                        Inspection,
                                    >(
                                        &format!(
                                            "/api/campaigns/{}/recipients/{recipient}/events/{id}{suffix}",
                                            campaign.get_value(),
                                        ),
                                    )
                                    .await
                                {
                                    Ok(mut value) => {
                                        patch(history, &value.record);
                                        if older {
                                            inspection
                                                .update(|v| {
                                                    if let Some(old) = v {
                                                        old.audit.append(&mut value.audit);
                                                        old.next_before = value.next_before;
                                                        old.record = value.record;
                                                    }
                                                });
                                        } else {
                                            inspection.set(Some(value));
                                        }
                                    }
                                    Err(error) => audit_error.set(error.message),
                                }
                                audit_busy.set(false);
                            });
                    });
                    view! {
                        <tr class:event-excluded=move || current.get().is_some_and(|r| r.excluded)>
                            <td>
                                {move || instant(
                                    &current
                                        .get()
                                        .map(|r| json!(r.current.time))
                                        .unwrap_or_else(|| row.get()["time"].clone()),
                                )}
                            </td>
                            <td>
                                {move || ruphish_ui::localization::event_label(
                                    i18n,
                                    &current
                                        .get()
                                        .map(|r| r.current.kind.label().to_owned())
                                        .unwrap_or_else(|| crate::campaigns::text(
                                            &row.get(),
                                            "message",
                                        )),
                                )}
                                <small>
                                    {move || {
                                        current
                                            .get()
                                            .and_then(|r| r.current.presence.password_present)
                                            .map(|v| {
                                                if v {
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
                                }}<small>{move || current.get().map(|r| r.source)}</small>
                            </td>
                            <td>
                                <Show when=move || current.get().is_some_and(|r| r.edited())>
                                    <StatusBadge tone=StatusTone::Warning>
                                        {tr!(ui.event_edited)}
                                    </StatusBadge>
                                </Show>
                                <Show when=move || current.get().is_some_and(|r| r.excluded)>
                                    <StatusBadge tone=StatusTone::Neutral>
                                        {tr!(ui.event_excluded)}
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
                                            disabled=Signal::derive(move || {
                                                editor.with(|s| s.draft.is_some())
                                            })
                                            on_click=move |_| {
                                                open.run((Action::Edit, current.get_untracked()))
                                            }
                                        >
                                            {tr!(ui.event_edit)}
                                        </ActionButton>
                                        <Show when=move || {
                                            current.get().is_some_and(|r| !r.excluded)
                                        }>
                                            <ActionButton
                                                kind="button"
                                                disabled=Signal::derive(move || {
                                                    editor.with(|s| s.draft.is_some())
                                                })
                                                on_click=move |_| {
                                                    open.run((Action::Exclude, current.get_untracked()))
                                                }
                                            >
                                                {tr!(ui.delete)}
                                            </ActionButton>
                                        </Show>
                                        <Show when=move || {
                                            current.get().is_some_and(|r| r.excluded || r.edited())
                                        }>
                                            <ActionButton
                                                kind="button"
                                                disabled=Signal::derive(move || {
                                                    editor.with(|s| s.draft.is_some())
                                                })
                                                on_click=move |_| {
                                                    open.run((Action::Restore, current.get_untracked()))
                                                }
                                            >
                                                {tr!(ui.event_restore)}
                                            </ActionButton>
                                        </Show>
                                    </Show>
                                    <ActionButton
                                        kind="button"
                                        disabled=Signal::derive(move || audit_busy.get())
                                        on_click=move |_| {
                                            if inspection.get_untracked().is_some() {
                                                inspection.set(None)
                                            } else {
                                                inspect.run(false)
                                            }
                                        }
                                    >
                                        {tr!(ui.event_audit)}
                                    </ActionButton>
                                </div>
                            </td>
                        </tr>
                        <Show when=move || {
                            editor
                                .with(|s| s.draft.as_ref().is_some_and(|d| d.event_id == Some(id)))
                        }>
                            <tr>
                                <td colspan="5">{form()}</td>
                            </tr>
                        </Show>
                        <Show when=move || {
                            inspection.get().is_some() || !audit_error.get().is_empty()
                        }>
                            <tr>
                                <td colspan="5">
                                    <p role="alert">{move || audit_error.get()}</p>
                                    {move || {
                                        inspection
                                            .get()
                                            .map(|v| {
                                                view! {
                                                    <p>
                                                        <strong>{tr!(ui.event_original)}</strong>
                                                        " "
                                                        {fact_label(&v.record.original, false)}
                                                    </p>
                                                    <ol>
                                                        {v
                                                            .audit
                                                            .into_iter()
                                                            .map(|a| {
                                                                view! {
                                                                    <li>
                                                                        {format!(
                                                                            "{} #{} / {} #{}",
                                                                            leptos_i18n::t_string!(i18n,ui.event_revision),
                                                                            a.revision,
                                                                            leptos_i18n::t_string!(i18n,ui.event_operator),
                                                                            a.actor,
                                                                        )} " / " {instant(&json!(a.performed_at))} " / "
                                                                        {action_label(a.action)}
                                                                        <p>
                                                                            {a
                                                                                .before
                                                                                .map(|b| {
                                                                                    format!("{} -> ", fact_label(&b.current, b.excluded))
                                                                                })}{fact_label(&a.after.current, a.after.excluded)}
                                                                        </p>
                                                                    </li>
                                                                }
                                                            })
                                                            .collect_view()}
                                                    </ol>
                                                }
                                            })
                                    }}
                                    <Show when=move || {
                                        inspection
                                            .with(|v| {
                                                v.as_ref().is_some_and(|v| v.next_before.is_some())
                                            })
                                    }>
                                        <ActionButton
                                            kind="button"
                                            disabled=Signal::derive(move || audit_busy.get())
                                            on_click=move |_| inspect.run(true)
                                        >
                                            {tr!(ui.load_earlier_history)}
                                        </ActionButton>
                                    </Show>
                                    <ActionButton
                                        kind="button"
                                        disabled=Signal::derive(move || audit_busy.get())
                                        on_click=move |_| inspect.run(false)
                                    >
                                        {tr!(ui.refresh)}
                                    </ActionButton>
                                </td>
                            </tr>
                        </Show>
                    }
                }
            />
        </DataTable>
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
        Action::Exclude => leptos_i18n::t_string!(i18n, ui.event_excluded).into(),
        Action::Restore => leptos_i18n::t_string!(i18n, ui.event_restore).into(),
    }
}
fn fact_label(fact: &ruphish_domain::event_edit::Fact, excluded: bool) -> String {
    let i18n = ruphish_ui::localization::use_i18n();
    let mut parts = vec![
        ruphish_ui::localization::event_label(i18n, fact.kind.label()),
        instant(&json!(fact.time)),
    ];
    for (username, present) in [
        (true, fact.presence.username_present),
        (false, fact.presence.password_present),
    ] {
        if let Some(present) = present {
            let label = if username {
                leptos_i18n::t_string!(i18n, ui.event_username_presence)
            } else {
                leptos_i18n::t_string!(i18n, ui.event_password_presence)
            };
            let value = if present {
                leptos_i18n::t_string!(i18n, ui.event_present)
            } else {
                leptos_i18n::t_string!(i18n, ui.event_absent)
            };
            parts.push(format!("{label}: {value}"));
        }
    }
    if excluded {
        parts.push(leptos_i18n::t_string!(i18n, ui.event_excluded).into());
    }
    parts.join(" / ")
}
