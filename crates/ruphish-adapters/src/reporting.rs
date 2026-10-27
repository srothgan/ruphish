//! Shared export orchestration over authorized provider snapshots.
use ruphish_application::{auth::Identity, reporting::*, resources::Error};
use std::sync::Arc;
pub(crate) struct Reporting {
    store: Arc<dyn Snapshots>,
    exports: Arc<tokio::sync::Semaphore>,
}
impl Reporting {
    pub(crate) fn new(store: Arc<dyn Snapshots>) -> Self {
        Self {
            store,
            exports: Arc::new(tokio::sync::Semaphore::new(2)),
        }
    }
}
#[async_trait::async_trait]
impl Repository for Reporting {
    async fn analysis(&self, caller: Identity, id: i64, filter: Filter) -> Result<Analysis, Error> {
        self.store.snapshot(caller, id, &filter, false).await
    }
    async fn export(
        &self,
        caller: Identity,
        id: i64,
        filter: Filter,
        format: &str,
        scope: &str,
    ) -> Result<Download, Error> {
        if !["csv", "xlsx"].contains(&format)
            || !["events", "recipients", "all"].contains(&scope)
            || format == "csv" && scope == "all"
        {
            return Err(Error::Invalid("export_invalid_format_or_scope".into()));
        }
        let permit = self
            .exports
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Invalid("export_busy".into()))?;
        let snapshot = self.store.snapshot(caller, id, &filter, true).await?;
        let format = format.to_owned();
        let scope = scope.to_owned();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            crate::report_exports::generate(snapshot, &filter, &format, &scope)
        })
        .await
        .map_err(|_| Error::Internal)?
    }
}

/// Only typed, closed SQL fragments reach ORDER BY; user strings never do.
pub(crate) fn recipient_order(filter: &Filter, provider: &str) -> String {
    use ruphish_domain::reporting::{RecipientSort as S, SortDirection};
    let (column, text) = match filter.sort {
        S::Email => ("email", true),
        S::Name => ("TRIM(name)", true),
        S::Delivery => ("delivery", true),
        S::Accepted => ("accepted", false),
        S::Opened => ("opened", false),
        S::Clicked => ("clicked", false),
        S::Submitted => ("submitted", false),
        S::Reported => ("reports", false),
    };
    let column = if text {
        match provider {
            "mysql" => format!("CAST({column} AS BINARY)"),
            "postgres" => format!("{column} COLLATE \"C\""),
            _ => format!("{column} COLLATE BINARY"),
        }
    } else {
        column.to_owned()
    };
    let direction = match filter.order {
        SortDirection::Asc => "ASC",
        SortDirection::Desc => "DESC",
    };
    format!("{column} {direction},id ASC")
}

pub(crate) fn event_order(filter: &Filter, provider: &str) -> String {
    use ruphish_domain::reporting::{EventSort as S, SortDirection};
    let normalized_time = format!(
        "COALESCE(({}),'')",
        crate::campaign_sort::timestamp_key("time", provider)
    );
    let column = match (filter.event_sort, provider) {
        (S::Time, _) => &normalized_time,
        (S::Kind, "sqlite") => "message COLLATE BINARY",
        (S::Kind, "mysql") => "CAST(message AS BINARY)",
        (S::Kind, _) => "message COLLATE \"C\"",
        (S::Source, "sqlite") => "source COLLATE BINARY",
        (S::Source, "mysql") => "CAST(source AS BINARY)",
        (S::Source, _) => "source COLLATE \"C\"",
        (S::PasswordPresent, "sqlite") => {
            "CASE WHEN json_valid(details) THEN CASE json_type(details,'$.password_present') WHEN 'true' THEN 1 WHEN 'false' THEN 0 ELSE -1 END ELSE -1 END"
        }
        (S::PasswordPresent, "mysql") => {
            "CASE WHEN json_valid(details) THEN CASE WHEN JSON_TYPE(JSON_EXTRACT(details,'$.password_present')) COLLATE utf8mb4_bin='BOOLEAN' THEN CASE WHEN JSON_EXTRACT(details,'$.password_present')=CAST('true' AS JSON) THEN 1 ELSE 0 END ELSE -1 END ELSE -1 END"
        }
        (S::PasswordPresent, _) => {
            "CASE WHEN details IS JSON OBJECT THEN CASE details::jsonb->'password_present' WHEN 'true'::jsonb THEN 1 WHEN 'false'::jsonb THEN 0 ELSE -1 END ELSE -1 END"
        }
    };
    let direction = if filter.event_order == SortDirection::Asc {
        "ASC"
    } else {
        "DESC"
    };
    format!("{column} {direction},id {direction}")
}
