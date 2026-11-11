//! Typed recipient edits and the identity snapshot at the HTTP boundary.
use crate::shared::table_sort::Sort;
use ruphish_contracts::{
    resources::request::{Group, Recipient},
    validation::ValidationReport,
};
use ruphish_domain::reporting::SortDirection;

/// One recipient and its last confirmed values; never indexed by its email.
#[derive(Clone, Debug, PartialEq)]
pub struct RowDraft {
    /// Current authored fields.
    pub value: Recipient,
    baseline: Option<Recipient>,
}
impl RowDraft {
    /// Existing rows have a checkpoint; newly inserted rows do not.
    pub fn new(value: Recipient, baseline: Option<Recipient>) -> Self {
        Self { value, baseline }
    }
    /// Editing back to the checkpoint clears the row's dirty state.
    pub fn dirty(&self) -> bool {
        self.baseline.as_ref() != Some(&self.value)
    }
    /// Whether deleting this row removes a confirmed recipient.
    pub fn persisted(&self) -> bool {
        self.baseline.is_some()
    }
    /// Adopt the response without overwriting edits made after submission.
    pub fn saved(&mut self, submitted: &Recipient, returned: Recipient) {
        if &self.value == submitted {
            self.value = returned.clone();
        }
        self.baseline = Some(returned);
    }
}

/// Sort only while constructing an editor. Ties retain response order.
pub fn initially_ordered(rows: Vec<RowDraft>) -> Vec<RowDraft> {
    Sort {
        column: "email".into(),
        order: SortDirection::Asc,
    }
    .rows(
        rows.into_iter().enumerate().collect(),
        |(_, row), _| row.value.email.clone().into(),
        |(index, _)| (*index).into(),
    )
    .into_iter()
    .map(|(_, row)| row)
    .collect()
}

/// The submitted wire values and corresponding editor identities, in the same order.
pub struct Submission {
    /// Existing API contract; editor identities are never serialized.
    pub value: Group,
    /// Stable row identities for mapping diagnostics and confirmed values.
    pub rows: Vec<usize>,
}
impl Submission {
    /// Translate only recipient paths from this request, never current array positions.
    pub fn map_validation(&self, report: &mut ValidationReport) {
        for issue in &mut report.issues {
            if let Some((index, field)) = issue
                .field
                .strip_prefix("targets.")
                .and_then(|path| path.split_once('.'))
                .and_then(|(index, field)| index.parse::<usize>().ok().map(|index| (index, field)))
                && let Some(id) = self.rows.get(index)
            {
                issue.field = format!("targets.row-{id}.{field}");
            }
        }
    }
}
