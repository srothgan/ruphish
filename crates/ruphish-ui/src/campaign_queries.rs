//! Campaign read ownership and mutation dependencies, separate from editable drafts.
use crate::{client::Api, overview_queries::OverviewQueries};
use leptos::prelude::*;
use leptos_fetch::QueryClient;
use ruphish_contracts::{campaign_reads as reads, reporting::Filter};
use ruphish_ui::{analysis_state, api_error::ApiError, query::Query};
use serde_json::Value;

#[derive(Clone, Debug)]
pub(crate) struct DeliverySnapshot {
    pub value: Value,
    pub refreshed_at: i64,
}

#[derive(Clone, Copy)]
pub(crate) struct CampaignQueries {
    overview: OverviewQueries,
    pub directory: Query<(String, String, String, String), reads::Directory>,
    pub delivery: Query<(String, String, String, String), DeliverySnapshot>,
    pub analysis: Query<(i64, String), reads::Analysis>,
    pub draft: Query<String, Value>,
}

impl CampaignQueries {
    pub fn provide(api: Api, client: QueryClient) {
        provide_context(Self {
            overview: expect_context(),
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
                    let query = url::form_urlencoded::Serializer::new(String::new())
                        .extend_pairs([("cursor", cursor), ("sort", sort), ("order", order)])
                        .finish();
                    let value: reads::Delivery = api
                        .read(&format!("/api/campaigns/{id}/delivery?{query}"))
                        .await?;
                    if value.campaign.id.to_string() != id {
                        return Err(api.invalid_read_err());
                    }
                    Ok(DeliverySnapshot {
                        value: encoded(api, value)?,
                        refreshed_at: ruphish_domain::datetime::now().as_second(),
                    })
                },
            ),
            analysis: Query::new(client, move |(id, query): (i64, String)| async move {
                Self::read_analysis_query(api, id, &query).await
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

    pub fn analysis_key(id: i64, filter: &Filter) -> (i64, String) {
        (id, analysis_state::query(filter, false))
    }

    pub async fn read_analysis(
        self,
        api: Api,
        id: i64,
        filter: &Filter,
    ) -> Result<reads::Analysis, ApiError> {
        let snapshot = self.analysis.fetch(Self::analysis_key(id, filter)).await;
        if let Some(error) = snapshot.error {
            return Err(error);
        }
        snapshot.data.ok_or_else(|| api.invalid_read_err())
    }

    async fn read_analysis_query(
        api: Api,
        id: i64,
        query: &str,
    ) -> Result<reads::Analysis, ApiError> {
        let value: reads::Analysis = api
            .read(&format!("/api/campaigns/{id}/analysis?{query}"))
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
        self.analysis.invalidate_where(|_| true);
    }

    pub fn resources_changed(self) {
        self.delivery.invalidate_where(|_| true);
    }

    pub fn changed(self, id: &str) {
        self.overview.campaigns_changed();
        self.directory.invalidate_where(|_| true);
        self.delivery
            .invalidate_where(|(campaign, _, _, _)| campaign == id);
        self.analysis
            .invalidate_where(|(campaign, _)| campaign.to_string() == id);
    }

    pub fn deleted(self, kind: &str, id: &str) {
        self.directory.confirmed_where(
            |_| true,
            |page| page.items.retain(|row| row.kind != kind || row.id != id),
        );
        if kind == "draft" {
            self.draft.forget(id.to_owned());
        } else {
            if let Ok(id) = id.parse() {
                self.overview.campaign_deleted(id);
            }
            self.delivery
                .forget_where(|(campaign, _, _, _)| campaign == id);
            self.analysis
                .forget_where(|(campaign, _)| campaign.to_string() == id);
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
