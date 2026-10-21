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
