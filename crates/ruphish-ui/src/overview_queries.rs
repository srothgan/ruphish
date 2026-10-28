//! Dashboard snapshots and the mutations that make them stale.
use crate::client::Api;
use leptos::prelude::*;
use leptos_fetch::QueryClient;
use ruphish_contracts::overview::Snapshot;
use ruphish_ui::query::Query;

#[derive(Clone, Copy)]
pub(crate) struct OverviewQueries {
    pub snapshot: Query<(), Snapshot>,
}

impl OverviewQueries {
    pub fn provide(api: Api, client: QueryClient) {
        provide_context(Self {
            snapshot: Query::new(
                client,
                move |()| async move { api.read("/api/overview").await },
            ),
        });
    }

    pub fn inventory_changed(self) {
        self.snapshot.invalidate(());
    }

    pub fn campaigns_changed(self) {
        self.snapshot.invalidate(());
    }

    pub fn campaign_deleted(self, id: i64) {
        self.snapshot.confirmed((), |snapshot| {
            snapshot
                .attention
                .campaign_ids
                .retain(|campaign| *campaign != id);
            snapshot.recent.retain(|row| row.id != id);
            if snapshot
                .attention
                .next_launch
                .as_ref()
                .is_some_and(|row| row.id == id)
            {
                snapshot.attention.next_launch = None;
            }
        });
    }
}
