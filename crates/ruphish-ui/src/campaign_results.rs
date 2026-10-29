//! Campaign analysis: shared filters, activity chart and recipient drilldown.
use crate::{
    campaign_queries::CampaignQueries,
    client::Api,
    feedback::Message,
    results_chart::{ActivityChart, labels},
    shared::components::*,
    time_display::{local_input, parse_input},
};
use leptos::{prelude::*, task::spawn_local_scoped_with_cancellation as spawn_local};
use leptos_router::hooks::{use_navigate, use_params_map, use_query_map};
use ruphish_contracts::reporting::Filter;
use ruphish_domain::reporting::SortDirection;
use ruphish_ui::results_state::address;
use ruphish_ui::{tr, trs};
use serde_json::Value;

pub(super) fn page(api: Api) -> impl IntoView {
    let params = use_params_map();
    view! {
        <For
            each=move || vec![params.with(|p| p.get("id")).unwrap_or_default()]
            key=|id| id.clone()
            children=move |id| {
                match id.parse::<i64>().ok().filter(|id| *id > 0) {
                    Some(id) => detail(api, id).into_any(),
                    None => {
                        view! {
                            <NotFoundPage
                                back_href="/campaigns"
                                back_label=trs!(ui.back_to_campaigns)
                            />
                        }
                            .into_any()
                    }
                }
            }
        />
    }
}

pub(super) fn filters() -> Memo<Filter> {
    let query = use_query_map();
    Memo::new(move |_| {
        let get = |key: &str| query.with(|q| q.get(key));
        Filter {
            recipient: None,
            recipients: get("recipients").unwrap_or_default(),
            from: get("from").and_then(|s| s.parse().ok()),
            until: get("until").and_then(|s| s.parse().ok()),
            event: get("event").unwrap_or_default(),
            search: get("search").unwrap_or_default(),
            source: get("source")
                .filter(|s| s != "application")
                .unwrap_or_default(),
            event_sort: get("event_sort")
                .and_then(|v| serde_json::from_value(Value::String(v)).ok())
                .unwrap_or_default(),
            event_order: get("event_order")
                .and_then(|v| serde_json::from_value(Value::String(v)).ok())
                .unwrap_or(SortDirection::Desc),
            sort: get("sort")
                .and_then(|v| serde_json::from_value(Value::String(v)).ok())
                .unwrap_or_default(),
            order: get("order")
                .and_then(|v| serde_json::from_value(Value::String(v)).ok())
                .unwrap_or_default(),
            limit: get("limit").and_then(|s| s.parse().ok()).unwrap_or(50),
            offset: get("offset").and_then(|s| s.parse().ok()).unwrap_or(0),
            ..Default::default()
        }
    })
}

fn detail(api: Api, id: i64) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    let navigate = StoredValue::new(use_navigate());
    let path = StoredValue::new(format!("/campaigns/{id}/results"));
    let location = leptos_router::hooks::use_location();
    let filters = filters();
    let selected = RwSignal::new(Vec::<i64>::new());
    let selected_event = RwSignal::new(String::new());
    let source = RwSignal::new(String::new());
    let from = RwSignal::new(String::new());
    let until = RwSignal::new(String::new());
    let queries = expect_context::<CampaignQueries>();
    let selection = Memo::new(move |_| CampaignQueries::results_key(id, &filters.get()));
    let read = queries.results.observe_polling(
        move || selection.get(),
        ruphish_ui::query::document_visible(),
    );
    crate::campaign_queries::RefreshState::register_read(
        read.fetching,
        Signal::derive(move || read.data.with(Option::is_some)),
    );
    let presentation = read.presentation(move || selection.get());
    let data = presentation.data;
    let positions = Memo::new(move |_| {
        data.with(|page| {
            page.as_ref()
                .map(|page| {
                    ruphish_ui::table_state::row_positions(page.recipients.iter().map(|row| row.id))
                })
                .unwrap_or_default()
        })
    });
    let loading = read.fetching;
    let loaded = Signal::derive(move || data.with(Option::is_some));
    let applying = presentation.pending;
    let message = Message::new(String::new());
    let go = move |filter: Filter| {
        if filter == filters.get_untracked() {
            return;
        }
        navigate.with_value(|navigate| {
            navigate(
                &address(&path.get_value(), &filter, false),
                Default::default(),
            )
        })
    };
    let sorting = TableSorting {
        current: Signal::derive(move || ruphish_ui::table_state::Sort {
            column: serde_json::to_value(filters.get().sort)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
            order: filters.get().order,
        }),
        change: Callback::new(move |column: String| {
            let mut filter = filters.get_untracked();
            let current = ruphish_ui::table_state::Sort {
                column: serde_json::to_value(filter.sort)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default(),
                order: filter.order,
            }
            .select(&column);
            if let Ok(sort) = serde_json::from_value(Value::String(column)) {
                filter.sort = sort;
                filter.order = current.order;
                filter.offset = 0;
                go(filter);
            }
        }),
    };
    Effect::new(move |_| {
        let filter = filters.get();
        selected.set(
            filter
                .recipients
                .split(',')
                .filter_map(|s| s.parse().ok())
                .collect(),
        );
        selected_event.set(filter.event.clone());
        source.set(filter.source.clone());
        from.set(local_input(filter.from));
        until.set(local_input(filter.until));
    });
    let option_search = RwSignal::new(String::new());
    let option_state = RwSignal::new(ruphish_ui::select_options::Options::default());
    let fetch_options = move |request: ruphish_ui::select_options::Request| {
        spawn_local(async move {
            let filter = Filter {
                search: request.search.clone(),
                offset: request.offset,
                ..Default::default()
            };
            let result = queries
                .read_results(api, id, &filter)
                .await
                .map_err(|e| e.message)
                .map(|value| {
                    let rows = value
                        .recipients
                        .iter()
                        .map(|r| {
                            (
                                r.id,
                                ruphish_ui::select_options::recipient_label(&r.name, &r.email),
                            )
                        })
                        .collect();
                    (rows, value.recipient_total as usize)
                });
            option_state.try_update(|state| state.finish(request, result));
        });
    };
    {
        Effect::new(move |_| {
            let search = option_search.get();
            let request = option_state.try_update(|state| state.search(search));
            if let Some(request) = request {
                fetch_options(request);
            }
        });
    }
    let more_options = move |retry: bool| {
        if let Some(Some(request)) = option_state.try_update(|state| state.more(retry)) {
            fetch_options(request);
        }
    };
    let apply = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        match (
            parse_input(&from.get_untracked(), filters.get_untracked().from),
            parse_input(&until.get_untracked(), filters.get_untracked().until),
        ) {
            (Ok(from), Ok(until)) if !matches!((from,until),(Some(a),Some(b)) if a>=b) => {
                let filter = Filter {
                    recipient: None,
                    recipients: selected
                        .get_untracked()
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                    from,
                    until,
                    event: selected_event.get_untracked(),
                    source: source.get_untracked(),
                    event_sort: filters.get_untracked().event_sort,
                    event_order: filters.get_untracked().event_order,
                    sort: filters.get_untracked().sort,
                    order: filters.get_untracked().order,
                    ..Default::default()
                };
                go(filter);
            }
            _ => message.error({
                leptos_i18n::t_string!(
                    i18n,
                    ui.enter_a_valid_time_range_with_the_end_after_the_start
                )
                .to_owned()
            }),
        }
    };
    let chart = Memo::new(move |_| {
        data.with(|value| {
            value
                .as_ref()
                .map(|v| {
                    ruphish_ui::results_state::participant_intervals(
                        &v.participant_buckets,
                        v.bucket_seconds,
                    )
                })
                .unwrap_or_default()
        })
    });
    let total =
        Signal::derive(move || data.with(|v| v.as_ref().map_or(0, |v| v.recipient_total)) as usize);
    let content = view! {
        <ManagementPage>
            {message.view()}
            <QueryBoundary
                fetching=loading
                has_data=loaded
                error=read.error
                noun=trs!(ui.results)
                empty=Signal::derive(move || {
                    data.with(|v| {
                        v.as_ref().is_some_and(|v| v.recipient_total == 0 && v.event_total == 0)
                    })
                })
                refresh=read.refresh
            >
                <QueryControls slot:controls>
                    <form class="analysis-filters" on:submit=apply>
                        <div class="analysis-filter-fields">
                            <MultiSelect
                                label=trs!(ui.recipients)
                                selected=selected
                                search=option_search
                                options=Signal::derive(move || option_state.get().rows)
                                loading=Signal::derive(move || option_state.get().loading)
                                error=Signal::derive(move || option_state.get().error)
                                has_more=Signal::derive(move || {
                                    option_state.with(ruphish_ui::select_options::Options::has_more)
                                })
                                on_more=Callback::new(move |()| more_options(false))
                                on_retry=Callback::new(move |()| more_options(true))
                            />
                            <Select
                                label=trs!(ui.source)
                                value=source
                                on_change=Callback::new(move |v| source.set(v))
                            >
                                <option value="">{tr!(ui.all_sources)}</option>
                                <option value="public">{tr!(ui.public_tracking)}</option>
                                <option value="mailbox">{tr!(ui.reporting_mailbox)}</option>
                                <option value="operator">{tr!(ui.event_operator)}</option>
                                <option value="legacy">{tr!(ui.legacy_history)}</option>
                            </Select>
                            <Field label=trs!(ui.from)>
                                <TextInput
                                    input_type="datetime-local"
                                    step="60"
                                    value=Signal::derive(move || from.get())
                                    on_input=move |e| from.set(event_target_value(&e))
                                />
                            </Field>
                            <Field label=trs!(ui.until)>
                                <TextInput
                                    input_type="datetime-local"
                                    step="60"
                                    value=Signal::derive(move || until.get())
                                    on_input=move |e| until.set(event_target_value(&e))
                                />
                            </Field>
                            <fieldset class="event-type-filter">
                                <legend>{tr!(ui.recipients_with_any_event)}</legend>
                                {[
                                    ruphish_domain::events::Kind::EmailOpened,
                                    ruphish_domain::events::Kind::LinkClicked,
                                    ruphish_domain::events::Kind::FormSubmitted,
                                    ruphish_domain::events::Kind::EmailReported,
                                ]
                                    .into_iter()
                                    .map(|kind| {
                                        view! {
                                            <label>
                                                <input
                                                    type="checkbox"
                                                    prop:checked=move || {
                                                        selected_event.get().split(',').any(|key| key == kind.key())
                                                    }
                                                    on:change=move |event| {
                                                        let mut values = selected_event
                                                            .get_untracked()
                                                            .split(',')
                                                            .filter(|key| !key.is_empty() && *key != kind.key())
                                                            .map(str::to_owned)
                                                            .collect::<Vec<_>>();
                                                        if event_target_checked(&event) {
                                                            values.push(kind.key().into());
                                                        }
                                                        values.sort();
                                                        selected_event.set(values.join(","));
                                                    }
                                                />
                                                {move || ruphish_ui::localization::event_label(
                                                    i18n,
                                                    kind.label(),
                                                )}
                                            </label>
                                        }
                                    })
                                    .collect_view()}
                            </fieldset>
                        </div>
                        <Show when=move || source.get() == "legacy">
                            <p class="analysis-context">{tr!(ui.legacy_history_help)}</p>
                        </Show>
                        <div class="analysis-filter-footer">
                            <span>
                                {move || {
                                    leptos_i18n::t_string!(
                                        i18n, messages.arg0_from_is_inclusive_until_is_exclusive, arg0 = (crate::time_display::zone_name()).to_string()
                                    )
                                }}
                            </span>
                            <div class="management-actions">
                                <ActionButton on_click=move |_| go(Filter {
                                    recipient: None,
                                    ..Default::default()
                                })>{tr!(ui.clear_filters)}</ActionButton>
                                <ActionButton
                                    kind="submit"
                                    style=ButtonStyle::Primary
                                    disabled=applying
                                    busy=Signal::derive(move || applying.get().to_string())
                                    aria_label=trs!(ui.apply_filters)
                                >
                                    <LoadingLabel loading=applying>
                                        {tr!(ui.apply_filters)}
                                    </LoadingLabel>
                                </ActionButton>
                            </div>
                        </div>
                        <Show when=move || !filters.get().search.is_empty()>
                            <p>
                                {move || {
                                    leptos_i18n::t_string!(
                                        i18n, messages.saved_recipient_search_arg0, arg0 = (filters.get().search).to_string()
                                    )
                                }}
                            </p>
                        </Show>
                    </form>
                </QueryControls>
                <Show when=move || { loaded.get() }>
                    <p class="analysis-context">{tr!(ui.unique_observed)}</p>
                    <div class="analysis-summary">
                        {labels()
                            .into_iter()
                            .enumerate()
                            .skip(1)
                            .map(move |(i, label)| {
                                view! {
                                    <Card class="analysis-metric">
                                        <span>{move || label.get()}</span>
                                        <strong>
                                            {move || {
                                                data.with(|v| {
                                                    v.as_ref().map_or(0, |v| { v.unique.values()[i] })
                                                })
                                            }}
                                        </strong>
                                        <small>{tr!(ui.participants)}</small>
                                    </Card>
                                }
                            })
                            .collect_view()}
                    </div>
                    <ActivityChart
                        data=chart
                        interval=Signal::derive(move || {
                            data.with(|v| v.as_ref().map_or(60, |v| v.bucket_seconds)).max(60)
                        })
                    />
                    <details class="analysis-explanation">
                        <summary>{tr!(ui.what_these_observations_mean)}</summary>
                        <p>
                            {tr!(
                                ui.the_chart_recipient_table_and_exports_show_recorded_events_only_opens_are_image
                            )}
                        </p>
                    </details>
                    <h2>{tr!(ui.participants)}</h2>
                    <DataTable
                        label=trs!(ui.recipient_observations)
                        sorting=sorting
                        columns=vec!["email", "opened", "clicked", "submitted", "reported", ""]
                        headers=vec![
                            trs!(ui.recipient),
                            trs!(ui.opened),
                            trs!(ui.clicked),
                            trs!(ui.submitted),
                            trs!(ui.reported),
                            trs!(ui.details),
                        ]
                    >
                        <For
                            each=move || {
                                data.with(|v| {
                                    v.as_ref()
                                        .map(|v| {
                                            v.recipients.iter().map(|r| r.id).collect::<Vec<_>>()
                                        })
                                        .unwrap_or_default()
                                })
                            }
                            key=|id| *id
                            children=move |recipient_id| {
                                let row = Signal::derive(move || {
                                    data.get()
                                        .zip(positions.with(|p| p.get(&recipient_id).copied()))
                                });
                                view! {
                                    <tr class="recipient-outcome">
                                        <th scope="row">
                                            <span>
                                                {move || {
                                                    row.get()
                                                        .and_then(|(page, index)| {
                                                            page.recipients.get(index).map(|r| r.email.clone())
                                                        })
                                                }}
                                            </span>
                                            <small>
                                                {move || {
                                                    row.get()
                                                        .and_then(|(page, index)| {
                                                            page.recipients.get(index).map(|r| r.name.clone())
                                                        })
                                                }}
                                            </small>
                                        </th>
                                        {(1..5)
                                            .map(move |index| {
                                                view! {
                                                    <td>
                                                        <ActivityIndicator recorded=Signal::derive(move || {
                                                            row.get()
                                                                .is_some_and(|(page, position)| {
                                                                    page.recipients
                                                                        .get(position)
                                                                        .is_some_and(|r| r.counts.values()[index] > 0)
                                                                })
                                                        }) />
                                                        <Show when=move || {
                                                            index == 4
                                                                && row
                                                                    .get()
                                                                    .is_some_and(|(page, position)| {
                                                                        page.recipients
                                                                            .get(position)
                                                                            .is_some_and(|r| r.historical_reported)
                                                                    })
                                                        }>
                                                            <small>{tr!(ui.legacy_report_time_unknown)}</small>
                                                        </Show>
                                                    </td>
                                                }
                                            })
                                            .collect_view()}
                                        <td>
                                            {move || {
                                                let search = location.search.get();
                                                let origin = if search.is_empty() {
                                                    location.pathname.get()
                                                } else {
                                                    format!(
                                                        "{}?{}",
                                                        location.pathname.get(),
                                                        search.trim_start_matches('?'),
                                                    )
                                                };
                                                view! {
                                                    <ActionLink href=ruphish_ui::results_state::recipient_address(
                                                        id,
                                                        recipient_id,
                                                        &origin,
                                                    )>{tr!(ui.recipient_analysis)}</ActionLink>
                                                }
                                            }}
                                        </td>
                                    </tr>
                                }
                            }
                        />

                    </DataTable>
                    <Show when=move || total.get() == 0>
                        <EmptyState title=trs!(ui.no_matching_results)>
                            <p>{tr!(ui.change_or_clear_the_filters_to_see_more_activity)}</p>
                        </EmptyState>
                    </Show>
                </Show>
                <Show when=move || {
                    loaded.get() && (filters.get().offset > 0 || filters.get().limit < total.get())
                }>
                    <div class="analysis-pagination">
                        <span>
                            {move || {
                                if !loaded.get() {
                                    "—".to_owned()
                                } else {
                                    leptos_i18n::t_string!(
                                        i18n, messages.count_matching_recipients, count = total.get()
                                    )
                                }
                            }}
                        </span>
                        <PaginationControls
                            disabled=Signal::derive(move || {
                                loading.get() || read.data.with(Option::is_none)
                            })
                            has_previous=Signal::derive(move || filters.get().offset > 0)
                            has_next=Signal::derive(move || {
                                filters.get().offset.saturating_add(filters.get().limit)
                                    < total.get()
                            })
                            on_previous=Callback::new(move |()| {
                                let mut filter = filters.get_untracked();
                                filter.offset = filter.offset.saturating_sub(filter.limit);
                                go(filter);
                            })
                            on_next=Callback::new(move |()| {
                                let mut filter = filters.get_untracked();
                                filter.offset = filter.offset.saturating_add(filter.limit);
                                go(filter);
                            })
                        />
                    </div>
                </Show>
            </QueryBoundary>
        </ManagementPage>
    };
    content.into_any()
}

/// Export controls are rendered by each page's own top-level header.
#[component]
pub(super) fn ExportMenu(
    api: Api,
    campaign: StoredValue<String>,
    filter: Signal<Filter>,
    ready: Signal<bool>,
    message: Message,
) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    let downloading = RwSignal::new(false);
    let tasks = ruphish_ui::view_tasks::ViewTasks::default();
    let export = move |format: &'static str, scope: &'static str| {
        if downloading.get_untracked() {
            return;
        }
        let id = campaign.get_value();
        let mut filter = filter.get_untracked();
        filter.format = format.into();
        filter.scope = scope.into();
        let url = address(&format!("/api/campaigns/{id}/export"), &filter, true);
        downloading.set(true);
        tasks.spawn(async move {
            match api
                .download(&url, &format!("campaign-{id}-{scope}.{format}"))
                .await
            {
                Ok(()) => message
                    .local_status(leptos_i18n::t_string!(i18n, ui.export_downloaded).to_owned()),
                Err(error) => message.error(error),
            }
            downloading.set(false);
        });
    };
    view! {
        <ActionMenu
            label=trs!(ui.export)
            style=ButtonStyle::Primary
            disabled=Signal::derive(move || downloading.get() || !ready.get())
        >
            <ActionButton
                disabled=Signal::derive(move || downloading.get() || !ready.get())
                on_click=move |_| export("csv", "recipients")
            >
                {tr!(ui.recipients_csv)}
            </ActionButton>
            <ActionButton
                disabled=Signal::derive(move || downloading.get() || !ready.get())
                on_click=move |_| export("csv", "events")
            >
                {tr!(ui.events_csv)}
            </ActionButton>
            <ActionButton
                disabled=Signal::derive(move || downloading.get() || !ready.get())
                on_click=move |_| export("xlsx", "all")
            >
                {tr!(ui.excel_workbook)}
            </ActionButton>
        </ActionMenu>
    }
}
