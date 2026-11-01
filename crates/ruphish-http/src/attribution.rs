//! Map persisted IDs to self-contained response DTOs with one batched lookup.
use crate::auth::{Context, Rejection};
use ruphish_application::attribution::Attribution as Stored;
use ruphish_contracts::attribution::{Attribution, UserReference};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct Lookup(BTreeMap<i64, UserReference>);
impl Lookup {
    pub(crate) async fn load<'a>(
        context: &Context,
        values: impl IntoIterator<Item = &'a Stored>,
    ) -> Result<Self, Rejection> {
        let ids = values
            .into_iter()
            .flat_map(|a| [a.created_by, a.edited_by])
            .flatten()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let users = context.display_identities(&ids).await?;
        Ok(Self(
            users
                .into_iter()
                .map(|user| {
                    (
                        user.id,
                        UserReference {
                            id: user.id,
                            username: Some(user.username),
                            deleted: user.deleted,
                        },
                    )
                })
                .collect(),
        ))
    }
    pub(crate) fn resolve(&self, value: &Stored) -> Attribution {
        let user = |id| {
            self.0.get(&id).cloned().unwrap_or(UserReference {
                id,
                username: None,
                deleted: false,
            })
        };
        Attribution {
            created_by: value.created_by.map(user),
            edited_by: value.edited_by.map(user),
            edited_at: value.edited_at.clone(),
        }
    }
}
