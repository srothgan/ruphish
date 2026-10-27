//! Resource summary queries and their mutation dependencies, owned by the shell.
use crate::{
    campaign_queries::CampaignQueries, client::Api, overview_queries::OverviewQueries,
    resources::Family,
};
use leptos::prelude::*;
use leptos_fetch::QueryClient;
use ruphish_contracts::resource_details::Resource;
use ruphish_contracts::resources::{GroupSummaries, ResourceSummaries};
use ruphish_ui::api_error::ApiError;
use ruphish_ui::query::{Query, QueryView};
use serde_json::Value;

#[derive(Clone, Debug)]
pub(crate) struct SummaryRow {
    pub id: i64,
    pub name: String,
    pub modified_date: Option<String>,
    pub count: usize,
}

#[derive(Clone, Copy)]
pub(crate) struct ResourceQueries {
    overview: OverviewQueries,
    campaigns: CampaignQueries,
    summaries: Query<Family, Vec<SummaryRow>>,
    pub details: Query<(Family, String), (Resource, Option<String>)>,
}

impl ResourceQueries {
    pub fn provide(api: Api, client: QueryClient) {
        let summaries = Query::new(client, move |family: Family| async move {
            let path = format!("/api/{}/summary", family.route());
            if family == Family::Groups {
                let response: GroupSummaries = api.read(&path).await?;
                Ok(response
                    .groups
                    .into_iter()
                    .map(|row| SummaryRow {
                        id: row.id,
                        name: row.name,
                        modified_date: row.modified_date,
                        count: row.num_targets,
                    })
                    .collect())
            } else {
                let response: ResourceSummaries = api.read(&path).await?;
                Ok(response
                    .items
                    .into_iter()
                    .map(|row| SummaryRow {
                        id: row.id,
                        name: row.name,
                        modified_date: row.modified_date,
                        count: row.active_campaigns,
                    })
                    .collect())
            }
        });
        let details = Query::new(client, move |(family, id): (Family, String)| async move {
            Self::read_detail(api, family, &id).await
        });
        provide_context(Self {
            summaries,
            details,
            overview: expect_context(),
            campaigns: expect_context(),
        });
    }

    pub async fn read_detail(
        api: Api,
        family: Family,
        id: &str,
    ) -> Result<(Resource, Option<String>), ApiError> {
        let result = api
            .read_resource(&format!("/api/{}/{id}", family.route()))
            .await?;
        let resource = Self::decode_detail(api, family, result.0, id.parse().ok())?;
        Ok((resource, result.1))
    }

    pub fn decode_detail(
        api: Api,
        family: Family,
        value: Value,
        expected_id: Option<i64>,
    ) -> Result<Resource, ApiError> {
        use ruphish_contracts::resource_details::{Kind, decode};
        let kind = match family {
            Family::Groups => Kind::Group,
            Family::Templates => Kind::Template,
            Family::Pages => Kind::Page,
            Family::LearningPages => Kind::LearningPage,
            Family::Profiles => Kind::Profile,
        };
        let resource = decode(kind, value).map_err(|_| api.invalid_read_err())?;
        let id = resource.metadata.id;
        if id <= 0 || expected_id.is_some_and(|expected| id != expected) {
            return Err(api.invalid_read_err());
        }
        Ok(resource)
    }

    pub fn list(self, family: Family) -> QueryView<Vec<SummaryRow>> {
        self.summaries.observe(move || family)
    }

    pub fn saved(self, family: Family, id: i64) {
        self.refresh_inventory(family);
        self.details.invalidate((family, id.to_string()));
    }

    /// Refresh after a create attempt, including an unknown transport outcome.
    pub fn refresh_inventory(self, family: Family) {
        self.overview.inventory_changed();
        self.campaigns.resources_changed();
        // TODO: If bug reports show confusion after a successful save followed by
        // a failed list refresh, consider applying confirmed summary fields from
        // the save response before invalidation. Do not invent missing counts;
        // cover failed refreshes and late responses. For now, retaining the last
        // confirmed list with the localized stale-data warning and retry is intentional.
        self.summaries.invalidate(family);
    }

    pub fn deleted(self, family: Family, id: i64) {
        self.overview.inventory_changed();
        self.campaigns.resources_changed();
        self.summaries
            .confirmed(family, |rows| rows.retain(|row| row.id != id));
        self.details.forget((family, id.to_string()));
    }
}
