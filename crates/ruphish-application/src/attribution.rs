//! Persisted authorship and display metadata captured by authorized operations.
use crate::auth::DisplayIdentity;
pub use ruphish_domain::attribution::Attribution;

/// A committed record and the display identities captured before its transaction committed.
/// Inspect through Deref; take `record` when transferring ownership to a response mapper.
pub struct Saved<T> {
    /// Confirmed record returned by persistence.
    pub record: T,
    /// Display-only identities; no additional read is required to construct the response.
    pub identities: Vec<DisplayIdentity>,
}
impl<T> std::ops::Deref for Saved<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.record
    }
}
/// Authorship required to project a record, including nested frozen content.
pub trait Attributed {
    /// All stored references in the projected record.
    fn attributions(&self) -> Vec<&Attribution>;
}
/// Deduplicated account identifiers for a single batched display lookup.
pub fn identity_ids<'a>(values: impl IntoIterator<Item = &'a Attribution>) -> Vec<i64> {
    values
        .into_iter()
        .flat_map(|a| [a.created_by, a.edited_by])
        .flatten()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}
impl Attributed for crate::resources::Resource {
    fn attributions(&self) -> Vec<&Attribution> {
        vec![&self.attribution]
    }
}
impl Attributed for crate::campaign::drafts::Draft {
    fn attributions(&self) -> Vec<&Attribution> {
        vec![&self.attribution]
    }
}
impl Attributed for crate::domains::Domain {
    fn attributions(&self) -> Vec<&Attribution> {
        vec![&self.attribution]
    }
}
impl Attributed for crate::integrations::Webhook {
    fn attributions(&self) -> Vec<&Attribution> {
        vec![&self.attribution]
    }
}
impl Attributed for crate::integrations::Mailbox {
    fn attributions(&self) -> Vec<&Attribution> {
        vec![&self.attribution]
    }
}
impl Attributed for crate::campaign::Campaign {
    fn attributions(&self) -> Vec<&Attribution> {
        std::iter::once(&self.attribution)
            .chain(
                self.resources
                    .iter()
                    .chain(&self.groups)
                    .map(|r| &r.attribution),
            )
            .collect()
    }
}
