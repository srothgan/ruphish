//! Stable, selection-bound continuations for sorted management collections.
use crate::reporting::SortDirection;
use serde::{Deserialize, Serialize};

/// Sort columns supported by the combined campaign directory.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectorySort {
    /// Display name across drafts and executions.
    #[default]
    Name,
    /// Draft or execution lifecycle.
    Status,
    /// Audience size; absent for drafts.
    Total,
    /// Scheduled execution time; absent for drafts.
    Launch,
}
/// Sort columns supported by operational delivery pages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliverySort {
    /// Original audience address.
    #[default]
    Email,
    /// Current delivery state.
    State,
    /// Current scheduled/retry due time.
    Due,
    /// Number of recorded attempts.
    AttemptCount,
}
/// Comparable SQL key captured at the page boundary, independent of later row edits/deletion.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    /// Whether the selected value exists (absent values sort first ascending).
    pub present: bool,
    /// Text part; empty for numeric keys.
    pub text: String,
    /// Numeric part; zero for text keys.
    pub number: i64,
    /// Stable identity family: zero draft, one execution/delivery.
    pub kind: i32,
    /// Draft identity; empty for executions/deliveries.
    pub draft: String,
    /// Execution/delivery identity; zero for drafts.
    pub id: i64,
}
/// Versioned continuation bound to the exact filter, entity and ordering selection.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cursor<S> {
    /// Cursor schema version.
    pub version: u8,
    /// Selected column.
    pub sort: S,
    /// Selected direction.
    pub order: SortDirection,
    /// Owner/entity/filter scope, assembled by the endpoint.
    pub scope: String,
    /// Last returned sort key and stable identity.
    pub position: Position,
}
impl<S: PartialEq> Cursor<S> {
    /// Reject incompatible or unbounded continuations before querying storage.
    pub fn valid(&self, sort: &S, order: SortDirection, scope: &str) -> bool {
        self.version == 1
            && &self.sort == sort
            && self.order == order
            && self.scope == scope
            && self.position.text.len() <= 4096
            && self.position.draft.len() <= 64
            && matches!(self.position.kind, 0 | 1)
            && self.position.id >= 0
    }
}
/// Explicit sorted query; an absent position starts a new traversal.
#[derive(Clone, Debug)]
pub struct SortedPage<S> {
    /// Selected column.
    pub sort: S,
    /// Selected direction.
    pub order: SortDirection,
    /// The canonical scope used when issuing continuations.
    pub scope: String,
    /// Last returned key; absence starts before all matching rows.
    pub after: Option<Position>,
}
impl<S: Clone> SortedPage<S> {
    /// Build a continuation without reading a mutable boundary row again.
    pub fn cursor(&self, position: Position) -> Cursor<S> {
        Cursor {
            version: 1,
            sort: self.sort.clone(),
            order: self.order,
            scope: self.scope.clone(),
            position,
        }
    }
}
