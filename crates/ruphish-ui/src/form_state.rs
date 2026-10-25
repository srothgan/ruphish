//! Form interaction policy independent of HTTP, rendering and validation rules.
use ruphish_contracts::validation::{Code, Issue, ValidationReport};
use std::collections::BTreeSet;

/// Tracks visibility and rejects validation responses for superseded submissions.
#[derive(Clone, Debug, Default)]
pub struct FormState {
    touched: BTreeSet<String>,
    generation: u64,
    server: Vec<Issue>,
}
impl FormState {
    /// Expose all applicable local errors after an attempted action.
    pub fn submit(&mut self, issues: &[Issue]) {
        // A deliberate retry must not keep an obsolete server verdict while revalidating.
        self.server.clear();
        self.touched
            .extend(issues.iter().map(|issue| issue.field.clone()));
    }
    /// Expose a field once the operator leaves it.
    pub fn touch(&mut self, field: &str) {
        self.touched.insert(field.to_owned());
    }
    /// Edits supersede pending responses and remove errors for the edited field.
    pub fn edit(&mut self, field: &str) {
        self.generation = self.generation.wrapping_add(1);
        self.server.retain(|issue| {
            issue.field != field
                && !issue.field.starts_with(&format!("{field}."))
                && !matches!(
                    issue.code,
                    Code::PasswordMismatch
                        | Code::ScheduleOrder
                        | Code::RecheckMargin
                        | Code::RedirectConflict
                )
        });
    }
    /// Snapshot identity for one request.
    pub fn ticket(&self) -> u64 {
        self.generation
    }
    /// Accept field failures only for the unchanged submitted form.
    pub fn receive(&mut self, ticket: u64, report: ValidationReport) -> bool {
        if ticket != self.generation {
            return false;
        }
        self.server = report.issues;
        true
    }
    /// Visible diagnostics; unrelated operational failures never enter this state.
    pub fn issues(&self, local: &[Issue]) -> Vec<Issue> {
        let mut issues: Vec<_> = local
            .iter()
            .filter(|issue| self.touched.contains(&issue.field))
            .cloned()
            .collect();
        for issue in &self.server {
            if !issues.iter().any(|local| local.field == issue.field) {
                issues.push(issue.clone());
            }
        }
        issues
    }
    /// The first visible reason for a control.
    pub fn field(&self, local: &[Issue], field: &str) -> Option<Code> {
        self.issues(local)
            .iter()
            .find(|issue| issue.field == field)
            .map(|issue| issue.code)
    }
}
