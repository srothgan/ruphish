//! Shared domain and integration reads and their mutation dependencies.
use crate::{
    account::Api,
    campaign_queries::CampaignQueries,
    integrations::{Kind, Record},
    overview_queries::OverviewQueries,
};
use leptos::prelude::*;
use leptos_fetch::QueryClient;
use ruphish_contracts::{
    auth::UserSummary,
    domains::Domain,
    integrations::{Mailbox, Webhook},
};
use ruphish_ui::query::Query;
use serde_json::Value;

#[derive(Clone, Copy)]
pub(crate) struct ManagementQueries {
    overview: OverviewQueries,
    campaigns: CampaignQueries,
    pub settings: Query<(), ruphish_contracts::settings::Settings>,
    pub users: Query<(), Vec<UserSummary>>,
    pub starters: Query<(), Vec<ruphish_contracts::starter_templates::Summary>>,
    pub starter: Query<String, ruphish_contracts::starter_templates::Entry>,
    pub domains: Query<(), Vec<Domain>>,
    pub validated_domains: Query<(), Vec<Domain>>,
    pub domain: Query<i64, Domain>,
    pub integrations: Query<Kind, Vec<Record>>,
    pub integration: Query<(Kind, i64), Record>,
}

impl ManagementQueries {
    pub fn provide(api: Api, client: QueryClient) {
        provide_context(Self {
            overview: expect_context(),
            campaigns: expect_context(),
            settings: Query::new(
                client,
                move |()| async move { api.read("/api/settings").await },
            ),
            users: Query::new(
                client,
                move |()| async move { api.read("/api/users/").await },
            ),
            starters: Query::new(client, move |()| async move {
                api.read("/api/starter-templates/").await
            }),
            starter: Query::new(client, move |id: String| async move {
                api.read(&format!("/api/starter-templates/{id}")).await
            }),
            domains: Query::new(
                client,
                move |()| async move { api.read("/api/domains/").await },
            ),
            validated_domains: Query::new(client, move |()| async move {
                api.read("/api/domains/validated/").await
            }),
            domain: Query::new(
                client,
                move |id| async move { Self::read_domain(api, id).await },
            ),
            integrations: Query::new(client, move |kind: Kind| async move {
                let path = format!("{}/", kind.api());
                if kind.webhook() {
                    let rows: Vec<Webhook> = api.read(&path).await?;
                    Ok(rows.into_iter().map(Record::Webhook).collect())
                } else {
                    let rows: Vec<Mailbox> = api.read(&path).await?;
                    Ok(rows.into_iter().map(Record::Mailbox).collect())
                }
            }),
            integration: Query::new(client, move |(kind, id): (Kind, i64)| async move {
                Self::read_integration(api, kind, id).await
            }),
        });
    }

    pub async fn read_domain(api: Api, id: i64) -> Result<Domain, String> {
        let value = api.get(&format!("/api/domains/{id}")).await?;
        Self::validate_domain(api, value, Some(id))
    }

    pub fn validate_domain(
        api: Api,
        value: Value,
        expected_id: Option<i64>,
    ) -> Result<Domain, String> {
        let value: Domain = serde_json::from_value(value).map_err(|_| api.invalid_read())?;
        if value.id <= 0 || expected_id.is_some_and(|id| id != value.id) {
            return Err(api.invalid_read());
        }
        Ok(value)
    }

    pub async fn read_integration(api: Api, kind: Kind, id: i64) -> Result<Record, String> {
        let path = format!("{}/{id}", kind.api());
        let value = api.get(&path).await?;
        Self::validate_integration(api, kind, value, Some(id))
    }

    pub fn validate_integration(
        api: Api,
        kind: Kind,
        value: Value,
        expected_id: Option<i64>,
    ) -> Result<Record, String> {
        if value["id"]
            .as_i64()
            .is_none_or(|id| id <= 0 || expected_id.is_some_and(|expected| expected != id))
        {
            return Err(api.invalid_read());
        }
        if kind.webhook() {
            serde_json::from_value::<Webhook>(value)
                .map(Record::Webhook)
                .map_err(|_| api.invalid_read())
        } else {
            serde_json::from_value::<Mailbox>(value)
                .map(Record::Mailbox)
                .map_err(|_| api.invalid_read())
        }
    }

    pub fn policy_changed(self) {
        self.domain_dependencies_changed();
        self.domain.invalidate_where(|_| true);
        self.integrations.invalidate_where(|_| true);
        self.integration.invalidate_where(|_| true);
        self.campaigns.directory.invalidate_where(|_| true);
        self.campaigns.delivery.invalidate_where(|_| true);
        self.campaigns.analysis.invalidate_where(|_| true);
        self.overview.campaigns_changed();
    }
    pub fn domain_verified(self, value: Domain) {
        let id = value.id;
        self.domain_dependencies_changed();
        self.domain.confirmed(id, |current| *current = value);
    }

    fn domain_dependencies_changed(self) {
        self.overview.inventory_changed();
        self.campaigns.resources_changed();
        self.domains.invalidate(());
        self.validated_domains.invalidate(());
    }

    pub fn domain_saved(self, id: i64) {
        self.domain_dependencies_changed();
        self.domain.invalidate(id);
    }

    pub fn domain_deleted(self, id: i64) {
        self.overview.inventory_changed();
        self.campaigns.resources_changed();
        self.domains
            .confirmed((), |rows| rows.retain(|row| row.id != id));
        self.domain.forget(id);
        self.validated_domains
            .confirmed((), |rows| rows.retain(|row| row.id != id));
    }

    pub fn integration_command(self, kind: Kind, id: i64, operation: &str, saved: Record) {
        if !kind.webhook() {
            self.overview.inventory_changed();
        }
        self.integrations.confirmed(kind, |rows| {
            if let Some(row) = rows.iter_mut().find(|row| row.id() == id) {
                *row = saved.clone();
            }
        });
        self.integration
            .confirmed((kind, id), |current| *current = saved);
        if !kind.webhook() && operation != "validate" {
            // Queued work can add report events. Revisited analysis and dashboard
            // queries must read the backend again instead of reusing fresh counts.
            self.campaigns.reporting_changed();
        }
    }

    pub fn integration_saved(self, kind: Kind, id: i64) {
        if !kind.webhook() {
            self.overview.inventory_changed();
        }
        self.integrations.invalidate(kind);
        self.integration.invalidate((kind, id));
    }

    pub fn integration_deleted(self, kind: Kind, id: i64) {
        if !kind.webhook() {
            self.overview.inventory_changed();
        }
        self.integrations
            .confirmed(kind, |rows| rows.retain(|row| row.id() != id));
        self.integration.forget((kind, id));
    }
}
