//! Saved-profile diagnostics: grouped checks, an email card and durable history.
use crate::api::client::Api;
use crate::api::query::QueryView;
use crate::management_queries::ManagementQueries;
use crate::shared::components::*;
use crate::shared::feedback::Message;
use crate::shared::table_sort::Value as SortValue;
use crate::tr;
use crate::trs;
use leptos::{prelude::*, task::spawn_local_scoped_with_cancellation as spawn_local};
use ruphish_contracts::profile_diagnostics::Attempt;
use ruphish_contracts::profile_diagnostics::History;
use ruphish_contracts::profile_diagnostics::Kind;
use ruphish_contracts::profile_diagnostics::Outcome;
use ruphish_contracts::profile_diagnostics::Request;
use ruphish_domain::reporting::SortDirection;
use serde_json::json;

#[derive(Clone, Copy)]
pub(super) struct Testing {
    api: Api,
    token: StoredValue<String>,
    id: i64,
    revision: Signal<i64>,
    can_edit: bool,
    blocked: Signal<bool>,
    read: QueryView<History>,
    busy: RwSignal<bool>,
    expanded: RwSignal<bool>,
    recipient: RwSignal<String>,
    pending: RwSignal<Option<Request>>,
    message: Message,
}

impl Testing {
    pub(super) fn refresh(self) {
        self.read.refresh.run(());
    }
    pub(super) fn new(
        api: Api,
        token: String,
        id: i64,
        revision: Signal<i64>,
        can_edit: bool,
        blocked: Signal<bool>,
    ) -> Self {
        let queries = expect_context::<ManagementQueries>();
        Self {
            api,
            token: StoredValue::new(token),
            id,
            revision,
            can_edit,
            blocked,
            read: queries
                .profile_tests
                .observe_polling(move || id, true.into()),
            busy: RwSignal::new(false),
            expanded: RwSignal::new(false),
            recipient: RwSignal::new(String::new()),
            pending: RwSignal::new(None),
            message: Message::new(String::new()),
        }
    }
    fn unavailable(self) -> bool {
        !self.can_edit
            || self.blocked.get()
            || self.busy.get()
            || self.read.fetching.get()
            || self
                .read
                .data
                .with(|value| value.as_ref().is_none_or(|h| h.summary.active.is_some()))
    }
    fn run(self, kind: Kind) {
        if self.unavailable() {
            return;
        }
        let i18n = self.api.i18n;
        let request = if let Some(request) = self.pending.get_untracked() {
            request
        } else {
            let Ok(id) = crate::campaigns::pages::random_key() else {
                self.message.error(self.api.invalid_read());
                return;
            };
            Request {
                id,
                revision: self.revision.get_untracked(),
                kind,
                email: (kind == Kind::Email)
                    .then(|| self.recipient.get_untracked().trim().to_owned()),
            }
        };
        self.pending.set(Some(request.clone()));
        self.busy.set(true);
        self.message.clear();
        spawn_local(async move {
            let result = self
                .api
                .resource_response(
                    "POST",
                    &format!("/api/smtp/{}/tests", self.id),
                    &self.token.get_value(),
                    Some(&json!(request)),
                    None,
                )
                .await;
            match result {
                Ok((value, _)) => match serde_json::from_value::<Attempt>(value) {
                    Ok(attempt) if attempt.id == request.id => {
                        self.pending.set(None);
                        if matches!(attempt.outcome, Outcome::Passed | Outcome::Accepted) {
                            #[cfg(feature = "demo")]
                            self.message
                                .local_status(if attempt.kind == Kind::Connection {
                                    leptos_i18n::t_string!(i18n, demo.profile_connection).into()
                                } else {
                                    leptos_i18n::t_string!(i18n, demo.test_email).into()
                                });
                            #[cfg(not(feature = "demo"))]
                            self.message.status(if attempt.kind == Kind::Connection {
                                leptos_i18n::t_string!(
                                    i18n,
                                    ui.connection_and_authentication_verified_no_email_sent
                                )
                                .into()
                            } else {
                                leptos_i18n::t_string!(
                                    i18n,
                                    ui.accepted_by_smtp_server_inbox_delivery_is_not_confirmed
                                )
                                .into()
                            });
                        } else if !attempt.outcome.active() {
                            self.message.error(diagnostic(&attempt));
                        }
                    }
                    _ => self
                        .message
                        .error(leptos_i18n::t_string!(i18n, ui.profile_test_reconcile).into()),
                },
                Err(error) => {
                    if (400..500).contains(&error.status) {
                        self.pending.set(None);
                        self.message.error(error.message);
                    } else {
                        self.message
                            .error(leptos_i18n::t_string!(i18n, ui.profile_test_reconcile).into());
                    }
                }
            }
            self.busy.set(false);
            self.read.refresh.run(());
        });
    }
    pub(super) fn actions(self) -> impl IntoView {
        view! {
            <Show when=move || self.can_edit>
                <ActionMenu
                    label=trs!(ui.profile_test_actions)
                    disabled=Signal::derive(move || self.unavailable())
                >
                    <ActionButton
                        disabled=Signal::derive(move || self.unavailable())
                        on_click=move |_| self.run(Kind::Connection)
                    >
                        {move || {
                            if self.pending.with(Option::is_some) {
                                leptos_i18n::t_string!(self.api.i18n, ui.profile_test_check_result)
                            } else {
                                leptos_i18n::t_string!(
                                    self.api.i18n, ui.profile_test_check_connection
                                )
                            }
                        }}
                    </ActionButton>
                    <ActionButton
                        expanded=Signal::derive(move || self.expanded.get().to_string())
                        controls="profile-test-email"
                        disabled=Signal::derive(move || self.pending.with(Option::is_some))
                        on_click=move |_| self.expanded.set(true)
                    >
                        {tr!(ui.send_test_email)}
                    </ActionButton>
                </ActionMenu>
            </Show>
        }
    }
    pub(super) fn email_form(self) -> impl IntoView {
        view! {
            <Show when=move || self.expanded.get() && self.can_edit>
                <DetailCard>
                    <section
                        id="profile-test-email"
                        class="resource-section"
                        aria-label=trs!(ui.send_test_email)
                    >
                        <h2>{tr!(ui.send_test_email)}</h2>
                        <p>{tr!(ui.profile_test_email_help)}</p>
                        <form on:submit=move |event: leptos::ev::SubmitEvent| {
                            event.prevent_default();
                            self.run(Kind::Email);
                        }>
                            <div class="input-actions">
                                <Field label=trs!(ui.recipient_email)>
                                    <TextInput
                                        input_type="email"
                                        required=true
                                        disabled=Signal::derive(move || {
                                            self.busy.get() || self.pending.with(Option::is_some)
                                        })
                                        value=self.recipient
                                        on_input=move |event| {
                                            self.recipient.set(event_target_value(&event))
                                        }
                                    />
                                </Field>
                                <ActionButton
                                    kind="submit"
                                    style=ButtonStyle::Primary
                                    disabled=Signal::derive(move || {
                                        self.unavailable() || self.recipient.get().trim().is_empty()
                                    })
                                >
                                    {tr!(ui.send_test_email)}
                                </ActionButton>
                                <ActionButton
                                    style=ButtonStyle::Ghost
                                    disabled=self.busy
                                    on_click=move |_| self.expanded.set(false)
                                >
                                    {tr!(ui.close)}
                                </ActionButton>
                            </div>
                        </form>
                    </section>
                </DetailCard>
            </Show>
            {self.message.view()}
        }
    }
    pub(super) fn results(self) -> impl IntoView {
        let sorting = TableSorting::new("profile-test-history", "finished", SortDirection::Desc);
        view! {
            <QueryBoundary
                fetching=self.read.fetching
                has_data=Signal::derive(move || self.read.data.with(Option::is_some))
                error=self.read.error
                noun=trs!(ui.profile_test_history)
                refresh=self.read.refresh
            >
                {move || {
                    self.read
                        .data
                        .get()
                        .map(|history| {
                            let summary = history.summary;
                            let rows = summary.history;
                            let operators = StoredValue::new(history.operators);
                            view! {
                                <DetailSection title=trs!(ui.profile_test_results)>
                                    <DetailGrid>
                                        <DetailField label=trs!(
                                            ui.profile_test_connection
                                        )>
                                            {badge(summary.connection, summary.generation)}
                                        </DetailField>
                                        <DetailField label=trs!(
                                            ui.profile_test_submission
                                        )>{badge(summary.email, summary.generation)}</DetailField>
                                    </DetailGrid>
                                    {summary
                                        .active
                                        .map(|_| view! { <p role="status">{tr!(ui.testing)}</p> })}
                                    <p>
                                        {if summary.required && !summary.eligible {
                                            leptos_i18n::t_string!(
                                                self.api.i18n, ui.profile_test_needed
                                            )
                                        } else if summary.required {
                                            leptos_i18n::t_string!(self.api.i18n, ui.profile_test_ready)
                                        } else {
                                            leptos_i18n::t_string!(
                                                self.api.i18n, ui.profile_test_not_required
                                            )
                                        }}
                                    </p>
                                </DetailSection>
                                <DetailSection title=trs!(ui.profile_test_history)>
                                    <p>{tr!(ui.profile_test_history_help)}</p>
                                    <DataTable
                                        sorting=sorting
                                        columns=vec![
                                            "finished",
                                            "kind",
                                            "operator",
                                            "email",
                                            "outcome",
                                            "duration",
                                            "details",
                                        ]
                                        label=trs!(ui.profile_test_history)
                                        empty=rows.is_empty()
                                        headers=vec![
                                            trs!(ui.finished),
                                            trs!(ui.profile_test_type),
                                            trs!(ui.profile_test_operator),
                                            trs!(ui.recipient_email),
                                            trs!(ui.outcome),
                                            trs!(ui.duration),
                                            trs!(ui.details),
                                        ]
                                    >
                                        <TableEmpty slot:empty_content>
                                            <p>{tr!(ui.profile_test_no_history)}</p>
                                        </TableEmpty>
                                        {sorting
                                            .rows(
                                                rows,
                                                |r, key| match key {
                                                    "finished" => {
                                                        r.finished_at.map_or(SortValue::Missing, Into::into)
                                                    }
                                                    "email" => r.email.clone().unwrap_or_default().into(),
                                                    "operator" => {
                                                        identity_name(
                                                                operators
                                                                    .with_value(|users| {
                                                                        users.iter().find(|u| u.id == r.operator).cloned()
                                                                    }),
                                                            )
                                                            .into()
                                                    }
                                                    "kind" => kind_label(r.kind).into(),
                                                    "outcome" => outcome_label(r.outcome).into(),
                                                    "duration" => {
                                                        r.finished_at
                                                            .map(|t| t - r.started_at)
                                                            .map_or(SortValue::Missing, Into::into)
                                                    }
                                                    _ => r.code.clone().into(),
                                                },
                                                |r| r.id.clone().into(),
                                            )
                                            .into_iter()
                                            .map(move |row| {
                                                let operator = identity_name(
                                                    operators
                                                        .with_value(|users| {
                                                            users.iter().find(|u| u.id == row.operator).cloned()
                                                        }),
                                                );
                                                let changed = row.generation != summary.generation;
                                                let details = diagnostic(&row);
                                                view! {
                                                    <tr>
                                                        <td>{row.finished_at.map(instant).unwrap_or_default()}</td>
                                                        <td>{kind_label(row.kind)}</td>
                                                        <td>{operator}</td>
                                                        <td>
                                                            <ReadValue
                                                                value=row.email.unwrap_or_default()
                                                                fallback=trs!(ui.not_applicable)
                                                            />
                                                        </td>
                                                        <td>
                                                            <StatusBadge tone=tone(
                                                                row.outcome,
                                                            )>{outcome_label(row.outcome)}</StatusBadge>
                                                        </td>
                                                        <td>
                                                            {row
                                                                .finished_at
                                                                .map(|t| format!("{} s", t - row.started_at))
                                                                .unwrap_or_default()}
                                                        </td>
                                                        <td>
                                                            {details}
                                                            {changed
                                                                .then(|| {
                                                                    view! {
                                                                        <span>{tr!(ui.profile_test_settings_changed)}</span>
                                                                    }
                                                                })}
                                                        </td>
                                                    </tr>
                                                }
                                            })
                                            .collect_view()}
                                    </DataTable>
                                </DetailSection>
                            }
                        })
                }}
            </QueryBoundary>
        }
    }
}

/// Read the selected profile without hiding or replacing an ineligible selection.
pub(super) fn campaign_readiness(id: Signal<i64>) -> impl IntoView {
    let queries = expect_context::<ManagementQueries>();
    view! {
        <For
            each=move || { if id.get() > 0 { vec![id.get()] } else { Vec::new() } }
            key=|id| *id
            children=move |id| {
                let read = queries.profile_tests.observe_polling(move || id, true.into());
                view! {
                    <QueryBoundary
                        fetching=read.fetching
                        has_data=Signal::derive(move || read.data.with(Option::is_some))
                        error=read.error
                        noun=trs!(ui.profile_test_results)
                        refresh=read.refresh
                    >
                        {move || {
                            read.data
                                .get()
                                .filter(|h| h.summary.required && !h.summary.eligible)
                                .map(|_| {
                                    view! {
                                        <p class="form-message error" role="status">
                                            {tr!(ui.profile_test_needed)}
                                            " "
                                            <a
                                                href=format!("/smtp/{id}")
                                                target="_blank"
                                                rel="noopener"
                                            >
                                                {tr!(ui.profile_test_open)}
                                            </a>
                                        </p>
                                    }
                                })
                        }}
                    </QueryBoundary>
                }
            }
        />
    }
}
fn instant(seconds: i64) -> String {
    crate::shared::datetime::display::instant(&json!(seconds))
}
fn kind_label(kind: Kind) -> String {
    let i18n = crate::localization::use_i18n();
    match kind {
        Kind::Connection => leptos_i18n::t_string!(i18n, ui.profile_test_connection),
        Kind::Email => leptos_i18n::t_string!(i18n, ui.profile_test_submission),
    }
    .into()
}
fn outcome_label(outcome: Outcome) -> String {
    let i18n = crate::localization::use_i18n();
    match outcome {
        Outcome::Running | Outcome::Submitting => leptos_i18n::t_string!(i18n, ui.testing),
        Outcome::Passed => leptos_i18n::t_string!(i18n, ui.passed),
        Outcome::Accepted => leptos_i18n::t_string!(i18n, ui.profile_test_accepted),
        Outcome::Failed => leptos_i18n::t_string!(i18n, ui.failed),
        Outcome::Interrupted => leptos_i18n::t_string!(i18n, ui.profile_test_interrupted),
        Outcome::Unknown => leptos_i18n::t_string!(i18n, ui.outcome_unknown),
    }
    .into()
}
fn tone(outcome: Outcome) -> StatusTone {
    match outcome {
        Outcome::Passed | Outcome::Accepted => StatusTone::Positive,
        Outcome::Failed => StatusTone::Negative,
        Outcome::Unknown | Outcome::Interrupted => StatusTone::Warning,
        _ => StatusTone::Active,
    }
}
fn badge(attempt: Option<Attempt>, generation: i64) -> AnyView {
    match attempt {
        None => view! { <StatusBadge tone=StatusTone::Neutral>{tr!(ui.profile_test_not_tested)}</StatusBadge> }.into_any(),
        Some(attempt) => {
            let changed = attempt.generation != generation;
            view! {
                <span class="status-summary">
                    <StatusBadge tone=if changed {
                        StatusTone::Warning
                    } else {
                        tone(attempt.outcome)
                    }>{outcome_label(attempt.outcome)}</StatusBadge>
                    <span>{attempt.finished_at.map(instant)}</span>
                    {changed
                        .then(|| view! { <span>{tr!(ui.profile_test_settings_changed)}</span> })}
                </span>
            }.into_any()
        }
    }
}
fn diagnostic(attempt: &Attempt) -> String {
    let i18n = crate::localization::use_i18n();
    let text = match attempt.code.as_str() {
        "" => String::new(),
        "profile_test_interrupted" => {
            leptos_i18n::t_string!(i18n, ui.profile_test_interrupted_help).into()
        }
        "delivery_content_invalid" => {
            leptos_i18n::t_string!(i18n, ui.profile_test_error_content).into()
        }
        "smtp_capacity_unavailable" => {
            leptos_i18n::t_string!(i18n, ui.profile_test_error_capacity).into()
        }
        "smtp_profile_blocked" => {
            leptos_i18n::t_string!(i18n, ui.profile_test_error_profile).into()
        }
        "smtp_dns_unavailable" => leptos_i18n::t_string!(i18n, ui.profile_test_error_dns).into(),
        "smtp_connect_unavailable" => {
            leptos_i18n::t_string!(i18n, ui.profile_test_error_connect).into()
        }
        "smtp_temporary_rejection" => {
            leptos_i18n::t_string!(i18n, ui.profile_test_error_temporary).into()
        }
        _ => crate::localization::delivery_failure_label(i18n, &attempt.code),
    };
    match attempt.smtp_code {
        Some(code) => format!("{text} (SMTP {code})"),
        None => text,
    }
}
