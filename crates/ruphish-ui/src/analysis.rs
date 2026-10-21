//! Campaign analysis: shared filters, activity chart and recipient drilldown.
use crate::{
    account::Api,
    analysis_chart::{ActivityChart, Bucket, labels},
    campaigns::{array, text},
    feedback::Message,
    shared::components::*,
    time_display::{instant as minute, local_input, parse_input},
};
use leptos::{prelude::*, task::spawn_local_scoped_with_cancellation as spawn_local};
use leptos_router::hooks::{use_navigate, use_params_map, use_query_map};
use ruphish_contracts::reporting::Filter;
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
    view! { <For each=move || vec![(params.with(|p|p.get("id")).unwrap_or_default(),params.with(|p|p.get("recipient")))] key=|key|key.clone() children=move |(id,recipient)| {
        match id.parse::<i64>() {
            Ok(id) => detail(api,id,recipient.and_then(|r|r.parse().ok())).into_any(),
            Err(_) => view!{<p role="alert">{tr!(ui.invalid_campaign_address)}</p>}.into_any(),
        }
    }/> }
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
            offset: get("offset").and_then(|s| s.parse().ok()).unwrap_or(0),
            ..Default::default()
        }
    });
    let selected = RwSignal::new(Vec::<i64>::new());
    let selected_event = RwSignal::new(String::new());
    let source = RwSignal::new(String::new());
    let from = RwSignal::new(String::new());
    let until = RwSignal::new(String::new());
    let data = RwSignal::new(Value::Null);
    let loading = RwSignal::new(false);
    let loaded = RwSignal::new(false);
    let confirmed_filter = RwSignal::new(None::<Filter>);
    let refresh = RwSignal::new(0u64);
    let generation = RwSignal::new(0u64);
    let message = Message::new(String::new());
    let go = move |filter: Filter| {
        navigate.with_value(|navigate| {
            navigate(
                &address(&path.get_value(), &filter, false),
                Default::default(),
            )
        })
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
    Effect::new(move |_| {
        let filter = filters.get();
        refresh.get();
        let current = generation.get_untracked() + 1;
        generation.set(current);
        loading.set(true);
        if confirmed_filter.get_untracked().as_ref() != Some(&filter) {
            loaded.set(false);
        }
        spawn_local(async move {
            let result = api
                .get(&address(
                    &format!("/api/campaigns/{id}/analysis"),
                    &filter,
                    false,
                ))
                .await;
            if current != generation.get_untracked() {
                return;
            }
            match result {
                Ok(value) => {
                    data.set(value);
                    confirmed_filter.set(Some(filter));
                    loaded.set(true);
                    message.clear();
                }
                Err(error) => message.error(error),
            }
            loading.set(false);
        });
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
            let result = api
                .get(&address(
                    &format!("/api/campaigns/{id}/analysis"),
                    &filter,
                    false,
                ))
                .await
                .map(|value| {
                    let rows = array(&value, "recipients")
                        .into_iter()
                        .filter_map(|r| {
                            Some((
                                r["id"].as_i64()?,
                                ruphish_ui::select_options::recipient_label(
                                    &text(&r, "name"),
                                    &text(&r, "email"),
                                ),
                            ))
                        })
                        .collect();
                    (
                        rows,
                        value["recipient_total"].as_u64().unwrap_or(0) as usize,
                    )
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
                    ..Default::default()
                };
                if filter == filters.get_untracked() {
                    refresh.update(|r| *r += 1);
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
                Ok(()) => {
                    message.status(leptos_i18n::t_string!(i18n, ui.export_downloaded).to_owned())
                }
                Err(error) => message.error(error),
            }
            downloading.set(false);
        });
    };
    let chart = Signal::derive(move || {
        array(&data.get(), "buckets")
            .into_iter()
            .map(|b| Bucket {
                time: b["time"].as_i64().unwrap_or(0),
                counts: std::array::from_fn(|i| b["counts"][i].as_i64().unwrap_or(0)),
            })
            .collect::<Vec<_>>()
    });
    let total = Signal::derive(move || {
        data.get()[if recipient.is_some() {
            "event_total"
        } else {
            "recipient_total"
        }]
        .as_u64()
        .unwrap_or(0) as usize
    });
    view! {
        <ManagementPage>
            <BackLink href=Signal::derive(move || {
                if recipient.is_some(){let mut f=filters.get();f.recipient=None;f.offset=0;address(&format!("/analysis/{id}"),&f,false)}else{"/analysis".into()}
            })>{move || if recipient.is_some(){leptos_i18n::t_string!(i18n, ui.back_to_campaign_analysis)}else{leptos_i18n::t_string!(i18n, ui.back_to_analysis)}}</BackLink>
            <PageHeader title=Signal::derive(move || {let name=text(&data.get(),"name");if name.is_empty(){leptos_i18n::t_string!(i18n, ui.campaign).into()}else{name}})
                subtitle=Signal::derive(move || if loaded.get(){leptos_i18n::t_string!(i18n, messages.updated_arg0, arg0 = (minute(&data.get()["as_of"])).to_string())}else if loading.get(){leptos_i18n::t_string!(i18n, ui.loading_analysis).into()}else{leptos_i18n::t_string!(i18n, ui.refresh_failed_retry_to_load_current_results).into()})>
                <ActionButton busy=Signal::derive(move ||loading.get().to_string()) on_click=move |_|refresh.update(|r|*r+=1)>{move ||if loading.get(){leptos_i18n::t_string!(i18n, ui.refreshing)}else{leptos_i18n::t_string!(i18n, ui.refresh)}}</ActionButton>
                <ActionMenu label=trs!(ui.export) style=ButtonStyle::Primary disabled=Signal::derive(move ||downloading.get()||!loaded.get())>
                    <ActionButton disabled=Signal::derive(move ||downloading.get()||!loaded.get()) on_click=move |_|export("csv","recipients")>{tr!(ui.recipients_csv)}</ActionButton>
                    <ActionButton disabled=Signal::derive(move ||downloading.get()||!loaded.get()) on_click=move |_|export("csv","events")>{tr!(ui.events_csv)}</ActionButton>
                    <ActionButton disabled=Signal::derive(move ||downloading.get()||!loaded.get()) on_click=move |_|export("xlsx","all")>{tr!(ui.excel_workbook)}</ActionButton>
                </ActionMenu>
                <ActionLink href=format!("/campaigns/{id}")>{tr!(ui.back_to_campaign)}</ActionLink>
            </PageHeader>
            {message.view()}
            <form class="analysis-filters" on:submit=apply>
                <div class="analysis-filter-fields">
                    {recipient.is_none().then(||view!{
                        <MultiSelect label=trs!(ui.recipients) selected=selected search=option_search
                            options=Signal::derive(move ||option_state.get().rows)
                            loading=Signal::derive(move ||option_state.get().loading) error=Signal::derive(move ||option_state.get().error)
                            has_more=Signal::derive(move ||option_state.with(ruphish_ui::select_options::Options::has_more))
                            on_more=Callback::new(move |()|more_options(false)) on_retry=Callback::new(move |()|more_options(true))/>

                    })}
                    <Select label=trs!(ui.event) value=selected_event on_change=Callback::new(move |v|selected_event.set(v))>
                        <option value="">{tr!(ui.all_events)}</option>{EVENTS.into_iter().map(|e|view!{<option value=e>{move || ruphish_ui::localization::event_label(i18n,e)}</option>}).collect_view()}
                    </Select>
                    <Select label=trs!(ui.source) value=source on_change=Callback::new(move |v|source.set(v))>
                        <option value="">{tr!(ui.all_sources)}</option><option value="public">{tr!(ui.public_tracking)}</option><option value="mailbox">{tr!(ui.reporting_mailbox)}</option><option value="application">{tr!(ui.application)}</option><option value="legacy">{tr!(ui.legacy_history)}</option>
                    </Select>
                    <Field label=trs!(ui.from)><TextInput input_type="datetime-local" step="60" value=Signal::derive(move ||from.get()) on_input=move |e|from.set(event_target_value(&e))/></Field>
                    <Field label=trs!(ui.until)><TextInput input_type="datetime-local" step="60" value=Signal::derive(move ||until.get()) on_input=move |e|until.set(event_target_value(&e))/></Field>
                </div>
                <div class="analysis-filter-footer"><span>{move || leptos_i18n::t_string!(i18n, messages.arg0_from_is_inclusive_until_is_exclusive, arg0 = (crate::time_display::zone_name()).to_string())}</span><div class="management-actions">
                    <ActionButton on_click=move |_|go(Filter{recipient,..Default::default()})>{tr!(ui.clear_filters)}</ActionButton>
                    <ActionButton kind="submit" style=ButtonStyle::Primary>{tr!(ui.apply_filters)}</ActionButton>
                </div></div>
                <Show when=move ||!filters.get().search.is_empty()><p>{move ||leptos_i18n::t_string!(i18n, messages.saved_recipient_search_arg0, arg0 = (filters.get().search).to_string())}</p></Show>
            </form>
            <Show when=move ||loaded.get()>
                {recipient.map(|_|view!{<h2 class="analysis-recipient-title">{move ||array(&data.get(),"recipients").first().map(|r|text(r,"email")).unwrap_or_else(||leptos_i18n::t_string!(i18n, ui.recipient_details).into())}</h2>})}
                <div class="analysis-summary">
                    {labels().into_iter().enumerate().map(|(i,label)|view!{
                        <Card class="analysis-metric"><span>{move || label.get()}</span><strong>{move ||data.get()["progression"][i].as_i64().unwrap_or(0)}</strong>
                            <small>{move ||leptos_i18n::t_string!(i18n, messages.arg0_recorded_events, arg0 = (data.get()["totals"][i].as_i64().unwrap_or(0)).to_string())}</small>
                        </Card>
                    }).collect_view()}
                </div>
                <p class="analysis-context">{move ||leptos_i18n::t_string!(i18n, messages.recipient_progression_within_the_selected_filters_opens_include_clicks_and_submi, arg0 = (data.get()["audience"]).to_string())}</p>
                <ActivityChart data=chart interval=Signal::derive(move ||data.get()["bucket_seconds"].as_i64().unwrap_or(60).max(60))/>
                <details class="analysis-explanation"><summary>{tr!(ui.what_these_observations_mean)}</summary><p>{tr!(ui.the_chart_recipient_table_and_exports_show_recorded_events_only_opens_are_image)}</p></details>
                <h2>{move || if recipient.is_some(){leptos_i18n::t_string!(i18n, ui.recipient_events)}else{leptos_i18n::t_string!(i18n, ui.recipients)}}</h2>
                {if recipient.is_none(){view!{
                    <DataTable label=trs!(ui.recipient_observations) headers=vec![trs!(ui.recipient),trs!(ui.delivery),trs!(ui.accepted),trs!(ui.opened),trs!(ui.clicked),trs!(ui.submitted),trs!(ui.reported),trs!(ui.details)]>
                        {move ||array(&data.get(),"recipients").into_iter().map(|row| {
                            let mut filter=filters.get();filter.offset=0;
                            let href=address(&format!("/analysis/{id}/recipients/{}",row["id"]),&filter,false);
                            view!{<tr><td>{text(&row,"email")}<small>{text(&row,"name")}</small></td><td>{ruphish_ui::localization::delivery_label(i18n,&text(&row,"delivery"))}</td>
                                {(0..5).map(|i|view!{<td>{row["counts"][i].as_i64().unwrap_or(0)}{(i==4&&row["historical_reported"]==true).then(||view!{<small>{tr!(ui.legacy_report_time_unknown)}</small>})}</td>}).collect_view()}
                                <td><ActionLink href=href>{tr!(ui.details)}</ActionLink></td>
                            </tr>}
                        }).collect_view()}
                    </DataTable>
                }.into_any()}else{view!{
                    <DataTable label=trs!(ui.recipient_events) headers=vec![trs!(ui.time),trs!(ui.event),trs!(ui.source),trs!(ui.details)]>
                        {move ||array(&data.get(),"events").into_iter().map(|e|view!{<tr><td>{minute(&e["time"])}</td><td>{ruphish_ui::localization::event_label(i18n,&text(&e,"kind"))}</td><td>{text(&e,"source")}</td><td>{e["password_present"].as_bool().map(|v|if v{leptos_i18n::t_string!(i18n, ui.password_field_was_nonempty)}else{leptos_i18n::t_string!(i18n, ui.password_field_was_empty)})}</td></tr>}).collect_view()}
                    </DataTable>
                }.into_any()}}
                <Show when=move ||total.get()==0><EmptyState title=trs!(ui.no_matching_results)><p>{tr!(ui.change_or_clear_the_filters_to_see_more_activity)}</p></EmptyState></Show>
                <div class="analysis-pagination">
                    <span>{move ||if recipient.is_some(){leptos_i18n::t_string!(i18n, messages.count_matching_events, count = total.get())}else{leptos_i18n::t_string!(i18n, messages.count_matching_recipients, count = total.get())}}</span>
                    <div class="management-actions">
                        <ActionButton disabled=Signal::derive(move ||loading.get()||filters.get().offset==0) on_click=move |_|{let mut f=filters.get_untracked();f.offset=f.offset.saturating_sub(f.limit);go(f);}>{tr!(ui.previous_page)}</ActionButton>
                        <ActionButton disabled=Signal::derive(move ||loading.get()||filters.get().offset.saturating_add(filters.get().limit)>=total.get()) on_click=move |_|{let mut f=filters.get_untracked();f.offset+=f.limit;go(f);}>{tr!(ui.next_page)}</ActionButton>
                    </div>
                </div>
            </Show>
        </ManagementPage>
    }
}
