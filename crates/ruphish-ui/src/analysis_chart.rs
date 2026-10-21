//! Responsive count chart: lines and visible observation dots, integer axes and keyboard-readable values.
use crate::shared::components::*;
use leptos::prelude::*;
use ruphish_ui::analysis_state::{count_axis, time_axis};
use ruphish_ui::{tr, trs};
use wasm_bindgen::prelude::*;

pub(super) fn labels() -> [Signal<String>; 5] {
    [
        trs!(ui.accepted),
        trs!(ui.opened),
        trs!(ui.clicked),
        trs!(ui.submitted),
        trs!(ui.reported),
    ]
}
const SERIES: [&str; 5] = [
    "series-accepted",
    "series-opened",
    "series-clicked",
    "series-submitted",
    "series-reported",
];

#[wasm_bindgen(module = "/web/analysis-time.mjs")]
extern "C" {
    #[wasm_bindgen(js_name=observeAnalysisWidth)]
    fn observe_width(element: &JsValue, callback: &js_sys::Function) -> JsValue;
    #[wasm_bindgen(js_name=unobserveAnalysisWidth)]
    fn unobserve_width(observer: &JsValue);
}

use crate::time_display::{instant as minute, tick, zone_name as zone};
fn time(seconds: i64) -> String {
    minute(&seconds.into())
}

#[derive(Clone, PartialEq)]
pub(super) struct Bucket {
    pub time: i64,
    pub counts: [i64; 5],
}

#[component]
pub(super) fn ActivityChart(data: Signal<Vec<Bucket>>, interval: Signal<i64>) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let labels = labels();
    let root = NodeRef::<leptos::html::Div>::new();
    let width = RwSignal::new(900usize);
    Effect::new(move |_| {
        if let Some(element) = root.get() {
            let callback = Closure::<dyn FnMut(f64)>::new(move |size| width.set(size as usize));
            let observer = observe_width(element.as_ref(), callback.as_ref().unchecked_ref());
            let cleanup = StoredValue::new_local((observer, callback));
            on_cleanup(move || cleanup.with_value(|(observer, _)| unobserve_width(observer)));
        }
    });
    let visible = RwSignal::new([true; 5]);
    let active = RwSignal::new(None::<i64>);
    let bounds = Memo::new(move |_| {
        let rows = data.get();
        let first = rows.first().map_or(0, |b| b.time) - interval.get();
        let last = rows.last().map_or(first, |b| b.time) + interval.get();
        let maximum = rows
            .iter()
            .flat_map(|b| {
                b.counts
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| visible.get()[*i])
                    .map(|(_, v)| *v)
            })
            .max()
            .unwrap_or(0);
        (first, last.max(first + 60), count_axis(maximum))
    });
    view! {
        <Card class="analysis-chart" label=trs!(ui.recorded_events_over_time)>
            <div class="analysis-section-heading"><h2>{tr!(ui.activity_over_time)}</h2>
                <span>{move || leptos_i18n::t_string!(i18n, messages.events_per_arg0_minute_arg1_arg2, arg0 = (interval.get()/60).to_string(), arg2 = (zone()).to_string())}</span>
            </div>
            <Show when=move || !data.with(Vec::is_empty) fallback=|| view! { <p>{tr!(ui.no_recorded_activity_matches_these_filters)}</p> }>
                <div class="analysis-plot" node_ref=root on:mouseleave=move |_|active.set(None) on:keydown=move |event| {if event.key()=="Escape" {active.set(None);}}>
                    <svg class="analysis-plot-svg" width="100%" height="260" role="img" aria-label=tr!(ui.event_counts_over_time_use_the_interval_buttons_below_for_exact_values)>
                        {move || { let (_,_,(top,step)) = bounds.get(); (0..=top/step).map(|n| {
                            let y=format!("{}%", 100.0 - n as f64*step as f64/top as f64*100.0);
                            view! { <line x1="0" x2="100%" y1=y.clone() y2=y class="analysis-grid" stroke-width="1"/> }
                        }).collect_view() }}
                        {move || { let (first,last,(top,_))=bounds.get(); let span=(last-first) as f64; let enabled=visible.get();
                            let rows = data.get();
                            (0..5).filter(move |i| enabled[*i]).map(move |i| {
                                let x = |time: i64| format!("{}%",(time-first) as f64/span*100.0);
                                let y = |count: i64| format!("{}%",100.0-count as f64/top as f64*100.0);
                                view! { <g class=SERIES[i]>
                                    {rows.windows(2).map(|pair| view! {
                                        <line x1=x(pair[0].time) y1=y(pair[0].counts[i]) x2=x(pair[1].time) y2=y(pair[1].counts[i]) stroke-width="2"/>
                                    }).collect_view()}
                                    {rows.iter().map(|bucket| view! {
                                        <circle cx=x(bucket.time) cy=y(bucket.counts[i]) r="4" class="analysis-point" stroke-width="1.5">
                                            <title>{format!("{}: {} {}",time(bucket.time),bucket.counts[i],labels[i].get())}</title>
                                        </circle>
                                    }).collect_view()}
                                </g> }
                            }).collect_view()
                        }}
                    </svg>
                    <div class="analysis-y-labels" aria-hidden="true">{move || {
                        let (_,_,(top,step))=bounds.get(); (0..=top/step).map(|n| view! {
                            <span style:bottom=format!("{}%",n as f64*step as f64/top as f64*100.0)>{n*step}</span>
                        }).collect_view()
                    }}</div>
                    <div class="analysis-bin-targets">{move || {
                        let (first,last,_)=bounds.get(); let span=(last-first) as f64;
                        data.get().into_iter().map(move |bucket| {
                            let title=format!("{}: {}",leptos_i18n::t_string!(i18n, messages.start_to_end, start = time(bucket.time), end = time(bucket.time+interval.get())),(0..5).map(|i|format!("{} {}",bucket.counts[i],labels[i].get())).collect::<Vec<_>>().join(", "));
                            view! { <button type="button" class="analysis-bin" aria-label=title aria-describedby="analysis-interval-details"
                                style:left=format!("{}%",(bucket.time-first-interval.get()/2) as f64/span*100.0) style:width=format!("{}%",interval.get() as f64/span*100.0)
                                on:mouseenter=move |_| active.set(Some(bucket.time)) on:focus=move |_| active.set(Some(bucket.time)) on:blur=move |_|active.set(None) on:click=move |_| active.set(Some(bucket.time))/> }
                        }).collect_view()
                    }}</div>
                    {move || {
                        let rows=data.get();
                        rows.iter().find(|bucket| Some(bucket.time)==active.get()).map(|bucket| {
                            let (first,last,_)=bounds.get();
                            let position=(bucket.time-first) as f64/(last-first) as f64*100.0;
                            view! {
                                <div class="analysis-hover-guide" style:left=format!("{position}%") aria-hidden="true"></div>
                                <div id="analysis-interval-details" role="tooltip" class="analysis-chart-tooltip"
                                    style:left=format!("clamp(0px, calc({position}% - 140px), max(0px, calc(100% - 280px)))")>
                                    <strong>{tr!(ui.details)}</strong>
                                    <span>{leptos_i18n::t_string!(i18n, messages.start_to_end, start = time(bucket.time), end = time(bucket.time+interval.get()))}</span>
                                    <dl>{(0..5).map(|i|view!{
                                        <div><dt><i class=format!("analysis-swatch {}", SERIES[i])></i>{labels[i].get()}</dt><dd>{bucket.counts[i]}</dd></div>
                                    }).collect_view()}</dl>
                                </div>
                            }
                        })
                    }}
                </div>
                <div class="analysis-x-labels" aria-hidden="true">{move || {
                    let (first,last,_)=bounds.get(); let span=last-first;
                    let (step,ticks)=time_axis(first,last,width.get());
                    ticks.into_iter().map(|value| {
                        let position=(value-first) as f64/span as f64*100.0;
                        view! { <span on:mouseenter=move |_| {active.set(data.with(|rows|rows.iter().min_by_key(|bucket|bucket.time.abs_diff(value)).map(|bucket|bucket.time)));} on:mouseleave=move |_|active.set(None) style:left=format!("{position}%") class:analysis-tick-start={position<10.0} class:analysis-tick-end={position>90.0}>
                            {tick(value,step,first,last)}
                        </span> }
                    }).collect_view()
                }}</div>
                <div class="analysis-legend" aria-label=tr!(ui.visible_event_types)>
                    {labels.into_iter().enumerate().map(|(i,label)| view! {
                        <ActionButton pressed=Signal::derive(move || visible.get()[i].to_string()) on_click=move |_| visible.update(|values| values[i] = !values[i])>
                            <span class=format!("analysis-swatch {}", SERIES[i])></span>{move || label.get()}
                        </ActionButton>
                    }).collect_view()}
                </div>
            </Show>
        </Card>
    }
}
