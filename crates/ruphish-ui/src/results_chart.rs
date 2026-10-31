//! Participant activity in cumulative and per-interval views, plus delivery summaries.
use crate::shared::components::*;
use leptos::prelude::*;
use ruphish_ui::results_state::{ChartInterval, ChartMode, count_axis, time_axis};
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

fn activity_labels() -> [Signal<String>; 4] {
    let [_, opened, clicked, submitted, reported] = labels();
    [opened, clicked, submitted, reported]
}

const SERIES: [&str; 4] = [
    "series-opened",
    "series-clicked",
    "series-submitted",
    "series-reported",
];

#[wasm_bindgen(module = "/web/results-time.mjs")]
extern "C" {
    #[wasm_bindgen(js_name=observeResultsWidth)]
    fn observe_width(element: &JsValue, callback: &js_sys::Function) -> JsValue;
    #[wasm_bindgen(js_name=unobserveResultsWidth)]
    fn unobserve_width(observer: &JsValue);
}

use crate::time_display::{instant as minute, tick, zone_name as zone};
fn time(seconds: i64) -> String {
    minute(&seconds.into())
}

fn interval_description(
    i18n: leptos_i18n::I18nContext<ruphish_ui::localization::Locale>,
    mode: ChartMode,
    bucket: ChartInterval,
) -> String {
    match mode {
        ChartMode::Cumulative => {
            leptos_i18n::t_string!(i18n, messages.participants_through, end = time(bucket.end))
                .to_string()
        }
        ChartMode::PerInterval => leptos_i18n::t_string!(
            i18n,
            messages.first_actions_between,
            start = time(bucket.start),
            end = time(bucket.end)
        )
        .to_string(),
    }
}

#[component]
pub(super) fn ActivityChart(
    data: Memo<Vec<ChartInterval>>,
    interval: Signal<i64>,
    track_opens: Signal<bool>,
) -> impl IntoView {
    let mode = RwSignal::new(ChartMode::default());
    let selected = RwSignal::new([true; 4]);
    let visible = Signal::derive(move || {
        let mut values = selected.get();
        values[0] &= track_opens.get();
        values
    });
    let displayed = Memo::new(move |_| data.with(|rows| mode.get().project(rows)));
    view! {
        <Card class="analysis-chart" label=trs!(ui.recorded_events_over_time)>
            <ChartHeading mode=mode interval=interval />
            <Show
                when=move || !data.with(Vec::is_empty)
                fallback=|| view! { <p>{tr!(ui.no_recorded_activity_matches_these_filters)}</p> }
            >
                <Show when=move || !visible.get().iter().any(|enabled| *enabled)>
                    <p role="status">{tr!(ui.select_event_type_to_show_activity)}</p>
                </Show>
                <div hidden=move || !visible.get().iter().any(|enabled| *enabled)>
                    <ActivityPlot data=displayed mode=mode visible=visible />
                </div>
                <ActivityLegend visible=selected track_opens=track_opens />
            </Show>
        </Card>
    }
}

#[component]
fn ChartHeading(mode: RwSignal<ChartMode>, interval: Signal<i64>) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    view! {
        <div class="analysis-section-heading">
            <div>
                <h2>{tr!(ui.activity_over_time)}</h2>
                <p>
                    {move || match mode.get() {
                        ChartMode::Cumulative => {
                            leptos_i18n::t_string!(
                                i18n, messages.participants_through_intervals,
                        minutes = (interval.get() / 60).to_string(), zone = zone()
                            )
                                .to_string()
                        }
                        ChartMode::PerInterval => {
                            leptos_i18n::t_string!(
                                i18n, messages.first_actions_intervals,
                        minutes = (interval.get() / 60).to_string(), zone = zone()
                            )
                                .to_string()
                        }
                    }}
                </p>
            </div>
            <Select
                label=trs!(ui.chart_view)
                value=Signal::derive(move || mode.get().value().to_owned())
                on_change=Callback::new(move |value: String| {
                    if let Some(selected) = ChartMode::from_value(&value) {
                        mode.set(selected);
                    }
                })
            >
                <option value="cumulative">{tr!(ui.chart_cumulative)}</option>
                <option value="per_interval">{tr!(ui.chart_per_interval)}</option>
            </Select>
        </div>
    }
}

#[derive(Clone, Copy, PartialEq)]
struct PlotBounds {
    first: i64,
    last: i64,
    top: i64,
    step: i64,
}

impl PlotBounds {
    fn x(self, time: i64) -> f64 {
        (time - self.first) as f64 / (self.last - self.first) as f64 * 100.0
    }

    fn y(self, count: i64) -> f64 {
        100.0 - count as f64 / self.top as f64 * 100.0
    }
}

#[component]
fn ActivityPlot(
    data: Memo<Vec<ChartInterval>>,
    mode: RwSignal<ChartMode>,
    visible: Signal<[bool; 4]>,
) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    let labels = activity_labels();
    let descriptions = [
        trs!(ui.chart_cumulative_description),
        trs!(ui.chart_per_interval_description),
    ];
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
    let active = RwSignal::new(None::<i64>);
    Effect::new(move |_| {
        data.track();
        mode.track();
        visible.track();
        active.set(None);
    });
    let selected = Signal::derive(move || {
        data.with(|rows| {
            rows.iter()
                .find(|bucket| Some(bucket.start) == active.get())
                .copied()
        })
    });
    let bounds = Memo::new(move |_| {
        data.with(|rows| {
            let first = rows.first().map_or(0, |bucket| bucket.start);
            let last = rows.last().map_or(first + 60, |bucket| bucket.end);
            let enabled = visible.get();
            let maximum = rows
                .iter()
                .flat_map(|bucket| {
                    bucket
                        .counts
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| enabled[*i])
                        .map(|(_, count)| *count)
                })
                .max()
                .unwrap_or(0);
            let (top, step) = count_axis(maximum);
            PlotBounds {
                first,
                last,
                top,
                step,
            }
        })
    });
    view! {
        <div
            class="analysis-plot"
            node_ref=root
            on:mouseleave=move |_| active.set(None)
            on:keydown=move |event| {
                if event.key() == "Escape" {
                    active.set(None);
                }
            }
        >
            <svg
                class="analysis-plot-svg"
                width="100%"
                height="260"
                role="img"
                aria-label=move || match mode.get() {
                    ChartMode::Cumulative => descriptions[0].get(),
                    ChartMode::PerInterval => descriptions[1].get(),
                }
            >
                {move || {
                    let bounds = bounds.get();
                    (0..=bounds.top / bounds.step)
                        .map(|n| {
                            let y = format!("{}%", bounds.y(n * bounds.step));
                            view! {
                                <line
                                    x1="0"
                                    x2="100%"
                                    y1=y.clone()
                                    y2=y
                                    class="analysis-grid"
                                    stroke-width="1"
                                />
                            }
                        })
                        .collect_view()
                }}
                {(0..4)
                    .map(move |index| {
                        view! {
                            <Show when=move || visible.get()[index]>
                                <ActivitySeries
                                    data=data
                                    mode=mode
                                    bounds=bounds
                                    index=index
                                    label=labels[index]
                                />
                            </Show>
                        }
                    })
                    .collect_view()}
            </svg>
            <div class="analysis-y-labels" aria-hidden="true">
                {move || {
                    let bounds = bounds.get();
                    (0..=bounds.top / bounds.step)
                        .map(|n| {
                            view! {
                                <span style:bottom=format!(
                                    "{}%",
                                    100.0 - bounds.y(n * bounds.step),
                                )>{n * bounds.step}</span>
                            }
                        })
                        .collect_view()
                }}
            </div>
            <div class="analysis-bin-targets">
                {move || {
                    let bounds = bounds.get();
                    let mode = mode.get();
                    let enabled = visible.get();
                    data.get()
                        .into_iter()
                        .map(move |bucket| {
                            let title = format!(
                                "{}: {}",
                                interval_description(i18n, mode, bucket),
                                (0..4)
                                    .filter(|i| enabled[*i])
                                    .map(|i| format!("{} {}", bucket.counts[i], labels[i].get()))
                                    .collect::<Vec<_>>()
                                    .join(", "),
                            );
                            view! {
                                <button
                                    type="button"
                                    class="analysis-bin"
                                    aria-label=title
                                    aria-describedby=move || {
                                        (active.get() == Some(bucket.start))
                                            .then_some("analysis-interval-details")
                                    }
                                    style:left=format!("{}%", bounds.x(bucket.start))
                                    style:width=format!(
                                        "{}%",
                                        bounds.x(bucket.end) - bounds.x(bucket.start),
                                    )
                                    on:mouseenter=move |_| active.set(Some(bucket.start))
                                    on:focus=move |_| active.set(Some(bucket.start))
                                    on:blur=move |_| active.set(None)
                                    on:click=move |_| active.set(Some(bucket.start))
                                />
                            }
                        })
                        .collect_view()
                }}
            </div>
            <IntervalDetails selected=selected mode=mode bounds=bounds visible=visible />
        </div>
        <div class="analysis-x-labels" aria-hidden="true">
            {move || {
                let bounds = bounds.get();
                let (step, ticks) = time_axis(bounds.first, bounds.last, width.get());
                ticks
                    .into_iter()
                    .map(|value| {
                        let position = bounds.x(value);
                        view! {
                            <span
                                style:left=format!("{position}%")
                                class:analysis-tick-start={position < 10.0}
                                class:analysis-tick-end={position > 90.0}
                            >
                                {tick(value, step, bounds.first, bounds.last)}
                            </span>
                        }
                    })
                    .collect_view()
            }}
        </div>
    }
}

#[component]
fn ActivitySeries(
    data: Memo<Vec<ChartInterval>>,
    mode: RwSignal<ChartMode>,
    bounds: Memo<PlotBounds>,
    index: usize,
    label: Signal<String>,
) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    view! {
        <g class=SERIES[index]>
            {move || {
                let rows = data.get();
                let mode = mode.get();
                let bounds = bounds.get();
                rows.windows(2)
                    .map(|pair| {
                        let x1 = format!("{}%", bounds.x(mode.point_time(&pair[0])));
                        let x2 = format!("{}%", bounds.x(mode.point_time(&pair[1])));
                        let y1 = format!("{}%", bounds.y(pair[0].counts[index]));
                        let y2 = format!("{}%", bounds.y(pair[1].counts[index]));
                        let end_y = if mode == ChartMode::Cumulative {
                            y1.clone()
                        } else {
                            y2.clone()
                        };
                        view! {
                            <line x1=x1 y1=y1.clone() x2=x2.clone() y2=end_y stroke-width="2" />
                            {(mode == ChartMode::Cumulative)
                                .then(|| {
                                    view! {
                                        <line x1=x2.clone() y1=y1 x2=x2 y2=y2 stroke-width="2" />
                                    }
                                })}
                        }
                    })
                    .collect_view()
            }}
            {move || {
                let mode = mode.get();
                let bounds = bounds.get();
                data.get()
                    .into_iter()
                    .map(move |bucket| {
                        view! {
                            <circle
                                cx=format!("{}%", bounds.x(mode.point_time(&bucket)))
                                cy=format!("{}%", bounds.y(bucket.counts[index]))
                                r="4"
                                class="analysis-point"
                                stroke-width="1.5"
                            >
                                <title>
                                    {format!(
                                        "{}: {} {}",
                                        interval_description(i18n, mode, bucket),
                                        bucket.counts[index],
                                        label.get(),
                                    )}
                                </title>
                            </circle>
                        }
                    })
                    .collect_view()
            }}
        </g>
    }
}

#[component]
fn IntervalDetails(
    selected: Signal<Option<ChartInterval>>,
    mode: RwSignal<ChartMode>,
    bounds: Memo<PlotBounds>,
    visible: Signal<[bool; 4]>,
) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    let labels = activity_labels();
    move || {
        selected.get().map(|bucket| {
            let mode = mode.get();
            let position = bounds.get().x(mode.point_time(&bucket));
            view! {
                <div
                    class="analysis-hover-guide"
                    style:left=format!("{position}%")
                    aria-hidden="true"
                ></div>
                <div
                    id="analysis-interval-details"
                    role="tooltip"
                    class="analysis-chart-tooltip"
                    style:left=format!(
                        "clamp(0px, calc({position}% - 140px), max(0px, calc(100% - 280px)))",
                    )
                >
                    <strong>{tr!(ui.details)}</strong>
                    <span>{interval_description(i18n, mode, bucket)}</span>
                    <dl>
                        {(0..4)
                            .filter(|i| visible.get()[*i])
                            .map(|i| {
                                view! {
                                    <div>
                                        <dt>
                                            <i class=format!("analysis-swatch {}", SERIES[i])></i>
                                            {labels[i].get()}
                                        </dt>
                                        <dd>{bucket.counts[i]}</dd>
                                    </div>
                                }
                            })
                            .collect_view()}
                    </dl>
                </div>
            }
        })
    }
}

#[component]
fn ActivityLegend(visible: RwSignal<[bool; 4]>, track_opens: Signal<bool>) -> impl IntoView {
    view! {
        <div class="analysis-legend" aria-label=tr!(ui.visible_event_types)>
            {activity_labels()
                .into_iter()
                .enumerate()
                .map(move |(i, label)| {
                    view! {
                        <Show when=move || i != 0 || track_opens.get()>
                            <ActionButton
                                pressed=Signal::derive(move || visible.get()[i].to_string())
                                on_click=move |_| visible.update(|values| values[i] = !values[i])
                            >
                                <span class=format!("analysis-swatch {}", SERIES[i])></span>
                                {move || label.get()}
                            </ActionButton>
                        </Show>
                    }
                })
                .collect_view()}
        </div>
    }
}

/// Mutually exclusive current delivery states; retries never double-count recipients.
pub(super) fn delivery_donut(
    data: Signal<Option<std::sync::Arc<ruphish_contracts::campaign_reads::Delivery>>>,
) -> impl IntoView {
    use ruphish_domain::campaign::DeliveryState;
    let states = [
        DeliveryState::Ready,
        DeliveryState::Preparing,
        DeliveryState::Submitting,
        DeliveryState::Retry,
        DeliveryState::Accepted,
        DeliveryState::Failed,
        DeliveryState::Blocked,
        DeliveryState::Unknown,
        DeliveryState::Cancelled,
    ];
    let total = Signal::derive(move || {
        states
            .iter()
            .map(|state| {
                data.with(|page| {
                    page.as_ref()
                        .and_then(|page| page.counts.get(state).copied())
                        .unwrap_or(0)
                })
                .max(0)
            })
            .sum::<i64>()
    });
    view! {
        <div class="delivery-donut" aria-hidden="true">
            <svg viewBox="0 0 120 120">
                <circle cx="60" cy="60" r="46" fill="none" stroke="var(--line)" stroke-width="15" />
                {move || {
                    let all = total.get();
                    let mut offset = 0.0;
                    states
                        .into_iter()
                        .map(|state| {
                            let color = delivery_color(state.as_str());
                            let count = data
                                .with(|page| {
                                    page.as_ref()
                                        .and_then(|page| page.counts.get(&state).copied())
                                        .unwrap_or(0)
                                })
                                .max(0);
                            let size = if all > 0 {
                                100.0 * count as f64 / all as f64
                            } else {
                                0.0
                            };
                            let start = offset;
                            offset += size;
                            view! {
                                <circle
                                    cx="60"
                                    cy="60"
                                    r="46"
                                    pathLength="100"
                                    fill="none"
                                    stroke=color
                                    stroke-width="15"
                                    stroke-dasharray=format!("{size} {}", 100.0 - size)
                                    stroke-dashoffset=(-start).to_string()
                                    transform="rotate(-90 60 60)"
                                />
                            }
                        })
                        .collect_view()
                }}
                <text x="60" y="64" text-anchor="middle">
                    {move || total.get()}
                </text>
            </svg>
        </div>
    }
}

pub(super) fn delivery_color(state: &str) -> &'static str {
    match state {
        "ready" => "#bacac7",
        "preparing" => "#77a8a0",
        "submitting" => "#3f8f84",
        "retry" => "#c89432",
        "accepted" => "#0b6b63",
        "failed" => "#be4652",
        "blocked" => "#cc7447",
        "unknown" => "#8a6eaa",
        _ => "#87938f",
    }
}
