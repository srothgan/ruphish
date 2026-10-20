//! Home dashboard: two independent reads, one existing administrative shell.
use crate::{account::Api, analysis_chart::minute, feedback::Message, shared::components::*};
use leptos::prelude::*;
use ruphish_contracts::overview::{PostureResponse, Snapshot};
use ruphish_ui::{
    overview_state::{ChangeGoal, ReadState, count_change, percentage},
    view_tasks::ViewTasks,
};

const METRICS_HELP: &str = "The period selects recipients whose first recorded email send falls within that period. Each count includes each recipient once per campaign, even after repeated interactions. Percentages divide these counts by emails sent. Changes compare totals with the preceding equal-length period (7, 30 or 365 days), counting each period's observations up to its own end. An increase is more recipients, not a percentage change. All time has no preceding period to compare. A dash means no emails were sent for a percentage to be calculated.";

fn read<T: serde::de::DeserializeOwned + Clone + Send + Sync + 'static>(
    api: Api,
    tasks: ViewTasks,
    state: RwSignal<ReadState<T>>,
    message: Message,
    key: &str,
    path: String,
) {
    let Some(request) = state.try_update(|state| state.begin(key)) else {
        return;
    };
    message.clear();
    tasks.spawn(async move {
        let result = api.get(&path).await.and_then(|value| {
            serde_json::from_value(value)
                .map_err(|_| "The dashboard response could not be read. Try refreshing.".into())
        });
        if let Some(true) = state.try_update(|state| state.finish(request, result)) {
            if let Some(error) = state.with_untracked(|state| state.error.clone()) {
                message.error(error);
            } else {
                message.clear();
            }
        }
    });
}

pub(super) fn page(api: Api) -> impl IntoView {
    let snapshot = RwSignal::new(ReadState::<Snapshot>::default());
    let posture = RwSignal::new(ReadState::<PostureResponse>::default());
    let snapshot_data = Memo::new(move |_| snapshot.with(|s| s.data.clone()));
    let posture_data = Memo::new(move |_| posture.with(|s| s.data.clone()));
    let snapshot_refresh = RwSignal::new(0u64);
    let posture_refresh = RwSignal::new(0u64);
    let window = RwSignal::new("30d".to_owned());
    let snapshot_message = Message::new(String::new());
    let posture_message = Message::new(String::new());
    let tasks = ViewTasks::default();
    let can_create =
        use_context::<crate::resources::ResourceAccess>().is_some_and(|access| access.0);
    Effect::new(move |_| {
        snapshot_refresh.get();
        read(
            api,
            tasks,
            snapshot,
            snapshot_message,
            "snapshot",
            "/api/overview".into(),
        );
    });
    Effect::new(move |_| {
        posture_refresh.get();
        let selected = window.get();
        read(
            api,
            tasks,
            posture,
            posture_message,
            &selected,
            format!("/api/overview/posture?window={selected}"),
        );
    });
    view! {<ManagementPage>
        <PageHeader title="Dashboard" subtitle="Campaign activity, observed outcomes and the resources behind them.">
            <ActionButton disabled=Signal::derive(move||snapshot.with(|s|s.loading)||posture.with(|s|s.loading)) on_click=move |_| {snapshot_refresh.update(|v|*v+=1);posture_refresh.update(|v|*v+=1);}>
                {move||if snapshot.with(|s|s.loading)||posture.with(|s|s.loading){"Refreshing..."}else{"Refresh"}}
            </ActionButton>
            <ActionLink style=ButtonStyle::Primary href="/campaigns/new" hidden=!can_create>"Create campaign"</ActionLink>
        </PageHeader>
        <div class="dashboard-stack">
            <Card class="dashboard-posture" label="Analysis overview">
                <div class="dashboard-section-heading">
                    <div><div class="dashboard-title"><h2><a href="/analysis">"Analysis overview"</a></h2><InfoPopover id="dashboard-metrics-help".to_owned() label="Dashboard counts and comparisons" explanation=METRICS_HELP/></div>
                    <p>"Outcomes for emails sent in the selected period."</p></div>
                    <Select label="Period" value=window on_change=Callback::new(move |value|window.set(value))>
                        <option value="all">"All time"</option><option value="365d">"Last year"</option><option value="30d">"Last 30 days"</option><option value="7d">"Last 7 days"</option>
                    </Select>
                </div>
                {posture_message.view()}
                <Show when=move||posture.with(|s|s.loading)&&posture_data.get().is_none()><LoadingState label="Updating analysis overview"/></Show>
                {move||posture_data.get().map(|p| {
                    let counts=p.counts;
                    let previous=p.previous.map(|previous|previous.counts);
                    let metrics = [
                        ("Emails sent", counts.delivered, None, false, ChangeGoal::Neutral),
                        ("Opened", counts.opened, previous.map(|p|p.opened), true, ChangeGoal::Decrease),
                        ("Clicked", counts.clicked, previous.map(|p|p.clicked), true, ChangeGoal::Decrease),
                        ("Reported", counts.reported, previous.map(|p|p.reported), true, ChangeGoal::Increase),
                        ("Credentials entered", counts.credentials_entered, previous.map(|p|p.credentials_entered), true, ChangeGoal::Decrease),
                    ];
                    let comparison = match p.window.as_str() {
                        "7d" => "Changes compare with the previous 7 days. ",
                        "30d" => "Changes compare with the previous 30 days. ",
                        "365d" => "Changes compare with the previous 365 days. ",
                        _ => "",
                    };
                    view! {<dl class="dashboard-metrics">
                        {metrics.into_iter().map(|(label,count,prior,rate,goal)|view!{<div><dt>{label}</dt>
                            <dd><span class="dashboard-metric-value">{count}{prior.map(|prior|view!{<small class=format!("dashboard-change {}",goal.tone(count,prior)) title=format!("{}: {} compared with the previous period",goal.tone(count,prior),count_change(count,prior))>{count_change(count,prior)}</small>})}</span>
                            {rate.then(||view!{<small>{format!("{} of emails sent",percentage(count,counts.delivered))}</small>})}</dd>
                        </div>}).collect_view()}
                    </dl><p class="dashboard-updated">{format!("{comparison}Observed through {}",minute(&p.as_of.into()))}</p>}

                })}
                <Show when=move||posture.with(|s|s.error.is_some()&&!s.loading)><ActionButton on_click=move |_|posture_refresh.update(|v|*v+=1)>"Retry analysis overview"</ActionButton></Show>
            </Card>
            {snapshot_message.view()}
            <Show when=move||snapshot.with(|s|s.loading)&&snapshot_data.get().is_none()><Card><LoadingState label="Loading campaigns and resources"/></Card></Show>
            <Show when=move||snapshot.with(|s|s.error.is_some()&&!s.loading)><div><ActionButton on_click=move |_|snapshot_refresh.update(|v|*v+=1)>"Retry campaigns and resources"</ActionButton></div></Show>
            {move||snapshot_data.get().map(snapshot_sections)}
        </div>
    </ManagementPage>}
}

fn snapshot_sections(s: Snapshot) -> impl IntoView {
    let inventory = s.inventory;
    let attention = s.attention;
    let clear = attention.unverified_enabled_domains == 0 && attention.delivery_problems == 0;
    view! {
        <div class="dashboard-work">
            <Card class="dashboard-campaigns">
                <div class="dashboard-section-heading"><h2><a href="/campaigns">"Recent campaigns"</a></h2></div>
                <dl class="dashboard-lifecycle">
                    <div><dt>"Active"</dt><dd>{s.campaigns.active}</dd></div>
                    <div><dt>"Scheduled"</dt><dd>{s.campaigns.scheduled}</dd></div>
                    <div><dt>"Completed"</dt><dd>{s.campaigns.completed}</dd></div>
                    <div><dt>"Total"</dt><dd>{s.campaigns.total}</dd></div>
                </dl>
                <DataTable label="Recent campaigns" headers=vec!["Campaign","Recipients","Clicked","Reported","Actions"] empty=s.recent.is_empty()>
                    {s.recent.into_iter().map(|c| {
                        let (action_href, action_label) = if c.status == "Completed" {
                            (format!("/analysis/{}",c.id), "Analysis")
                        } else {
                            (format!("/campaigns/{}",c.id), "Campaign")
                        };
                        view!{<tr>
                        <th scope="row"><a href=format!("/campaigns/{}",c.id)>{c.name}</a><span class="dashboard-campaign-meta">{c.status}</span>{(!c.launch.is_empty()&&!c.launch.starts_with("0001-")).then(||view!{<span class="dashboard-campaign-meta">{format!("Launch {}",minute(&c.launch.into()))}</span>})}</th>
                        <td class="data-value">{c.recipients}</td><td class="data-value">{percentage(c.outcomes.clicked,c.outcomes.delivered)}</td><td class="data-value">{percentage(c.outcomes.reported,c.outcomes.delivered)}</td>
                        <td><ActionLink href=action_href size=ControlSize::Small>{action_label}</ActionLink></td>
                    </tr>}}).collect_view()}
                </DataTable>
                {(s.campaigns.total==0).then(||view!{<EmptyState title="No campaigns yet"><p>"Prepare your resources, then create a campaign to begin."</p></EmptyState>})}
                <p class="dashboard-updated">"Recent campaign rates cover all recorded sends, independently of the period above."</p>
            </Card>
            <Card class="dashboard-attention"><h2>"Needs attention"</h2>
                {clear.then(||view!{<p>"No domain verification or delivery issues need attention."</p>})}
                <ul class="dashboard-attention-list">
                    {(attention.unverified_enabled_domains>0).then(||view!{<li><StatusBadge tone=StatusTone::Warning>"Verification needed"</StatusBadge><p>{format!("{} enabled domains are not verified.",attention.unverified_enabled_domains)}</p><a href="/domains">"Review domains"</a></li>})}
                    {(attention.delivery_problems>0).then(||view!{<li><StatusBadge tone=StatusTone::Warning>"Delivery needs review"</StatusBadge><p>{format!("{} failed, blocked or uncertain deliveries in unfinished campaigns.",attention.delivery_problems)}</p><a href="/campaigns">"Review campaigns"</a></li>})}
                </ul>
                {attention.next_launch.map(|n|view!{<div class="dashboard-next"><h3>"Next scheduled launch"</h3><a href=format!("/campaigns/{}",n.id)>{n.name}</a><p>{minute(&n.launch.into())}</p></div>})}
            </Card>
        </div>
        <Card class="dashboard-resources"><h2>"Resources"</h2>
            <ul class="dashboard-inventory">
                {[("Domains","/domains",inventory.domains),("Email templates","/templates",inventory.templates),("Landing pages","/pages",inventory.pages),("Recipient groups","/groups",inventory.groups),("Reporting mailboxes","/reporting-mailboxes",inventory.reporting_mailboxes),("Sending profiles","/smtp",inventory.profiles)].into_iter().map(|(label,href,count)|view!{<li><strong class="data-value">{count}</strong><a href=href>{label}</a></li>}).collect_view()}
            </ul>
            <p class="dashboard-updated">{format!("{} of {} domains verified. Updated {}.",inventory.domains_verified,inventory.domains,minute(&s.as_of.into()))}</p>
        </Card>
    }
}
