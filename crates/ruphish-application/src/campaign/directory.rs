//! Shared campaign-management projection and cursor semantics.
use crate::resources::Error;

/// Lifecycle filter applied before pagination.
#[derive(Clone, Copy, Default)]
pub enum Filter {
    /// Drafts followed by executions.
    #[default]
    All,
    /// Editable drafts only.
    Drafts,
    /// Executions that have not been completed.
    Active,
    /// Completed executions.
    Completed,
    /// All executions, for analysis navigation.
    Executions,
}
impl Filter {
    /// Whether draft records participate in this query.
    pub fn drafts(self) -> bool {
        matches!(self, Self::All | Self::Drafts)
    }
    /// Whether execution records participate in this query.
    pub fn executions(self) -> bool {
        !matches!(self, Self::Drafts)
    }
    /// Execution lifecycle selector: zero all, one completed, two active.
    pub fn completed(self) -> i32 {
        match self {
            Self::Completed => 1,
            Self::Active => 2,
            _ => 0,
        }
    }
}
/// Stable keyset position across both storage families.
#[derive(Default)]
pub struct Cursor {
    /// Explicit sorted traversal; absent retains the original ID cursor contract.
    pub sorted:
        Option<ruphish_domain::pagination::SortedPage<ruphish_domain::pagination::DirectorySort>>,
    /// Zero while in drafts, one after entering executions.
    pub kind: i32,
    /// Last draft identifier, or empty at the beginning.
    pub draft: String,
    /// Last execution identifier, or zero before executions.
    pub campaign: i64,
}
impl Cursor {
    /// Reject malformed cursors rather than silently restarting pagination.
    pub fn parse(value: &str) -> Result<Self, Error> {
        if value.is_empty() {
            return Ok(Self::default());
        }
        if let Some(id) = value.strip_prefix("draft:") {
            super::drafts::validate(id, &super::drafts::Configuration::default(), 0)?;
            return Ok(Self {
                draft: id.into(),
                ..Self::default()
            });
        }
        if let Some(id) = value.strip_prefix("campaign:")
            && let Ok(id) = id.parse::<i64>()
            && id > 0
        {
            return Ok(Self {
                kind: 1,
                campaign: id,
                draft: String::new(),
                sorted: None,
            });
        }
        Err(Error::Invalid("Invalid campaign directory cursor".into()))
    }
}
/// Lightweight row; no authored content, secrets, or draft configuration payload.
pub struct Entry {
    /// Draft ID, or decimal execution ID.
    pub id: String,
    /// Saved revision identifies a draft; executions have no editable draft revision.
    pub revision: Option<i64>,
    /// Display name.
    pub name: String,
    /// Draft or execution lifecycle.
    pub status: String,
    /// Audience snapshot size; absent before launch.
    pub total: Option<i64>,
    /// Actual execution schedule; absent for unscheduled drafts.
    pub launch: Option<String>,
}
impl Entry {
    /// Stable continuation key independent of names or mutable status.
    pub fn cursor(&self) -> String {
        format!(
            "{}:{}",
            if self.revision.is_some() {
                "draft"
            } else {
                "campaign"
            },
            self.id
        )
    }
}
/// One page spanning both families.
pub struct Page {
    /// Rows in stable draft-first, identifier order.
    pub items: Vec<Entry>,
    /// Present only if another matching row exists.
    pub next_after: Option<String>,
}
impl Page {
    /// Build the public page from a query that fetched one additional row.
    pub fn new(mut items: Vec<Entry>, limit: usize) -> Self {
        let more = items.len() > limit;
        items.truncate(limit);
        let next_after = items.last().filter(|_| more).map(Entry::cursor);
        Self { items, next_after }
    }
}
