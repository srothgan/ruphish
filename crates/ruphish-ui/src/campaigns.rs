//! Campaign preparation and operational delivery views within the shared shell.
use crate::campaign_queries::CampaignQueries;
use crate::{client::Api, feedback::Message, resources::ResourceAccess, shared::components::*};
use leptos::{prelude::*, task::spawn_local_scoped_with_cancellation as spawn_local};
use leptos_router::hooks::{use_navigate, use_params_map, use_query_map};
use ruphish_contracts::campaign_reads as reads;
use ruphish_domain::{
    campaign::{CampaignStatus, DeliveryState},
    reporting::SortDirection,
};
use ruphish_ui::campaign_state::Draft;
use ruphish_ui::{tr, trs};
use serde_json::{Value, json};
use std::sync::Arc;
use wasm_bindgen::prelude::*;

use crate::time_display::{instant, zone_name as local_zone};
#[wasm_bindgen(inline_js = "export function campaignKey(){return crypto.randomUUID()}")]
extern "C" {
    #[wasm_bindgen(catch,js_name=campaignKey)]
    pub(super) fn random_key() -> Result<String, JsValue>;
}
pub(super) fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_owned()
}
pub(super) fn array(value: &Value, key: &str) -> Vec<Value> {
    value[key].as_array().cloned().unwrap_or_default()
}
pub(super) fn editable() -> bool {
    use_context::<ResourceAccess>().is_some_and(|v| v.0)
}

pub(super) fn list(api: Api, token: String) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    let token = StoredValue::new(token);
    let query = use_query_map();
    let filter = RwSignal::new(
        query
            .with(|q| q.get("filter"))
            .filter(|f| matches!(f.as_str(), "drafts" | "active" | "completed"))
            .unwrap_or_else(|| "all".into()),
    );
    let queries = expect_context::<CampaignQueries>();
    let cursors = RwSignal::new(vec![String::new()]);
    let preference = TableSorting::new("campaign-directory", "name", SortDirection::Asc);
    let sorting = TableSorting {
        current: preference.current,
        change: Callback::new(move |column: String| {
            batch(move || {
                cursors.set(vec![String::new()]);
                preference.change.run(column);
            })
        }),
    };
    let after = Memo::new(move |_| cursors.with(|v| v.last().cloned().unwrap_or_default()));
    let selection = Memo::new(move |_| {
        (
            filter.get(),
            after.get(),
            sorting.current.get().column,
            sorting.order(),
        )
    });
    let read = queries.directory.observe(move || selection.get());
    let presentation = read.presentation(move || selection.get());
    let rows = Signal::derive(move || {
        presentation
            .data
            .get()
            .map(|page| {
                page.items
                    .into_iter()
                    .map(|row| json!(row))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });
    let next = Signal::derive(move || {
        read.data
            .get()
            .and_then(|page| page.next_after)
            .filter(|next| *next != after.get())
    });
    let busy = read.fetching;
    let loaded = Signal::derive(move || presentation.data.with(Option::is_some));
    let delete_message = Message::new(String::new());
    let can_edit = editable();
    let deleting = RwSignal::new(None::<Value>);
    let mutating = RwSignal::new(false);
    let tasks = ruphish_ui::view_tasks::ViewTasks::default();
    let confirm = Callback::new(move |()| {
        let Some(target) = deleting.get_untracked() else {
            return;
        };
        if mutating.get_untracked() {
            return;
        }
        mutating.set(true);
        delete_message.clear();
        tasks.spawn(async move {
            let id = text(&target, "id");
            let path = if target["kind"] == "draft" {
                format!("/api/campaign-drafts/{id}?revision={}", target["revision"])
            } else {
                format!("/api/campaigns/{id}")
            };
            match api
                .command("DELETE", &path, &token.get_value(), None, None)
                .await
            {
                Ok(_) => {
                    queries.deleted(&text(&target, "kind"), &id);
                    deleting.set(None);
                    // Deleting the last row on a later page returns to the previous page.
                    batch(move || {
                        if rows.with_untracked(Vec::is_empty)
                            && cursors.with_untracked(|c| c.len() > 1)
                        {
                            cursors.update(|c| {
                                c.pop();
                            });
                        }
                    });
                }
                Err(e) => delete_message.error(e),
            }
            mutating.set(false);
        });
    });
    view! {
        <ManagementPage>
            <PageHeader
                title=trs!(ui.campaigns)
                subtitle=trs!(ui.prepare_a_simulation_and_follow_its_delivery_progress)
            >
                <RefreshButton
                    fetching=read.fetching
                    on_refresh=read.refresh
                    disabled=Signal::derive(move || busy.get() || mutating.get())
                />
                <ActionLink style=ButtonStyle::Primary hidden={!can_edit} href="/campaigns/new">
                    {tr!(ui.create_campaign)}
                </ActionLink>
            </PageHeader>
            <QueryBoundary
                fetching=busy
                has_data=loaded
                error=read.error
                noun=trs!(ui.campaigns)
                refresh=read.refresh
            >
                <QueryControls slot:controls>
                    <TableToolbar title=trs!(ui.campaigns)>
                        <Select
                            label=trs!(ui.campaign_filter)
                            value=Signal::derive(move || filter.get())
                            on_change=Callback::new(move |v: String| batch(move || {
                                cursors.set(vec![String::new()]);
                                filter.set(v);
                            }))
                        >
                            <option value="all">{tr!(ui.all_campaigns)}</option>
                            <option value="drafts">{tr!(ui.drafts)}</option>
                            <option value="active">{tr!(ui.active_campaigns_filter)}</option>
                            <option value="completed">{tr!(ui.completed_campaigns_filter)}</option>
                        </Select>
                    </TableToolbar>
                </QueryControls>
                <DataTable
                    sorting=sorting
                    columns=vec!["name", "status", "total", "launch", ""]
                    label=trs!(ui.campaigns)
                    empty=Signal::derive(move || rows.get().is_empty())
                    headers=vec![
                        trs!(ui.campaign),
                        trs!(ui.lifecycle),
                        trs!(ui.recipients),
                        trs!(ui.launch),
                        trs!(ui.actions),
                    ]
                >
                    {move || {
                        rows.get()
                            .into_iter()
                            .map(|r| {
                                let draft = r["kind"] == "draft";
                                let id = text(&r, "id");
                                let href = if draft {
                                    format!("/campaigns/drafts/{id}")
                                } else {
                                    ruphish_domain::campaign::CampaignStatus::from_stored(
                                            &text(&r, "status"),
                                        )
                                        .map(|status| {
                                            format!(
                                                "/campaigns/{id}/{}",
                                                Section::landing(status).path(),
                                            )
                                        })
                                        .unwrap_or_else(|| format!("/campaigns/{id}"))
                                };
                                let view_href = href.clone();
                                let name = text(&r, "name");
                                let target = r.clone();
                                view! {
                                    <tr>
                                        <th scope="row">
                                            <a href=view_href
                                                .clone()>
                                                {if name.is_empty() {
                                                    leptos_i18n::t_string!(i18n, ui.untitled_campaign)
                                                        .to_owned()
                                                } else {
                                                    name
                                                }}
                                            </a>
                                        </th>
                                        <td>
                                            {if draft {
                                                view! {
                                                    <StatusBadge tone=StatusTone::Warning>
                                                        {tr!(ui.draft)}
                                                    </StatusBadge>
                                                }
                                                    .into_any()
                                            } else {
                                                ruphish_ui::localization::event_label(
                                                        i18n,
                                                        &text(&r, "status"),
                                                    )
                                                    .into_any()
                                            }}
                                        </td>
                                        <td>
                                            {r["total"]
                                                .as_i64()
                                                .map(|n| n.to_string())
                                                .unwrap_or_else(|| {
                                                    leptos_i18n::t_string!(i18n,ui.not_set).to_owned()
                                                })}
                                        </td>
                                        <td>
                                            {if draft {
                                                leptos_i18n::t_string!(i18n,ui.not_scheduled).to_owned()
                                            } else {
                                                instant(&r["launch_date"])
                                            }}
                                        </td>
                                        <td>
                                            <ResourceActions
                                                view_href=view_href
                                                edit_href=format!("{href}/edit")
                                                can_edit=can_edit
                                                hide_edit=!draft
                                                disabled=Signal::derive(move || mutating.get())
                                                on_delete=Callback::new(move |()| {
                                                    delete_message.clear();
                                                    deleting.set(Some(target.clone()));
                                                })
                                            ></ResourceActions>
                                        </td>
                                    </tr>
                                }
                            })
                            .collect_view()
                    }}
                </DataTable>
                <Show when=move || {
                    loaded.get() && !busy.get() && read.error.get().is_none()
                        && rows.get().is_empty()
                }>
                    <EmptyState title=Signal::derive(move || {
                        if filter.get() == "drafts" {
                            leptos_i18n::t_string!(i18n,ui.no_drafts).to_owned()
                        } else {
                            leptos_i18n::t_string!(i18n,ui.no_campaigns_on_this_page).to_owned()
                        }
                    })>
                        <p>
                            {tr!(
                                ui.create_a_campaign_to_select_an_audience_review_content_and_choose_a_schedule
                            )}
                        </p>
                    </EmptyState>
                </Show>
                <PaginationControls
                    disabled=Signal::derive(move || busy.get() || mutating.get())
                    has_previous=Signal::derive(move || cursors.with(|c| c.len() > 1))
                    has_next=Signal::derive(move || next.get().is_some())
                    on_previous=Callback::new(move |()| {
                        cursors
                            .update(|c| {
                                c.pop();
                            })
                    })
                    on_next=Callback::new(move |()| {
                        if let Some(cursor) = next.get_untracked() {
                            cursors.update(|c| c.push(cursor));
                        }
                    })
                />
                {move || {
                    deleting
                        .get()
                        .map(|target| {
                            let draft = target["kind"] == "draft";
                            view! {
                                <DeleteConfirmation
                                    title=if draft {
                                        trs!(ui.delete_draft)
                                    } else {
                                        trs!(ui.delete_campaign)
                                    }
                                    description=if draft {
                                        trs!(ui.delete_draft_description)
                                    } else {
                                        trs!(
                                            ui.campaign_history_will_be_removed_already_submitted_mail_cannot_be_recalled
                                        )
                                    }
                                    confirm_label=trs!(ui.delete)
                                    busy=Signal::derive(move || mutating.get())
                                    message=delete_message
                                    on_confirm=confirm
                                    on_cancel=Callback::new(move |()| deleting.set(None))
                                />
                            }
                        })
                }}
            </QueryBoundary>
        </ManagementPage>
    }
}
/// Resolve the campaign entry URL before mounting a section. Results has one route owner.
pub(super) fn entry() -> impl IntoView {
    let params = use_params_map();
    view! {
        <For
            each=move || vec![params.with(|p| p.get("id")).unwrap_or_default()]
            key=|id| id.clone()
            children=move |id| {
                if id.parse::<i64>().ok().is_some_and(|id| id > 0) {
                    entry_redirect(id).into_any()
                } else {
                    view! {
                        <NotFoundPage
                            back_href="/campaigns"
                            back_label=trs!(ui.back_to_campaigns)
                        />
                    }
                        .into_any()
                }
            }
        />
    }
}

fn entry_redirect(id: String) -> impl IntoView {
    let id = StoredValue::new(id);
    let location = leptos_router::hooks::use_location();
    let read = expect_context::<CampaignQueries>()
        .context
        .observe(move || id.get_value());
    let loaded = Signal::derive(move || read.data.with(Option::is_some));
    view! {
        <ReadBoundary
            failure=read.failure
            has_data=loaded
            fetching=read.fetching
            retry=read.refresh
            back_href="/campaigns"
            back_label=trs!(ui.back_to_campaigns)
        >
            <QueryBoundary
                fetching=read.fetching
                has_data=loaded
                error=read.error
                noun=trs!(ui.campaign)
                refresh=read.refresh
            >
                {move || {
                    read.data
                        .get()
                        .map(|campaign| {
                            let mut target = format!(
                                "/campaigns/{}/{}",
                                id.get_value(),
                                Section::landing(campaign.status).path(),
                            );
                            let search = location.search.get_untracked();
                            if !search.is_empty() {
                                target.push('?');
                                target.push_str(search.trim_start_matches('?'));
                            }
                            let hash = location.hash.get_untracked();
                            if !hash.is_empty() {
                                target.push('#');
                                target.push_str(hash.trim_start_matches('#'));
                            }
                            view! {
                                <leptos_router::components::Redirect
                                    path=target
                                    options=leptos_router::NavigateOptions {
                                        replace: true,
                                        scroll: false,
                                        ..Default::default()
                                    }
                                />
                            }
                        })
                }}
            </QueryBoundary>
        </ReadBoundary>
    }
}

pub(super) fn new(api: Api, token: String) -> impl IntoView {
    let query = use_query_map();
    let drafts = use_context::<ruphish_ui::import_drafts::ImportDrafts>();
    let key = query.with(|q| q.get("draft"));
    let initial = key
        .as_ref()
        .and_then(|key| drafts.and_then(|store| store.get("campaigns", key)))
        .map(|stored| stored.draft.get_untracked().value().clone());
    if let Some(key) = key {
        on_cleanup(move || {
            if let Some(store) = drafts {
                store.remove(&key);
            }
        });
        if initial.is_none() {
            return view! {
                <ManagementPage>
                    <h1>{tr!(ui.draft_unavailable)}</h1>
                    <a href="/campaigns">{tr!(ui.return_to_campaigns)}</a>
                </ManagementPage>
            }
            .into_any();
        }
    }
    view! { <CreateCampaign api=api token=token initial=initial saved=None /> }.into_any()
}

pub(super) fn detail(api: Api, token: String, section: Section) -> impl IntoView {
    let params = use_params_map();
    let token = StoredValue::new(token);
    view! {
        <For
            each=move || vec![params.with(|p| p.get("id").unwrap_or_default())]
            key=|id| id.clone()
            children=move |id| {
                if id.parse::<i64>().ok().is_some_and(|id| id > 0) {
                    show(api, token.get_value(), id, section).into_any()
                } else {
                    view! {
                        <NotFoundPage
                            back_href="/campaigns"
                            back_label=trs!(ui.back_to_campaigns)
                        />
                    }
                        .into_any()
                }
            }
        />
    }
}

fn input(draft: RwSignal<Draft>, key: &'static str, label: Signal<String>) -> impl IntoView {
    view! {
        <Field label=label>
            <TextInput
                validation_field=key
                value=Signal::derive(move || text(&draft.get().value, key))
                on_input=move |e| {
                    draft.update(|d| d.edit(|v| v[key] = json!(event_target_value(&e))))
                }
            />
        </Field>
    }
}
fn selector(
    draft: RwSignal<Draft>,
    resources: Signal<Value>,
    kind: &'static str,
    label: Signal<String>,
) -> impl IntoView {
    view! {
        <Select
            validation_field=format!("selection.{kind}.id")
            label=label
            value=Signal::derive(move || draft.get().value["selection"][kind]["id"].to_string())
            on_change=Callback::new(move |v: String| {
                let id = v.parse::<i64>().unwrap_or(0);
                draft.update(|d| d.edit(|node| node["selection"][kind] = json!({"id":id})));
            })
        >
            <option value="0">{tr!(ui.choose_a_resource)}</option>
            {move || {
                array(&resources.get(), kind)
                    .into_iter()
                    .map(|r| {
                        view! { <option value=r["id"].to_string()>{text(&r, "name")}</option> }
                    })
                    .collect_view()
            }}
        </Select>
    }
}
fn domain_selector(draft: RwSignal<Draft>, resources: Signal<Value>) -> impl IntoView {
    let selected = move || {
        let value = text(&draft.get().value, "url");
        ruphish_domain::domains::origin(&value, true)
            .ok()
            .map(|o| o.value)
            .unwrap_or_default()
    };
    let path = move || {
        let value = text(&draft.get().value, "url");
        // Preserve authored Go expressions and escaping; URL::path() percent-encodes
        // template delimiters and must not become the editor's source of truth.
        let suffix = value
            .strip_prefix("https://")
            .or_else(|| value.strip_prefix("http://"))
            .map(|rest| rest.find(['/', '?']).map(|at| &rest[at..]).unwrap_or(""))
            .unwrap_or(value.as_str());
        if suffix.starts_with('/') {
            suffix.to_owned()
        } else {
            format!("/{suffix}")
        }
    };
    view! {
        <div>
            <Select
                validation_field="url"
                label=trs!(ui.campaign_origin)
                value=Signal::derive(selected)
                on_change=Callback::new(move |origin: String| {
                    let path = path();
                    draft.update(|d| d.edit(|node| node["url"] = json!(format!("{origin}{path}"))));
                })
            >
                <option value="">{tr!(ui.choose_campaign_origin)}</option>
                {move || {
                    array(&resources.get(), "domains")
                        .into_iter()
                        .map(|d| {
                            view! {
                                <option value=text(
                                    &d,
                                    "origin",
                                )>
                                    {format!("{} ({})", text(&d, "name"), text(&d, "hostname"))}
                                </option>
                            }
                        })
                        .collect_view()
                }}
            </Select>
            <Field label=trs!(ui.landing_path_optional)>
                <TextInput
                    value=path
                    on_input=move |e| {
                        let path = event_target_value(&e);
                        let origin = selected();
                        let path = if path.starts_with('/') { path } else { format!("/{path}") };
                        draft.update(|d| d.edit(|v| v["url"] = json!(format!("{origin}{path}"))));
                    }
                />
            </Field>
            <p>{move || text(&draft.get().value, "url")}</p>
        </div>
    }
}
fn load_page_defaults(
    tasks: ruphish_ui::view_tasks::ViewTasks,
    api: Api,
    draft: RwSignal<Draft>,
    id: i64,
    message: Message,
) {
    let i18n = ruphish_ui::localization::use_i18n();

    message.clear();
    let Some(request) = draft.try_update(|d| d.select_page(id)).flatten() else {
        return;
    };
    tasks.spawn(async move {
        let result = crate::resource_queries::ResourceQueries::read_detail(
            api,
            crate::resources::Family::Pages,
            &id.to_string(),
        )
        .await
        .and_then(|(resource, _)| match resource.content {
            ruphish_contracts::resource_details::Content::Page(page) => {
                Ok((resource.metadata.id, page.redirect_url))
            }
            _ => Err(api.invalid_read_err()),
        })
        .map_err(|e| e.message);
        let accepted = draft
            .try_update(|d| {
                d.page_loaded(
                    request,
                    result
                        .as_ref()
                        .ok()
                        .map(|(id, redirect)| (*id, redirect.as_str())),
                )
            })
            .unwrap_or(false);
        if accepted && !draft.with(Draft::policy_ready) {
            message.error(match result {
                Err(e) => {
                    leptos_i18n::t_string!(
                        i18n,
                        messages
                            .cannot_load_landing_page_defaults_arg0_retry_or_select_another_page,
                        arg0 = (e).to_string()
                    )
                }
                Ok(_) => leptos_i18n::t_string!(
                    i18n,
                    ui.landing_page_defaults_are_incomplete_retry_or_select_another_page
                )
                .into(),
            });
        }
    });
}
fn page_selector(
    tasks: ruphish_ui::view_tasks::ViewTasks,
    api: Api,
    draft: RwSignal<Draft>,
    resources: Signal<Value>,
    message: Message,
) -> impl IntoView {
    view! {
        <Select
            validation_field="selection.page.id"
            label=trs!(ui.landing_page)
            value=Signal::derive(move || draft.get().value["selection"]["page"]["id"].to_string())
            on_change=Callback::new(move |v: String| load_page_defaults(
                tasks,
                api,
                draft,
                v.parse::<i64>().unwrap_or(0),
                message,
            ))
        >
            <option value="0">{tr!(ui.choose_a_resource)}</option>
            {move || {
                array(&resources.get(), "page")
                    .into_iter()
                    .map(|r| {
                        view! { <option value=r["id"].to_string()>{text(&r, "name")}</option> }
                    })
                    .collect_view()
            }}
        </Select>
    }
}
fn schedule(
    draft: RwSignal<Draft>,
    key: &'static str,
    label: Signal<String>,
    zone: StoredValue<String>,
) -> impl IntoView {
    view! {
        <section class="resource-section">
            <h2>{move || label.get()}</h2>
            <p>
                {move || {
                    if key == "launch_local" {
                        tr!(validation.scheduled_help).into_any()
                    } else {
                        tr!(
                            ui.leave_blank_for_no_explicit_time_daylight_saving_changes_are_handled_automatical
                        )
                            .into_any()
                    }
                }}
            </p>
            <div class="resource-columns">
                <Field label=trs!(ui.local_date_and_time)>
                    <TextInput
                        validation_field=format!("{key}.local")
                        input_type="datetime-local"
                        value=Signal::derive(move || text(&draft.get().value[key], "local"))
                        on_input=move |e| {
                            let local = event_target_value(&e);
                            draft
                                .update(|d| {
                                    d.edit(|v| {
                                        if local.is_empty() && key != "launch_local" {
                                            if let Some(v) = v.as_object_mut() {
                                                v.remove(key);
                                            }
                                        } else {
                                            let selected_zone = v[key]["time_zone"]
                                                .as_str()
                                                .unwrap_or(&zone.get_value())
                                                .to_owned();
                                            v[key] = json!({"local":local,"time_zone":selected_zone});
                                        }
                                    })
                                });
                        }
                    />
                </Field>
                <Field label=trs!(ui.time_zone)>
                    <TextInput
                        validation_field=format!("{key}.time_zone")
                        disabled=Signal::derive(move || draft.get().value[key].is_null())
                        placeholder=zone.get_value()
                        value=Signal::derive(move || text(&draft.get().value[key], "time_zone"))
                        on_input=move |e| {
                            draft
                                .update(|d| {
                                    d.edit(|v| v[key]["time_zone"] = json!(event_target_value(&e)))
                                })
                        }
                    />
                </Field>
            </div>
        </section>
    }
}
#[component]
pub(super) fn CreateCampaign(
    api: Api,
    token: String,
    initial: Option<Value>,
    saved: Option<Value>,
) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let queries = expect_context::<CampaignQueries>();
    let tasks = ruphish_ui::view_tasks::ViewTasks::default();
    let token = StoredValue::new(token);
    let navigate = use_navigate();
    let zone = StoredValue::new(local_zone());
    let generation = u64::from(initial.is_some() && saved.is_none());
    let persistence = crate::campaign_drafts::Persistence::new(saved.as_ref());
    let initial_step = saved
        .as_ref()
        .and_then(|s| s["step"].as_u64())
        .unwrap_or(0)
        .min(3) as usize;
    let draft = RwSignal::new(Draft {
        generation,
        value: initial.unwrap_or_else(||json!({"name":"","url":"","selection":{"groups":[],"template":{"id":0},"page":{"id":0},"smtp":{"id":0}},"policy":{"redirect_url":"", "track_opens":true}})),
        ..Default::default()
    });
    let resource_read = CampaignQueries::resources(true);
    let resources = Signal::derive(move || resource_read.data.get().unwrap_or(Value::Null));
    let resources_known = Signal::derive(move || resource_read.data.with(Option::is_some));
    let busy = RwSignal::new(false);
    let loading = resource_read.fetching;
    let step = RwSignal::new(initial_step);
    let validation_time = RwSignal::new(ruphish_domain::datetime::now().as_second());
    let readiness = move |through: usize| {
        let available = (resources_known.get() && !loading.get()).then(|| resources.get());
        ruphish_ui::form_validation::campaign_with_resources(
            &draft.get().value,
            available.as_ref(),
            through,
            validation_time.get(),
        )
    };
    let validation = FormValidation::new(move || readiness(step.get()));
    let review_ready = Memo::new(move |_| {
        resources_known.get()
            && !loading.get()
            && resource_read.error.get().is_none()
            && readiness(3).is_empty()
    });
    validation.on_reveal(Callback::new(move |field: String| {
        step.set(ruphish_domain::campaign_draft::field_step(&field))
    }));
    let message = Message::new(String::new());
    let can_edit = editable();
    crate::navigation::protect_draft(move || {
        draft.with(|d| d.generation != persistence.saved_generation.get() || d.launch.is_some())
    });
    let recipient = RwSignal::new(String::new());
    let retry_review = RwSignal::new(0u64);
    let review_flow = StoredValue::new(ruphish_ui::campaign_state::ReviewFlow::default());
    // Review output/loading/error updates must not retrigger their own request.
    let review_inputs = Memo::new(move |_| {
        (
            step.get(),
            retry_review.get(),
            loading.get() || !resources_known.get() || resource_read.error.get().is_some(),
            draft.with(|d| (d.generation, d.policy_ready(), d.launch.is_some())),
        )
    });
    Effect::new(move |_| {
        let (step, retry, loading, _) = review_inputs.get();
        if step == 3 && !loading && draft.with_untracked(|d| d.launch.is_none()) {
            validation_time.set(ruphish_domain::datetime::now().as_second());
            if !untrack(|| validation.check()) {
                draft.update(Draft::clear_review);
                return;
            }
        }
        let mut request = None;
        draft.update(|state| {
            review_flow.update_value(|flow| {
                request = flow.update(state, step == 3, loading, can_edit, retry);
            })
        });
        let Some((request, body)) = request else {
            return;
        };
        let validation_ticket = validation.ticket();
        spawn_local(async move {
            let result = api
                .resource_response(
                    "POST",
                    "/api/campaign-drafts/prepare",
                    &token.get_value(),
                    Some(&body),
                    None,
                )
                .await
                .and_then(|(value, _)| {
                    serde_json::from_value::<ruphish_contracts::campaign_workflow::Review>(value)
                        .map_err(|_| api.invalid_read_err())
                });
            let error = result.as_ref().err().cloned();
            let accepted = draft
                .try_update(|state| state.reviewed(request, result.map_err(|e| e.message)))
                .unwrap_or(false);
            if accepted && let Some(error) = error {
                validation.server(validation_ticket, &error);
            }
        });
    });
    let send_test = move |_| {
        if busy.get_untracked() {
            return;
        }
        let d = draft.get_untracked();
        let Some(review) = d.review else {
            return;
        };
        let mut campaign = d.value;
        campaign["selection"] = json!(ruphish_contracts::campaign::Selections::from(
            &review.selection
        ));
        campaign["policy"] = json!(review.policy);
        busy.set(true);
        tasks.spawn(async move {
            match api
                .command(
                    "POST",
                    "/api/campaigns/test",
                    &token.get_value(),
                    Some(&json!({"campaign":campaign,"email":recipient.get_untracked()})),
                    None,
                )
                .await
            {
                Ok(_) => {
                    #[cfg(not(feature = "demo"))]
                    message.status(
                        leptos_i18n::t_string!(i18n, ui.test_accepted_by_smtp_server).to_owned(),
                    );
                    #[cfg(feature = "demo")]
                    message.local_status(leptos_i18n::t_string!(i18n, demo.test_email).to_owned());
                }
                Err(e) => message.error(e),
            }
            busy.set(false);
        });
    };
    let save_navigate = navigate.clone();
    let save = Callback::new(move |destination: Option<usize>| {
        if busy.get_untracked()
            || draft.with_untracked(|d| d.launch.is_some() || d.page_request.is_some())
        {
            return;
        }
        if destination == Some(step.get_untracked()) {
            return;
        }
        if let Some(next) = destination.filter(|next| *next > step.get_untracked()) {
            if loading.get_untracked() {
                return;
            }
            validation_time.set(ruphish_domain::datetime::now().as_second());
            // A jump must validate every preceding step, including skipped sections.
            if !validation.check_report(&untrack(|| readiness(next.saturating_sub(1)))) {
                return;
            }
            if next == 3 && !review_ready.get_untracked() {
                return;
            }
        }
        busy.set(true);
        message.clear();
        let snapshot = draft.get_untracked();
        let next = destination.unwrap_or(step.get_untracked());
        let first = persistence.revision.get_untracked().is_none();
        let navigate = save_navigate.clone();
        tasks.spawn(async move {
            match persistence
                .save(api, &token.get_value(), &snapshot, next)
                .await
            {
                Ok(_) => {
                    if destination.is_none() {
                        #[cfg(feature = "demo")]
                        api.notifications.show(DemoNotice::DraftSaved);
                        navigate(
                            &format!("/campaigns/drafts/{}", persistence.id.get_untracked()),
                            Default::default(),
                        );
                    } else if first && draft.with_untracked(|d| d.generation == snapshot.generation)
                    {
                        navigate(
                            &format!("/campaigns/drafts/{}/edit", persistence.id.get_untracked()),
                            Default::default(),
                        );
                    } else {
                        step.set(next);
                    }
                }
                Err(e) => message.error(e),
            }
            busy.set(false);
        });
    });
    let launch = move |_| {
        if draft.with_untracked(|d| d.launch.is_none()) {
            validation_time.set(ruphish_domain::datetime::now().as_second());
            if !validation.check() {
                return;
            }
        }
        if busy.get_untracked() {
            return;
        }
        let key = match random_key() {
            Ok(v) => v,
            Err(_) => {
                message.error(
                    leptos_i18n::t_string!(i18n, ui.cannot_generate_a_secure_launch_key).to_owned(),
                );
                return;
            }
        };
        let Some((key, body)) = draft.try_update(|d| d.launch(key)).flatten() else {
            return;
        };
        busy.set(true);
        let validation_ticket = validation.ticket();
        let navigate = navigate.clone();
        tasks.spawn(async move {
            let snapshot = draft.get_untracked();
            if (snapshot.generation != persistence.saved_generation.get_untracked()
                || persistence.revision.get_untracked().is_none())
                && let Err(e) = persistence
                    .save(api, &token.get_value(), &snapshot, step.get_untracked())
                    .await
            {
                draft.update(Draft::rejected);
                message.error(e);
                busy.set(false);
                return;
            }
            let request = json!({"revision":persistence.revision.get_untracked(),"campaign":body});
            let path = format!(
                "/api/campaign-drafts/{}/launch",
                persistence.id.get_untracked()
            );
            match api.launch(&path, &token.get_value(), &key, &request).await {
                Ok(v) => {
                    queries.launched(&persistence.id.get_untracked(), &v["id"].to_string());
                    draft.update(|d| {
                        d.generation = persistence.saved_generation.get_untracked();
                        d.launch = None;
                    });
                    navigate(
                        &format!("/campaigns/{}/delivery", v["id"]),
                        Default::default(),
                    );
                }
                Err(e) => {
                    if (400..500).contains(&e.status) {
                        draft.update(Draft::rejected);
                        retry_review.update(|generation| *generation += 1);
                        validation.server(validation_ticket, &e);
                    }
                    message.error(e.message);
                }
            }
            busy.set(false);
        });
    };
    view! {
        <ManagementPage>
            <BackLink href="/campaigns">{tr!(ui.back_to_campaigns)}</BackLink>
            <PageHeader
                title=Signal::derive(move || {
                    if persistence.revision.get().is_some() {
                        leptos_i18n::t_string!(i18n,ui.edit_campaign_draft).to_string()
                    } else {
                        leptos_i18n::t_string!(i18n,ui.create_campaign).to_string()
                    }
                })
                subtitle=trs!(ui.review_the_audience_content_and_schedule_before_launching)
            >
                <span role="status">
                    {move || {
                        if persistence.saving.get() {
                            leptos_i18n::t_string!(i18n,ui.draft_saving).to_string()
                        } else if draft.with(|d| d.generation != persistence.saved_generation.get())
                            || persistence.revision.get().is_none()
                        {
                            leptos_i18n::t_string!(i18n,ui.draft_unsaved).to_string()
                        } else {
                            format!(
                                "{}: {}",
                                leptos_i18n::t_string!(i18n,ui.draft_saved),
                                instant(&persistence.modified.get()),
                            )
                        }
                    }}
                </span>
                <RefreshButton
                    fetching=Signal::derive(move || {
                        resource_read.fetching.get() || draft.with(|d| d.page_request.is_some())
                    })
                    on_refresh=resource_read.refresh
                    label=trs!(ui.refresh_resources)
                    disabled=Signal::derive(move || {
                        busy.get() || loading.get() || draft.with(|d| d.launch.is_some())
                    })
                />
            </PageHeader>
            {message.view()}
            <ValidationSummary form=validation />
            <QueryBoundary
                fetching=loading
                has_data=resources_known
                error=resource_read.error
                noun=trs!(ui.resources)
                refresh=resource_read.refresh
            >
                <Show when=move || {
                    resources_known.get() && !loading.get()
                        && ["template", "page", "smtp", "groups"]
                            .into_iter()
                            .any(|key| {
                                let value = draft.get().value;
                                let selections = if key == "groups" {
                                    array(&value["selection"], "groups")
                                } else {
                                    vec![value["selection"][key].clone()]
                                };
                                selections
                                    .iter()
                                    .any(|selected| {
                                        selected["id"].as_i64().is_some_and(|id| id > 0)
                                            && !array(&resources.get(), key)
                                                .iter()
                                                .any(|r| r["id"] == selected["id"])
                                    })
                            })
                }>
                    <p class="form-message error">{tr!(ui.missing_draft_resource)}</p>
                </Show>
                <StepNavigation
                    label=trs!(ui.campaign_steps)
                    labels=vec![
                        trs!(ui.audience),
                        trs!(ui.content_and_destination),
                        trs!(ui.schedule),
                        trs!(ui.review_and_launch),
                    ]
                    current=step
                    disabled=Signal::derive(move || {
                        busy.get() || loading.get() || !can_edit
                            || draft.with(|d| d.launch.is_some() || d.page_request.is_some())
                    })
                    disabled_steps=Signal::derive(move || {
                        if review_ready.get() { vec![] } else { vec![3] }
                    })
                    on_select=Callback::new(move |index| save.run(Some(index)))
                />
                <FormFields disabled=Signal::derive(move || {
                    busy.get() || loading.get() || !can_edit || draft.with(|d| d.launch.is_some())
                })>
                    <section hidden=move || step.get() != 0>
                        <h2>{tr!(ui.audience)}</h2>
                        {input(draft, "name", trs!(ui.campaign_name))}
                        <ValidationAnchor field="selection.groups" />
                        <p>
                            {tr!(
                                ui.choose_recipient_groups_overlaps_are_counted_once_by_exact_email_address_the_fir
                            )}
                        </p>
                        {move || {
                            array(&resources.get(), "groups")
                                .into_iter()
                                .map(|r| {
                                    let id = r["id"].as_i64().unwrap_or(0);
                                    view! {
                                        <Checkbox
                                            label=leptos_i18n::t_string!(
                                                i18n, messages.group_count, name = text(&r,"name"), count = r["num_targets"].to_string()
                                            )
                                            checked=Signal::derive(move || {
                                                draft
                                                    .with(|d| {
                                                        array(&d.value["selection"], "groups")
                                                            .iter()
                                                            .any(|r| r["id"] == id)
                                                    })
                                            })
                                            on_toggle=Callback::new(move |checked| {
                                                validation.edited("selection.groups");
                                                draft
                                                    .update(|d| {
                                                        d.edit(|v| {
                                                            let mut groups = array(&v["selection"], "groups");
                                                            groups.retain(|r| r["id"] != id);
                                                            if checked {
                                                                groups.push(json!({"id":id}));
                                                            }
                                                            v["selection"]["groups"] = json!(groups);
                                                        })
                                                    })
                                            })
                                        />
                                    }
                                })
                                .collect_view()
                        }}
                        {move || {
                            array(&draft.get().value["selection"], "groups")
                                .into_iter()
                                .filter(|selected| {
                                    resources_known.get() && !loading.get()
                                        && !array(&resources.get(), "groups")
                                            .iter()
                                            .any(|r| r["id"] == selected["id"])
                                })
                                .map(|selected| {
                                    let id = selected["id"].as_i64().unwrap_or(0);
                                    view! {
                                        <Checkbox
                                            label=leptos_i18n::t_string!(
                                                i18n,messages.unavailable_draft_group,id=id.to_string()
                                            )
                                            checked=true
                                            on_toggle=Callback::new(move |_: bool| {
                                                validation.edited("selection.groups");
                                                draft
                                                    .update(|d| {
                                                        d.edit(|v| {
                                                            let mut groups = array(&v["selection"], "groups");
                                                            groups.retain(|r| r["id"] != id);
                                                            v["selection"]["groups"] = json!(groups);
                                                        })
                                                    })
                                            })
                                        />
                                    }
                                })
                                .collect_view()
                        }}
                        <a href="/groups/new" target="_blank" rel="noopener">
                            {tr!(ui.create_a_group_in_a_new_tab)}
                        </a>
                    </section>
                    <section hidden=move || step.get() != 1>
                        <h2>{tr!(ui.content_and_destination)}</h2>
                        <div class="resource-columns">
                            {selector(draft, resources, "template", trs!(ui.email_template))}
                            {selector(draft, resources, "smtp", trs!(ui.sending_profile))}
                            {page_selector(tasks, api, draft, resources, message)}
                            {domain_selector(draft, resources)}
                        </div>
                        <ul
                            class="campaign-resource-links"
                            aria-label=tr!(ui.create_resources_in_a_new_tab)
                        >
                            <li>
                                <a href="/templates/new" target="_blank" rel="noopener">
                                    {tr!(ui.create_template)}
                                </a>
                            </li>
                            <li>
                                <a href="/pages/new" target="_blank" rel="noopener">
                                    {tr!(ui.create_landing_page)}
                                </a>
                            </li>
                            <li>
                                <a href="/smtp/new" target="_blank" rel="noopener">
                                    {tr!(ui.create_sending_profile)}
                                </a>
                            </li>
                            <li>
                                <a href="/domains" target="_blank" rel="noopener">
                                    {tr!(ui.manage_and_verify_domains)}
                                </a>
                            </li>
                        </ul>
                        <p class="resource-usage">
                            {tr!(
                                ui.changes_to_saved_templates_or_sending_profiles_can_affect_scheduled_emails_and_r
                            )}
                        </p>
                        <p>{tr!(ui.all_submissions_recorded)}</p>
                        <section class="resource-section">
                            <Checkbox
                                label=trs!(ui.track_email_opens)
                                checked=Signal::derive(move || {
                                    draft
                                        .with(|d| {
                                            d.value["policy"]["track_opens"].as_bool().unwrap_or(true)
                                        })
                                })
                                on_toggle=Callback::new(move |checked| {
                                    draft.update(|d| d.set_track_opens(checked))
                                })
                            />
                            <h2>{tr!(ui.redirect_after_submission_optional)}</h2>
                            <p>{tr!(ui.campaign_redirect_defaults)}</p>
                            <Show when=move || {
                                draft.with(|d| d.page_request.is_none() && !d.policy_ready())
                            }>
                                <ActionButton
                                    kind="button"
                                    on_click=move |_| load_page_defaults(
                                        tasks,
                                        api,
                                        draft,
                                        draft
                                            .get_untracked()
                                            .value["selection"]["page"]["id"]
                                            .as_i64()
                                            .unwrap_or(0),
                                        message,
                                    )
                                >
                                    {tr!(ui.retry_loading_defaults)}
                                </ActionButton>
                            </Show>
                            <FormFields disabled=Signal::derive(move || {
                                !draft.with(Draft::policy_ready)
                            })>
                                <Select
                                    label=trs!(ui.redirect_destination)
                                    validation_field="policy"
                                    value=Signal::derive(move || {
                                        if draft
                                            .get()
                                            .value["policy"]["learning_page_id"]
                                            .is_number()
                                        {
                                            "learning".to_owned()
                                        } else {
                                            "custom".to_owned()
                                        }
                                    })
                                    on_change=Callback::new(move |value: String| {
                                        draft
                                            .update(|d| {
                                                d.edit(|v| {
                                                    let track_opens = v["policy"]["track_opens"]
                                                        .as_bool()
                                                        .unwrap_or(true);
                                                    v["policy"] = if value == "learning" {
                                                        json!(
                                                            {"redirect_url":"", "learning_page_id":0, "track_opens":track_opens}
                                                        )
                                                    } else {
                                                        json!({"redirect_url":"", "track_opens":track_opens})
                                                    };
                                                })
                                            })
                                    })
                                >
                                    <option value="custom">{tr!(ui.custom_redirect)}</option>
                                    <option value="learning">{tr!(ui.learning_page)}</option>
                                </Select>
                                <Show when=move || {
                                    draft.get().value["policy"]["learning_page_id"].is_number()
                                }>
                                    <Select
                                        label=trs!(ui.learning_page)
                                        validation_field="policy.learning_page_id"
                                        value=Signal::derive(move || {
                                            draft
                                                .get()
                                                .value["policy"]["learning_page_id"]
                                                .as_i64()
                                                .unwrap_or(0)
                                                .to_string()
                                        })
                                        on_change=Callback::new(move |value: String| {
                                            draft
                                                .update(|d| {
                                                    d.edit(|v| {
                                                        v["policy"]["learning_page_id"] = json!(
                                                            value.parse::<i64>().unwrap_or(0)
                                                        );
                                                    })
                                                })
                                        })
                                    >
                                        <option value="0">{tr!(ui.select_learning_page)}</option>
                                        {move || {
                                            let selected = draft
                                                .get()
                                                .value["policy"]["learning_page_id"]
                                                .as_i64()
                                                .unwrap_or(0);
                                            let rows = array(&resources.get(), "learning_page");
                                            (selected > 0
                                                && !rows
                                                    .iter()
                                                    .any(|row| row["id"].as_i64() == Some(selected)))
                                                .then(|| {
                                                    view! {
                                                        <option value=selected.to_string() disabled=true>
                                                            {tr!(ui.unavailable)}
                                                        </option>
                                                    }
                                                })
                                        }}
                                        {move || {
                                            array(&resources.get(), "learning_page")
                                                .into_iter()
                                                .map(|row| {
                                                    view! {
                                                        <option value=row["id"]
                                                            .to_string()>{text(&row, "name")}</option>
                                                    }
                                                })
                                                .collect_view()
                                        }}
                                    </Select>
                                    <a href="/learning-pages/new" target="_blank" rel="noopener">
                                        {tr!(ui.create_learning_page)}
                                    </a>
                                </Show>
                                <Show when=move || {
                                    !draft.get().value["policy"]["learning_page_id"].is_number()
                                }>
                                    <Field label=trs!(
                                        ui.redirect_url_optional_leave_empty_for_no_redirect
                                    )>
                                        <TextInput
                                            validation_field="policy.redirect_url"
                                            value=Signal::derive(move || text(
                                                &draft.get().value["policy"],
                                                "redirect_url",
                                            ))
                                            on_input=move |e| {
                                                draft
                                                    .update(|d| {
                                                        d.edit(|v| {
                                                            v["policy"]["redirect_url"] = json!(event_target_value(&e));
                                                        })
                                                    })
                                            }
                                        />
                                    </Field>
                                </Show>
                            </FormFields>
                        </section>
                    </section>
                    <section hidden=move || { step.get() != 2 }>
                        <Select
                            label=trs!(validation.delivery_mode)
                            value=Signal::derive(move || {
                                if draft.get().value["launch_local"].is_null() {
                                    "immediate".to_owned()
                                } else {
                                    "scheduled".to_owned()
                                }
                            })
                            on_change=Callback::new(move |mode: String| {
                                validation.edited("launch_local");
                                draft
                                    .update(|d| {
                                        d.edit(|v| {
                                            v["launch_local"] = if mode == "scheduled" {
                                                json!({"local":"","time_zone":zone.get_value()})
                                            } else {
                                                Value::Null
                                            };
                                        })
                                    })
                            })
                        >
                            <option value="immediate">{tr!(validation.immediate)}</option>
                            <option value="scheduled">{tr!(validation.scheduled)}</option>
                        </Select>
                        <Show when=move || {
                            !draft.get().value["launch_local"].is_null()
                        }>{schedule(draft, "launch_local", trs!(ui.launch_time), zone)}</Show>
                        {schedule(
                            draft,
                            "send_by_local",
                            trs!(ui.initial_distribution_end_optional),
                            zone,
                        )}
                        <p>
                            {tr!(
                                ui.with_no_launch_time_delivery_starts_after_launch_the_distribution_end_spaces_ini
                            )}
                        </p>
                    </section>
                    <section
                        hidden=move || step.get() != 3
                        aria-busy=move || draft.with(|d| d.review_request.is_some()).to_string()
                    >
                        <h2>{tr!(ui.review)}</h2>
                        <p>
                            {tr!(
                                ui.check_the_validated_audience_content_and_schedule_before_launching
                            )}
                        </p>
                        <Show when=move || draft.with(|d| d.review_request.is_some())>
                            <p role="status">{tr!(ui.validating_campaign_and_preparing_preview)}</p>
                        </Show>
                        <Show when=move || !loading.get() && !draft.with(Draft::policy_ready)>
                            <p role="status">
                                {tr!(
                                    ui.landing_page_defaults_must_finish_loading_before_review_if_loading_failed_return
                                )}
                            </p>
                        </Show>
                        {move || {
                            draft
                                .get()
                                .review_error
                                .map(|error| {
                                    view! {
                                        <div class="form-message error" role="alert">
                                            <p>{error}</p>
                                            <p>
                                                {tr!(
                                                    ui.go_back_to_correct_the_campaign_or_retry_if_the_request_failed
                                                )}
                                            </p>
                                            <ActionButton
                                                kind="button"
                                                on_click=move |_| retry_review.update(|n| *n += 1)
                                            >
                                                {tr!(ui.retry_validation)}
                                            </ActionButton>
                                        </div>
                                    }
                                })
                        }}
                        {move || {
                            draft
                                .get()
                                .review
                                .map(|r| {
                                    view! {
                                        <div class="campaign-review">
                                            {crate::resources::page_warnings(
                                                Signal::derive(move || r.warnings.clone()),
                                            )}
                                            <h3>
                                                {leptos_i18n::t!(
                                                    i18n, messages.arg0_unique_recipients, arg0 = (r.total).to_string()
                                                )}
                                            </h3>
                                            <p>
                                                {leptos_i18n::t!(
                                                    i18n, messages.arg0_overlapping_recipient_entries_removed, arg0 = (r.duplicate_count).to_string()
                                                )}
                                            </p>
                                            {(r.conflict_count > 0)
                                                .then(|| {
                                                    view! {
                                                        <details>
                                                            <summary>
                                                                {leptos_i18n::t!(
                                                                    i18n, messages.arg0_conflicting_duplicate_entries_first_group_wins, arg0 = (r.conflict_count).to_string()
                                                                )}
                                                            </summary>
                                                            <p>
                                                                {tr!(
                                                                    ui.showing_up_to_100_conflicts_review_the_retained_and_discarded_recipient_attribut
                                                                )}
                                                            </p>
                                                            <ul>
                                                                {r
                                                                    .conflicts
                                                                    .into_iter()
                                                                    .map(|c| {
                                                                        view! {
                                                                            <li>
                                                                                {leptos_i18n::t_string!(
                                                                                    i18n, messages.email_retained_kept_discarded_discarded, email = c.email, kept = json!(c.kept).to_string(), discarded = json!(c.discarded).to_string()
                                                                                )}
                                                                            </li>
                                                                        }
                                                                    })
                                                                    .collect_view()}
                                                            </ul>
                                                        </details>
                                                    }
                                                })}
                                            <dl class="resource-facts">
                                                {[
                                                    ("template", r.selection.template),
                                                    ("page", r.selection.page),
                                                    ("smtp", r.selection.smtp),
                                                ]
                                                    .into_iter()
                                                    .map(|(key, selected)| {
                                                        view! {
                                                            <div>
                                                                <dt>
                                                                    {match key {
                                                                        "template" => {
                                                                            leptos_i18n::t_string!(i18n, ui.email_template)
                                                                        }
                                                                        "page" => leptos_i18n::t_string!(i18n, ui.landing_page),
                                                                        _ => leptos_i18n::t_string!(i18n, ui.sending_profile),
                                                                    }}
                                                                </dt>
                                                                <dd>
                                                                    <a
                                                                        target="_blank"
                                                                        rel="noopener"
                                                                        href=format!(
                                                                            "/{}/{}",
                                                                            match key {
                                                                                "template" => "templates",
                                                                                "page" => "pages",
                                                                                _ => "smtp",
                                                                            },
                                                                            selected.id,
                                                                        )
                                                                    >
                                                                        {selected.name}
                                                                    </a>
                                                                </dd>
                                                            </div>
                                                        }
                                                    })
                                                    .collect_view()} <div>
                                                    <dt>{tr!(ui.launch)}</dt>
                                                    <dd class="data-value">{instant(&json!(r.launch_date))}</dd>
                                                </div><div>
                                                    <dt>{tr!(ui.distribution_end)}</dt>
                                                    <dd class="data-value">
                                                        {instant(&json!(r.send_by_date))}
                                                    </dd>
                                                </div><div>
                                                    <dt>{tr!(ui.submission_tracking)}</dt>
                                                    <dd>{tr!(ui.yes)}</dd>
                                                </div><div>
                                                    <dt>{tr!(ui.password_presence_only)}</dt>
                                                    <dd>{tr!(ui.yes)}</dd>
                                                </div><div>
                                                    <dt>{tr!(ui.track_email_opens)}</dt>
                                                    <dd>
                                                        {if r.policy.track_opens {
                                                            tr!(ui.enabled).into_any()
                                                        } else {
                                                            tr!(ui.not_tracked).into_any()
                                                        }}
                                                    </dd>
                                                </div><div>
                                                    <dt>{tr!(ui.redirect)}</dt>
                                                    <dd>
                                                        {match r.policy.learning_page_id {
                                                            Some(id) => {
                                                                view! {
                                                                    <a href=format!(
                                                                        "/learning-pages/{id}",
                                                                    )>
                                                                        {array(&resources.get(), "learning_page")
                                                                            .iter()
                                                                            .find(|row| row["id"].as_i64() == Some(id))
                                                                            .map(|row| text(row, "name"))
                                                                            .unwrap_or_else(|| {
                                                                                leptos_i18n::t_string!(i18n, ui.unavailable).to_owned()
                                                                            })}
                                                                    </a>
                                                                }
                                                                    .into_any()
                                                            }
                                                            None => r.policy.redirect_url.into_any(),
                                                        }}
                                                    </dd>
                                                </div>
                                            </dl> <section class="resource-section">
                                                <h3>{tr!(ui.rendered_sample)}</h3>
                                                <p>
                                                    {move || {
                                                        #[cfg(feature = "demo")]
                                                        { leptos_i18n::t_string!(i18n, demo.email_preview_help) }
                                                        #[cfg(not(feature = "demo"))]
                                                        {
                                                            leptos_i18n::t_string!(
                                                                i18n, ui.uses_the_first_recipient_s_details_test_emails_use_sample_recipient_preview_link
                                                            )
                                                        }
                                                    }}
                                                </p>
                                                <dl class="resource-facts">
                                                    <div>
                                                        <dt>{tr!(ui.visible_from)}</dt>
                                                        <dd>{r.preview.visible_sender}</dd>
                                                    </div>
                                                    <div>
                                                        <dt>{tr!(ui.transport_sender)}</dt>
                                                        <dd>{r.preview.transport_sender}</dd>
                                                    </div>
                                                    <div>
                                                        <dt>{tr!(ui.subject)}</dt>
                                                        <dd>{r.preview.subject}</dd>
                                                    </div>
                                                </dl>
                                                <iframe
                                                    class="saved-content-preview"
                                                    title=trs!(ui.rendered_email_sample)
                                                    sandbox=""
                                                    referrerpolicy="no-referrer"
                                                    srcdoc=ruphish_ui::preview_document(&r.preview.html)
                                                ></iframe>
                                                <details>
                                                    <summary>{tr!(ui.plain_text)}</summary>
                                                    <pre>{r.preview.text}</pre>
                                                </details>
                                                <Field label=trs!(ui.test_recipient_email)>
                                                    <TextInput
                                                        input_type="email"
                                                        value=Signal::derive(move || recipient.get())
                                                        on_input=move |e| recipient.set(event_target_value(&e))
                                                    />
                                                </Field>
                                                <ActionButton
                                                    disabled=Signal::derive(move || {
                                                        busy.get() || recipient.get().is_empty()
                                                    })
                                                    on_click=send_test
                                                >
                                                    {tr!(ui.send_test_email)}
                                                </ActionButton>
                                            </section>
                                            {(!r.mailer_enabled)
                                                .then(|| {
                                                    view! {
                                                        <p class="form-message error">
                                                            {move || {
                                                                #[cfg(feature = "demo")]
                                                                { leptos_i18n::t_string!(i18n, demo.launch_notice) }
                                                                #[cfg(not(feature = "demo"))]
                                                                {
                                                                    leptos_i18n::t_string!(
                                                                        i18n, ui.the_mailer_is_disabled_this_campaign_will_remain_queued_until_an_enabled_admin_p
                                                                    )
                                                                }
                                                            }}
                                                        </p>
                                                    }
                                                })}
                                        </div>
                                    }
                                })
                        }}
                    </section>
                </FormFields>
                <FormFooter>
                    <ActionLink href="/campaigns" style=ButtonStyle::Ghost>
                        {tr!(ui.cancel)}
                    </ActionLink>
                    <ActionButton
                        disabled=Signal::derive(move || {
                            busy.get() || !can_edit || step.get() == 0
                                || draft.with(|d| d.launch.is_some() || d.page_request.is_some())
                        })
                        on_click=move |_| save.run(Some(step.get_untracked().saturating_sub(1)))
                    >
                        {tr!(ui.back)}
                    </ActionButton>
                    <ActionButton
                        disabled=Signal::derive(move || {
                            busy.get() || !can_edit
                                || draft.with(|d| d.launch.is_some() || d.page_request.is_some())
                        })
                        on_click=move |_| save.run(None)
                    >
                        {move || {
                            if persistence.revision.get().is_some() {
                                leptos_i18n::t_string!(i18n,ui.save_changes)
                            } else {
                                leptos_i18n::t_string!(i18n,ui.save_as_draft)
                            }
                        }}
                    </ActionButton>
                    <ActionButton
                        style=ButtonStyle::Primary
                        hidden=Signal::derive(move || step.get() == 3)
                        disabled=Signal::derive(move || {
                            busy.get() || loading.get() || !can_edit
                                || draft.with(|d| d.page_request.is_some())
                        })
                        on_click=move |_| save.run(Some((step.get_untracked() + 1).min(3)))
                    >
                        {tr!(ui.continue_action)}
                    </ActionButton>
                    <ActionButton
                        style=ButtonStyle::Primary
                        hidden=Signal::derive(move || step.get() != 3)
                        disabled=Signal::derive(move || {
                            busy.get() || loading.get() || !can_edit
                                || draft.with(|d| d.review.is_none() || d.review_request.is_some())
                        })
                        on_click=launch
                    >
                        {move || {
                            if draft.with(|d| d.launch.is_some()) {
                                leptos_i18n::t_string!(i18n, ui.retry_same_launch)
                            } else {
                                #[cfg(feature = "demo")]
                                { leptos_i18n::t_string!(i18n, demo.simulate_launch) }
                                #[cfg(not(feature = "demo"))]
                                {
                                    if !draft.get().value["launch_local"].is_null() {
                                        leptos_i18n::t_string!(i18n,ui.schedule_campaign)
                                    } else {
                                        leptos_i18n::t_string!(i18n, ui.launch_campaign)
                                    }
                                }
                            }
                        }}
                    </ActionButton>
                </FormFooter>
            </QueryBoundary>
        </ManagementPage>
    }
}

fn show(api: Api, token: String, id: String, section: Section) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let id = StoredValue::new(id);
    let drafts = use_context::<ruphish_ui::import_drafts::ImportDrafts>();
    let duplicate_navigate = use_navigate();
    let token = StoredValue::new(token);
    let navigate = use_navigate();
    let can_edit = editable();
    let refresh_state = crate::campaign_queries::RefreshState::provide();
    let query_params = leptos_router::hooks::use_query_map();
    let selected_recipient = Memo::new(move |_| {
        query_params
            .with(|q| q.get("recipient"))
            .and_then(|v| v.parse::<i64>().ok())
            .filter(|id| *id > 0)
    });
    let queries = expect_context::<CampaignQueries>();
    let cursors = RwSignal::new(Vec::<String>::new());
    let preference = TableSorting::new("campaign-delivery", "email", SortDirection::Asc);
    let sorting = TableSorting {
        current: preference.current,
        change: Callback::new(move |column: String| {
            batch(move || {
                cursors.set(Vec::new());
                preference.change.run(column);
            })
        }),
    };
    let after = Memo::new(move |_| cursors.with(|v| v.last().cloned().unwrap_or_default()));
    let busy = RwSignal::new(false);
    let visible = ruphish_ui::query::document_visible();
    let context = queries
        .context
        .observe_polling(move || id.get_value(), visible);
    let delivery_key = Memo::new(move |_| {
        if let Some(recipient) = selected_recipient.get() {
            (
                id.get_value(),
                recipient.to_string(),
                "recipient".into(),
                "asc".into(),
            )
        } else {
            (
                id.get_value(),
                after.get(),
                sorting.current.get().column,
                sorting.order(),
            )
        }
    });
    let read = if section == Section::Delivery {
        queries.delivery.observe_polling(
            move || delivery_key.get(),
            Signal::derive(move || visible.get() && !busy.get() && section == Section::Delivery),
        )
    } else {
        ruphish_ui::query::QueryView {
            data: Signal::derive(|| None),
            error: Signal::derive(|| None),
            failure: Signal::derive(|| None),
            fetching: false.into(),
            refresh: Callback::new(|_| {}),
        }
    };
    let delivery = read.presentation(move || delivery_key.get());
    let data = delivery.data;
    let positions = Memo::new(move |_| {
        data.with(|page| {
            page.as_ref()
                .map(|page| {
                    ruphish_ui::table_state::row_positions(page.items.iter().map(|row| row.id))
                })
                .unwrap_or_default()
        })
    });
    let confirmed = Signal::derive(move || context.data.with(Option::is_some));
    let next = Signal::derive(move || {
        data.with(|page| page.as_ref().and_then(|page| page.next_cursor.clone()))
            .filter(|next| *next != after.get())
    });
    crate::campaign_queries::RefreshState::register(context.fetching);
    crate::campaign_queries::RefreshState::register(read.fetching);
    let header_loading = refresh_state.fetching();
    let content_ready = refresh_state.ready();
    let loading = read.fetching;
    let tasks = ruphish_ui::view_tasks::ViewTasks::default();
    let message = Message::new(String::new());
    let action = RwSignal::new(None::<(String, Value)>);
    let duplicate = Callback::new(move |_| {
        if busy.get_untracked() {
            return;
        }
        let navigate = duplicate_navigate.clone();
        busy.set(true);
        tasks.spawn(async move {
            match api
                .get(&format!("/api/campaigns/{}/draft", id.get_value()))
                .await
                .map_err(|e| e.message)
                .and_then(|value| {
                    serde_json::from_value::<ruphish_contracts::campaign_workflow::Duplicate>(value)
                        .map_err(|_| api.invalid_read())
                }) {
                Ok(value) => {
                    let inserted = drafts
                        .ok_or_else(|| {
                            leptos_i18n::t_string!(i18n, ui.draft_store_unavailable).to_owned()
                        })
                        .and_then(|store| {
                            store
                                .insert(
                                    "campaigns",
                                    ruphish_ui::resource_state::ResourceDraft::new(
                                        json!(value),
                                        None,
                                    ),
                                    ruphish_ui::import_state::ImportReview {
                                        notes: vec![],
                                        rejected_rows: vec![],
                                        requires_acknowledgement: false,
                                        acknowledged: true,
                                    },
                                )
                                .map_err(|e| ruphish_ui::localization::import_error(i18n, &e))
                        });
                    match inserted {
                        Ok(key) => {
                            navigate(&format!("/campaigns/new?draft={key}"), Default::default())
                        }
                        Err(e) => message.error(e),
                    }
                }
                Err(e) => message.error(e),
            }
            busy.set(false);
        });
    });
    let confirm = Callback::new(move |()| {
        let Some((kind, row)) = action.get_untracked() else {
            return;
        };
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        let navigate = navigate.clone();
        tasks.spawn(async move {
            let (method, path, body) = match kind.as_str() {
                "delete" => ("DELETE", format!("/api/campaigns/{}", id.get_value()), None),
                "complete" => (
                    "POST",
                    format!("/api/campaigns/{}/complete", id.get_value()),
                    None,
                ),
                _ => (
                    "POST",
                    format!("/api/campaigns/{}/delivery/{}", id.get_value(), row["id"]),
                    Some(json!({"action":kind,"generation":row["generation"]})),
                ),
            };
            match api
                .command(method, &path, &token.get_value(), body.as_ref(), None)
                .await
            {
                Ok(_) => {
                    action.set(None);
                    message.status(leptos_i18n::t_string!(i18n, ui.campaign_updated).to_owned());
                    if kind == "delete" {
                        queries.deleted("campaign", &id.get_value());
                        navigate("/campaigns", Default::default());
                    } else {
                        queries.changed(&id.get_value());
                    }
                }
                Err(e) => message.error(e),
            }
            busy.set(false);
        });
    });
    let content = view! {
        <ManagementPage>
            <BackLink href="/campaigns">{tr!(ui.back_to_campaigns)}</BackLink>
            <PageHeader
                title=Signal::derive(move || {
                    if confirmed.get() {
                        context
                            .data
                            .with(|c| c.as_ref().map(|c| c.name.clone()).unwrap_or_default())
                    } else {
                        leptos_i18n::t_string!(i18n,ui.campaign).to_owned()
                    }
                })
                badge=Signal::derive(move || ruphish_ui::localization::event_label(
                    i18n,
                    context.data.with(|c| c.as_ref().map(|c| c.status.as_str()).unwrap_or_default()),
                ))
            >
                <RefreshButton
                    fetching=header_loading
                    disabled=busy
                    on_refresh=Callback::new(move |_| queries.changed(&id.get_value()))
                />
                <Show when=move || section == Section::Results>
                    <crate::campaign_results::ExportMenu
                        api=api
                        campaign=id
                        filter=Signal::derive({
                            let filters = crate::campaign_results::filters();
                            move || filters.get()
                        })
                        ready=Signal::derive(move || confirmed.get() && content_ready.get())
                        message=message
                    />
                </Show>
                <Show when=move || can_edit>
                    <ActionMenu
                        label=trs!(ui.actions)
                        disabled=Signal::derive(move || busy.get() || !confirmed.get())
                    >
                        <ActionButton
                            hidden=!can_edit
                            disabled=Signal::derive(move || busy.get() || !confirmed.get())
                            on_click=duplicate
                        >
                            {tr!(ui.duplicate)}
                        </ActionButton>
                        <ActionButton
                            hidden=!can_edit
                            disabled=Signal::derive(move || {
                                busy.get() || !confirmed.get()
                                    || context
                                        .data
                                        .with(|c| {
                                            c.as_ref()
                                                .is_some_and(|c| c.status == CampaignStatus::Completed)
                                        })
                            })
                            on_click=move |_| {
                                action.set(Some(("complete".into(), Value::Null)));
                            }
                        >
                            {tr!(ui.complete_campaign)}
                        </ActionButton>
                        <ActionButton
                            style=ButtonStyle::Ghost
                            hidden=!can_edit
                            disabled=Signal::derive(move || busy.get() || !confirmed.get())
                            on_click=move |_| {
                                action.set(Some(("delete".into(), Value::Null)));
                            }
                        >
                            {tr!(ui.delete)}
                        </ActionButton>
                    </ActionMenu>
                </Show>
            </PageHeader>
            <div class="campaign-heading-facts" hidden=move || !confirmed.get()>
                <span>
                    {tr!(ui.scheduled_launch)}": "
                    {move || {
                        context
                            .data
                            .with(|c| instant(
                                &c.as_ref().and_then(|c| c.launch_date.clone()).into(),
                            ))
                    }}
                </span>
                <span>
                    <strong>{tr!(ui.recipients)}":"</strong>
                    " "
                    {move || context.data.with(|c| c.as_ref().map(|c| c.total))}
                </span>
            </div>
            <nav class="campaign-sections" aria-label=move || trs!(ui.campaign).get()>
                {[
                    (Section::Setup, trs!(ui.setup)),
                    (Section::Delivery, trs!(ui.delivery)),
                    (Section::Results, trs!(ui.results)),
                    (Section::Connections, trs!(ui.resources)),
                ]
                    .into_iter()
                    .map(|(item, label)| {
                        view! {
                            <a
                                href=format!("/campaigns/{}/{}", id.get_value(), item.path())
                                aria-current=if section == item { Some("page") } else { None }
                            >
                                {move || label.get()}
                            </a>
                        }
                    })
                    .collect_view()}
            </nav>
            <QueryBoundary
                fetching=context.fetching
                has_data=confirmed
                error=context.error
                noun=trs!(ui.campaign)
                refresh=context.refresh
            >

                {message.view()}
                {match section {
                    Section::Delivery => {
                        view! {
                            <section>
                                <QueryBoundary
                                    fetching=loading
                                    has_data=Signal::derive(move || {
                                        delivery.data.with(Option::is_some)
                                    })
                                    error=read.error
                                    noun=trs!(ui.campaign_delivery)
                                    refresh=read.refresh
                                >
                                    <Show when=move || {
                                        data.with(|page| {
                                            page.as_ref().is_some_and(|page| !page.mailer_enabled)
                                        })
                                    }>
                                        <p class="form-message error">
                                            {tr!(ui.delivery_processing_is_disabled_in_this_process)}
                                        </p>
                                    </Show>
                                    <p>
                                        {tr!(
                                            ui.acceptance_means_the_smtp_server_accepted_the_message_participant_credentials_ar
                                        )}
                                    </p>
                                    <Show when=move || { confirmed.get() }>
                                        <Show when=move || selected_recipient.get().is_some()>
                                            <BackLink href=format!(
                                                "/campaigns/{}/delivery",
                                                id.get_value(),
                                            )>{tr!(ui.recipients)}</BackLink>
                                        </Show>
                                        {delivery_progress(data)}
                                        <DataTable
                                            sorting=sorting
                                            columns=vec!["", "email", "state", "due", "attempt_count"]
                                            label=trs!(ui.campaign_delivery)
                                            headers=vec![
                                                trs!(ui.history),
                                                trs!(ui.recipient),
                                                trs!(ui.delivery),
                                                trs!(ui.next_last_due),
                                                trs!(ui.attempts_and_actions),
                                            ]
                                        >
                                            <For
                                                each=move || {
                                                    data.with(|page| {
                                                        page.as_ref()
                                                            .map(|page| {
                                                                page.items.iter().map(|row| row.id).collect::<Vec<_>>()
                                                            })
                                                            .unwrap_or_default()
                                                    })
                                                }
                                                key=|id| *id
                                                children=move |result| {
                                                    crate::campaign_delivery::recipient_row(
                                                        api,
                                                        id,
                                                        result,
                                                        Signal::derive(move || {
                                                            let index = positions
                                                                .with(|positions| positions.get(&result).copied());
                                                            data.with(|page| {
                                                                page.as_ref()
                                                                    .and_then(|page| {
                                                                        index.and_then(|index| page.items.get(index))
                                                                    })
                                                                    .cloned()
                                                            })
                                                        }),
                                                        crate::campaign_delivery::Controls {
                                                            busy,
                                                            can_edit,
                                                            action,
                                                        },
                                                    )
                                                }
                                            />
                                        </DataTable>
                                    </Show>
                                    <PaginationControls
                                        disabled=Signal::derive(move || loading.get() || busy.get())
                                        has_previous=Signal::derive(move || {
                                            !cursors.with(Vec::is_empty)
                                        })
                                        has_next=Signal::derive(move || next.get().is_some())
                                        on_previous=Callback::new(move |()| {
                                            cursors
                                                .update(|v| {
                                                    v.pop();
                                                })
                                        })
                                        on_next=Callback::new(move |()| {
                                            if let Some(next) = next.get_untracked() {
                                                cursors.update(|v| v.push(next));
                                            }
                                        })
                                    />

                                </QueryBoundary>
                            </section>
                        }
                            .into_any()
                    }
                    Section::Results => crate::campaign_results::page(api).into_any(),
                    Section::Setup => {
                        view! { {move || context.data.with(|c| c.as_ref().map(campaign_setup))} }
                            .into_any()
                    }
                    Section::Connections => {
                        view! { {move || context.data.with(|c| c.as_ref().map(campaign_context))} }
                            .into_any()
                    }
                }}
                {move || {
                    action
                        .get()
                        .map(|(kind, _)| {
                            let (title, description, label) = match kind.as_str() {
                                "delete" => {
                                    (
                                        leptos_i18n::t_string!(i18n, ui.delete_campaign),
                                        leptos_i18n::t_string!(
                                            i18n, ui.campaign_history_will_be_removed_already_submitted_mail_cannot_be_recalled
                                        ),
                                        leptos_i18n::t_string!(i18n, ui.delete_campaign_2),
                                    )
                                }
                                "complete" => {
                                    (
                                        leptos_i18n::t_string!(i18n, ui.complete_campaign_2),
                                        leptos_i18n::t_string!(
                                            i18n, ui.pending_delivery_will_stop_already_armed_or_in_flight_submissions_may_still_fini
                                        ),
                                        leptos_i18n::t_string!(i18n, ui.complete_campaign),
                                    )
                                }
                                "resend_unknown" => {
                                    (
                                        leptos_i18n::t_string!(
                                            i18n, ui.resend_despite_an_unknown_outcome
                                        ),
                                        leptos_i18n::t_string!(
                                            i18n, ui.check_your_mail_server_first_the_earlier_attempt_may_have_been_accepted_resendin
                                        ),
                                        leptos_i18n::t_string!(i18n, ui.resend),
                                    )
                                }
                                "confirm_accepted" => {
                                    (
                                        leptos_i18n::t_string!(i18n, ui.confirm_smtp_acceptance),
                                        leptos_i18n::t_string!(
                                            i18n, ui.use_the_attempt_message_id_to_verify_acceptance_in_your_mail_server_before_recor
                                        ),
                                        leptos_i18n::t_string!(i18n, ui.confirm_accepted),
                                    )
                                }
                                _ => {
                                    (
                                        leptos_i18n::t_string!(i18n, ui.retry_this_delivery),
                                        leptos_i18n::t_string!(
                                            i18n, ui.correct_the_reported_content_account_or_sending_profile_problem_before_retrying
                                        ),
                                        leptos_i18n::t_string!(i18n, ui.retry),
                                    )
                                }
                            };
                            let counts = data
                                .with_untracked(|page| {
                                    page.as_ref()
                                        .map(|page| page.counts.clone())
                                        .unwrap_or_default()
                                });
                            let description = if matches!(kind.as_str(), "complete" | "delete") {
                                leptos_i18n::t_string!(
                                    i18n, messages.arg0_pending_arg1_preparing_arg2_submitting_arg3, arg0 = (description).to_string(), arg1 = ([DeliveryState::Ready,DeliveryState::Retry,DeliveryState::Blocked].iter().map(|k|counts.get(k).copied().unwrap_or(0)).sum::<i64>()).to_string(), arg2 = (counts.get(&DeliveryState::Preparing).copied().unwrap_or(0)).to_string(), arg3 = (counts.get(&DeliveryState::Submitting).copied().unwrap_or(0)).to_string()
                                )
                            } else {
                                description.to_owned()
                            };
                            view! {
                                <DeleteConfirmation
                                    title=title
                                    description=description
                                    confirm_label=label
                                    busy=Signal::derive(move || busy.get())
                                    on_confirm=confirm
                                    on_cancel=Callback::new(move |()| action.set(None))
                                    message=message
                                />
                            }
                        })
                }}
            </QueryBoundary>
        </ManagementPage>
    };
    view! {
        <ReadBoundary
            failure=context.failure
            has_data=Signal::derive(move || context.data.with(Option::is_some))
            fetching=context.fetching
            retry=context.refresh
            back_href="/campaigns"
            back_label=trs!(ui.back_to_campaigns)
        >
            {content}
        </ReadBoundary>
    }
}

fn delivery_progress(data: Signal<Option<Arc<reads::Delivery>>>) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    let states = [
        (
            DeliveryState::Ready,
            "neutral",
            trs!(ui.queued_waiting_for_its_scheduled_time_or_an_available_worker),
        ),
        (
            DeliveryState::Preparing,
            "active",
            trs!(
                ui.a_worker_claimed_this_recipient_and_is_preparing_the_sending_attempt_after_a_cra
            ),
        ),
        (
            DeliveryState::Submitting,
            "active",
            trs!(
                ui.submission_has_been_authorized_but_smtp_acceptance_is_not_yet_recorded_after_a_c
            ),
        ),
        (
            DeliveryState::Retry,
            "warning",
            trs!(ui.a_retryable_failure_was_recorded_another_attempt_is_scheduled_with_a_delay),
        ),
        (
            DeliveryState::Accepted,
            "success",
            trs!(
                ui.smtp_acceptance_was_recorded_or_explicitly_confirmed_by_an_operator_using_mail_s
            ),
        ),
        (
            DeliveryState::Failed,
            "failure",
            trs!(
                ui.content_preparation_failed_the_message_was_permanently_rejected_or_the_retry_lim
            ),
        ),
        (
            DeliveryState::Blocked,
            "warning",
            trs!(
                ui.an_account_resource_or_smtp_configuration_problem_prevents_automatic_progress_co
            ),
        ),
        (
            DeliveryState::Unknown,
            "warning",
            trs!(
                ui.the_message_may_have_been_accepted_but_its_outcome_could_not_be_confirmed_check
            ),
        ),
        (
            DeliveryState::Cancelled,
            "neutral",
            trs!(ui.pending_work_was_cancelled_messages_already_submitted_cannot_be_recalled),
        ),
    ];
    view! {
        <Card class="resource-read-panel delivery-progress" label=trs!(ui.delivery_progress)>
            <h2>{tr!(ui.delivery_progress)}</h2>
            <div class="delivery-chart-layout">
                {crate::results_chart::delivery_donut(data)}
                <dl class="delivery-metrics">
                    {states
                        .into_iter()
                        .map(move |(state, tone, explanation)| {
                            let label = Signal::derive(move || ruphish_ui::localization::delivery_label(
                                i18n,
                                state.as_str(),
                            ));
                            view! {
                                <div
                                    class=format!("delivery-metric {tone}")
                                    class:has-count={move || {
                                        data
                                            .with(|page| {
                                                page.as_ref()
                                                    .and_then(|page| page.counts.get(&state).copied())
                                                    .unwrap_or(0)
                                            }) > 0
                                    }}
                                >
                                    <dt>
                                        <span>{move || label.get()}</span>
                                        <InfoPopover
                                            id=format!("delivery-help-{}", state.as_str())
                                            label=label
                                            explanation=explanation
                                        />
                                    </dt>
                                    <dd>
                                        {move || {
                                            data.with(|page| {
                                                page.as_ref()
                                                    .and_then(|page| page.counts.get(&state).copied())
                                                    .unwrap_or(0)
                                            })
                                        }}
                                    </dd>
                                </div>
                            }
                        })
                        .collect_view()}
                </dl>
            </div>
        </Card>
    }
}

fn campaign_context(c: &reads::Header) -> impl IntoView + use<> {
    let i18n = ruphish_ui::localization::use_i18n();

    let resource = |resource: &reads::Resource, label: Signal<String>, route: &str| {
        let id = resource.id;
        let name = resource.name.clone();
        view! {
            <div>
                <dt>{move || label.get()}</dt>
                <dd>
                    {match name {
                        Some(name) => {
                            view! { <a href=format!("/{route}/{id}")>{name}</a> }.into_any()
                        }
                        None => {
                            view! {
                                <span>
                                    {move || {
                                        if id == 0 {
                                            leptos_i18n::t_string!(i18n, ui.not_selected).into()
                                        } else {
                                            leptos_i18n::t_string!(
                                                i18n, messages.unavailable_id_arg0, arg0 = (id).to_string()
                                            )
                                        }
                                    }}
                                </span>
                            }
                                .into_any()
                        }
                    }}
                </dd>
            </div>
        }
    };
    let resources = view! {
        {resource(&c.template, trs!(ui.email_template), "templates")}
        {resource(&c.page, trs!(ui.landing_page), "pages")}
        {resource(&c.smtp, trs!(ui.sending_profile), "smtp")}
    };
    let c = c.clone();
    let template_sender = c.template_sender.clone().unwrap_or_default();
    let destination = c.policy.clone();
    let track_opens = c.policy.as_ref().is_none_or(|policy| policy.track_opens);
    view! {
        <Card class="resource-read-panel campaign-context" label=trs!(ui.campaign_details)>
            <h2>{tr!(ui.campaign_details)}</h2>
            <dl class="resource-facts">
                {resources} <div>
                    <dt>{tr!(ui.track_email_opens)}</dt>
                    <dd>
                        {if track_opens {
                            tr!(ui.enabled).into_any()
                        } else {
                            tr!(ui.not_tracked).into_any()
                        }}
                    </dd>
                </div> <div>
                    <dt>{tr!(ui.redirect_destination)}</dt>
                    <dd>
                        {match destination.as_ref().and_then(|policy| policy.learning_page_id) {
                            Some(id) => {
                                view! {
                                    <a href=format!(
                                        "/learning-pages/{id}",
                                    )>{tr!(ui.learning_page)}" #"{id}</a>
                                }
                                    .into_any()
                            }
                            None => {
                                destination
                                    .map(|policy| policy.redirect_url)
                                    .unwrap_or_default()
                                    .into_any()
                            }
                        }}
                    </dd>
                </div> <div>
                    <dt>{tr!(ui.email_subject_template)}</dt>
                    <dd>
                        {c
                            .subject
                            .as_deref()
                            .map(|s| {
                                if s.is_empty() {
                                    leptos_i18n::t_string!(i18n, ui.no_subject)
                                } else {
                                    s
                                }
                            })
                            .unwrap_or(leptos_i18n::t_string!(i18n, ui.unavailable))
                            .to_owned()}
                    </dd>
                </div> <div>
                    <dt>{tr!(ui.sending_profile_from_address)}</dt>
                    <dd>
                        {c
                            .profile_sender
                            .as_deref()
                            .unwrap_or(leptos_i18n::t_string!(i18n, ui.unavailable))
                            .to_owned()}
                    </dd>
                </div> <div>
                    <dt>{tr!(ui.template_from_override)}</dt>
                    <dd>
                        {move || {
                            if template_sender.is_empty() {
                                leptos_i18n::t_string!(i18n, ui.use_sending_profile_address).into()
                            } else {
                                template_sender.clone()
                            }
                        }}
                    </dd>
                </div>
            </dl>
            <p class="resource-usage">
                {leptos_i18n::t!(
                    i18n, messages.times_shown_in_arg0_template_and_sending_profile_details_reflect_the_current_sav, arg0 = (local_zone()).to_string()
                )}
            </p>
        </Card>
    }
}

fn campaign_setup(c: &reads::Header) -> impl IntoView + use<> {
    let c = c.clone();
    let i18n = ruphish_ui::localization::use_i18n();
    view! {
        <Card class="resource-read-panel" label=trs!(ui.setup)>
            <h2>{tr!(ui.setup)}</h2>
            <dl class="resource-facts">
                <div>
                    <dt>{tr!(ui.recipients)}</dt>
                    <dd>{c.total}</dd>
                </div>
                <div>
                    <dt>{tr!(ui.scheduled_launch)}</dt>
                    <dd class="data-value">{instant(&c.launch_date.clone().into())}</dd>
                </div>
                <div>
                    <dt>{tr!(ui.initial_distribution_end)}</dt>
                    <dd>
                        {if c.send_by_date.is_none() {
                            leptos_i18n::t_string!(i18n, ui.no_distribution_window).into()
                        } else {
                            instant(&c.send_by_date.clone().into())
                        }}
                    </dd>
                </div>
                <div>
                    <dt>{tr!(ui.public_campaign_url)}</dt>
                    <dd>{c.url.clone()}</dd>
                </div>
            </dl>
        </Card>
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Section {
    Setup,
    Delivery,
    Results,
    Connections,
}
impl Section {
    pub(super) fn landing(status: ruphish_domain::campaign::CampaignStatus) -> Self {
        match status {
            ruphish_domain::campaign::CampaignStatus::Completed => Self::Results,
            _ => Self::Delivery,
        }
    }

    pub(super) fn path(self) -> &'static str {
        match self {
            Self::Setup => "setup",
            Self::Delivery => "delivery",
            Self::Results => "results",
            Self::Connections => "connections",
        }
    }
}
