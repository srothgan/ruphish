//! Home dashboard: two independent reads, one existing administrative shell.
use crate::{
    overview_queries::OverviewQueries, shared::components::*, time_display::instant as minute,
};
use leptos::prelude::*;
use ruphish_contracts::overview::Snapshot;
use ruphish_domain::reporting::SortDirection;
use ruphish_ui::overview_state::percentage;
use ruphish_ui::{tr, trs};

pub(super) fn page() -> impl IntoView {
    let queries = expect_context::<OverviewQueries>();
    let snapshot = queries.snapshot.observe(|| ());
    let snapshot_data = snapshot.data;
    let drafts = expect_context::<crate::campaign_queries::CampaignQueries>()
        .directory
        .observe(|| ("drafts".into(), String::new(), "name".into(), "asc".into()));
    let can_create =
        use_context::<crate::resources::ResourceAccess>().is_some_and(|access| access.0);
    view! {
        <ManagementPage>
            <PageHeader title=trs!(ui.dashboard)>
                <RefreshButton
                    fetching=Signal::derive(move || {
                        snapshot.fetching.get() || drafts.fetching.get()
                    })
                    on_refresh=Callback::new(move |_| {
                        snapshot.refresh.run(());
                        drafts.refresh.run(());
                    })
                />
                <ActionLink style=ButtonStyle::Primary href="/campaigns/new" hidden=!can_create>
                    {tr!(ui.create_campaign)}
                </ActionLink>
            </PageHeader>
            <div class="dashboard-stack">

                <QueryBoundary
                    fetching=snapshot.fetching
                    has_data=Signal::derive(move || snapshot_data.with(Option::is_some))
                    error=snapshot.error
                    noun=trs!(ui.dashboard)
                    refresh=snapshot.refresh
                >
                    <div class="dashboard-stack">
                        {move || snapshot_data.get().map(snapshot_sections)}
                        <Show when=move || {
                            drafts.error.get().is_some()
                                || drafts
                                    .data
                                    .with(|v| v.as_ref().is_some_and(|v| !v.items.is_empty()))
                        }>
                            <Card label=trs!(ui.drafts)>
                                <h2>{tr!(ui.drafts)}</h2>
                                <QueryBoundary
                                    fetching=drafts.fetching
                                    has_data=Signal::derive(move || {
                                        drafts.data.with(Option::is_some)
                                    })
                                    error=drafts.error
                                    noun=trs!(ui.drafts)
                                    refresh=drafts.refresh
                                >
                                    <ul>
                                        {move || {
                                            drafts
                                                .data
                                                .get()
                                                .map(|page| {
                                                    page.items
                                                        .into_iter()
                                                        .take(5)
                                                        .map(|draft| {
                                                            view! {
                                                                <li>
                                                                    <a href=format!(
                                                                        "/campaigns/drafts/{}",
                                                                        draft.id,
                                                                    )>{draft.name}</a>
                                                                </li>
                                                            }
                                                        })
                                                        .collect_view()
                                                })
                                        }}
                                    </ul>
                                    <Show when=move || {
                                        drafts
                                            .data
                                            .with(|page| {
                                                page.as_ref().is_some_and(|page| page.items.is_empty())
                                            })
                                    }>
                                        <p>{tr!(ui.no_drafts)}</p>
                                    </Show>
                                    <ActionLink href="/campaigns?filter=drafts">
                                        {tr!(ui.campaigns)}
                                    </ActionLink>
                                </QueryBoundary>
                            </Card>
                        </Show>
                    </div>
                </QueryBoundary>
            </div>
        </ManagementPage>
    }
}

fn snapshot_sections(s: Snapshot) -> impl IntoView {
    let sorting = TableSorting::new("recent-campaigns", "name", SortDirection::Asc);
    let api = expect_context::<crate::client::Api>();
    let i18n = ruphish_ui::localization::use_i18n();

    let no_campaigns = s.recent.is_empty();
    let inventory = s.inventory;
    let attention = s.attention;
    let clear = attention.unverified_enabled_domains == 0 && attention.delivery_problems == 0;
    view! {
        <div class="dashboard-work">
            <Card class="dashboard-campaigns" label=trs!(ui.recent_campaigns)>
                <div class="dashboard-section-heading">
                    <h2>
                        <a href="/campaigns">{tr!(ui.recent_campaigns)}</a>
                    </h2>
                </div>
                <DataTable
                    sorting=sorting
                    columns=vec!["name", "recipients", "clicked", "reported", ""]
                    label=trs!(ui.recent_campaigns)
                    headers=vec![
                        trs!(ui.campaign),
                        trs!(ui.recipients),
                        trs!(ui.clicked),
                        trs!(ui.reported),
                        trs!(ui.actions),
                    ]
                    empty=no_campaigns
                >
                    {move || {
                        ruphish_ui::overview_state::recent_campaigns(
                                &s.recent,
                                &sorting.current.get(),
                            )
                            .into_iter()
                            .map(|c| {
                                let section = crate::campaigns::Section::landing(c.status);
                                let action_href = format!("/campaigns/{}/{}", c.id, section.path());
                                let action_label = if section == crate::campaigns::Section::Results
                                {
                                    leptos_i18n::t_string!(i18n, ui.results)
                                } else {
                                    leptos_i18n::t_string!(i18n, ui.delivery)
                                };
                                view! {
                                    <tr>
                                        <th scope="row">
                                            <a href=action_href.clone()>{c.name}</a>
                                            <span class="dashboard-campaign-meta">
                                                {ruphish_ui::localization::event_label(
                                                    i18n,
                                                    c.status.as_str(),
                                                )}
                                            </span>
                                            {c
                                                .launch
                                                .map(|launch| {
                                                    view! {
                                                        <span class="dashboard-campaign-meta">
                                                            {leptos_i18n::t!(
                                                                i18n, messages.launch_arg0, arg0 = (minute(&launch.into())).to_string()
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
                            .collect_view()
                    }}
                </DataTable>
                {no_campaigns
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
                                    {attention
                                        .campaign_ids
                                        .iter()
                                        .copied()
                                        .map(|id| {
                                            view! {
                                                <a href=format!(
                                                    "/campaigns/{id}/delivery",
                                                )>{tr!(ui.campaign)}" #"{id}</a>
                                            }
                                        })
                                        .collect_view()}
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
                                <a href=format!("/campaigns/{}/delivery", n.id)>{n.name}</a>
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
