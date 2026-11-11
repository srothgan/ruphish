//! Group editing owns typed row values; the generic resource draft is its checkpoint only.
use crate::resources::draft::ResourceDraft;
use crate::resources::recipients::rows::RowDraft;
use crate::resources::recipients::rows::Submission;
use crate::resources::recipients::rows::initially_ordered;
use leptos::prelude::*;
use ruphish_contracts::resources::request::{Group, Recipient};

/// Stable identity and independently reactive recipient values.
#[derive(Clone)]
pub struct Row {
    /// Editor identity, independent of email and request position.
    pub id: usize,
    /// Released with the last mounted row or collection reference.
    pub draft: ArcRwSignal<RowDraft>,
}

/// One owner for group edits; collection notifications describe membership changes only.
#[derive(Clone, Copy)]
pub struct GroupDraft {
    /// Rows in display and submission order.
    pub rows: RwSignal<Vec<Row>>,
    /// Editable group name.
    pub name: RwSignal<String>,
    baseline_name: RwSignal<String>,
    baseline_count: StoredValue<usize>,
    changes: RwSignal<usize>,
    next_id: StoredValue<usize>,
}
impl GroupDraft {
    /// Create an empty editor in the current reactive owner.
    pub fn new() -> Self {
        Self {
            rows: RwSignal::new(Vec::new()),
            name: RwSignal::new(String::new()),
            baseline_name: RwSignal::new(String::new()),
            baseline_count: StoredValue::new(0),
            changes: RwSignal::new(0),
            next_id: StoredValue::new(0),
        }
    }
    fn row(self, draft: RowDraft) -> Row {
        let id = self.next_id.get_value();
        self.next_id.set_value(id + 1);
        Row {
            id,
            draft: ArcRwSignal::new(draft),
        }
    }
    /// Replace the draft on open/reload, sorting recipients once.
    pub fn load(self, draft: &ResourceDraft) -> Result<(), serde_json::Error> {
        let value: Group = serde_json::from_value(draft.value().clone())?;
        let mut checkpoint = draft.clone();
        checkpoint.cancel();
        let baseline: Group = serde_json::from_value(checkpoint.value().clone())?;
        let rows = initially_ordered(
            value
                .targets
                .into_iter()
                .enumerate()
                .map(|(i, row)| RowDraft::new(row, baseline.targets.get(i).cloned()))
                .collect(),
        );
        batch(move || {
            self.name.set(value.name);
            self.baseline_name.set(baseline.name);
            self.baseline_count.set_value(baseline.targets.len());
            self.rows
                .set(rows.into_iter().map(|row| self.row(row)).collect());
            self.recount();
        });
        Ok(())
    }
    /// Aggregate dirty state without scanning recipients during typing.
    pub fn dirty(self) -> bool {
        self.changes.get() != 0
            || self
                .name
                .with(|name| self.baseline_name.with(|baseline| name != baseline))
    }
    /// Insert a blank recipient at the top.
    pub fn prepend(self) {
        let row = self.row(RowDraft::new(Recipient::default(), None));
        self.rows.update(|rows| rows.insert(0, row));
        self.changes.update(|changes| *changes += 1);
    }
    /// Remove a recipient by identity; retained inputs keep their values.
    pub fn remove(self, id: usize) {
        self.rows.update(|rows| rows.retain(|row| row.id != id));
        self.recount();
    }
    /// Change one mounted row and update dirty status only on transitions.
    pub fn edit(self, row: &ArcRwSignal<RowDraft>, edit: impl FnOnce(&mut Recipient)) {
        let was_dirty = row.with_untracked(RowDraft::dirty);
        row.update(|draft| edit(&mut draft.value));
        let dirty = row.with_untracked(RowDraft::dirty);
        if was_dirty != dirty {
            self.changes.update(|changes| {
                if dirty {
                    *changes += 1;
                } else {
                    *changes -= 1;
                }
            });
        }
    }
    fn recount(self) {
        let changes = self.rows.with_untracked(|rows| {
            let persisted = rows
                .iter()
                .filter(|row| row.draft.with_untracked(RowDraft::persisted))
                .count();
            self.baseline_count.get_value() - persisted
                + rows
                    .iter()
                    .filter(|row| row.draft.with_untracked(RowDraft::dirty))
                    .count()
        });
        self.changes.set(changes);
    }
    /// Capture values and identities together for one request.
    pub fn submission(self, id: i64) -> Submission {
        self.rows.with_untracked(|rows| Submission {
            value: Group {
                id,
                name: self.name.get_untracked(),
                targets: rows
                    .iter()
                    .map(|row| row.draft.with_untracked(|draft| draft.value.clone()))
                    .collect(),
            },
            rows: rows.iter().map(|row| row.id).collect(),
        })
    }
    /// Advance checkpoints after an ordered response, retaining newer local edits.
    pub fn saved(self, submitted: &Submission, returned: Group) {
        let positions = crate::shared::table_sort::row_positions(submitted.rows.iter().copied());
        batch(move || {
            if self.name.get_untracked() == submitted.value.name {
                self.name.set(returned.name.clone());
            }
            self.baseline_name.set(returned.name);
            self.baseline_count.set_value(returned.targets.len());
            self.rows.with_untracked(|rows| {
                for row in rows {
                    if let Some(&index) = positions.get(&row.id)
                        && let (Some(sent), Some(confirmed)) = (
                            submitted.value.targets.get(index),
                            returned.targets.get(index),
                        )
                    {
                        row.draft
                            .update(|draft| draft.saved(sent, confirmed.clone()));
                    }
                }
            });
            self.recount();
        });
    }
}

impl Default for GroupDraft {
    fn default() -> Self {
        Self::new()
    }
}
