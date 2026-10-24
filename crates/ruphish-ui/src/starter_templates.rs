//! Read-only catalog selection. All writes go through ordinary resource creation.
use crate::{
    account::Api, management_queries::ManagementQueries, resource_queries::ResourceQueries,
    resources::Family, shared::components::*,
};
use leptos::prelude::*;
use leptos::task::spawn_local_scoped_with_cancellation as spawn_local;
use ruphish_contracts::{
    resources::ResourceSummaries,
    starter_templates::{Entry, Kind},
};
use ruphish_ui::{
    preview_document, preview_state::ContentPreview, starter_names::available_name, tr, trs,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/web/generated/starter-templates.js")]
extern "C" {
    #[wasm_bindgen(catch, js_name = prepareLanding)]
    fn prepare_landing(
        source: &str,
        prefill: bool,
        branding: &str,
        logo: &str,
    ) -> Result<String, JsValue>;
    #[wasm_bindgen(catch, js_name = readLogo)]
    async fn read_logo(input: &web_sys::HtmlInputElement) -> Result<JsValue, JsValue>;
}

#[derive(Clone, Copy, Default)]
struct Options {
    prefill: bool,
    logo: bool,
}

#[derive(Clone, Copy)]
struct Selection {
    entries: RwSignal<BTreeMap<String, Options>>,
    logo: RwSignal<String>,
    uploading: RwSignal<bool>,
    busy: RwSignal<bool>,
}

fn prepare(entry: &Entry, options: Options, logo: &str) -> Result<Value, String> {
    let i18n = ruphish_ui::localization::use_i18n();
    let mut resource = entry.resource.clone();
    if entry.summary.kind == Kind::LandingPage {
        if options.logo && logo.is_empty() {
            return Err(leptos_i18n::t_string!(i18n, ui.starter_logo_required).into());
        }
        let branding = if options.logo {
            entry
                .summary
                .company_logo
                .as_ref()
                .and_then(|v| serde_json::to_string(v).ok())
        } else {
            None
        };
        let html = prepare_landing(
            resource["html"].as_str().unwrap_or_default(),
            options.prefill && entry.summary.prefill,
            branding.as_deref().unwrap_or_default(),
            logo,
        )
        .map_err(|_| leptos_i18n::t_string!(i18n, ui.starter_invalid).to_string())?;
        resource["html"] = html.into();
    }
    Ok(resource)
}

pub(crate) fn page(api: Api, token: String) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    let queries: ManagementQueries = expect_context();
    let resources: ResourceQueries = expect_context();
    let catalog = queries.starters.observe(|| ());
    let token = StoredValue::new(token);
    let tab = RwSignal::new(0usize);
    let search = RwSignal::new(String::new());
    let focus = RwSignal::new(None::<String>);
    let state = Selection {
        entries: RwSignal::new(BTreeMap::new()),
        logo: RwSignal::new(String::new()),
        uploading: RwSignal::new(false),
        busy: RwSignal::new(false),
    };
    let results = RwSignal::new(Vec::<(String, Result<String, String>)>::new());
    let logo_error = RwSignal::new(None::<String>);
    let upload_generation = RwSignal::new(0u64);
    let needs_logo = Memo::new(move |_| state.entries.with(|rows| rows.values().any(|v| v.logo)));
    Effect::new(move |_| {
        if !needs_logo.get() {
            // Removing the last branded selection also cancels its pending upload result.
            upload_generation.update(|generation| *generation += 1);
            state.uploading.set(false);
            logo_error.set(None);
        }
    });
    let visible = Memo::new(move |_| {
        let kind = if tab.get() == 0 {
            Kind::Email
        } else {
            Kind::LandingPage
        };
        let search = search.get().trim().to_lowercase();
        catalog
            .data
            .get()
            .unwrap_or_default()
            .into_iter()
            .filter(|row| {
                row.kind == kind
                    && format!("{} {}", row.name, row.brand)
                        .to_lowercase()
                        .contains(&search)
            })
            .collect::<Vec<_>>()
    });
    Effect::new(move |_| {
        let rows = visible.get();
        if !rows
            .iter()
            .any(|row| Some(&row.id) == focus.get_untracked().as_ref())
        {
            focus.set(rows.first().map(|row| row.id.clone()));
        }
    });
    crate::navigation::protect_draft(move || state.busy.get());
    let add = move |_| {
        if state.busy.get_untracked() || state.uploading.get_untracked() {
            return;
        }
        let selected = state.entries.get_untracked();
        if selected.is_empty()
            || (needs_logo.get_untracked() && state.logo.get_untracked().is_empty())
        {
            return;
        }
        state.busy.set(true);
        results.set(Vec::new());
        let logo = state.logo.get_untracked();
        spawn_local(async move {
            let mut email_names = None;
            let mut page_names = None;
            for (id, options) in selected {
                let mut display_name = catalog
                    .data
                    .get_untracked()
                    .unwrap_or_default()
                    .into_iter()
                    .find(|row| row.id == id)
                    .map(|row| row.name)
                    .unwrap_or_else(|| id.clone());
                let result = async {
                    let entry: Entry = api.read(&format!("/api/starter-templates/{id}")).await?;
                    let family = if entry.summary.kind == Kind::Email {
                        Family::Templates
                    } else {
                        Family::Pages
                    };
                    let names = if family == Family::Templates {
                        &mut email_names
                    } else {
                        &mut page_names
                    };
                    if names.is_none() {
                        let rows: ResourceSummaries = api
                            .read(&format!("/api/{}/summary", family.route()))
                            .await?;
                        *names = Some(
                            rows.items
                                .into_iter()
                                .map(|row| row.name)
                                .collect::<HashSet<_>>(),
                        );
                    }
                    let mut payload = prepare(&entry, options, &logo)?;
                    if let Some(names) = names {
                        display_name = available_name(&entry.summary.name, names);
                        names.insert(display_name.clone());
                    }
                    payload["name"] = display_name.clone().into();
                    let response = api
                        .resource(
                            "POST",
                            &format!("/api/{}/", family.route()),
                            &token.get_value(),
                            Some(&payload),
                            None,
                        )
                        .await
                        .and_then(|(saved, _)| {
                            ResourceQueries::validate_detail(api, family, &saved, None)?;
                            saved["id"].as_i64().ok_or_else(|| api.invalid_read())
                        });
                    match response {
                        Ok(id) => {
                            resources.saved(family, id);
                            Ok(format!("/{}/{id}", family.route()))
                        }
                        Err(error) => {
                            resources.refresh_inventory(family);
                            Err(error)
                        }
                    }
                }
                .await;
                // Every attempt is explicit. Never retry an ambiguous POST automatically.
                state.entries.update(|rows| {
                    rows.remove(&id);
                });
                results.update(|rows| rows.push((display_name, result)));
            }
            state.busy.set(false);
        });
    };
    view! {
        <ManagementPage>
            <BackLink href=Signal::derive(|| {
                "/settings".to_owned()
            })>{tr!(ui.company_settings)}</BackLink>
            <PageHeader title=trs!(ui.starter_title) subtitle=trs!(ui.starter_description) />
            <QueryStatus
                fetching=catalog.fetching
                has_data=Signal::derive(move || catalog.data.get().is_some())
                error=catalog.error
                noun=trs!(ui.starter_title)
                refresh=catalog.refresh
            />
            <Tabs
                label=trs!(ui.starter_title)
                selected=tab
                tabs=vec![
                    Tab {
                        id: "starter-emails",
                        label: trs!(ui.email_templates),
                        disabled: false.into(),
                    },
                    Tab {
                        id: "starter-pages",
                        label: trs!(ui.landing_pages),
                        disabled: false.into(),
                    },
                ]
            />
            <ListToolbar
                search=search
                count=Signal::derive(move || visible.get().len())
                count_known=Signal::derive(move || catalog.data.get().is_some())
                label=trs!(ui.starter_search)
                noun=trs!(ui.starter_items)
            />
            {["starter-emails", "starter-pages"]
                .into_iter()
                .enumerate()
                .map(move |(index, panel_id)| {
                    view! {
                        <TabPanel id=panel_id selected=Signal::derive(move || tab.get() == index)>
                            <Show when=move || tab.get() == index>
                                <ListEmptyState
                                    ready=Signal::derive(move || catalog.data.get().is_some())
                                    count=Signal::derive(move || visible.get().len())
                                    search=search
                                    guidance=trs!(ui.starter_empty)
                                />
                                <div class="starter-workspace">
                                    <Card class="starter-list" label=trs!(ui.starter_items)>
                                        <For
                                            each=move || visible.get()
                                            key=|row| row.id.clone()
                                            children=move |row| {
                                                let id = StoredValue::new(row.id);
                                                let name = StoredValue::new(row.name);
                                                view! {
                                                    <div
                                                        class="starter-row"
                                                        class:is-current=move || {
                                                            focus.get().as_ref() == Some(&id.get_value())
                                                        }
                                                    >
                                                        <Checkbox
                                                            label=Signal::derive(move || name.get_value())
                                                            checked=Signal::derive(move || {
                                                                state.entries.with(|v| v.contains_key(&id.get_value()))
                                                            })
                                                            disabled=Signal::derive(move || state.busy.get())
                                                            on_toggle=Callback::new(move |checked| {
                                                                state
                                                                    .entries
                                                                    .update(|rows| {
                                                                        if checked {
                                                                            rows.entry(id.get_value()).or_default();
                                                                        } else {
                                                                            rows.remove(&id.get_value());
                                                                        }
                                                                    });
                                                                focus.set(Some(id.get_value()));
                                                            })
                                                        />
                                                        <ActionButton
                                                            size=ControlSize::Small
                                                            pressed=Signal::derive(move || {
                                                                (focus.get().as_ref() == Some(&id.get_value())).to_string()
                                                            })
                                                            on_click=move |_| focus.set(Some(id.get_value()))
                                                        >
                                                            {tr!(ui.preview)}
                                                        </ActionButton>
                                                    </div>
                                                }
                                            }
                                        />
                                    </Card>
                                    {move || {
                                        focus.get().map(|id| preview_panel(api, token, id, state))
                                    }}
                                </div>
                            </Show>
                        </TabPanel>
                    }
                })
                .collect_view()}
            <Show when=move || needs_logo.get()>
                <Card class="starter-upload">
                    <Field label=trs!(ui.starter_logo_upload)>
                        <input
                            type="file"
                            accept="image/png,image/jpeg,image/webp"
                            disabled=move || state.busy.get()
                            on:change=move |event| {
                                let input = event_target::<web_sys::HtmlInputElement>(&event);
                                upload_generation.update(|v| *v += 1);
                                let generation = upload_generation.get_untracked();
                                state.logo.set(String::new());
                                logo_error.set(None);
                                state.uploading.set(true);
                                spawn_local(async move {
                                    let result = read_logo(&input).await;
                                    if generation != upload_generation.get_untracked() {
                                        return;
                                    }
                                    match result.ok().and_then(|v| v.as_string()) {
                                        Some(value) => state.logo.set(value),
                                        None => {
                                            logo_error
                                                .set(
                                                    Some(
                                                        leptos_i18n::t_string!(i18n, ui.starter_logo_invalid).into(),
                                                    ),
                                                )
                                        }
                                    }
                                    state.uploading.set(false);
                                });
                            }
                        />
                    </Field>
                    <p>{tr!(ui.starter_logo_help)}</p>
                    <Show when=move || !state.logo.get().is_empty()>
                        <img
                            class="starter-logo-sample"
                            alt=trs!(ui.starter_logo_upload)
                            src=move || state.logo.get()
                        />
                    </Show>
                    <Show when=move || state.uploading.get()>
                        <p role="status">{tr!(ui.starter_logo_loading)}</p>
                    </Show>
                    {move || {
                        logo_error
                            .get()
                            .map(|error| {
                                view! {
                                    <p role="alert" class="form-message error has-content">
                                        {error}
                                    </p>
                                }
                            })
                    }}
                </Card>
            </Show>
            <FormFooter spread=true>
                <p role="status">
                    {move || {
                        format!(
                            "{} {}",
                            state.entries.with(|v| v.len()),
                            leptos_i18n::t_string!(i18n, ui.starter_selected),
                        )
                    }}
                </p>
                <ActionButton
                    style=ButtonStyle::Primary
                    disabled=Signal::derive(move || {
                        state.busy.get() || state.uploading.get()
                            || state.entries.with(BTreeMap::is_empty)
                            || (needs_logo.get() && state.logo.get().is_empty())
                    })
                    on_click=add
                >
                    {move || {
                        if state.busy.get() {
                            leptos_i18n::t_string!(i18n, ui.starter_adding)
                        } else {
                            leptos_i18n::t_string!(i18n, ui.starter_add)
                        }
                    }}
                </ActionButton>
            </FormFooter>
            <Show when=move || !results.get().is_empty()>
                <Card label=trs!(ui.starter_results)>
                    <h2>{tr!(ui.starter_results)}</h2>
                    <ul aria-live="polite">
                        {move || {
                            results
                                .get()
                                .into_iter()
                                .map(|(name, result)| {
                                    view! {
                                        <li>
                                            {match result {
                                                Ok(path) => {
                                                    view! {
                                                        <span>
                                                            {tr!(ui.starter_added)}" "<a href=path>{name}</a>
                                                        </span>
                                                    }
                                                        .into_any()
                                                }
                                                Err(error) => {
                                                    view! {
                                                        <span class="error">
                                                            {name}": "{error}" "{tr!(ui.starter_check_before_retry)}
                                                        </span>
                                                    }
                                                        .into_any()
                                                }
                                            }}
                                        </li>
                                    }
                                })
                                .collect_view()
                        }}
                    </ul>
                </Card>
            </Show>
        </ManagementPage>
    }
}

fn preview_panel(
    api: Api,
    token: StoredValue<String>,
    id: String,
    state: Selection,
) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    let queries: ManagementQueries = expect_context();
    let id = StoredValue::new(id);
    let detail = queries.starter.observe(move || id.get_value());
    let preview = RwSignal::new(ContentPreview::default());
    let retry = RwSignal::new(0u64);
    let selected = Signal::derive(move || state.entries.with(|v| v.contains_key(&id.get_value())));
    let options = Signal::derive(move || {
        state
            .entries
            .with(|v| v.get(&id.get_value()).copied().unwrap_or_default())
    });
    Effect::new(move |_| {
        let entry = detail.data.get();
        let options = options.get();
        let logo = state.logo.get();
        retry.get();
        preview.update(ContentPreview::clear);
        let Some(entry) = entry else {
            return;
        };
        let payload = match prepare(&entry, options, &logo) {
            Ok(value) => value,
            Err(error) => {
                preview.update(|p| p.error = Some(error));
                return;
            }
        };
        let source = payload["html"].as_str().unwrap_or_default().to_owned();
        let Some(generation) = preview.try_update(|p| p.begin(source.clone())) else {
            return;
        };
        spawn_local(async move {
            let body = json!({"html":source,"recipient":{"email":"alice@example.test","first_name":"Alice","last_name":"Example","position":"Analyst"}});
            let result = api
                .resource(
                    "POST",
                    "/api/util/preview",
                    &token.get_value(),
                    Some(&body),
                    None,
                )
                .await
                .and_then(|(value, _)| {
                    value["html"]
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| api.invalid_read())
                });
            preview.update(|p| p.finish(generation, &source, result));
        });
    });
    view! {
        <Card class="starter-preview">
            <QueryStatus
                fetching=detail.fetching
                has_data=Signal::derive(move || detail.data.get().is_some())
                error=detail.error
                noun=trs!(ui.preview)
                refresh=detail.refresh
            />
            {move || {
                detail
                    .data
                    .get()
                    .map(|entry| {
                        let prefill = entry.summary.prefill;
                        let branding = entry.summary.company_logo;
                        view! {
                            <h2>{entry.summary.name}</h2>
                            <Show when=move || prefill>
                                <p>{tr!(ui.starter_options_help)}</p>
                                <Checkbox
                                    label=trs!(ui.starter_prefill)
                                    checked=Signal::derive(move || options.get().prefill)
                                    disabled=Signal::derive(move || {
                                        !selected.get() || state.busy.get()
                                    })
                                    on_toggle=Callback::new(move |checked| {
                                        state
                                            .entries
                                            .update(|v| {
                                                if let Some(options) = v.get_mut(&id.get_value()) {
                                                    options.prefill = checked;
                                                }
                                            })
                                    })
                                />
                            </Show>
                            {branding
                                .map(|branding| {
                                    view! {
                                        <Checkbox
                                            label=trs!(ui.starter_use_logo)
                                            checked=Signal::derive(move || options.get().logo)
                                            disabled=Signal::derive(move || {
                                                !selected.get() || state.busy.get()
                                            })
                                            on_toggle=Callback::new(move |checked| {
                                                state
                                                    .entries
                                                    .update(|v| {
                                                        if let Some(options) = v.get_mut(&id.get_value()) {
                                                            options.logo = checked;
                                                        }
                                                    })
                                            })
                                        />
                                        <p class="field-help">{branding.context}</p>
                                    }
                                })}
                        }
                    })
            }}
            <Show when=move || preview.with(|p| p.loading)>
                <p role="status">{tr!(ui.starter_preview_loading)}</p>
            </Show>
            {move || {
                preview
                    .with(|p| p.error.clone())
                    .map(|error| {
                        view! {
                            <p class="form-message error has-content" role="alert">
                                {error}
                            </p>
                            <ActionButton on_click=move |_| {
                                retry.update(|v| *v += 1)
                            }>{tr!(ui.starter_retry_preview)}</ActionButton>
                        }
                    })
            }}
            {move || {
                preview
                    .with(|p| p.html.clone())
                    .map(|html| {
                        view! {
                            <iframe
                                title=leptos_i18n::t_string!(i18n, ui.preview)
                                class="starter-preview-frame"
                                sandbox=""
                                referrerpolicy="no-referrer"
                                srcdoc=preview_document(&html)
                            ></iframe>
                        }
                    })
            }}
        </Card>
    }
}
