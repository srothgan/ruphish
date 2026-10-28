//! Read-only catalog selection. All writes go through ordinary resource creation.
use crate::{
    client::Api, management_queries::ManagementQueries, resource_queries::ResourceQueries,
    resources::Family, shared::components::*,
};
use leptos::prelude::*;
use leptos::task::spawn_local_scoped_with_cancellation as spawn_local;
use ruphish_contracts::{
    resources::ResourceSummaries,
    starter_templates::{Entry, Kind, Summary},
};
use ruphish_domain::reporting::SortDirection;
use ruphish_ui::table_state::Value as SortValue;
use ruphish_ui::{
    preview_document,
    preview_state::ContentPreview,
    starter_names::available_name,
    starter_selection::{LogoUpload, Options, ReviewPosition, StarterSelection},
    tr, trs,
};
use serde_json::{Value, json};
use std::collections::HashSet;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/web/generated/starter-templates.js")]
extern "C" {
    #[wasm_bindgen(catch, js_name = preparePage)]
    fn prepare_page(
        source: &str,
        prefill: bool,
        branding: &str,
        logo: &str,
        static_content: bool,
    ) -> Result<String, JsValue>;
    #[wasm_bindgen(catch, js_name = readLogo)]
    async fn read_logo(input: &web_sys::HtmlInputElement) -> Result<JsValue, JsValue>;
}

#[derive(Clone, Copy)]
struct RepositoryState {
    draft: RwSignal<StarterSelection>,
    upload: RwSignal<LogoUpload>,
    busy: RwSignal<bool>,
}

fn prepare(entry: &Entry, options: Options, logo: &str) -> Result<Value, String> {
    let i18n = ruphish_ui::localization::use_i18n();
    let mut resource = entry.resource.clone();
    if matches!(entry.summary.kind, Kind::LandingPage | Kind::LearningPage) {
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
        let html = prepare_page(
            resource["html"].as_str().unwrap_or_default(),
            entry.summary.kind == Kind::LandingPage && options.prefill && entry.summary.prefill,
            branding.as_deref().unwrap_or_default(),
            logo,
            entry.summary.kind == Kind::LearningPage,
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
    let state = RepositoryState {
        draft: RwSignal::new(StarterSelection::default()),
        upload: RwSignal::new(LogoUpload::default()),
        busy: RwSignal::new(false),
    };
    let results = RwSignal::new(Vec::<(String, Result<String, String>)>::new());
    let needs_logo = Memo::new(move |_| state.draft.with(StarterSelection::needs_logo));
    let logo_expanded = RwSignal::new(false);
    let logo_picker = NodeRef::<leptos::html::Button>::new();
    let open_logo = Callback::new(move |()| {
        logo_expanded.set(true);
        let picker = logo_picker.get_untracked();
        leptos::leptos_dom::helpers::request_animation_frame(move || {
            if let Some(picker) = picker.filter(|picker| picker.is_connected()) {
                picker.scroll_into_view();
                let _ = picker.focus();
            }
        });
    });
    let visible = Memo::new(move |_| {
        let kind = match tab.get() {
            0 => Kind::Email,
            1 => Kind::LandingPage,
            _ => Kind::LearningPage,
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
            focus.set(None);
        }
    });
    crate::navigation::protect_draft(move || state.busy.get());
    let add = move |_| {
        if state.busy.get_untracked() || state.upload.with_untracked(|upload| upload.loading) {
            return;
        }
        let selected = state.draft.with_untracked(StarterSelection::snapshot);
        if selected.is_empty()
            || (needs_logo.get_untracked()
                && state.upload.with_untracked(|upload| upload.data.is_empty()))
        {
            return;
        }
        state.busy.set(true);
        results.set(Vec::new());
        let logo = state.upload.with_untracked(|upload| upload.data.clone());
        spawn_local(async move {
            let mut email_names = None;
            let mut page_names = None;
            let mut learning_names = None;
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
                    let entry: Entry = api
                        .read(&format!("/api/starter-templates/{id}"))
                        .await
                        .map_err(|e| e.message)?;
                    let family = match entry.summary.kind {
                        Kind::Email => Family::Templates,
                        Kind::LandingPage => Family::Pages,
                        Kind::LearningPage => Family::LearningPages,
                    };
                    let names = match entry.summary.kind {
                        Kind::Email => &mut email_names,
                        Kind::LandingPage => &mut page_names,
                        Kind::LearningPage => &mut learning_names,
                    };
                    if names.is_none() {
                        let rows: ResourceSummaries = api
                            .read(&format!("/api/{}/summary", family.route()))
                            .await
                            .map_err(|e| e.message)?;
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
                            ResourceQueries::decode_detail(api, family, saved, None)
                                .map(|resource| resource.metadata.id)
                                .map_err(|e| e.message)
                        });
                    match response {
                        Ok(id) => {
                            resources.saved(family, id);
                            #[cfg(feature = "demo")]
                            api.notifications.show(DemoNotice::Created);
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
                state.draft.update(|draft| draft.select(&id, false));
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
            <QueryBoundary
                fetching=catalog.fetching
                has_data=Signal::derive(move || catalog.data.get().is_some())
                error=catalog.error
                noun=trs!(ui.starter_title)
                refresh=catalog.refresh
            >
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
                        Tab {
                            id: "starter-learning-pages",
                            label: trs!(ui.learning_pages),
                            disabled: false.into(),
                        },
                    ]
                />
                // One upload serves both page families and remains mounted across tab changes.
                <div hidden=move || {
                    tab.get() == 0
                }>{logo_panel(state, logo_expanded, logo_picker)}</div>
                {["starter-emails", "starter-pages", "starter-learning-pages"]
                    .into_iter()
                    .enumerate()
                    .map(move |(index, panel_id)| {
                        let sorting = TableSorting::new(panel_id, "name", SortDirection::Asc);
                        let mut headers = vec![trs!(ui.starter_select), trs!(ui.name)];
                        if index != 0 {
                            headers.push(trs!(ui.starter_available_options));
                        }
                        headers.push(trs!(ui.actions));
                        view! {
                            <TabPanel
                                id=panel_id
                                class="starter-tab"
                                selected=Signal::derive(move || tab.get() == index)
                            >
                                <Show when=move || tab.get() == index>
                                    <ListToolbar
                                        search=search
                                        count=Signal::derive(move || visible.get().len())
                                        count_known=Signal::derive(move || {
                                            catalog.data.get().is_some()
                                        })
                                        label=trs!(ui.starter_search)
                                        noun=trs!(ui.starter_items)
                                    />
                                    <ListEmptyState
                                        ready=Signal::derive(move || catalog.data.get().is_some())
                                        count=Signal::derive(move || visible.get().len())
                                        search=search
                                        guidance=Signal::derive(move || {
                                            if index == 2 {
                                                leptos_i18n::t_string!(i18n, ui.starter_learning_empty)
                                                    .to_string()
                                            } else {
                                                leptos_i18n::t_string!(i18n, ui.starter_empty).to_string()
                                            }
                                        })
                                    />
                                    <DataTable
                                        sorting=sorting
                                        columns=if index == 0 {
                                            vec!["", "name", ""]
                                        } else {
                                            vec!["", "name", "options", ""]
                                        }
                                        class="starter-table"
                                        compact=true
                                        label=trs!(ui.starter_title)
                                        empty=Signal::derive(move || visible.with(Vec::is_empty))
                                        headers=headers.clone()
                                    >
                                        <For
                                            each=move || {
                                                sorting
                                                    .rows(
                                                        visible.get(),
                                                        |row, key| {
                                                            if key == "options" {
                                                                SortValue::Tuple(
                                                                    vec![row.prefill.into(), row.company_logo.is_some().into()],
                                                                )
                                                            } else {
                                                                row.name.clone().into()
                                                            }
                                                        },
                                                        |row| row.id.clone().into(),
                                                    )
                                            }
                                            key=|row| row.id.clone()
                                            children=move |row| {
                                                let id = StoredValue::new(row.id);
                                                let name = StoredValue::new(row.name);
                                                let prefill = row.prefill;
                                                let branding = row.company_logo.is_some();
                                                view! {
                                                    <tr>
                                                        <td>
                                                            <Checkbox
                                                                label=Signal::derive(String::new)
                                                                aria_label=Signal::derive(move || {
                                                                    format!(
                                                                        "{}: {}",
                                                                        leptos_i18n::t_string!(i18n, ui.starter_select),
                                                                        name.get_value(),
                                                                    )
                                                                })
                                                                compact=true
                                                                checked=Signal::derive(move || {
                                                                    state.draft.with(|draft| draft.contains(&id.get_value()))
                                                                })
                                                                disabled=Signal::derive(move || state.busy.get())
                                                                on_toggle=Callback::new(move |checked| {
                                                                    state
                                                                        .draft
                                                                        .update(|draft| draft.select(&id.get_value(), checked))
                                                                })
                                                            />
                                                        </td>
                                                        <th scope="row">{move || name.get_value()}</th>
                                                        <Show when=move || index != 0>
                                                            <td>
                                                                <div class="starter-capabilities">
                                                                    {prefill
                                                                        .then(|| {
                                                                            view! { <span>{tr!(ui.starter_prefill_short)}</span> }
                                                                        })}
                                                                    {branding
                                                                        .then(|| {
                                                                            view! { <span>{tr!(ui.starter_logo_upload)}</span> }
                                                                        })} {(!prefill && !branding).then_some("—")}
                                                                </div>
                                                            </td>
                                                        </Show>
                                                        <td>
                                                            <ActionButton
                                                                size=ControlSize::Small
                                                                aria_label=Signal::derive(move || {
                                                                    format!(
                                                                        "{}: {}",
                                                                        leptos_i18n::t_string!(i18n, ui.preview),
                                                                        name.get_value(),
                                                                    )
                                                                })
                                                                on_click=move |_| focus.set(Some(id.get_value()))
                                                            >
                                                                {tr!(ui.preview)}
                                                            </ActionButton>
                                                        </td>
                                                    </tr>
                                                }
                                            }
                                        />
                                    </DataTable>
                                </Show>
                            </TabPanel>
                        }
                    })
                    .collect_view()}
                <Show when=move || {
                    focus.get().is_some()
                }>{move || review_dialog(api, token, visible, focus, state, open_logo)}</Show>
                <FormFooter spread=true>
                    <p role="status">
                        {move || {
                            format!(
                                "{} {}",
                                state.draft.with(StarterSelection::count),
                                leptos_i18n::t_string!(i18n, ui.starter_selected),
                            )
                        }}
                    </p>
                    <ActionButton
                        style=ButtonStyle::Primary
                        disabled=Signal::derive(move || {
                            state.busy.get() || state.upload.with(|upload| upload.loading)
                                || state.draft.with(|draft| draft.count() == 0)
                                || (needs_logo.get()
                                    && state.upload.with(|upload| upload.data.is_empty()))
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
            </QueryBoundary>
        </ManagementPage>
    }
}

fn review_dialog(
    api: Api,
    token: StoredValue<String>,
    rows: Memo<Vec<Summary>>,
    focus: RwSignal<Option<String>>,
    state: RepositoryState,
    open_logo: Callback<()>,
) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    let dialog = NodeRef::<leptos::html::Dialog>::new();
    let current = Memo::new(move |_| {
        let id = focus.get()?;
        rows.with(|rows| rows.iter().find(|row| row.id == id).cloned())
    });
    let position = Memo::new(move |_| {
        let id = focus.get()?;
        let ids = rows.with(|rows| rows.iter().map(|row| row.id.clone()).collect::<Vec<_>>());
        ReviewPosition::in_list(&ids, &id)
    });
    let title = Signal::derive(move || current.get().map(|row| row.name).unwrap_or_default());
    let close = Callback::new(move |()| {
        // Close before unmounting so the native dialog restores the initiating button's focus.
        if let Some(dialog) = dialog.get_untracked() {
            dialog.close();
        }
        focus.set(None);
    });
    let upload = Callback::new(move |()| {
        close.run(());
        open_logo.run(());
    });
    view! {
        <Dialog node=dialog title=title class="starter-review-dialog" on_cancel=close>
            <div class="starter-review-heading">
                <h2>{move || title.get()}</h2>
                <ActionButton
                    style=ButtonStyle::Ghost
                    aria_label=trs!(ui.close)
                    title=trs!(ui.close)
                    autofocus=true
                    on_click=move |_| close.run(())
                >
                    <svg
                        viewBox="0 0 24 24"
                        width="20"
                        height="20"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                        aria-hidden="true"
                    >
                        <path d="m6 6 12 12M18 6 6 18" />
                    </svg>
                </ActionButton>
            </div>
            <div class="starter-review-content">
                // A keyed preview owns its reads and rendering lifetime; the dialog stays open.
                <For
                    each=move || { current.get().into_iter().collect::<Vec<_>>() }
                    key=|row| row.id.clone()
                    children=move |row| { preview_panel(api, token, row.id, state, upload) }
                />
            </div>
            <div class="starter-review-footer">
                <div class="starter-review-navigation">
                    <ActionButton
                        disabled=Signal::derive(move || {
                            position.with(|p| p.as_ref().is_none_or(|p| p.previous.is_none()))
                        })
                        on_click=move |_| {
                            if let Some(id) = position
                                .with_untracked(|p| p.as_ref().and_then(|p| p.previous.clone()))
                            {
                                focus.set(Some(id));
                            }
                        }
                    >
                        {tr!(ui.starter_previous)}
                    </ActionButton>
                    <span role="status" aria-live="polite">
                        {move || {
                            position
                                .get()
                                .map(|p| {
                                    leptos_i18n::t_string!(
                                        i18n, ui.starter_position, number=p.number, total=p.total
                                    )
                                })
                        }}
                    </span>
                    <ActionButton
                        disabled=Signal::derive(move || {
                            position.with(|p| p.as_ref().is_none_or(|p| p.next.is_none()))
                        })
                        on_click=move |_| {
                            if let Some(id) = position
                                .with_untracked(|p| p.as_ref().and_then(|p| p.next.clone()))
                            {
                                focus.set(Some(id));
                            }
                        }
                    >
                        {tr!(ui.starter_next)}
                    </ActionButton>
                </div>
                <Checkbox
                    label=trs!(ui.starter_selected_for_import)
                    compact=true
                    checked=Signal::derive(move || {
                        focus
                            .with(|id| {
                                id.as_ref()
                                    .is_some_and(|id| state.draft.with(|draft| draft.contains(id)))
                            })
                    })
                    disabled=Signal::derive(move || state.busy.get())
                    on_toggle=Callback::new(move |selected| {
                        if let Some(id) = focus.get_untracked() {
                            state.draft.update(|draft| draft.select(&id, selected));
                        }
                    })
                />
            </div>
        </Dialog>
    }
}

fn preview_panel(
    api: Api,
    token: StoredValue<String>,
    id: String,
    state: RepositoryState,
    open_logo: Callback<()>,
) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    let queries: ManagementQueries = expect_context();
    let id = StoredValue::new(id);
    let detail = queries.starter.observe(move || id.get_value());
    let interactive = Signal::derive(move || {
        detail
            .data
            .get()
            .is_some_and(|entry| entry.summary.kind == Kind::LandingPage)
    });
    let preview = RwSignal::new(ContentPreview::default());
    let retry = RwSignal::new(0u64);
    let options = Memo::new(move |_| state.draft.with(|draft| draft.options(&id.get_value())));
    let logo = Memo::new(move |_| state.upload.with(|upload| upload.data.clone()));
    let missing_logo = Memo::new(move |_| options.get().logo && logo.with(String::is_empty));
    Effect::new(move |_| {
        let entry = detail.data.get();
        let options = options.get();
        let logo = logo.get();
        retry.get();
        preview.update(ContentPreview::clear);
        let Some(entry) = entry else {
            return;
        };
        // Keep the chosen option, but render the standard mark until an image is supplied.
        let preview_options = Options {
            logo: options.logo && !logo.is_empty(),
            ..options
        };
        let payload = match prepare(&entry, preview_options, &logo) {
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
            let (path, body) = if entry.summary.kind == Kind::LearningPage {
                ("/api/learning-pages/preview", json!({"html":source}))
            } else {
                (
                    if entry.summary.kind == Kind::LandingPage {
                        "/api/pages/preview"
                    } else {
                        "/api/util/preview"
                    },
                    json!({"html":source,"recipient":{"email":"alice@example.test","first_name":"Alice","last_name":"Example","position":"Analyst"}}),
                )
            };
            let result = api
                .resource("POST", path, &token.get_value(), Some(&body), None)
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
        <div class="starter-preview">
            <div class="starter-preview-settings">
                <QueryBoundary
                    fetching=detail.fetching
                    has_data=Signal::derive(move || detail.data.get().is_some())
                    error=detail.error
                    noun=trs!(ui.preview)
                    refresh=detail.refresh
                >
                    {move || {
                        detail
                            .data
                            .get()
                            .map(|entry| {
                                let prefill = entry.summary.prefill;
                                let branding = entry.summary.company_logo;
                                view! {
                                    <div class="starter-options">
                                        <Show when=move || prefill>
                                            <div>
                                                <Checkbox
                                                    label=trs!(ui.starter_prefill)
                                                    checked=Signal::derive(move || options.get().prefill)
                                                    disabled=Signal::derive(move || { state.busy.get() })
                                                    on_toggle=Callback::new(move |checked| {
                                                        state
                                                            .draft
                                                            .update(|draft| {
                                                                draft
                                                                    .configure(
                                                                        &id.get_value(),
                                                                        |options| {
                                                                            options.prefill = checked;
                                                                        },
                                                                    );
                                                            })
                                                    })
                                                />
                                                <p class="field-help">{tr!(ui.starter_options_help)}</p>
                                            </div>
                                        </Show>
                                        {branding
                                            .map(|branding| {
                                                view! {
                                                    <div>
                                                        <Checkbox
                                                            label=trs!(ui.starter_use_logo)
                                                            checked=Signal::derive(move || options.get().logo)
                                                            disabled=Signal::derive(move || { state.busy.get() })
                                                            on_toggle=Callback::new(move |checked| {
                                                                state
                                                                    .draft
                                                                    .update(|draft| {
                                                                        draft
                                                                            .configure(
                                                                                &id.get_value(),
                                                                                |options| {
                                                                                    options.logo = checked;
                                                                                },
                                                                            );
                                                                    })
                                                            })
                                                        />
                                                        <p class="field-help">{branding.context}</p>
                                                    </div>
                                                }
                                            })}
                                    </div>
                                }
                            })
                    }}
                    <Show when=move || missing_logo.get()>
                        <div class="starter-logo-notice" role="status">
                            <p>
                                {move || {
                                    if state.upload.with(|upload| upload.loading) {
                                        leptos_i18n::t_string!(i18n, ui.starter_logo_loading)
                                    } else {
                                        leptos_i18n::t_string!(i18n, ui.starter_logo_required)
                                    }
                                }}
                            </p>
                            <ActionButton
                                size=ControlSize::Small
                                disabled=Signal::derive(move || state.busy.get())
                                on_click=move |_| open_logo.run(())
                            >
                                {tr!(ui.starter_open_logo)}
                            </ActionButton>
                        </div>
                    </Show>
                </QueryBoundary>
            </div>
            <div class="starter-preview-canvas">
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
                                    sandbox=move || {
                                        if interactive.get() {
                                            ruphish_domain::landing::SANDBOX
                                        } else {
                                            ""
                                        }
                                    }
                                    referrerpolicy="no-referrer"
                                    srcdoc=if interactive.get() {
                                        ruphish_ui::landing_preview_document(&html)
                                    } else {
                                        preview_document(&html)
                                    }
                                ></iframe>
                            }
                        })
                }}
            </div>
        </div>
    }
}

fn logo_panel(
    state: RepositoryState,
    expanded: RwSignal<bool>,
    picker: NodeRef<leptos::html::Button>,
) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    let input = NodeRef::<leptos::html::Input>::new();
    let has_logo = Memo::new(move |_| state.upload.with(|upload| !upload.data.is_empty()));
    view! {
        <div class="starter-logo-section">
            <ActionButton
                expanded=Signal::derive(move || expanded.get().to_string())
                controls="starter-logo-upload"
                on_click=move |_| expanded.update(|open| *open = !*open)
            >
                {tr!(ui.starter_use_logo)}
                <svg
                    viewBox="0 0 24 24"
                    width="16"
                    height="16"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    aria-hidden="true"
                >
                    <path d=move || if expanded.get() { "m6 15 6-6 6 6" } else { "m6 9 6 6 6-6" } />
                </svg>
            </ActionButton>
            <p id="starter-logo-help" class="field-help">
                {tr!(ui.starter_logo_help)}
            </p>
            <div
                id="starter-logo-upload"
                class="starter-logo-upload"
                hidden=move || !expanded.get()
                role="group"
                aria-label=trs!(ui.starter_logo_upload)
                aria-describedby="starter-logo-help starter-logo-limits"
            >
                <div
                    class="starter-logo-art"
                    class:has-image=move || has_logo.get()
                    aria-busy=move || state.upload.with(|upload| upload.loading).to_string()
                >
                    <Show
                        when=move || has_logo.get()
                        fallback=move || {
                            view! {
                                <svg
                                    viewBox="0 0 48 48"
                                    width="40"
                                    height="40"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="2"
                                    aria-hidden="true"
                                >
                                    <rect x="6" y="8" width="36" height="32" rx="3" />
                                    <circle cx="17" cy="19" r="4" />
                                    <path d="m7 34 10-9 8 7 7-13 10 15" />
                                </svg>
                            }
                        }
                    >
                        <img
                            alt=trs!(ui.starter_logo_upload)
                            src=move || state.upload.with(|upload| upload.data.clone())
                        />
                    </Show>
                </div>
                <div class="starter-logo-details">
                    <strong class="starter-logo-name">
                        {move || {
                            if has_logo.get() {
                                state.upload.with(|upload| upload.name.clone())
                            } else {
                                leptos_i18n::t_string!(i18n, ui.starter_logo_empty).to_string()
                            }
                        }}
                    </strong>
                    <p id="starter-logo-limits" class="field-help">
                        {tr!(ui.starter_logo_limits)}
                    </p>
                    <div class="management-actions">
                        <ActionButton
                            node=picker
                            disabled=Signal::derive(move || state.busy.get())
                            on_click=move |_| {
                                if let Some(input) = input.get_untracked() {
                                    input.set_value("");
                                    input.click();
                                }
                            }
                        >
                            {move || {
                                if has_logo.get() {
                                    leptos_i18n::t_string!(i18n, ui.starter_logo_replace)
                                } else {
                                    leptos_i18n::t_string!(i18n, ui.starter_logo_choose)
                                }
                            }}
                        </ActionButton>
                        <Show when=move || {
                            has_logo.get() || state.upload.with(|upload| upload.loading)
                        }>
                            <ActionButton
                                style=ButtonStyle::Ghost
                                disabled=Signal::derive(move || state.busy.get())
                                on_click=move |_| {
                                    state.upload.update(LogoUpload::clear);
                                    if let Some(input) = input.get_untracked() {
                                        input.set_value("");
                                    }
                                    if let Some(picker) = picker.get_untracked() {
                                        let _ = picker.focus();
                                    }
                                }
                            >
                                {tr!(ui.starter_logo_remove)}
                            </ActionButton>
                        </Show>
                    </div>
                    <Show when=move || state.upload.with(|upload| upload.loading)>
                        <p class="field-help" role="status">
                            {tr!(ui.starter_logo_loading)}
                        </p>
                    </Show>
                    <Show when=move || state.upload.with(|upload| upload.failed)>
                        <p role="alert" class="form-message error has-content">
                            {tr!(ui.starter_logo_invalid)}
                        </p>
                    </Show>
                </div>
                <input
                    node_ref=input
                    type="file"
                    hidden
                    aria-label=trs!(ui.starter_logo_upload)
                    accept="image/png,image/jpeg,image/webp"
                    disabled=move || state.busy.get()
                    on:change=move |event| {
                        let input = event_target::<web_sys::HtmlInputElement>(&event);
                        let Some(file) = input.files().and_then(|files| files.get(0)) else {
                            return;
                        };
                        let name = file.name();
                        let Some(generation) = state.upload.try_update(LogoUpload::begin) else {
                            return;
                        };
                        spawn_local(async move {
                            let result = read_logo(&input)
                                .await
                                .ok()
                                .and_then(|value| value.as_string());
                            state.upload.update(|upload| upload.finish(generation, name, result));
                        });
                    }
                />
            </div>
        </div>
    }
}
