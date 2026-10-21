//! Dashboard snapshots and the mutations that make them stale.
use crate::account::Api;
use leptos::prelude::*;
use leptos_fetch::QueryClient;
use ruphish_contracts::overview::{PostureResponse, Snapshot};
use ruphish_ui::query::Query;

#[derive(Clone, Copy)]
pub(crate) struct OverviewQueries {
    pub snapshot: Query<(), Snapshot>,
    pub posture: Query<String, PostureResponse>,
}

impl OverviewQueries {
    pub fn provide(api: Api, client: QueryClient) {
        provide_context(Self {
            snapshot: Query::new(
                client,
                move |()| async move { api.read("/api/overview").await },
            ),
            posture: Query::new(client, move |window: String| async move {
                let value: PostureResponse = api
                    .read(&format!("/api/overview/posture?window={window}"))
                    .await?;
                if value.window != window {
                    return Err(api.invalid_read());
                }
                Ok(value)
            }),
        });
    }

    pub fn inventory_changed(self) {
        self.snapshot.invalidate(());
    }

    pub fn campaigns_changed(self) {
        self.snapshot.invalidate(());
        self.posture.invalidate_where(|_| true);
    }

    pub fn campaign_deleted(self, id: i64) {
        self.snapshot.confirmed((), |snapshot| {
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
        self.posture.invalidate_where(|_| true);
    }
}
