//! Independent recipient results page: identity and individual observations.
use crate::{
    campaign_queries::CampaignQueries, client::Api, feedback::Message, shared::components::*,
    time_display::instant,
};
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use ruphish_contracts::{campaign_reads::Results, reporting::Filter};
use ruphish_ui::{tr, trs};

pub(super) fn page(api: Api) -> impl IntoView {
    let params = use_params_map();
    view! {
        <For
            each=move || vec![(params.with(|p| p.get("id")), params.with(|p| p.get("recipient")))]
            key=|key| key.clone()
            children=move |(campaign, recipient)| {
                let parse = |v: Option<String>| {
                    v.and_then(|v| v.parse::<i64>().ok()).filter(|v| *v > 0)
                };
                match (parse(campaign), parse(recipient)) {
                    (Some(campaign), Some(recipient)) => detail(api, campaign, recipient).into_any(),
                    _ => {
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

fn detail(api: Api, campaign: i64, recipient: i64) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    let query = leptos_router::hooks::use_query_map();
    let queries = expect_context::<CampaignQueries>();
    let back = Signal::derive(move || {
        query.with(|q| {
            ruphish_ui::results_state::results_return(campaign, q.get("return_to").as_deref())
        })
    });
    let visible = RwSignal::new(50usize);
    let read = queries.recipient_results.observe_polling(
        move || (campaign, recipient, visible.get()),
        ruphish_ui::query::document_visible(),
    );
    let context = queries.context.observe(move || campaign.to_string());
    let data = read.data;
    let person = Signal::derive(move || {
        data.with(|v| v.as_ref().and_then(|v| v.recipients.first()).cloned())
    });
    let loaded = Signal::derive(move || data.with(Option::is_some));
    let message = Message::new(String::new());
    let expanding = RwSignal::new(false);
    let fetching =
        Signal::derive(move || read.fetching.get() || context.fetching.get() || expanding.get());
    let tasks = ruphish_ui::view_tasks::ViewTasks::default();
    let more = Callback::new(move |_| {
        if fetching.get_untracked() {
            return;
        }
        let count = visible.get_untracked().saturating_add(50);
        expanding.set(true);
        tasks.spawn(async move {
            let snapshot = queries
                .recipient_results
                .fetch((campaign, recipient, count))
                .await;
            if let Some(error) = snapshot.error {
                if error.is_terminal() {
                    queries
                        .recipient_results
                        .forget_where(|(id, person, _)| *id == campaign && *person == recipient);
                }
                message.error(error.message);
            } else if snapshot.data.is_some() {
                visible.set(count);
            }
            expanding.set(false);
        });
    });
    let campaign_key = StoredValue::new(campaign.to_string());
    let export_filter = Signal::derive(move || Filter {
        recipient: Some(recipient),
        event_order: ruphish_domain::reporting::SortDirection::Asc,
        ..Default::default()
    });
    let content = view! {
        <ManagementPage>
            <BackLink href=back>{tr!(ui.back_to_campaign_results)}</BackLink>
            <PageHeader
                title=Signal::derive(move || {
                    person
                        .get()
                        .map(|r| if r.name.trim().is_empty() { r.email } else { r.name })
                        .unwrap_or_else(|| leptos_i18n::t_string!(i18n, ui.recipient).into())
                })
                subtitle=Signal::derive(move || {
                    person
                        .get()
                        .filter(|r| !r.name.trim().is_empty())
                        .map(|r| r.email)
                        .unwrap_or_default()
                })
            >
                <RefreshButton
                    fetching=fetching
                    on_refresh=Callback::new(move |_| {
                        read.refresh.run(());
                        context.refresh.run(());
                    })
                />
                <crate::campaign_results::ExportMenu
                    api=api
                    campaign=campaign_key
                    filter=export_filter
                    ready=loaded
                    message=message
                />
            </PageHeader>
            {message.view()}
            <QueryBoundary
                fetching=fetching
                has_data=Signal::derive(move || loaded.get() && context.data.with(Option::is_some))
                error=Signal::derive(move || read.error.get().or_else(|| context.error.get()))
                noun=trs!(ui.recipient)
                refresh=Callback::new(move |_| {
                    read.refresh.run(());
                    context.refresh.run(());
                })
            >
                <Show when=move || loaded.get()>
                    <div class="recipient-context">
                        <span>
                            {tr!(ui.campaign)}": "
                            {move || data.with(|v| v.as_ref().map(|v| v.name.clone()))}
                        </span>
                        <span>
                            {move || {
                                person
                                    .get()
                                    .map(|r| ruphish_ui::localization::delivery_label(
                                        i18n,
                                        r.delivery.map_or("historical", |s| s.as_str()),
                                    ))
                            }}
                        </span>
                    </div>
                    <Card class="recipient-outcome-summary" label=trs!(ui.recipient_outcomes)>
                        <dl class="recipient-outcomes">
                            {crate::results_chart::labels()
                                .into_iter()
                                .enumerate()
                                .skip(1)
                                .map(move |(index, label)| {
                                    view! {
                                        <div>
                                            <dt>{move || label.get()}</dt>
                                            <dd class:outcome-observed=move || {
                                                data.with(|v| {
                                                    v.as_ref().is_some_and(|v| v.totals.values()[index] > 0)
                                                })
                                            }>
                                                {move || {
                                                    let count = data
                                                        .with(|v| {
                                                            v.as_ref().map_or(0, |v| v.totals.values()[index])
                                                        });
                                                    if count > 0 {
                                                        format!("{count}×")
                                                    } else if index == 4
                                                        && person.get().is_some_and(|r| r.historical_reported)
                                                    {
                                                        leptos_i18n::t_string!(i18n, ui.recipient_historical_report)
                                                            .into()
                                                    } else {
                                                        leptos_i18n::t_string!(i18n, ui.no_observation).into()
                                                    }
                                                }}
                                            </dd>
                                        </div>
                                    }
                                })
                                .collect_view()}
                        </dl>
                        <p>{tr!(ui.recipient_observation_counts)}</p>
                        <p class="recipient-reaction-time">
                            {move || data.with(|v| v.as_ref().map(reaction_time))}
                        </p>
                    </Card>
                    <Card class="recipient-message-context" label=trs!(ui.message_context)>
                        <h2>{tr!(ui.message_context)}</h2>
                        <p class="analysis-context">{tr!(ui.current_message_context)}</p>

                        <Show when=move || context.data.with(Option::is_some)>
                            <dl class="recipient-context-facts">
                                <div>
                                    <dt>{tr!(ui.email_template)}</dt>
                                    <dd>
                                        {move || {
                                            context
                                                .data
                                                .with(|v| v.as_ref().and_then(|v| v.template.name.clone()))
                                                .filter(|v| !v.is_empty())
                                                .unwrap_or_else(|| {
                                                    leptos_i18n::t_string!(i18n, ui.unavailable).into()
                                                })
                                        }}
                                    </dd>
                                </div>
                                <div>
                                    <dt>{tr!(ui.landing_page)}</dt>
                                    <dd>
                                        {move || {
                                            context
                                                .data
                                                .with(|v| v.as_ref().and_then(|v| v.page.name.clone()))
                                                .filter(|v| !v.is_empty())
                                                .unwrap_or_else(|| {
                                                    leptos_i18n::t_string!(i18n, ui.unavailable).into()
                                                })
                                        }}
                                    </dd>
                                </div>
                                <div>
                                    <dt>{tr!(ui.subject)}</dt>
                                    <dd>
                                        {move || {
                                            context
                                                .data
                                                .with(|v| v.as_ref().and_then(|v| v.subject.clone()))
                                                .filter(|v| !v.is_empty())
                                                .unwrap_or_else(|| {
                                                    leptos_i18n::t_string!(i18n, ui.unavailable).into()
                                                })
                                        }}
                                    </dd>
                                </div>
                                <div>
                                    <dt>{tr!(ui.sender_address)}</dt>
                                    <dd>
                                        {move || {
                                            context
                                                .data
                                                .with(|v| {
                                                    v.as_ref()
                                                        .and_then(|v| {
                                                            v.template_sender
                                                                .clone()
                                                                .filter(|v| !v.trim().is_empty())
                                                                .or_else(|| v.profile_sender.clone())
                                                        })
                                                })
                                                .filter(|v| !v.is_empty())
                                                .unwrap_or_else(|| {
                                                    leptos_i18n::t_string!(i18n, ui.unavailable).into()
                                                })
                                        }}
                                    </dd>
                                </div>
                            </dl>
                        </Show>

                    </Card>
                    <Card label=trs!(ui.recipient_history)>
                        <div class="recipient-history-heading">
                            <h2>{tr!(ui.recipient_history)}</h2>
                            <span>
                                {move || {
                                    leptos_i18n::t_string!(
                                        i18n, messages.times_shown_in_arg0, arg0 = crate::time_display::zone_name()
                                    )
                                }}
                            </span>
                        </div>
                        <EventHistory data=data />
                        <Show when=move || {
                            data.with(|v| v.as_ref().is_some_and(|v| v.events.is_empty()))
                        }>
                            <EmptyState title=trs!(ui.no_recipient_activity)>
                                <p>{tr!(ui.new_events_appear_here)}</p>
                            </EmptyState>
                        </Show>
                        <Show when=move || {
                            data.with(|v| {
                                v.as_ref().is_some_and(|v| v.event_total > visible.get() as i64)
                            })
                        }>
                            <FormFooter>
                                <ActionButton disabled=fetching on_click=more>
                                    {tr!(ui.load_more_events)}
                                </ActionButton>
                            </FormFooter>
                        </Show>
                    </Card>
                </Show>
            </QueryBoundary>
        </ManagementPage>
    };
    view! {
        <ReadBoundary
            failure=read.failure
            has_data=loaded
            fetching=read.fetching
            retry=read.refresh
            back_href=back
            back_label=trs!(ui.back_to_campaign_results)
        >
            {content}
        </ReadBoundary>
    }
}

#[component]
pub(crate) fn EventHistory(data: Signal<Option<Results>>) -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();
    view! {
        <div class="recipient-observations">
            {move || {
                data.with(|v| {
                    v.as_ref()
                        .map(|v| {
                            v.events
                                .iter()
                                .map(|event| {
                                    let label = ruphish_ui::localization::event_label(
                                        i18n,
                                        event.kind.label(),
                                    );
                                    view! {
                                        <article class="recipient-observation">
                                            <time>{instant(&event.time.clone().into())}</time>
                                            <div>
                                                <strong>{label}</strong>
                                                {matches!(
                                                    event.kind,
                                                    ruphish_domain::events::RecordedKind::Known(
                                                        ruphish_domain::events::Kind::FormSubmitted,
                                                    )
                                                )
                                                    .then(|| {
                                                        view! {
                                                            <SubmissionFields
                                                                username=Signal::stored(event.username_present)
                                                                password=Signal::stored(event.password_present)
                                                            />
                                                        }
                                                    })}
                                                <div class="timeline-labels">
                                                    <span>
                                                        {if event.source
                                                            == ruphish_domain::reporting::Source::Operator
                                                        {
                                                            leptos_i18n::t_string!(i18n,ui.event_operator)
                                                        } else {
                                                            leptos_i18n::t_string!(i18n,ui.event_system)
                                                        }}
                                                    </span>
                                                    {(event.edited)
                                                        .then(|| view! { <span>{tr!(ui.event_edited)}</span> })}
                                                </div>
                                            </div>
                                        </article>
                                    }
                                })
                                .collect_view()
                        })
                })
            }}
        </div>
    }
}

fn reaction_time(results: &Results) -> String {
    use ruphish_ui::results_state::{InteractionTiming, first_interaction};
    let i18n = ruphish_ui::localization::use_i18n();
    match first_interaction(results.first_observed.values(), results.totals.values()) {
        InteractionTiming::NotObserved => {
            leptos_i18n::t_string!(i18n, ui.no_interaction_observed).into()
        }
        InteractionTiming::Unavailable => {
            leptos_i18n::t_string!(i18n, ui.interaction_time_unavailable).into()
        }
        InteractionTiming::AfterAcceptance(seconds) => {
            let duration = if seconds < 60 {
                leptos_i18n::t_string!(i18n, ui.less_than_a_minute).to_owned()
            } else if seconds < 3600 {
                format!("{} min", seconds / 60)
            } else {
                format!("{} h {} min", seconds / 3600, seconds % 3600 / 60)
            };
            leptos_i18n::t_string!(
                i18n,
                messages.first_interaction_after_acceptance,
                duration = duration
            )
        }
    }
}
