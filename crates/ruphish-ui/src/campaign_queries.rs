//! Campaign read ownership and mutation dependencies, separate from editable drafts.
use crate::{client::Api, overview_queries::OverviewQueries};
use leptos::prelude::*;
use leptos_fetch::QueryClient;
use ruphish_contracts::{campaign_reads as reads, reporting::Filter};
use ruphish_ui::{api_error::ApiError, query::Query, results_state};
use serde_json::Value;

#[derive(Clone, Debug)]
pub(crate) struct DeliverySnapshot {
    pub value: Value,
}

#[derive(Clone, Copy)]
pub(crate) struct CampaignQueries {
    overview: OverviewQueries,
    pub context: Query<String, reads::Header>,
    pub directory: Query<(String, String, String, String), reads::Directory>,
    pub delivery: Query<(String, String, String, String), DeliverySnapshot>,
    pub results: Query<(i64, String), reads::Results>,
    pub recipient_results: Query<(i64, i64, usize), reads::Results>,
    pub draft: Query<String, Value>,
}

impl CampaignQueries {
    pub fn provide(api: Api, client: QueryClient) {
        provide_context(Self {
            overview: expect_context(),
            context: Query::new(client, move |id: String| async move {
                let value: reads::Header =
                    api.read(&format!("/api/campaigns/{id}/context")).await?;
                if value.id.to_string() != id {
                    return Err(api.invalid_read_err());
                }
                Ok(value)
            })
            .with_automatic_refresh(|v| {
                v.status != ruphish_domain::campaign::CampaignStatus::Completed
            }),
            directory: Query::new(
                client,
                move |(filter, after, sort, order): (String, String, String, String)| async move {
                    let query = url::form_urlencoded::Serializer::new(String::new())
                        .extend_pairs([
                            ("filter", filter),
                            ("after", after),
                            ("sort", sort),
                            ("order", order),
                        ])
                        .finish();
                    api.read(&format!("/api/campaigns/directory?{query}")).await
                },
            ),
            delivery: Query::new(
                client,
                move |(id, cursor, sort, order): (String, String, String, String)| async move {
                    let recipient = (sort == "recipient")
                        .then(|| cursor.parse::<i64>().ok())
                        .flatten();
                    let query = if let Some(recipient) = recipient {
                        format!("after={}&limit=1", recipient.saturating_sub(1))
                    } else {
                        url::form_urlencoded::Serializer::new(String::new())
                            .extend_pairs([("cursor", cursor), ("sort", sort), ("order", order)])
                            .finish()
                    };
                    let mut value: reads::Delivery = api
                        .read(&format!("/api/campaigns/{id}/delivery?{query}"))
                        .await?;
                    if value.campaign.id.to_string() != id {
                        return Err(api.invalid_read_err());
                    }
                    if let Some(recipient) = recipient {
                        value.items.retain(|row| row.id == recipient);
                        if value.items.is_empty() {
                            return Err(ApiError::http(
                                404,
                                "RECIPIENT_NOT_FOUND".into(),
                                leptos_i18n::t_string!(api.i18n, ui.record_not_found_body).into(),
                            ));
                        }
                        value.next_after = None;
                        value.next_cursor = None;
                    }
                    Ok(DeliverySnapshot {
                        value: encoded(api, value)?,
                    })
                },
            )
            .with_automatic_refresh(|v| v.value["campaign"]["status"] != "Completed"),
            results: Query::new(client, move |(id, query): (i64, String)| async move {
                Self::read_results_query(api, id, &query).await
            })
            .with_automatic_refresh(|v| {
                v.status != ruphish_domain::campaign::CampaignStatus::Completed
            }),
            recipient_results: Query::new(
                client,
                move |(id, recipient, count): (i64, i64, usize)| async move {
                    let mut filter = Filter {
                        recipient: Some(recipient),
                        limit: count.clamp(1, 200),
                        event_order: ruphish_domain::reporting::SortDirection::Asc,
                        ..Default::default()
                    };
                    let mut result =
                        Self::read_results_query(api, id, &results_state::query(&filter, false))
                            .await?;
                    if !result.recipients.iter().any(|r| r.id == recipient) {
                        return Err(ApiError::http(
                            404,
                            "RECIPIENT_NOT_FOUND".into(),
                            leptos_i18n::t_string!(api.i18n, ui.record_not_found_body).into(),
                        ));
                    }
                    // One cached prefix owns the visible history. Reload revalidates the entire
                    // prefix; loading more never replaces already visible events with another page.
                    let mut offset = result.events.len();
                    while offset < count && (offset as i64) < result.event_total {
                        filter.offset = offset;
                        filter.limit = (count - offset).min(200);
                        let page = Self::read_results_query(
                            api,
                            id,
                            &results_state::query(&filter, false),
                        )
                        .await?;
                        if page.events.is_empty() {
                            break;
                        }
                        offset += page.events.len();
                        result.events.extend(page.events);
                    }
                    let mut seen = std::collections::HashSet::new();
                    result.events.retain(|event| seen.insert(event.id));
                    Ok(result)
                },
            )
            .with_automatic_refresh(|v| {
                v.status != ruphish_domain::campaign::CampaignStatus::Completed
            }),
            draft: Query::new(client, move |id: String| async move {
                Self::read_draft(api, &id).await
            }),
        });
    }

    pub fn resources(include_domains: bool) -> ruphish_ui::query::QueryView<Value> {
        use crate::{
            management_queries::ManagementQueries, resource_queries::ResourceQueries,
            resources::Family,
        };
        let resources = expect_context::<ResourceQueries>();
        let reads = [
            ("groups", resources.list(Family::Groups)),
            ("template", resources.list(Family::Templates)),
            ("page", resources.list(Family::Pages)),
            ("learning_page", resources.list(Family::LearningPages)),
            ("smtp", resources.list(Family::Profiles)),
        ];
        let domains = include_domains.then(|| {
            expect_context::<ManagementQueries>()
                .validated_domains
                .observe(|| ())
        });
        ruphish_ui::query::QueryView {
            data: Signal::derive(move || {
                let mut value = serde_json::json!({});
                for (key, read) in reads {
                    value[key] = serde_json::json!(read.data.get()?.into_iter().map(|row| serde_json::json!({"id":row.id,"name":row.name,"num_targets":row.count})).collect::<Vec<_>>());
                }
                if let Some(read) = domains {
                    value["domains"] = serde_json::json!(read.data.get()?);
                }
                Some(value)
            }),
            error: Signal::derive(move || {
                reads
                    .into_iter()
                    .find_map(|(_, read)| read.error.get())
                    .or_else(|| domains.and_then(|read| read.error.get()))
            }),
            failure: Signal::derive(move || {
                reads
                    .into_iter()
                    .find_map(|(_, read)| read.failure.get())
                    .or_else(|| domains.and_then(|read| read.failure.get()))
            }),
            fetching: Signal::derive(move || {
                reads.into_iter().any(|(_, read)| read.fetching.get())
                    || domains.is_some_and(|read| read.fetching.get())
            }),
            refresh: Callback::new(move |()| {
                for (_, read) in reads {
                    read.refresh.run(());
                }
                if let Some(read) = domains {
                    read.refresh.run(());
                }
            }),
        }
    }

    pub fn results_key(id: i64, filter: &Filter) -> (i64, String) {
        (id, results_state::query(filter, false))
    }

    pub async fn read_results(
        self,
        api: Api,
        id: i64,
        filter: &Filter,
    ) -> Result<reads::Results, ApiError> {
        let snapshot = self.results.fetch(Self::results_key(id, filter)).await;
        if let Some(error) = snapshot.error {
            return Err(error);
        }
        snapshot.data.ok_or_else(|| api.invalid_read_err())
    }

    async fn read_results_query(
        api: Api,
        id: i64,
        query: &str,
    ) -> Result<reads::Results, ApiError> {
        let value: reads::Results = api
            .read(&format!("/api/campaigns/{id}/results?{query}"))
            .await?;
        if value.id != id {
            return Err(api.invalid_read_err());
        }
        Ok(value)
    }

    pub async fn read_draft(api: Api, id: &str) -> Result<Value, ApiError> {
        let value: reads::Draft = api.read(&format!("/api/campaign-drafts/{id}")).await?;
        if value.id != id {
            return Err(api.invalid_read_err());
        }
        encoded(api, value)
    }

    pub fn draft_saved(self, id: &str) {
        self.directory.invalidate_where(|_| true);
        self.draft.invalidate(id.to_owned());
    }

    pub fn reporting_changed(self) {
        self.overview.campaigns_changed();
        self.delivery.invalidate_where(|_| true);
        self.results.invalidate_where(|_| true);
        self.recipient_results.invalidate_where(|_| true);
    }

    pub fn resources_changed(self) {
        self.context.invalidate_where(|_| true);
        self.delivery.invalidate_where(|_| true);
    }

    pub fn changed(self, id: &str) {
        self.context.invalidate(id.to_owned());
        self.overview.campaigns_changed();
        self.directory.invalidate_where(|_| true);
        self.delivery
            .invalidate_where(|(campaign, _, _, _)| campaign == id);
        self.results
            .invalidate_where(|(campaign, _)| campaign.to_string() == id);
        self.recipient_results
            .invalidate_where(|(campaign, _, _)| campaign.to_string() == id);
    }

    pub fn deleted(self, kind: &str, id: &str) {
        self.directory.confirmed_where(
            |_| true,
            |page| {
                page.items.retain(|row| {
                    let row_kind = match row.state {
                        ruphish_domain::campaign::DirectoryState::Draft(_) => "draft",
                        ruphish_domain::campaign::DirectoryState::Campaign(_) => "campaign",
                    };
                    row_kind != kind || row.id != id
                });
            },
        );
        if kind == "draft" {
            self.draft.forget(id.to_owned());
        } else {
            self.context.forget(id.to_owned());
            if let Ok(id) = id.parse() {
                self.overview.campaign_deleted(id);
            }
            self.delivery
                .forget_where(|(campaign, _, _, _)| campaign == id);
            self.results
                .forget_where(|(campaign, _)| campaign.to_string() == id);
            self.recipient_results
                .forget_where(|(campaign, _, _)| campaign.to_string() == id);
        }
    }

    pub fn launched(self, draft: &str, campaign: &str) {
        self.deleted("draft", draft);
        self.changed(campaign);
    }
}

fn encoded(api: Api, value: impl serde::Serialize) -> Result<Value, ApiError> {
    serde_json::to_value(value).map_err(|_| api.invalid_read_err())
}

pub(crate) use ruphish_ui::query::RefreshState;
