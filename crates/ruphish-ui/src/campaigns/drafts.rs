//! Durable campaign drafts use the shared management surface and request lifecycle.
use crate::api::client::Api;
use crate::campaigns::pages::array;
use crate::campaigns::pages::text;
use crate::campaigns::queries::CampaignQueries;
use crate::shared::components::*;
use crate::shared::feedback::Message;
use crate::shared::tasks::ViewTasks;
use crate::tr;
use crate::trs;
use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_params_map};
use serde_json::{Value, json};

#[derive(Clone, Copy)]
pub(crate) struct Persistence {
    queries: CampaignQueries,
    state: RwSignal<super::persistence_state::PersistenceState>,
    pub id: Signal<String>,
    pub revision: Signal<Option<i64>>,
    pub saved_generation: Signal<u64>,
    pub modified: Signal<Value>,
    pub attribution: Signal<ruphish_contracts::attribution::Attribution>,
    pub unconfirmed: Signal<bool>,
    pub saving: RwSignal<bool>,
    pub write_error: RwSignal<Option<crate::api::error::ApiError>>,
}
impl Persistence {
    pub fn new(api: Api, saved: Option<&Value>) -> Result<Self, String> {
        use super::persistence_state::PersistenceState;
        let initial = match saved {
            Some(saved) => {
                let parsed =
                    serde_json::from_value(saved.clone()).map_err(|_| api.invalid_read())?;
                PersistenceState::loaded(parsed).ok_or_else(|| api.invalid_read())?
            }
            None => PersistenceState::default(),
        };
        let state = RwSignal::new(initial);
        Ok(Self {
            queries: expect_context::<CampaignQueries>(),
            state,
            id: Signal::derive(move || {
                state.with(|s| s.id().map(|id| id.to_string()).unwrap_or_default())
            }),
            revision: Signal::derive(move || state.with(PersistenceState::revision)),
            saved_generation: Signal::derive(move || {
                state.with(PersistenceState::saved_generation)
            }),
            modified: Signal::derive(move || {
                state.with(|s| json!(s.confirmed().and_then(|r| r.modified.clone())))
            }),
            attribution: Signal::derive(move || {
                state.with(|s| {
                    s.confirmed()
                        .map(|r| r.attribution.clone())
                        .unwrap_or_default()
                })
            }),
            unconfirmed: Signal::derive(move || state.with(PersistenceState::unconfirmed)),
            saving: RwSignal::new(false),
            write_error: RwSignal::new(None),
        })
    }
    pub fn accept(
        self,
        api: Api,
        saved: &Value,
        generation: u64,
    ) -> Result<(), crate::api::error::ApiError> {
        let record = serde_json::from_value(saved.clone()).map_err(|_| api.invalid_read_err())?;
        if !self
            .state
            .try_update(|s| s.accept(record, generation))
            .unwrap_or(false)
        {
            return Err(api.invalid_read_err());
        }
        self.write_error.set(None);
        Ok(())
    }
    pub async fn save(
        self,
        api: Api,
        token: &str,
        state: &crate::campaigns::draft_state::Draft,
        step: usize,
    ) -> Result<Value, String> {
        let configuration =
            serde_json::from_value(state.value.clone()).map_err(|_| api.invalid_read())?;
        let create = serde_json::to_value(ruphish_contracts::campaign::request::DraftCreate {
            configuration,
            step: i64::try_from(step).map_err(|_| api.invalid_read())?,
        })
        .map_err(|_| api.invalid_read())?;
        if self.state.with_untracked(|s| s.is_new()) {
            let key = crate::campaigns::pages::random_key()
                .map_err(|_| crate::tr!(ui.cannot_generate_a_secure_launch_key)())?;
            self.state.update(|s| s.begin_creation(key, create.clone()));
        }
        self.saving.set(true);
        let result = async {
            if let Some((key, body)) = self.state.with_untracked(|s| s.creation()) {
                let saved = api
                    .create_response("/api/campaign-drafts", token, &key, &body)
                    .await?;
                let parsed: ruphish_contracts::campaign::reads::Draft =
                    serde_json::from_value(saved.clone()).map_err(|_| api.invalid_read_err())?;
                if !self
                    .state
                    .try_update(|s| s.created(parsed.id))
                    .unwrap_or(false)
                {
                    return Err(api.invalid_read_err());
                }
                if saved["configuration"] == create["configuration"]
                    && saved["step"] == create["step"]
                    && parsed.campaign_id.is_none()
                {
                    return Ok(saved);
                }
            }
            let mut body = create;
            body["revision"] = json!(self.revision.get_untracked());
            api.resource_response(
                "PUT",
                &format!("/api/campaign-drafts/{}", self.id.get_untracked()),
                token,
                Some(&body),
                None,
            )
            .await
            .map(|(saved, _)| saved)
        }
        .await
        .and_then(|saved| {
            self.accept(api, &saved, state.generation)?;
            Ok(saved)
        });
        self.saving.set(false);
        let saved = result.map_err(|error| {
            if matches!(error.status, 400 | 401 | 403 | 422) {
                self.state.update(|s| s.rejected());
            }
            self.write_error.set(Some(error.clone()));
            error.message
        })?;
        self.queries.draft_saved(&self.id.get_untracked());
        Ok(saved)
    }
}

pub(crate) fn page(api: Api, token: String, editing: bool) -> impl IntoView {
    let params = use_params_map();
    let token = StoredValue::new(token);
    view! {
        <For
            each=move || vec![params.with(|p| p.get("id").unwrap_or_default())]
            key=|id| id.clone()
            children=move |id| load(api, token.get_value(), id, editing)
        />
    }
}
fn load(api: Api, token: String, id: String, editing: bool) -> impl IntoView {
    if !editing {
        return read_only(api, token, id).into_any();
    }
    let id = StoredValue::new(id);
    let token = StoredValue::new(token);
    let tasks = ViewTasks::default();
    let data = RwSignal::new(None::<Value>);
    let load_error = RwSignal::new(None::<crate::api::error::ApiError>);
    let loading = RwSignal::new(false);
    let refresh = RwSignal::new(0u64);
    let navigate = use_navigate();
    Effect::new(move |_| {
        refresh.get();
        loading.set(true);
        let navigate = navigate.clone();
        tasks.spawn(async move {
            match CampaignQueries::read_draft(api, &id.get_value()).await {
                Ok(v) => {
                    load_error.set(None);
                    if let Some(campaign) = v["campaign_id"].as_i64() {
                        navigate(
                            &format!("/campaigns/{campaign}/delivery"),
                            Default::default(),
                        );
                    } else {
                        data.set(Some(v));
                    }
                }
                Err(e) => load_error.set(Some(e)),
            }
            loading.set(false);
        });
    });
    // Only a newly loaded record replaces the mounted form.
    let content = view! {
        <QueryBoundary
            fetching=loading.into()
            has_data=Signal::derive(move || data.with(Option::is_some))
            error=Signal::derive(move || load_error.get().map(|e| e.message))
            noun=trs!(ui.draft)
            refresh=Callback::new(move |_| refresh.update(|n| *n += 1))
        >

            {move || {
                data.get()
                    .map(|saved| {
                        view! {
                            <crate::campaigns::pages::CreateCampaign
                                api=api
                                token=token.get_value()
                                initial=Some(saved["configuration"].clone())
                                saved=Some(saved)
                            />
                        }
                    })
            }}

        </QueryBoundary>
    };
    view! {
        <ReadBoundary
            failure=load_error
            has_data=Signal::derive(move || data.with(Option::is_some))
            fetching=loading
            retry=Callback::new(move |()| refresh.update(|n| *n += 1))
            back_href="/campaigns"
            back_label=trs!(ui.back_to_campaigns)
        >
            {content}
        </ReadBoundary>
    }
    .into_any()
}
fn read_only(api: Api, token: String, id: String) -> impl IntoView {
    let queries = expect_context::<CampaignQueries>();
    let id = StoredValue::new(id);
    let read = queries.draft.observe(move || id.get_value());
    let saved = Signal::derive(move || read.data.get().unwrap_or(Value::Null));
    let confirmed = Signal::derive(move || read.data.with(Option::is_some));
    let config = Signal::derive(move || saved.get()["configuration"].clone());
    let token = StoredValue::new(token);
    let resources = CampaignQueries::resources(false);
    let names = Signal::derive(move || resources.data.get().unwrap_or(Value::Null));
    let names_known = Signal::derive(move || resources.data.with(Option::is_some));
    let tasks = ViewTasks::default();
    let busy = RwSignal::new(false);
    let deleting = RwSignal::new(None::<i64>);
    let message = Message::new(String::new());
    let delete_error = RwSignal::new(None::<crate::api::error::ApiError>);
    let needs_review = Signal::derive(move || {
        delete_error.with(|e| {
            e.as_ref()
                .is_some_and(crate::api::error::ApiError::needs_write_recovery)
        })
    });
    let navigate = use_navigate();
    let redirect = navigate.clone();
    Effect::new(move |_| {
        if let Some(campaign) = saved.get()["campaign_id"].as_i64() {
            redirect(
                &format!("/campaigns/{campaign}/delivery"),
                Default::default(),
            );
        }
    });
    let can_edit = crate::campaigns::pages::editable();
    let i18n = crate::localization::use_i18n();
    let title = Signal::derive(move || {
        let name = text(&config.get(), "name");
        if name.is_empty() {
            leptos_i18n::t_string!(i18n, ui.untitled_campaign).to_owned()
        } else {
            name
        }
    });
    let delete = Callback::new(move |()| {
        if busy.get_untracked() || needs_review.get_untracked() {
            return;
        }
        let Some(revision) = deleting.get_untracked() else {
            return;
        };
        busy.set(true);
        let navigate = navigate.clone();
        tasks.spawn(async move {
            match api
                .command_response(
                    "DELETE",
                    &format!(
                        "/api/campaign-drafts/{}?revision={revision}",
                        id.get_value()
                    ),
                    &token.get_value(),
                    None,
                    None,
                )
                .await
            {
                Ok(_) => {
                    queries.deleted("draft", &id.get_value());
                    navigate("/campaigns", Default::default());
                }
                Err(e) => {
                    message.error(e.message.clone());
                    delete_error.set(Some(e));
                }
            }
            busy.set(false);
        });
    });
    let content = view! {
        <ManagementPage>
            <BackLink href="/campaigns">{tr!(ui.back_to_campaigns)}</BackLink>
            <PageHeader title=title>
                <RefreshButton fetching=read.fetching on_refresh=read.refresh />
                <ResourceActions
                    disabled=Signal::derive(move || !confirmed.get() || busy.get())
                    edit_href=format!("/campaigns/drafts/{}/edit", id.get_value())
                    can_edit=can_edit
                    on_delete=Callback::new(move |()| {
                        delete_error.set(None);
                        message.clear();
                        deleting.set(saved.get_untracked()["revision"].as_i64())
                    })
                />
            </PageHeader>
            <QueryBoundary
                fetching=read.fetching
                has_data=confirmed
                error=read.error
                noun=trs!(ui.draft)
                refresh=read.refresh
            >
                <QueryBoundary
                    fetching=resources.fetching
                    has_data=names_known
                    error=resources.error
                    noun=trs!(ui.resources)
                    refresh=resources.refresh
                >
                    <Show when=move || confirmed.get()>
                        <DetailCard>
                            {message.view()} <DetailSection title=trs!(ui.details)>
                                <DetailGrid>
                                    <DetailField label=trs!(ui.status)>
                                        <StatusBadge tone=StatusTone::Warning>
                                            {tr!(ui.draft)}
                                        </StatusBadge>
                                    </DetailField>

                                    {[
                                        ("template", trs!(ui.email_template)),
                                        ("page", trs!(ui.landing_page)),
                                        ("smtp", trs!(ui.sending_profile)),
                                    ]
                                        .into_iter()
                                        .map(move |(key, label)| {
                                            view! {
                                                <div>
                                                    <dt>{move || label.get()}</dt>
                                                    <dd>
                                                        {move || {
                                                            let selected = config.get()["selection"][key]["id"].clone();
                                                            if !names_known.get() {
                                                                return leptos_i18n::t_string!(i18n,ui.unavailable)
                                                                    .to_owned();
                                                            }
                                                            array(&names.get(), key)
                                                                .iter()
                                                                .find(|v| v["id"] == selected)
                                                                .map(|v| text(v, "name"))
                                                                .unwrap_or_else(|| {
                                                                    if selected.as_i64().is_some_and(|id| id > 0) {
                                                                        leptos_i18n::t_string!(i18n,ui.unavailable).to_owned()
                                                                    } else {
                                                                        crate::tr!(ui.not_set)()
                                                                    }
                                                                })
                                                        }}
                                                    </dd>
                                                </div>
                                            }
                                        })
                                        .collect_view()}
                                    <DetailText
                                        label=trs!(ui.recipient_groups)
                                        value=Signal::derive(move || {
                                            if !names_known.get() {
                                                return leptos_i18n::t_string!(i18n,ui.unavailable)
                                                    .to_owned();
                                            }
                                            array(&config.get()["selection"], "groups")
                                                .iter()
                                                .map(|selected| {
                                                    array(&names.get(), "groups")
                                                        .iter()
                                                        .find(|v| v["id"] == selected["id"])
                                                        .map(|v| text(v, "name"))
                                                        .unwrap_or_else(|| crate::tr!(ui.missing_draft_resource)())
                                                })
                                                .collect::<Vec<_>>()
                                                .join(", ")
                                        })
                                    />
                                    <DetailText
                                        label=trs!(ui.public_url)
                                        value=Signal::derive(move || text(&config.get(), "url"))
                                    />
                                </DetailGrid>
                            </DetailSection> <DetailSection title=trs!(ui.schedule)>
                                <DetailGrid>
                                    {[
                                        ("launch_local", trs!(ui.launch_time)),
                                        (
                                            "send_by_local",
                                            trs!(ui.initial_distribution_end_optional),
                                        ),
                                    ]
                                        .into_iter()
                                        .map(move |(key, label)| {
                                            view! {
                                                <div>
                                                    <dt>{move || label.get()}</dt>
                                                    <dd>
                                                        {move || {
                                                            let value = config.get();
                                                            let time = &value[key];
                                                            if time.is_null() {
                                                                crate::tr!(ui.not_set)()
                                                            } else {
                                                                match ruphish_domain::datetime::resolve_local(
                                                                    &text(time, "local"),
                                                                    &text(time, "time_zone"),
                                                                ) {
                                                                    Ok(time) => {
                                                                        crate::shared::datetime::display::instant(
                                                                            &json!(ruphish_domain::datetime::utc(time)),
                                                                        )
                                                                    }
                                                                    Err(_) => crate::tr!(ui.time_not_recorded)(),
                                                                }
                                                            }
                                                        }}
                                                    </dd>
                                                </div>
                                            }
                                        })
                                        .collect_view()}
                                </DetailGrid>
                            </DetailSection>
                            <AttributionInfo value=Signal::derive(move || {
                                saved
                                    .with(|value| {
                                        <ruphish_contracts::attribution::Attribution as serde::Deserialize>::deserialize(
                                                value,
                                            )
                                            .unwrap_or_default()
                                    })
                            }) />
                        </DetailCard>
                    </Show>
                    <Show when=move || deleting.get().is_some()>
                        <DeleteConfirmation
                            title=trs!(ui.delete_draft)
                            description=trs!(ui.delete_draft_description)
                            confirm_label=trs!(ui.delete)
                            busy=Signal::derive(move || busy.get())
                            message=message
                            confirm_disabled=needs_review
                            on_confirm=delete
                            on_cancel=Callback::new(move |()| deleting.set(None))
                        >
                            <DeleteRecovery
                                error=delete_error
                                saved_href=Signal::derive(move || {
                                    format!("/campaigns/drafts/{}", id.get_value())
                                })
                            />
                        </DeleteConfirmation>
                    </Show>
                </QueryBoundary>
            </QueryBoundary>
        </ManagementPage>
    };
    view! {
        <ReadBoundary
            failure=read.failure
            has_data=Signal::derive(move || read.data.with(Option::is_some))
            fetching=read.fetching
            retry=read.refresh
            back_href="/campaigns"
            back_label=trs!(ui.back_to_campaigns)
        >
            {content}
        </ReadBoundary>
    }
}
