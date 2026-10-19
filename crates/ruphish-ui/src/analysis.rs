//! Campaign analysis: shared filters, activity chart and recipient drilldown.
use crate::{
    account::Api,
    analysis_chart::{ActivityChart, Bucket, LABELS, minute},
    campaigns::{array, text},
    feedback::Message,
    shared::components::*,
};
use leptos::{prelude::*, task::spawn_local_scoped_with_cancellation as spawn_local};
use leptos_router::hooks::{use_navigate, use_params_map, use_query_map};
use ruphish_contracts::reporting::Filter;
use ruphish_ui::analysis_state::address;
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
            Err(_) => view!{<p role="alert">"Invalid campaign address"</p>}.into_any(),
        }
    }/> }
}

fn detail(api: Api, id: i64, recipient: Option<i64>) -> impl IntoView {
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
        let parse = |value: String| -> Result<Option<i64>, ()> {
            if value.is_empty() {
                return Ok(None);
            }
            let ms = js_sys::Date::new(&value.into()).get_time();
            if ms.is_finite() {
                Ok(Some((ms / 1000.0) as i64))
            } else {
                Err(())
            }
        };
        match (parse(from.get_untracked()), parse(until.get_untracked())) {
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
            _ => message.error("Enter a valid time range with the end after the start.".into()),
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
                Ok(()) => message.status("Export downloaded.".into()),
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
            <a class="breadcrumb" href=move || {
                if recipient.is_some(){let mut f=filters.get();f.recipient=None;f.offset=0;address(&format!("/analysis/{id}"),&f,false)}else{format!("/campaigns/{id}")}
            }>{if recipient.is_some(){"Back to campaign analysis"}else{"Back to campaign"}}</a>
            <PageHeader title=Signal::derive(move || {let name=text(&data.get(),"name");if name.is_empty(){"Campaign".into()}else{name}})
                subtitle=Signal::derive(move || if loaded.get(){format!("Updated {}",minute(&data.get()["as_of"]))}else if loading.get(){"Loading analysis...".into()}else{"Refresh failed. Retry to load current results.".into()})>
                <ActionButton busy=Signal::derive(move ||loading.get().to_string()) on_click=move |_|refresh.update(|r|*r+=1)>{move ||if loading.get(){"Refreshing…"}else{"Refresh"}}</ActionButton>
                <ActionButton disabled=Signal::derive(move ||downloading.get()||!loaded.get()) on_click=move |_|export("csv","recipients")>"Recipients CSV"</ActionButton>
                <ActionButton disabled=Signal::derive(move ||downloading.get()||!loaded.get()) on_click=move |_|export("csv","events")>"Events CSV"</ActionButton>
                <ActionButton disabled=Signal::derive(move ||downloading.get()||!loaded.get()) on_click=move |_|export("xlsx","all")>"Export Excel"</ActionButton>
            </PageHeader>
            {message.view()}
            <form class="analysis-filters" on:submit=apply>
                <div class="analysis-filter-fields">
                    {recipient.is_none().then(||view!{
                        <MultiSelect label="Recipients" selected=selected search=option_search
                            options=Signal::derive(move ||option_state.get().rows)
                            loading=Signal::derive(move ||option_state.get().loading) error=Signal::derive(move ||option_state.get().error)
                            has_more=Signal::derive(move ||option_state.with(ruphish_ui::select_options::Options::has_more))
                            on_more=Callback::new(move |()|more_options(false)) on_retry=Callback::new(move |()|more_options(true))/>

                    })}
                    <Field label="Event"><select prop:value=move ||selected_event.get() on:change=move |e|selected_event.set(event_target_value(&e))>
                        <option value="">"All events"</option>{EVENTS.into_iter().map(|e|view!{<option value=e>{e}</option>}).collect_view()}
                    </select></Field>
                    <Field label="Source"><select prop:value=move ||source.get() on:change=move |e|source.set(event_target_value(&e))>
                        <option value="">"All sources"</option><option value="public">"Public tracking"</option><option value="mailbox">"Reporting mailbox"</option><option value="application">"Application"</option><option value="legacy">"Legacy history"</option>
                    </select></Field>
                    <Field label="From"><input type="datetime-local" step="60" prop:value=move ||from.get() on:input=move |e|from.set(event_target_value(&e))/></Field>
                    <Field label="Until"><input type="datetime-local" step="60" prop:value=move ||until.get() on:input=move |e|until.set(event_target_value(&e))/></Field>
                </div>
                <div class="analysis-filter-footer"><span>"Local time. From is inclusive; until is exclusive."</span><div class="management-actions">
                    <ActionButton on_click=move |_|go(Filter{recipient,..Default::default()})>"Clear filters"</ActionButton>
                    <ActionButton kind="submit" style=ButtonStyle::Primary>"Apply filters"</ActionButton>
                </div></div>
                <Show when=move ||!filters.get().search.is_empty()><p>{move ||format!("Saved recipient search: {}",filters.get().search)}</p></Show>
            </form>
            <Show when=move ||loaded.get()>
                {recipient.map(|_|view!{<h2 class="analysis-recipient-title">{move ||array(&data.get(),"recipients").first().map(|r|text(r,"email")).unwrap_or_else(||"Recipient details".into())}</h2>})}
                <div class="analysis-summary">
                    {LABELS.into_iter().enumerate().map(|(i,label)|view!{
                        <div class="analysis-metric"><span>{label}</span><strong>{move ||data.get()["unique"][i].as_i64().unwrap_or(0)}</strong>
                            <small>{move ||format!("{} recorded events",data.get()["totals"][i].as_i64().unwrap_or(0))}</small>
                        </div>
                    }).collect_view()}
                </div>
                <p class="analysis-context">{move ||format!("Unique recipients per event type, within the selected filters. Campaign audience: {}.",data.get()["audience"])}</p>
                <ActivityChart data=chart interval=Signal::derive(move ||data.get()["bucket_seconds"].as_i64().unwrap_or(60).max(60))/>
                <details class="analysis-explanation"><summary>"What these observations mean"</summary><p>"Opens are image fetches; clicks are page visits. Neither proves a human action. Submissions record the action and optional password presence, never entered values. Reports identify the campaign message, not a verified reporter."</p></details>
                <h2>{if recipient.is_some(){"Recipient events"}else{"Recipients"}}</h2>
                {if recipient.is_none(){view!{
                    <DataTable label="Recipient observations" headers=vec!["Recipient","Delivery","Accepted","Opened","Clicked","Submitted","Reported","Details"]>
                        {move ||array(&data.get(),"recipients").into_iter().map(|row| {
                            let mut filter=filters.get();filter.offset=0;
                            let href=address(&format!("/analysis/{id}/recipients/{}",row["id"]),&filter,false);
                            view!{<tr><td>{text(&row,"email")}<small>{text(&row,"name")}</small></td><td>{text(&row,"delivery")}</td>
                                {(0..5).map(|i|view!{<td>{row["counts"][i].as_i64().unwrap_or(0)}{(i==4&&row["historical_reported"]==true).then(||view!{<small>"Legacy report; time unknown"</small>})}</td>}).collect_view()}
                                <td><ActionLink href=href>"Details"</ActionLink></td>
                            </tr>}
                        }).collect_view()}
                    </DataTable>
                }.into_any()}else{view!{
                    <DataTable label="Recipient events" headers=vec!["Time","Event","Source","Details"]>
                        {move ||array(&data.get(),"events").into_iter().map(|e|view!{<tr><td>{minute(&e["time"])}</td><td>{text(&e,"kind")}</td><td>{text(&e,"source")}</td><td>{e["password_present"].as_bool().map(|v|if v{"Password field was nonempty"}else{"Password field was empty"})}</td></tr>}).collect_view()}
                    </DataTable>
                }.into_any()}}
                <Show when=move ||total.get()==0><EmptyState title="No matching results"><p>"Change or clear the filters to see more activity."</p></EmptyState></Show>
                <div class="analysis-pagination">
                    <span>{move ||format!("{} matching {}",total.get(),if recipient.is_some(){"events"}else{"recipients"})}</span>
                    <div class="management-actions">
                        <ActionButton disabled=Signal::derive(move ||loading.get()||filters.get().offset==0) on_click=move |_|{let mut f=filters.get_untracked();f.offset=f.offset.saturating_sub(f.limit);go(f);}>"Previous page"</ActionButton>
                        <ActionButton disabled=Signal::derive(move ||loading.get()||filters.get().offset.saturating_add(filters.get().limit)>=total.get()) on_click=move |_|{let mut f=filters.get_untracked();f.offset+=f.limit;go(f);}>"Next page"</ActionButton>
                    </div>
                </div>
            </Show>
        </ManagementPage>
    }
}
fn local_input(seconds: Option<i64>) -> String {
    let Some(seconds) = seconds else {
        return String::new();
    };
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(seconds as f64 * 1000.0));
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}",
        date.get_full_year(),
        date.get_month() + 1,
        date.get_date(),
        date.get_hours(),
        date.get_minutes()
    )
}
