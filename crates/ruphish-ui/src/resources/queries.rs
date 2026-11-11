//! Resource summary queries and their mutation dependencies, owned by the shell.
use crate::api::client::Api;
use crate::api::error::ApiError;
use crate::api::query::Query;
use crate::api::query::QueryView;
use crate::campaigns::queries::CampaignQueries;
use crate::overview::queries::OverviewQueries;
use crate::resources::inventory as resource_inventory;
use crate::resources::inventory::SummaryRow;
use crate::resources::pages::Family;
use leptos::prelude::*;
use leptos_fetch::QueryClient;
use ruphish_contracts::resources::details::Resource;
use ruphish_contracts::resources::{GroupSummaries, ResourceSummaries};
use serde_json::Value;

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
                        revision: row.revision,
                        name: row.name,
                        modified_date: row.modified_date,
                        count: Some(row.num_targets),
                    })
                    .collect())
            } else {
                let response: ResourceSummaries = api.read(&path).await?;
                Ok(response
                    .items
                    .into_iter()
                    .map(|row| SummaryRow {
                        id: row.id,
                        revision: row.revision,
                        name: row.name,
                        modified_date: row.modified_date,
                        count: Some(row.active_campaigns),
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
        use ruphish_contracts::resources::details::decode;
        let kind = family.detail_kind();
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

    pub fn saved(self, family: Family, resource: &Resource) {
        if family == Family::Profiles {
            expect_context::<crate::management_queries::ManagementQueries>()
                .profile_tests
                .invalidate(resource.metadata.id);
        }
        // Query::confirmed updates the existing cache and cancels obsolete reads
        // before refetching; a failed refresh retains this confirmed metadata.
        self.summaries
            .confirmed(family, |rows| resource_inventory::saved(rows, resource));
        self.overview.inventory_changed();
        self.campaigns.resources_changed();
        self.details
            .invalidate((family, resource.metadata.id.to_string()));
    }

    /// Refresh after a create attempt, including an unknown transport outcome.
    pub fn refresh_inventory(self, family: Family) {
        self.overview.inventory_changed();
        self.campaigns.resources_changed();
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
