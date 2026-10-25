//! Home dashboard: two independent reads, one existing administrative shell.
use crate::{
    overview_queries::OverviewQueries, shared::components::*, time_display::instant as minute,
};
use leptos::prelude::*;
use ruphish_contracts::overview::Snapshot;
use ruphish_ui::overview_state::{ChangeGoal, count_change, percentage};
use ruphish_ui::{tr, trs};

pub(super) fn page() -> impl IntoView {
    let i18n = ruphish_ui::localization::use_i18n();

    let change = move |current, previous| {
        if current == previous {
            leptos_i18n::t_string!(i18n, ui.no_change).to_owned()
        } else {
            count_change(current, previous)
        }
    };

    let queries = expect_context::<OverviewQueries>();
    let window = RwSignal::new("30d".to_owned());
    let snapshot = queries.snapshot.observe(|| ());
    let posture = queries.posture.observe(move || window.get());
    let snapshot_data = snapshot.data;
    let posture_data = posture.data;
    let can_create =
        use_context::<crate::resources::ResourceAccess>().is_some_and(|access| access.0);
    view! {
        <ManagementPage>
            <PageHeader
                title=trs!(ui.dashboard)
                subtitle=trs!(ui.campaign_activity_recipient_outcomes_and_the_resources_behind_them)
            >
                <ActionButton
                    disabled=Signal::derive(move || {
                        snapshot.fetching.get() || posture.fetching.get()
                    })
                    on_click=move |_| {
                        snapshot.refresh.run(());
                        posture.refresh.run(());
                    }
                >
                    {move || {
                        if snapshot.fetching.get() || posture.fetching.get() {
                            leptos_i18n::t_string!(i18n, ui.refreshing_2)
                        } else {
                            leptos_i18n::t_string!(i18n, ui.refresh)
                        }
                    }}
                </ActionButton>
                <ActionLink style=ButtonStyle::Primary href="/campaigns/new" hidden=!can_create>
                    {tr!(ui.create_campaign)}
                </ActionLink>
            </PageHeader>
            <div class="dashboard-stack">
                <Card class="dashboard-posture" label=trs!(ui.analysis_overview)>
                    <div class="dashboard-section-heading">
                        <div>
                            <div class="dashboard-title">
                                <h2>
                                    <a href="/analysis">{tr!(ui.analysis_overview)}</a>
                                </h2>
                                <InfoPopover
                                    id="dashboard-metrics-help".to_owned()
                                    label=trs!(ui.dashboard_counts_and_comparisons)
                                    explanation=trs!(
                                        ui.the_period_selects_recipients_whose_first_recorded_email_send_falls_within_that
                                    )
                                />
                            </div>
                            <p>{tr!(ui.outcomes_for_emails_sent_in_the_selected_period)}</p>
                        </div>
                        <Select
                            label=trs!(ui.period)
                            value=window
                            on_change=Callback::new(move |value| window.set(value))
                        >
                            <option value="all">{tr!(ui.all_time)}</option>
                            <option value="365d">{tr!(ui.last_year)}</option>
                            <option value="30d">{tr!(ui.last_30_days)}</option>
                            <option value="7d">{tr!(ui.last_7_days)}</option>
                        </Select>
                    </div>
                    <QueryStatus
                        fetching=posture.fetching
                        has_data=Signal::derive(move || posture_data.with(Option::is_some))
                        error=posture.error
                        noun=trs!(ui.analysis_overview)
                        refresh=posture.refresh
                    />
                    {move || {
                        posture_data
                            .get()
                            .map(|p| {
                                let counts = p.counts;
                                let previous = p.previous.map(|previous| previous.counts);
                                let metrics = [
                                    (
                                        trs!(ui.emails_sent),
                                        counts.delivered,
                                        None,
                                        false,
                                        ChangeGoal::Neutral,
                                    ),
                                    (
                                        trs!(ui.opened),
                                        counts.opened,
                                        previous.map(|p| p.opened),
                                        true,
                                        ChangeGoal::Decrease,
                                    ),
                                    (
                                        trs!(ui.clicked),
                                        counts.clicked,
                                        previous.map(|p| p.clicked),
                                        true,
                                        ChangeGoal::Decrease,
                                    ),
                                    (
                                        trs!(ui.credentials_entered),
                                        counts.credentials_entered,
                                        previous.map(|p| p.credentials_entered),
                                        true,
                                        ChangeGoal::Decrease,
                                    ),
                                    (
                                        trs!(ui.reported),
                                        counts.reported,
                                        previous.map(|p| p.reported),
                                        true,
                                        ChangeGoal::Increase,
                                    ),
                                ];
                                let comparison = match p.window.as_str() {
                                    "7d" => {
                                        leptos_i18n::t_string!(
                                            i18n, ui.changes_compare_with_the_previous_7_days
                                        )
                                    }
                                    "30d" => {
                                        leptos_i18n::t_string!(
                                            i18n, ui.changes_compare_with_the_previous_30_days
                                        )
                                    }
                                    "365d" => {
                                        leptos_i18n::t_string!(
                                            i18n, ui.changes_compare_with_the_previous_365_days
                                        )
                                    }
                                    _ => "",
                                };
                                view! {
                                    <dl class="dashboard-metrics">
                                        {metrics
                                            .into_iter()
                                            .map(|(label, count, prior, rate, goal)| {
                                                view! {
                                                    <div>
                                                        <dt>{move || label.get()}</dt>
                                                        <dd>
                                                            <span class="dashboard-metric-value">
                                                                {count}
                                                                {prior
                                                                    .map(|prior| {
                                                                        view! {
                                                                            <small
                                                                                class=format!(
                                                                                    "dashboard-change {}",
                                                                                    goal.tone(count, prior),
                                                                                )
                                                                                title=leptos_i18n::t_string!(
                                                                                    i18n, messages.arg0_arg1_compared_with_the_previous_period, arg0 = ruphish_ui::localization::trend_label(i18n, goal.tone(count,prior)), arg1 = (change(count,prior)).to_string()
                                                                                )
                                                                            >
                                                                                {change(count, prior)}
                                                                            </small>
                                                                        }
                                                                    })}
                                                            </span>
                                                            {rate
                                                                .then(|| {
                                                                    view! {
                                                                        <small>
                                                                            {leptos_i18n::t!(
                                                                                i18n, messages.arg0_of_emails_sent, arg0 = (percentage(count,counts.delivered)).to_string()
                                                                            )}
                                                                        </small>
                                                                    }
                                                                })}
                                                        </dd>
                                                    </div>
                                                }
                                            })
                                            .collect_view()}
                                    </dl>
                                    <p class="dashboard-updated">
                                        {leptos_i18n::t!(
                                            i18n, messages.arg0_observed_through_arg1, arg0 = (comparison).to_string(), arg1 = (minute(&p.as_of.into())).to_string()
                                        )}
                                    </p>
                                }
                            })
                    }}
                </Card>
                <QueryStatus
                    fetching=snapshot.fetching
                    has_data=Signal::derive(move || snapshot_data.with(Option::is_some))
                    error=snapshot.error
                    noun=trs!(ui.dashboard)
                    refresh=snapshot.refresh
                />
                {move || snapshot_data.get().map(snapshot_sections)}
            </div>
        </ManagementPage>
    }
}

fn snapshot_sections(s: Snapshot) -> impl IntoView {
    let api = expect_context::<crate::client::Api>();
    let i18n = ruphish_ui::localization::use_i18n();

    let inventory = s.inventory;
    let attention = s.attention;
    let clear = attention.unverified_enabled_domains == 0 && attention.delivery_problems == 0;
    view! {
        <div class="dashboard-work">
            <Card class="dashboard-campaigns" label=trs!(ui.recent_campaigns) attr:tabindex="0">
                <div class="dashboard-section-heading">
                    <h2>
                        <a href="/campaigns">{tr!(ui.recent_campaigns)}</a>
                    </h2>
                </div>
                <dl class="dashboard-lifecycle">
                    <div>
                        <dt>{tr!(ui.active)}</dt>
                        <dd>{s.campaigns.active}</dd>
                    </div>
                    <div>
                        <dt>{tr!(ui.scheduled)}</dt>
                        <dd>{s.campaigns.scheduled}</dd>
                    </div>
                    <div>
                        <dt>{tr!(ui.completed)}</dt>
                        <dd>{s.campaigns.completed}</dd>
                    </div>
                    <div>
                        <dt>{tr!(ui.total)}</dt>
                        <dd>{s.campaigns.total}</dd>
                    </div>
                </dl>
                <DataTable
                    label=trs!(ui.recent_campaigns)
                    headers=vec![
                        trs!(ui.campaign),
                        trs!(ui.recipients),
                        trs!(ui.clicked),
                        trs!(ui.reported),
                        trs!(ui.actions),
                    ]
                    empty=s.recent.is_empty()
                >
                    {s
                        .recent
                        .into_iter()
                        .take(3)
                        .map(|c| {
                            let (action_href, action_label) = if c.status == "Completed" {
                                (
                                    format!("/analysis/{}", c.id),
                                    leptos_i18n::t_string!(i18n, ui.analysis),
                                )
                            } else {
                                (
                                    format!("/campaigns/{}", c.id),
                                    leptos_i18n::t_string!(i18n, ui.campaign),
                                )
                            };
                            view! {
                                <tr>
                                    <th scope="row">
                                        <a href=format!("/campaigns/{}", c.id)>{c.name}</a>
                                        <span class="dashboard-campaign-meta">
                                            {ruphish_ui::localization::event_label(i18n, &c.status)}
                                        </span>
                                        {(!c.launch.is_empty() && !c.launch.starts_with("0001-"))
                                            .then(|| {
                                                view! {
                                                    <span class="dashboard-campaign-meta">
                                                        {leptos_i18n::t!(
                                                            i18n, messages.launch_arg0, arg0 = (minute(&c.launch.into())).to_string()
                                                        )}
                                                    </span>
                                                }
                                            })}
                                    </th>
                                    <td class="data-value">{c.recipients}</td>
                                    <td class="data-value">
                                        {percentage(c.outcomes.clicked, c.outcomes.delivered)}
                                    </td>
                                    <td class="data-value">
                                        {percentage(c.outcomes.reported, c.outcomes.delivered)}
                                    </td>
                                    <td>
                                        <ActionLink href=action_href size=ControlSize::Small>
                                            {action_label}
                                        </ActionLink>
                                    </td>
                                </tr>
                            }
                        })
                        .collect_view()}
                </DataTable>
                {(s.campaigns.total == 0)
                    .then(|| {
                        view! {
                            <EmptyState title=trs!(ui.no_campaigns_yet)>
                                <p>
                                    {tr!(ui.prepare_your_resources_then_create_a_campaign_to_begin)}
                                </p>
                            </EmptyState>
                        }
                    })}
                <p class="dashboard-updated">
                    {tr!(
                        ui.recent_campaign_rates_cover_all_recorded_sends_independently_of_the_period_above
                    )}
                </p>
            </Card>
            <Card class="dashboard-attention">
                <h2>{tr!(ui.needs_attention)}</h2>
                {clear
                    .then(|| {
                        view! {
                            <p>
                                {tr!(ui.no_domain_verification_or_delivery_issues_need_attention)}
                            </p>
                        }
                    })}
                <ul class="dashboard-attention-list">
                    {(attention.unverified_enabled_domains > 0)
                        .then(|| {
                            view! {
                                <li>
                                    <StatusBadge tone=StatusTone::Warning>
                                        {tr!(ui.verification_needed)}
                                    </StatusBadge>
                                    <p>
                                        {leptos_i18n::t!(
                                            i18n, messages.arg0_enabled_domains_are_not_verified, arg0 = (attention.unverified_enabled_domains).to_string()
                                        )}
                                    </p>
                                    <a href="/domains">{tr!(ui.review_domains)}</a>
                                </li>
                            }
                        })}
                    {(attention.delivery_problems > 0)
                        .then(|| {
                            view! {
                                <li>
                                    <StatusBadge tone=StatusTone::Warning>
                                        {tr!(ui.delivery_needs_review)}
                                    </StatusBadge>
                                    <p>
                                        {leptos_i18n::t!(
                                            i18n, messages.arg0_failed_blocked_or_uncertain_deliveries_in_unfinished_campaigns, arg0 = (attention.delivery_problems).to_string()
                                        )}
                                    </p>
                                    <a href="/campaigns">{tr!(ui.review_campaigns)}</a>
                                </li>
                            }
                        })}
                </ul>
                {attention
                    .next_launch
                    .map(|n| {
                        view! {
                            <div class="dashboard-next">
                                <h3>{tr!(ui.next_scheduled_launch)}</h3>
                                <a href=format!("/campaigns/{}", n.id)>{n.name}</a>
                                <p>{minute(&n.launch.into())}</p>
                            </div>
                        }
                    })}
            </Card>
        </div>
        <Card class="dashboard-resources">
            <h2>{tr!(ui.resources)}</h2>
            <ul class="dashboard-inventory">
                {[
                    (trs!(ui.domains), "/domains", inventory.domains),
                    (trs!(ui.email_templates), "/templates", inventory.templates),
                    (trs!(ui.landing_pages), "/pages", inventory.pages),
                    (trs!(ui.recipient_groups), "/groups", inventory.groups),
                    (
                        trs!(ui.reporting_mailboxes),
                        "/reporting-mailboxes",
                        inventory.reporting_mailboxes,
                    ),
                    (trs!(ui.sending_profiles), "/smtp", inventory.profiles),
                ]
                    .into_iter()
                    .map(|(label, href, count)| {
                        view! {
                            <li hidden=move || {
                                href == "/reporting-mailboxes"
                                    && !api.features.get().reporting_mailboxes_enabled
                            }>
                                <strong class="data-value">{count}</strong>
                                <a href=href>{label}</a>
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>
            <p class="dashboard-updated" hidden=move || !api.features.get().verification_enabled>
                {leptos_i18n::t!(
                    i18n, messages.arg0_of_arg1_domains_verified_updated_arg2, arg0 = (inventory.domains_verified).to_string(), arg1 = (inventory.domains).to_string(), arg2 = (minute(&s.as_of.into())).to_string()
                )}
            </p>
        </Card>
    }
}
