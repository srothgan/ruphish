//! Bounded, value-free validation diagnostics shared by native and browser callers.
use serde::{Deserialize, Serialize};

/// Stable reasons translated by the presentation layer, never submitted values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Code {
    /// A required value is absent.
    Required,
    /// The supplied value does not satisfy the field's grammar or policy.
    Invalid,
    /// URL syntax could not be parsed.
    InvalidUrl,
    /// A collection or value exceeds its supported bound.
    TooLong,
    /// At least one authored email body is required.
    EmailContent,
    /// The password contains fewer than eight UTF-8 bytes.
    PasswordShort,
    /// The password exceeds 1024 UTF-8 bytes.
    PasswordLong,
    /// Confirmation differs from the candidate.
    PasswordMismatch,
    /// A scheduled draft must use a current or future instant.
    SchedulePast,
    /// Distribution cannot end before launch.
    ScheduleOrder,
    /// The selected resource is no longer available.
    Unavailable,
    /// Another record already owns this value.
    Conflict,
    /// Exactly one redirect destination may be selected.
    RedirectConflict,
    /// Verification validity is outside the supported bounds.
    ValidityRange,
    /// Verification recheck is outside the supported bounds.
    RecheckRange,
    /// Verification needs enough time for another attempt.
    RecheckMargin,
    /// Forward-compatible fallback for a newer server reason.
    #[serde(other)]
    Unknown,
}

/// A schema-owned field path and reason. Never contains authored field values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Issue {
    /// Dot-separated property names and zero-based collection positions.
    pub field: String,
    /// Stable failure identity.
    pub code: Code,
}

/// Maximum diagnostics returned for one operation, including large audiences.
pub const MAX_ISSUES: usize = 100;

/// Validation failure retaining the first established diagnostic for legacy callers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    issues: Vec<Issue>,
    message: &'static str,
}

impl Report {
    /// Add a diagnostic without allowing unbounded responses.
    pub fn reject(&mut self, field: impl Into<String>, code: Code, message: &'static str) {
        if self.issues.is_empty() {
            self.message = message;
        }
        if self.issues.len() < MAX_ISSUES {
            self.issues.push(Issue {
                field: field.into(),
                code,
            });
        }
    }

    /// Add all diagnostics from another independent rule set.
    pub fn extend(&mut self, other: Self) {
        for issue in other.issues {
            self.reject(issue.field, issue.code, other.message);
        }
    }

    /// Inspect diagnostics without transferring ownership.
    pub fn issues(&self) -> &[Issue] {
        &self.issues
    }

    /// Preserve the historical first-error diagnostic at compatibility boundaries.
    pub fn message(&self) -> &'static str {
        self.message
    }

    /// Whether every evaluated rule passed.
    pub fn is_empty(&self) -> bool {
        self.issues.is_empty()
    }

    /// Finish validation; success never carries a partial error report.
    pub fn finish(self) -> Result<(), Self> {
        if self.is_empty() { Ok(()) } else { Err(self) }
    }
}

impl std::fmt::Display for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message)
    }
}
impl std::error::Error for Report {}
