//! Company policy and the separately confirmed installation-wide closure action.
use crate::api::client::Api;
use crate::management_queries::ManagementQueries;
use crate::settings::draft::Draft;
use crate::shared::components::*;
use crate::shared::feedback::Message;
use crate::tr;
use crate::trs;
use leptos::prelude::*;
use ruphish_contracts::settings::{ClosurePreview, ClosureResult, Features, Settings};
use serde_json::json;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Activity {
    Idle,
    ConfirmingPolicy,
    SavingPolicy,
    PreviewingClosure,
    ClosingCampaigns,
}

pub(crate) fn disabled(admin: bool) -> impl IntoView {
    view! {
        <ManagementPage>
            <Card>
                <h1>{tr!(ui.feature_disabled_title)}</h1>
                <p role="status">{tr!(ui.feature_disabled_description)}</p>
                {admin
                    .then(|| {
                        view! {
                            <ActionLink href="/settings">{tr!(ui.company_settings)}</ActionLink>
                        }
                    })}
            </Card>
        </ManagementPage>
    }
}

pub(crate) fn page(api: Api, token: String) -> impl IntoView {
    let i18n = crate::localization::use_i18n();
    let queries = expect_context::<ManagementQueries>();
    let read = queries.settings.observe(|| ());
    let draft = RwSignal::new(None::<Draft>);
    let validity = Signal::derive(move || {
        draft.with(|d| d.as_ref().map(|d| d.validity.clone()).unwrap_or_default())
    });
    let recheck = Signal::derive(move || {
        draft.with(|d| d.as_ref().map(|d| d.recheck.clone()).unwrap_or_default())
    });
    let token = StoredValue::new(token);
    let tasks = crate::shared::tasks::ViewTasks::default();
    let activity = RwSignal::new(Activity::Idle);
    let busy = Signal::derive(move || {
        matches!(
            activity.get(),
            Activity::SavingPolicy | Activity::PreviewingClosure | Activity::ClosingCampaigns
        )
    });
    let message = Message::new(String::new());
    let closure = RwSignal::new(None::<ClosurePreview>);
    let confirmation = RwSignal::new(String::new());
    let closure_message = Message::new(String::new());
    Effect::new(move || {
        if let Some(saved) = read.data.get() {
            draft.update(|value| match value {
                Some(value) => value.refresh(Some(saved)),
                None => *value = Some(Draft::from_saved(saved)),
            });
        }
    });
    let edited = move || draft.with(|d| d.as_ref().and_then(Draft::edited));
    let dirty = move || draft.with(|d| d.as_ref().is_some_and(Draft::dirty));
    let conflict = move || {
        read.data.with(|saved| {
            draft.with(|d| {
                d.as_ref()
                    .zip(saved.as_ref())
                    .is_some_and(|(d, saved)| d.conflicts(saved))
            })
        })
    };
    crate::shared::navigation::protect_draft(move || {
        dirty() || busy.get() || closure.get().is_some()
    });
    let validation = FormValidation::new(move || {
        let Some(current) = draft.get() else {
            return Default::default();
        };
        let mut policy = current.policy;
        let mut report = ruphish_domain::validation::Report::default();
        match current.validity.parse::<i64>() {
            Ok(value) => policy.verification_validity_seconds = value,
            Err(_) => report.reject(
                "verification_validity_seconds",
                ruphish_domain::validation::Code::Invalid,
                "Enter whole seconds",
            ),
        }
        match current.recheck.parse::<i64>() {
            Ok(value) => policy.verification_recheck_seconds = value,
            Err(_) => report.reject(
                "verification_recheck_seconds",
                ruphish_domain::validation::Code::Invalid,
                "Enter whole seconds",
            ),
        }
        report.extend(policy.validation());
        report
    });
    let save = Callback::new(move |()| {
        if activity.get_untracked() != Activity::ConfirmingPolicy || !dirty() || conflict() {
            return;
        }
        if !validation.check() {
            return;
        }
        let Some((revision, policy)) = edited() else {
            return;
        };
        let ticket = validation.ticket();
        message.clear();
        activity.set(Activity::SavingPolicy);
        tasks.spawn(async move {
            let result = api
                .resource_response(
                    "PUT",
                    "/api/settings",
                    &token.get_value(),
                    Some(&json!({"policy":policy})),
                    Some(&revision.to_string()),
                )
                .await
                .and_then(|(value, _)| {
                    serde_json::from_value::<Settings>(value).map_err(|_| api.invalid_read_err())
                });
            match result {
                Ok(saved) => {
                    #[cfg(feature = "demo")]
                    api.notifications.show(DemoNotice::Saved);
                    api.set_features(Features::from(&saved.policy));
                    draft.set(Some(Draft::from_saved(saved.clone())));
                    queries.settings.confirmed((), |value| *value = saved);
                    queries.policy_changed();
                    message.status(leptos_i18n::t_string!(i18n, ui.settings_saved).to_owned());
                }
                Err(error) => {
                    validation.server(ticket, &error);
                    message.error(error.message);
                }
            }
            activity.set(Activity::Idle);
        });
    });
    let preview = move |_| {
        if busy.get_untracked() {
            return;
        }
        message.clear();
        closure_message.clear();
        activity.set(Activity::PreviewingClosure);
        tasks.spawn(async move {
            match api
                .resource(
                    "POST",
                    "/api/campaigns/complete-all/preview",
                    &token.get_value(),
                    None,
                    None,
                )
                .await
                .and_then(|(value, _)| {
                    serde_json::from_value::<ClosurePreview>(value).map_err(|_| api.invalid_read())
                }) {
                Ok(value) => {
                    confirmation.set(String::new());
                    closure.set(Some(value));
                }
                Err(error) => message.error(error),
            }
            activity.set(Activity::Idle);
        });
    };
    let close = Callback::new(move |()| {
        if busy.get_untracked() || confirmation.get_untracked() != "CLOSE ALL" {
            return;
        }
        let Some(review) = closure.get_untracked() else {
            return;
        };
        closure_message.clear();
        activity.set(Activity::ClosingCampaigns);
        tasks.spawn(async move {
            match api
                .resource(
                    "POST",
                    "/api/campaigns/complete-all",
                    &token.get_value(),
                    Some(&json!({"token":review.token,"confirmation":"CLOSE ALL"})),
                    None,
                )
                .await
                .and_then(|(value, _)| {
                    serde_json::from_value::<ClosureResult>(value).map_err(|_| api.invalid_read())
                }) {
                Ok(_) => {
                    closure.set(None);
                    #[cfg(feature = "demo")]
                    api.notifications.show(DemoNotice::External);
                    queries.policy_changed();
                    message
                        .status(leptos_i18n::t_string!(i18n, ui.all_campaigns_closed).to_owned());
                }
                Err(error) => closure_message.error(error),
            }
            activity.set(Activity::Idle);
        });
    });
    view! {
        <ManagementPage>
            <PageHeader title=trs!(ui.company_settings) subtitle=trs!(ui.settings_scope)>
                <RefreshButton fetching=read.fetching on_refresh=read.refresh disabled=busy />
            </PageHeader>
            {message.view()}
            <Show when=move || activity.get() == Activity::SavingPolicy>
                <p role="status">{tr!(ui.saving)}</p>
            </Show>
            <ValidationSummary form=validation />
            <QueryBoundary
                fetching=read.fetching
                has_data=Signal::derive(move || read.data.get().is_some())
                error=read.error
                noun=trs!(ui.company_settings)
                refresh=read.refresh
            >
                <div class="settings-stack">
                    <form
                        novalidate
                        on:input=move |_| message.clear_status()
                        on:change=move |_| message.clear_status()
                        on:submit=move |event: leptos::ev::SubmitEvent| {
                            event.prevent_default();
                            if activity.get_untracked() == Activity::Idle && dirty() && !conflict()
                            {
                                message.clear();
                                if validation.check() {
                                    activity.set(Activity::ConfirmingPolicy);
                                }
                            }
                        }
                    >
                        <FormFields disabled=Signal::derive(move || {
                            busy.get() || draft.get().is_none()
                        })>
                            <div class="settings-sections">
                                <section class="settings-section">
                                    <h2>{tr!(ui.domain_verification)}</h2>
                                    <Checkbox
                                        label=trs!(ui.settings_verification_enabled)
                                        checked=Signal::derive(move || {
                                            draft
                                                .with(|d| {
                                                    d.as_ref().is_some_and(|d| d.policy.verification_enabled)
                                                })
                                        })
                                        on_toggle=Callback::new(move |v| {
                                            draft
                                                .update(|d| {
                                                    if let Some(d) = d {
                                                        d.policy.verification_enabled = v;
                                                    }
                                                })
                                        })
                                    />
                                    <p>{tr!(ui.settings_domains_retained)}</p>
                                    <Show when=move || {
                                        draft
                                            .with(|d| {
                                                d.as_ref().is_some_and(|d| d.policy.verification_enabled)
                                            })
                                            || validation
                                                .error("verification_validity_seconds")
                                                .is_some()
                                            || validation
                                                .error("verification_recheck_seconds")
                                                .is_some()
                                    }>
                                        <Checkbox
                                            label=trs!(ui.settings_verification_expires)
                                            checked=Signal::derive(move || {
                                                draft
                                                    .with(|d| {
                                                        d.as_ref().is_some_and(|d| d.policy.verification_expires)
                                                    })
                                            })
                                            on_toggle=Callback::new(move |v| {
                                                draft
                                                    .update(|d| {
                                                        if let Some(d) = d {
                                                            d.policy.verification_expires = v;
                                                        }
                                                    })
                                            })
                                        />
                                        <div class="settings-fields">
                                            <Field label=trs!(ui.settings_validity_seconds)>
                                                <TextInput
                                                    input_type="number"
                                                    step="1"
                                                    validation_field="verification_validity_seconds"
                                                    value=validity
                                                    on_input=move |e| {
                                                        draft
                                                            .update(|d| {
                                                                if let Some(d) = d {
                                                                    d.validity = event_target_value(&e);
                                                                }
                                                            })
                                                    }
                                                />
                                            </Field>
                                            <Field label=trs!(ui.settings_recheck_seconds)>
                                                <TextInput
                                                    input_type="number"
                                                    step="1"
                                                    validation_field="verification_recheck_seconds"
                                                    value=recheck
                                                    on_input=move |e| {
                                                        draft
                                                            .update(|d| {
                                                                if let Some(d) = d {
                                                                    d.recheck = event_target_value(&e);
                                                                }
                                                            })
                                                    }
                                                />
                                            </Field>
                                        </div>
                                        <p>{tr!(ui.settings_duration_help)}</p>
                                    </Show>
                                </section>
                                <section class="settings-section">
                                    <h2>{tr!(ui.sending_profiles)}</h2>
                                    <Checkbox
                                        label=trs!(ui.profile_test_policy)
                                        checked=Signal::derive(move || {
                                            draft
                                                .with(|d| {
                                                    d.as_ref()
                                                        .is_some_and(|d| d.policy.sending_profile_test_required)
                                                })
                                        })
                                        on_toggle=Callback::new(move |v| {
                                            draft
                                                .update(|d| {
                                                    if let Some(d) = d {
                                                        d.policy.sending_profile_test_required = v;
                                                    }
                                                })
                                        })
                                    />
                                    <p>{tr!(ui.profile_test_policy_help)}</p>
                                </section>
                                <section class="settings-section">
                                    <h2>{tr!(ui.settings_integrations)}</h2>
                                    <Checkbox
                                        label=trs!(ui.reporting_mailboxes)
                                        checked=Signal::derive(move || {
                                            draft
                                                .with(|d| {
                                                    d.as_ref()
                                                        .is_some_and(|d| d.policy.reporting_mailboxes_enabled)
                                                })
                                        })
                                        on_toggle=Callback::new(move |v| {
                                            draft
                                                .update(|d| {
                                                    if let Some(d) = d {
                                                        d.policy.reporting_mailboxes_enabled = v;
                                                    }
                                                })
                                        })
                                    />
                                    <p>{tr!(ui.settings_mailboxes_help)}</p>
                                    <Checkbox
                                        label=trs!(ui.webhooks)
                                        checked=Signal::derive(move || {
                                            draft
                                                .with(|d| {
                                                    d.as_ref().is_some_and(|d| d.policy.webhooks_enabled)
                                                })
                                        })
                                        on_toggle=Callback::new(move |v| {
                                            draft
                                                .update(|d| {
                                                    if let Some(d) = d {
                                                        d.policy.webhooks_enabled = v;
                                                    }
                                                })
                                        })
                                    />
                                    <p>{tr!(ui.settings_webhooks_help)}</p>
                                </section>
                                <Show when=conflict>
                                    <p role="status">{tr!(ui.settings_changed_review)}</p>
                                </Show>
                                <div class="management-actions">
                                    <ActionButton
                                        kind="submit"
                                        style=ButtonStyle::Primary
                                        disabled=Signal::derive(move || {
                                            busy.get() || !dirty() || conflict()
                                        })
                                    >
                                        {tr!(ui.save_changes)}
                                    </ActionButton>
                                    <ActionButton on_click=move |_| {
                                        if let Some(saved) = read.data.get_untracked() {
                                            draft.set(Some(Draft::from_saved(saved)));
                                            activity.set(Activity::Idle);
                                            message.clear();
                                        }
                                    }>{tr!(ui.settings_discard)}</ActionButton>
                                </div>
                            </div>
                        </FormFields>
                    </form>
                    <Card>
                        <h2>{tr!(ui.starter_title)}</h2>
                        <p>{tr!(ui.starter_description)}</p>
                        <ActionLink href="/settings/starter-templates">
                            {tr!(ui.starter_browse)}
                        </ActionLink>
                    </Card>
                    <Card class="settings-danger">
                        <h2>{tr!(ui.settings_danger_zone)}</h2>
                        <p>{tr!(ui.close_all_description)}</p>
                        <ActionButton style=ButtonStyle::Danger disabled=busy on_click=preview>
                            {tr!(ui.close_all_campaigns)}
                        </ActionButton>
                    </Card>
                </div>
                <Show when=move || activity.get() == Activity::ConfirmingPolicy>
                    <Dialog
                        title=trs!(ui.settings_confirm_title)
                        busy=busy
                        on_cancel=Callback::new(move |()| activity.set(Activity::Idle))
                    >
                        <h2>{tr!(ui.settings_confirm_title)}</h2>
                        <p>{tr!(ui.settings_confirm_description)}</p>
                        <div class="confirmation-actions">
                            <ActionButton
                                disabled=busy
                                on_click=move |_| activity.set(Activity::Idle)
                            >
                                {tr!(ui.cancel)}
                            </ActionButton>
                            <ActionButton
                                disabled=Signal::derive(move || busy.get() || conflict())
                                style=ButtonStyle::Primary
                                on_click=move |_| save.run(())
                            >
                                {tr!(ui.save_changes)}
                            </ActionButton>
                        </div>
                    </Dialog>
                </Show>
                <Show when=move || closure.get().is_some()>
                    <Dialog
                        title=trs!(ui.close_all_campaigns)
                        busy=busy
                        on_cancel=Callback::new(move |()| closure.set(None))
                    >
                        <h2>{tr!(ui.close_all_campaigns)}</h2>
                        <p>{tr!(ui.close_all_description)}</p>
                        <dl class="resource-facts">
                            <div>
                                <dt>{tr!(ui.campaigns)}</dt>
                                <dd>{move || closure.get().map(|r| r.campaigns)}</dd>
                            </div>
                            <div>
                                <dt>{tr!(ui.settings_pending_deliveries)}</dt>
                                <dd>{move || closure.get().map(|r| r.pending_deliveries)}</dd>
                            </div>
                        </dl>
                        <p>{tr!(ui.close_all_retry_help)}</p>
                        <Field label=trs!(ui.close_all_type_confirmation)>
                            <TextInput
                                value=confirmation
                                disabled=busy
                                on_input=move |e| confirmation.set(event_target_value(&e))
                            />
                        </Field>
                        {closure_message.view()}
                        <div class="confirmation-actions">
                            <ActionButton disabled=busy on_click=move |_| closure.set(None)>
                                {tr!(ui.cancel)}
                            </ActionButton>
                            <ActionButton
                                style=ButtonStyle::Danger
                                disabled=Signal::derive(move || {
                                    busy.get() || confirmation.get() != "CLOSE ALL"
                                        || closure.get().is_none_or(|r| r.campaigns == 0)
                                })
                                on_click=move |_| close.run(())
                            >
                                {tr!(ui.close_all_campaigns)}
                            </ActionButton>
                        </div>
                    </Dialog>
                </Show>
            </QueryBoundary>
        </ManagementPage>
    }
}
