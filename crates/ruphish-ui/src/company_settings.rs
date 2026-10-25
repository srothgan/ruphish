//! Company policy and the separately confirmed installation-wide closure action.
use crate::{
    client::Api, feedback::Message, management_queries::ManagementQueries, shared::components::*,
};
use leptos::{prelude::*, task::spawn_local_scoped_with_cancellation as spawn_local};
use ruphish_contracts::settings::{ClosurePreview, ClosureResult, Features, Policy, Settings};
use ruphish_ui::{tr, trs};
use serde_json::json;

pub(super) fn disabled(admin: bool) -> impl IntoView {
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

pub(super) fn page(api: Api, token: String) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    let queries = expect_context::<ManagementQueries>();
    let read = queries.settings.observe(|| ());
    let draft = RwSignal::new(None::<(i64, Policy)>);
    let validity = RwSignal::new(String::new());
    let recheck = RwSignal::new(String::new());
    let token = StoredValue::new(token);
    let busy = RwSignal::new(false);
    let message = Message::new(String::new());
    let confirming_policy = RwSignal::new(false);
    let closure = RwSignal::new(None::<ClosurePreview>);
    let confirmation = RwSignal::new(String::new());
    let closure_message = Message::new(String::new());
    Effect::new(move || {
        if draft.get().is_none()
            && let Some(saved) = read.data.get()
        {
            validity.set(saved.policy.verification_validity_seconds.to_string());
            recheck.set(saved.policy.verification_recheck_seconds.to_string());
            draft.set(Some((saved.revision, saved.policy)));
        }
    });
    let edited = move || {
        let (revision, mut policy) = draft.get()?;
        policy.verification_validity_seconds = validity.get().parse::<i64>().ok()?;
        policy.verification_recheck_seconds = recheck.get().parse::<i64>().ok()?;
        Some((revision, policy))
    };
    let dirty = move || match (edited(), read.data.get()) {
        (Some((_, p)), Some(saved)) => p != saved.policy,
        (None, Some(_)) => true,
        _ => false,
    };
    crate::navigation::protect_draft(move || dirty() || busy.get() || closure.get().is_some());
    let save = Callback::new(move |()| {
        if busy.get_untracked() {
            return;
        }
        let Some((revision, policy)) = edited() else {
            message.error(leptos_i18n::t_string!(i18n, ui.settings_invalid_durations).to_owned());
            return;
        };
        if policy.validate().is_err() {
            message.error(leptos_i18n::t_string!(i18n, ui.settings_invalid_durations).to_owned());
            return;
        }
        busy.set(true);
        spawn_local(async move {
            let result = api
                .resource(
                    "PUT",
                    "/api/settings",
                    &token.get_value(),
                    Some(&json!({"policy":policy})),
                    Some(&revision.to_string()),
                )
                .await
                .and_then(|(value, _)| {
                    serde_json::from_value::<Settings>(value).map_err(|_| api.invalid_read())
                });
            match result {
                Ok(saved) => {
                    api.set_features(Features::from(&saved.policy));
                    draft.set(Some((saved.revision, saved.policy.clone())));
                    queries.settings.confirmed((), |value| *value = saved);
                    queries.policy_changed();
                    confirming_policy.set(false);
                    message.status(leptos_i18n::t_string!(i18n, ui.settings_saved).to_owned());
                }
                Err(error) => message.error(error),
            }
            busy.set(false);
        });
    });
    let preview = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        spawn_local(async move {
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
            busy.set(false);
        });
    };
    let close = Callback::new(move |()| {
        if busy.get_untracked() || confirmation.get_untracked() != "CLOSE ALL" {
            return;
        }
        let Some(review) = closure.get_untracked() else {
            return;
        };
        busy.set(true);
        spawn_local(async move {
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
                    queries.policy_changed();
                    message
                        .status(leptos_i18n::t_string!(i18n, ui.all_campaigns_closed).to_owned());
                }
                Err(error) => closure_message.error(error),
            }
            busy.set(false);
        });
    });
    view! {
        <ManagementPage>
            <PageHeader title=trs!(ui.company_settings) subtitle=trs!(ui.settings_scope)>
                <ActionButton disabled=busy on_click=move |_| read.refresh.run(())>
                    {tr!(ui.refresh)}
                </ActionButton>
            </PageHeader>
            {message.view()}
            <QueryStatus
                fetching=read.fetching
                has_data=Signal::derive(move || read.data.get().is_some())
                error=read.error
                noun=trs!(ui.company_settings)
                refresh=read.refresh
            />
            <div class="settings-stack">
                <form on:submit=move |event: leptos::ev::SubmitEvent| {
                    event.prevent_default();
                    confirming_policy.set(true);
                }>
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
                                                d.as_ref().is_some_and(|(_, p)| p.verification_enabled)
                                            })
                                    })
                                    on_toggle=Callback::new(move |v| {
                                        draft
                                            .update(|d| {
                                                if let Some((_, p)) = d {
                                                    p.verification_enabled = v;
                                                }
                                            })
                                    })
                                />
                                <p>{tr!(ui.settings_domains_retained)}</p>
                                <Show when=move || {
                                    draft
                                        .with(|d| {
                                            d.as_ref().is_some_and(|(_, p)| p.verification_enabled)
                                        })
                                }>
                                    <Checkbox
                                        label=trs!(ui.settings_verification_expires)
                                        checked=Signal::derive(move || {
                                            draft
                                                .with(|d| {
                                                    d.as_ref().is_some_and(|(_, p)| p.verification_expires)
                                                })
                                        })
                                        on_toggle=Callback::new(move |v| {
                                            draft
                                                .update(|d| {
                                                    if let Some((_, p)) = d {
                                                        p.verification_expires = v;
                                                    }
                                                })
                                        })
                                    />
                                    <div class="settings-fields">
                                        <Field label=trs!(ui.settings_validity_seconds)>
                                            <TextInput
                                                input_type="number"
                                                step="1"
                                                value=validity
                                                on_input=move |e| validity.set(event_target_value(&e))
                                                disabled=Signal::derive(move || {
                                                    draft
                                                        .with(|d| {
                                                            d.as_ref().is_none_or(|(_, p)| !p.verification_expires)
                                                        })
                                                })
                                            />
                                        </Field>
                                        <Field label=trs!(ui.settings_recheck_seconds)>
                                            <TextInput
                                                input_type="number"
                                                step="1"
                                                value=recheck
                                                on_input=move |e| recheck.set(event_target_value(&e))
                                            />
                                        </Field>
                                    </div>
                                    <p>{tr!(ui.settings_duration_help)}</p>
                                </Show>
                            </section>
                            <section class="settings-section">
                                <h2>{tr!(ui.settings_integrations)}</h2>
                                <Checkbox
                                    label=trs!(ui.reporting_mailboxes)
                                    checked=Signal::derive(move || {
                                        draft
                                            .with(|d| {
                                                d.as_ref()
                                                    .is_some_and(|(_, p)| p.reporting_mailboxes_enabled)
                                            })
                                    })
                                    on_toggle=Callback::new(move |v| {
                                        draft
                                            .update(|d| {
                                                if let Some((_, p)) = d {
                                                    p.reporting_mailboxes_enabled = v;
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
                                                d.as_ref().is_some_and(|(_, p)| p.webhooks_enabled)
                                            })
                                    })
                                    on_toggle=Callback::new(move |v| {
                                        draft
                                            .update(|d| {
                                                if let Some((_, p)) = d {
                                                    p.webhooks_enabled = v;
                                                }
                                            })
                                    })
                                />
                                <p>{tr!(ui.settings_webhooks_help)}</p>
                            </section>
                            <div class="management-actions">
                                <ActionButton
                                    kind="submit"
                                    style=ButtonStyle::Primary
                                    disabled=Signal::derive(move || !dirty())
                                >
                                    {tr!(ui.save_changes)}
                                </ActionButton>
                                <ActionButton on_click=move |_| {
                                    draft.set(None);
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
            <Show when=move || confirming_policy.get()>
                <Dialog
                    title=trs!(ui.settings_confirm_title)
                    busy=busy
                    on_cancel=Callback::new(move |()| confirming_policy.set(false))
                >
                    <h2>{tr!(ui.settings_confirm_title)}</h2>
                    <p>{tr!(ui.settings_confirm_description)}</p>
                    {message.view()}
                    <div class="confirmation-actions">
                        <ActionButton disabled=busy on_click=move |_| confirming_policy.set(false)>
                            {tr!(ui.cancel)}
                        </ActionButton>
                        <ActionButton
                            disabled=busy
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
        </ManagementPage>
    }
}
