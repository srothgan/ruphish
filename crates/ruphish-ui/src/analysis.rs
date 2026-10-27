//! Campaign analysis: shared filters, activity chart and recipient drilldown.
use crate::{
    analysis_chart::{ActivityChart, Bucket, labels},
    campaign_queries::CampaignQueries,
    client::Api,
    feedback::Message,
    shared::components::*,
    time_display::{instant as minute, local_input, parse_input},
};
use leptos::{prelude::*, task::spawn_local_scoped_with_cancellation as spawn_local};
use leptos_router::hooks::{use_navigate, use_params_map, use_query_map};
use ruphish_contracts::reporting::Filter;
use ruphish_domain::reporting::SortDirection;
use ruphish_ui::analysis_state::address;
use ruphish_ui::{tr, trs};
use serde_json::Value;

const EVENTS: [&str; 8] = [
    "Campaign Created",
    "Email Sent",
    "Error Sending Email",
    "Email Opened",
    "Clicked Link",
    "Submitted Data",
    "Email Reported",
    "Delivery reconciled",
];

pub(super) fn page(api: Api) -> impl IntoView {
    let params = use_params_map();
    view! {
        <For
            each=move || {
                vec![
                    (
                        params.with(|p| p.get("id")).unwrap_or_default(),
                        params.with(|p| p.get("recipient")),
                    ),
                ]
            }
            key=|key| key.clone()
            children=move |(id, recipient)| {
                let id = id.parse::<i64>().ok().filter(|id| *id > 0);
                let recipient = recipient.map(|r| r.parse::<i64>().ok().filter(|id| *id > 0));
                match (id, recipient) {
                    (Some(id), None) => detail(api, id, None).into_any(),
                    (Some(id), Some(Some(recipient))) => detail(api, id, Some(recipient)).into_any(),
                    _ => {
                        view! {
                            <NotFoundPage
                                back_href="/analysis"
                                back_label=trs!(ui.back_to_analysis)
                            />
                        }
                            .into_any()
                    }
                }
            }
        />
    }
}

fn detail(api: Api, id: i64, recipient: Option<i64>) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let query = use_query_map();
    let navigate = StoredValue::new(use_navigate());
    let path = StoredValue::new(recipient.map_or_else(
        || format!("/analysis/{id}"),
        |r| format!("/analysis/{id}/recipients/{r}"),
    ));
    let filters = Memo::new(move |_| {
        let get = |key: &str| query.with(|q| q.get(key));
        Filter {
            recipient,
            recipients: get("recipients").unwrap_or_default(),
            from: get("from").and_then(|s| s.parse().ok()),
            until: get("until").and_then(|s| s.parse().ok()),
            event: get("event").unwrap_or_default(),
            search: get("search").unwrap_or_default(),
            source: get("source").unwrap_or_default(),
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
    });
    let selected = RwSignal::new(Vec::<i64>::new());
    let selected_event = RwSignal::new(String::new());
    let source = RwSignal::new(String::new());
    let from = RwSignal::new(String::new());
    let until = RwSignal::new(String::new());
    let queries = expect_context::<CampaignQueries>();
    let read = queries.analysis.observe_polling(
        move || CampaignQueries::analysis_key(id, &filters.get()),
        ruphish_ui::query::document_visible(),
    );
    let data = read.data;
    let loading = read.fetching;
    let loaded = Signal::derive(move || read.data.with(Option::is_some));
    let message = Message::new(String::new());
    let go = move |filter: Filter| {
        navigate.with_value(|navigate| {
            navigate(
                &address(&path.get_value(), &filter, false),
                Default::default(),
            )
        })
    };
    let event_sorting = TableSorting {
        current: Signal::derive(move || ruphish_ui::table_state::Sort {
            column: serde_json::to_value(filters.get().event_sort)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
            order: filters.get().event_order,
        }),
        change: Callback::new(move |column: String| {
            let mut filter = filters.get_untracked();
            let current = ruphish_ui::table_state::Sort {
                column: serde_json::to_value(filter.event_sort)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default(),
                order: filter.event_order,
            }
            .select(&column);
            if let Ok(sort) = serde_json::from_value(Value::String(column)) {
                filter.event_sort = sort;
                filter.event_order = current.order;
                filter.offset = 0;
                go(filter);
            }
        }),
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
                .read_analysis(api, id, &filter)
                .await
                .map_err(|e| e.message)
                .map(|value| {
                    let rows = value
                        .recipients
                        .into_iter()
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
    if recipient.is_none() {
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
                    recipient,
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
                if filter == filters.get_untracked() {
                    read.refresh.run(());
                } else {
                    go(filter);
                }
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
    let downloading = RwSignal::new(false);
    let tasks = ruphish_ui::view_tasks::ViewTasks::default();
    let export = move |format: &'static str, scope: &'static str| {
        if downloading.get_untracked() {
            return;
        }
        let mut filter = filters.get_untracked();
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
    let chart = Signal::derive(move || {
        data.with(|value| {
            value
                .as_ref()
                .map(|v| {
                    v.buckets
                        .iter()
                        .map(|b| Bucket {
                            time: b.time,
                            counts: b.counts,
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
    });
    let total = Signal::derive(move || {
        data.with(|value| {
            value.as_ref().map_or(0, |v| {
                if recipient.is_some() {
                    v.event_total
                } else {
                    v.recipient_total
                }
            })
        }) as usize
    });
    let content = view! {
        <ManagementPage>
            <BackLink href=Signal::derive(move || {
                if recipient.is_some() {
                    let mut f = filters.get();
                    f.recipient = None;
                    f.offset = 0;
                    address(&format!("/analysis/{id}"), &f, false)
                } else {
                    "/analysis".into()
                }
            })>
                {move || {
                    if recipient.is_some() {
                        leptos_i18n::t_string!(i18n, ui.back_to_campaign_analysis)
                    } else {
                        leptos_i18n::t_string!(i18n, ui.back_to_analysis)
                    }
                }}
            </BackLink>
            <PageHeader
                title=Signal::derive(move || {
                    let name = data
                        .with(|v| v.as_ref().map(|v| v.name.clone()).unwrap_or_default());
                    if name.is_empty() {
                        leptos_i18n::t_string!(i18n, ui.campaign).into()
                    } else {
                        name
                    }
                })
                subtitle=Signal::derive(move || {
                    if loaded.get() {
                        leptos_i18n::t_string!(
                            i18n, messages.updated_arg0, arg0 = (minute(&serde_json::json!(data.with(|v| v.as_ref().map(|v| v.as_of))))).to_string()
                        )
                    } else if loading.get() {
                        leptos_i18n::t_string!(i18n, ui.loading_analysis).into()
                    } else {
                        leptos_i18n::t_string!(
                            i18n, ui.refresh_failed_retry_to_load_current_results
                        )
                            .into()
                    }
                })
            >
                <ActionButton
                    busy=Signal::derive(move || loading.get().to_string())
                    on_click=move |_| read.refresh.run(())
                >
                    {move || {
                        if loading.get() {
                            leptos_i18n::t_string!(i18n, ui.refreshing)
                        } else {
                            leptos_i18n::t_string!(i18n, ui.refresh)
                        }
                    }}
                </ActionButton>
                <ActionMenu
                    label=trs!(ui.export)
                    style=ButtonStyle::Primary
                    disabled=Signal::derive(move || downloading.get() || !loaded.get())
                >
                    <ActionButton
                        disabled=Signal::derive(move || downloading.get() || !loaded.get())
                        on_click=move |_| export("csv", "recipients")
                    >
                        {tr!(ui.recipients_csv)}
                    </ActionButton>
                    <ActionButton
                        disabled=Signal::derive(move || downloading.get() || !loaded.get())
                        on_click=move |_| export("csv", "events")
                    >
                        {tr!(ui.events_csv)}
                    </ActionButton>
                    <ActionButton
                        disabled=Signal::derive(move || downloading.get() || !loaded.get())
                        on_click=move |_| export("xlsx", "all")
                    >
                        {tr!(ui.excel_workbook)}
                    </ActionButton>
                </ActionMenu>
                <ActionLink href=format!("/campaigns/{id}")>{tr!(ui.back_to_campaign)}</ActionLink>
            </PageHeader>
            {message.view()}
            <QueryStatus
                fetching=loading
                has_data=loaded
                error=read.error
                noun=trs!(ui.campaign_analysis)
                refresh=read.refresh
            />
            <form class="analysis-filters" on:submit=apply>
                <div class="analysis-filter-fields">
                    {recipient
                        .is_none()
                        .then(|| {
                            view! {
                                <MultiSelect
                                    label=trs!(ui.recipients)
                                    selected=selected
                                    search=option_search
                                    options=Signal::derive(move || option_state.get().rows)
                                    loading=Signal::derive(move || option_state.get().loading)
                                    error=Signal::derive(move || option_state.get().error)
                                    has_more=Signal::derive(move || {
                                        option_state
                                            .with(ruphish_ui::select_options::Options::has_more)
                                    })
                                    on_more=Callback::new(move |()| more_options(false))
                                    on_retry=Callback::new(move |()| more_options(true))
                                />
                            }
                        })}
                    <Select
                        label=trs!(ui.event)
                        value=selected_event
                        on_change=Callback::new(move |v| selected_event.set(v))
                    >
                        <option value="">{tr!(ui.all_events)}</option>
                        {EVENTS
                            .into_iter()
                            .map(|e| {
                                view! {
                                    <option value=e>
                                        {move || ruphish_ui::localization::event_label(i18n, e)}
                                    </option>
                                }
                            })
                            .collect_view()}
                    </Select>
                    <Select
                        label=trs!(ui.source)
                        value=source
                        on_change=Callback::new(move |v| source.set(v))
                    >
                        <option value="">{tr!(ui.all_sources)}</option>
                        <option value="public">{tr!(ui.public_tracking)}</option>
                        <option value="mailbox">{tr!(ui.reporting_mailbox)}</option>
                        <option value="application">{tr!(ui.application)}</option>
                        <option value="legacy">{tr!(ui.legacy_history)}</option>
                    </Select> <Field label=trs!(ui.from)>
                        <TextInput
                            input_type="datetime-local"
                            step="60"
                            value=Signal::derive(move || from.get())
                            on_input=move |e| from.set(event_target_value(&e))
                        />
                    </Field> <Field label=trs!(ui.until)>
                        <TextInput
                            input_type="datetime-local"
                            step="60"
                            value=Signal::derive(move || until.get())
                            on_input=move |e| until.set(event_target_value(&e))
                        />
                    </Field>
                </div>
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
                            recipient,
                            ..Default::default()
                        })>{tr!(ui.clear_filters)}</ActionButton>
                        <ActionButton kind="submit" style=ButtonStyle::Primary>
                            {tr!(ui.apply_filters)}
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
            <Show when=move || {
                loaded.get()
            }>
                {recipient
                    .map(|_| {
                        view! {
                            <h2 class="analysis-recipient-title">
                                {move || {
                                    data.with(|v| {
                                            v.as_ref()
                                                .and_then(|v| v.recipients.first())
                                                .map(|r| r.email.clone())
                                        })
                                        .unwrap_or_else(|| {
                                            leptos_i18n::t_string!(i18n, ui.recipient_details).into()
                                        })
                                }}
                            </h2>
                        }
                    })}
                <div class="analysis-summary">
                    {labels()
                        .into_iter()
                        .enumerate()
                        .map(|(i, label)| {
                            view! {
                                <Card class="analysis-metric">
                                    <span>{move || label.get()}</span>
                                    <strong>
                                        {move || {
                                            data.with(|v| {
                                                v.as_ref()
                                                    .map_or(
                                                        0,
                                                        |v| {
                                                            if recipient.is_some() {
                                                                v.totals[i]
                                                            } else {
                                                                v.progression[i]
                                                            }
                                                        },
                                                    )
                                            })
                                        }}
                                    </strong>
                                    <Show when=move || recipient.is_none()>
                                        <small>
                                            {move || {
                                                leptos_i18n::t_string!(
                                                    i18n, messages.arg0_recorded_events, arg0 = (data.with(|v| v.as_ref().map_or(0, |v| v.totals[i]))).to_string()
                                                )
                                            }}
                                        </small>
                                    </Show>
                                </Card>
                            }
                        })
                        .collect_view()}
                </div> <Show when=move || recipient.is_some()>
                    <p class="analysis-context">{tr!(ui.recipient_observation_counts)}</p>
                </Show> <Show when=move || recipient.is_none()>
                    <p class="analysis-context">
                        {move || {
                            leptos_i18n::t_string!(
                                i18n, messages.recipient_progression_within_the_selected_filters_opens_include_clicks_and_submi, arg0 = (data.with(|v| v.as_ref().map_or(0, |v| v.audience))).to_string()
                            )
                        }}
                    </p>
                </Show>
                <ActivityChart
                    data=chart
                    interval=Signal::derive(move || {
                        data.with(|v| v.as_ref().map_or(60, |v| v.bucket_seconds)).max(60)
                    })
                /> <details class="analysis-explanation">
                    <summary>{tr!(ui.what_these_observations_mean)}</summary>
                    <p>
                        {tr!(
                            ui.the_chart_recipient_table_and_exports_show_recorded_events_only_opens_are_image
                        )}
                    </p>
                </details>
                <h2>
                    {move || {
                        if recipient.is_some() {
                            leptos_i18n::t_string!(i18n, ui.recipient_events)
                        } else {
                            leptos_i18n::t_string!(i18n, ui.recipients)
                        }
                    }}
                </h2>
                {if recipient.is_none() {
                    view! {
                        <DataTable
                            label=trs!(ui.recipient_observations)
                            sorting=sorting
                            columns=vec![
                                "email",
                                "name",
                                "delivery",
                                "accepted",
                                "opened",
                                "clicked",
                                "submitted",
                                "reported",
                                "",
                            ]
                            headers=vec![
                                trs!(ui.recipient),
                                trs!(ui.name),
                                trs!(ui.delivery),
                                trs!(ui.accepted),
                                trs!(ui.opened),
                                trs!(ui.clicked),
                                trs!(ui.submitted),
                                trs!(ui.reported),
                                trs!(ui.details),
                            ]
                        >
                            {move || {
                                data.with(|v| {
                                        v.as_ref().map(|v| v.recipients.clone()).unwrap_or_default()
                                    })
                                    .into_iter()
                                    .map(|row| {
                                        let mut filter = filters.get();
                                        filter.offset = 0;
                                        let href = address(
                                            &format!("/analysis/{id}/recipients/{}", row.id),
                                            &filter,
                                            false,
                                        );
                                        view! {
                                            <tr>
                                                <td>{row.email}</td>
                                                <td>{row.name}</td>
                                                <td>
                                                    {ruphish_ui::localization::delivery_label(
                                                        i18n,
                                                        &row.delivery,
                                                    )}
                                                </td>
                                                {(0..5)
                                                    .map(|i| {
                                                        view! {
                                                            <td>
                                                                {row.counts[i]}
                                                                {(i == 4 && row.historical_reported)
                                                                    .then(|| {
                                                                        view! {
                                                                            <small>{tr!(ui.legacy_report_time_unknown)}</small>
                                                                        }
                                                                    })}
                                                            </td>
                                                        }
                                                    })
                                                    .collect_view()}
                                                <td>
                                                    <ActionLink href=href>{tr!(ui.details)}</ActionLink>
                                                </td>
                                            </tr>
                                        }
                                    })
                                    .collect_view()
                            }}
                        </DataTable>
                    }
                        .into_any()
                } else {
                    view! {
                        <DataTable
                            sorting=event_sorting
                            columns=vec!["time", "kind", "source", "password_present"]
                            label=trs!(ui.recipient_events)
                            headers=vec![
                                trs!(ui.time),
                                trs!(ui.event),
                                trs!(ui.source),
                                trs!(ui.details),
                            ]
                        >
                            {move || {
                                data.with(|v| {
                                        v.as_ref().map(|v| v.events.clone()).unwrap_or_default()
                                    })
                                    .into_iter()
                                    .map(|e| {
                                        view! {
                                            <tr>
                                                <td>{minute(&Value::String(e.time))}</td>
                                                <td>
                                                    {ruphish_ui::localization::event_label(i18n, &e.kind)}
                                                </td>
                                                <td>{e.source}</td>
                                                <td>
                                                    {e
                                                        .password_present
                                                        .map(|v| {
                                                            if v {
                                                                leptos_i18n::t_string!(i18n, ui.password_field_was_nonempty)
                                                            } else {
                                                                leptos_i18n::t_string!(i18n, ui.password_field_was_empty)
                                                            }
                                                        })}
                                                </td>
                                            </tr>
                                        }
                                    })
                                    .collect_view()
                            }}
                        </DataTable>
                    }
                        .into_any()
                }} <Show when=move || total.get() == 0>
                    <EmptyState title=trs!(ui.no_matching_results)>
                        <p>{tr!(ui.change_or_clear_the_filters_to_see_more_activity)}</p>
                    </EmptyState>
                </Show>
            </Show>
            <div class="analysis-pagination">
                <span>
                    {move || {
                        if !loaded.get() {
                            "—".to_owned()
                        } else if recipient.is_some() {
                            leptos_i18n::t_string!(
                                i18n, messages.count_matching_events, count = total.get()
                            )
                        } else {
                            leptos_i18n::t_string!(
                                i18n, messages.count_matching_recipients, count = total.get()
                            )
                        }
                    }}
                </span>
                <PaginationControls
                    disabled=Signal::derive(move || loading.get() || !loaded.get())
                    has_previous=Signal::derive(move || filters.get().offset > 0)
                    has_next=Signal::derive(move || {
                        filters.get().offset.saturating_add(filters.get().limit) < total.get()
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
        </ManagementPage>
    };
    view! {
        <ReadBoundary
            failure=read.failure
            has_data=Signal::derive(move || read.data.with(Option::is_some))
            fetching=read.fetching
            retry=read.refresh
            back_href="/analysis"
            back_label=trs!(ui.back_to_analysis)
        >
            {content}
        </ReadBoundary>
    }
}
