//! Import preparation for new resources, without persistence or inherited identity.
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::collections::HashSet;

/// Stable import failure categories with actionable user-facing explanations.
#[derive(Debug, PartialEq, Eq)]
pub enum ImportError {
    /// JSON could not be parsed.
    InvalidJson,
    /// Fields do not match the selected resource contract.
    WrongResourceType,
    /// Multiple resources were supplied to a single-resource import.
    BatchNotSupported(&'static str),
    /// File format cannot populate this resource.
    UnsupportedFormat,
    /// Input exceeds the documented size limit.
    TooLarge,
    /// CSV contains no usable recipients.
    NoValidRecipients,
    /// The selected local file could not be read.
    ReadFailed,
    /// Server parsing or fetching failed; retain its useful explanation.
    RequestFailed(String),
    /// The draft is absent from this authenticated tab.
    DraftUnavailable,
    /// Secure random ID generation failed.
    IdUnavailable,
    /// Retained drafts reached the bounded capacity.
    Capacity,
}

impl ImportError {
    /// Stable code suitable for support reports and regression assertions.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidJson => "IMPORT_INVALID_JSON",
            Self::WrongResourceType => "IMPORT_WRONG_RESOURCE_TYPE",
            Self::BatchNotSupported(_) => "IMPORT_BATCH_NOT_SUPPORTED",
            Self::UnsupportedFormat => "IMPORT_UNSUPPORTED_FORMAT",
            Self::TooLarge => "IMPORT_TOO_LARGE",
            Self::NoValidRecipients => "IMPORT_NO_VALID_RECIPIENTS",
            Self::ReadFailed => "IMPORT_READ_FAILED",
            Self::RequestFailed(_) => "IMPORT_REQUEST_FAILED",
            Self::DraftUnavailable => "IMPORT_DRAFT_UNAVAILABLE",
            Self::IdUnavailable => "IMPORT_ID_UNAVAILABLE",
            Self::Capacity => "IMPORT_DRAFT_LIMIT",
        }
    }
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::BatchNotSupported(resource) => {
                return write!(
                    f,
                    "This import accepts one {resource} at a time. Split the file and import each separately. ({})",
                    self.code()
                );
            }
            Self::InvalidJson => "Invalid JSON. Check the file syntax and import again.",
            Self::WrongResourceType => {
                "The imported fields do not match this resource type. Choose the correct resource overview or fix the field types."
            }
            Self::UnsupportedFormat => {
                "This format is not supported for this resource. Choose a supported file or import format."
            }
            Self::TooLarge => "Import content exceeds 8 MiB. Use a smaller file.",
            Self::NoValidRecipients => {
                "No valid recipients were found. Correct the CSV and import it again."
            }
            Self::ReadFailed => "Could not read this file. Select it again.",
            Self::RequestFailed(message) => message,
            Self::DraftUnavailable => {
                "This draft is no longer available. Return to the overview and import again."
            }
            Self::IdUnavailable => {
                "Cannot generate a secure draft identifier. Try again in a supported browser over HTTPS or localhost."
            }
            Self::Capacity => "Finish or discard an existing import before opening another draft.",
        };
        write!(f, "{message} ({})", self.code())
    }
}
impl std::error::Error for ImportError {}

/// Parse resource JSON without carrying its data in a URL or browser storage.
pub fn parse_json(source: &str) -> Result<Value, ImportError> {
    if source.len() > 8 * 1024 * 1024 {
        return Err(ImportError::TooLarge);
    }
    serde_json::from_str(source).map_err(|_| ImportError::InvalidJson)
}

/// Review information travels with the editable draft until creation or discard.
#[derive(Clone, Debug)]
pub struct ImportReview {
    /// Persistent explanations of imported and omitted content.
    pub notes: Vec<ImportNote>,
    /// CSV errors retained for inspection and download.
    pub rejected_rows: Vec<Value>,
    /// Lossy imports need explicit acceptance before creation.
    pub requires_acknowledgement: bool,
    /// The user's acceptance of the reported omissions.
    pub acknowledged: bool,
}

/// Import facts rendered in the currently selected language by the UI.
#[derive(Clone, Debug)]
pub enum ImportNote {
    /// The imported resource still needs review and creation.
    Unsaved,
    /// Accepted, rejected and duplicate CSV row counts.
    Csv {
        /// Number of accepted recipients.
        loaded: usize,
        /// Number of rejected rows.
        rejected: usize,
        /// Number of omitted duplicate addresses.
        duplicates: usize,
    },
    /// Email import excludes attachments and the visible sender.
    Email,
    /// HTML import excludes other resource settings.
    Html,
}

impl ImportReview {
    /// Whether creation can proceed after reviewing omissions.
    pub fn ready(&self) -> bool {
        !self.requires_acknowledgement || self.acknowledged
    }
}

/// Validated editable fields plus review information; never a saved resource.
pub struct PreparedImport {
    /// Canonical wire fields with all inherited identity removed.
    pub fields: Value,
    /// Import limitations and rejected rows.
    pub review: ImportReview,
}

fn canonical<T: DeserializeOwned + Serialize>(value: Value) -> Result<Value, ImportError> {
    let typed: T = serde_json::from_value(value).map_err(|_| ImportError::WrongResourceType)?;
    serde_json::to_value(typed).map_err(|_| ImportError::WrongResourceType)
}

/// Prepare a new resource from a supported import result. The server still validates Create.
pub fn prepare_creation(
    family: &str,
    format: &str,
    mut value: Value,
) -> Result<PreparedImport, ImportError> {
    use ruphish_contracts::legacy;
    let supported = match format {
        "json" => matches!(
            family,
            "groups" | "templates" | "pages" | "learning-pages" | "smtp"
        ),
        "csv" => family == "groups",
        "email" => family == "templates",
        "html" => matches!(family, "templates" | "pages" | "learning-pages"),
        "site" => family == "pages",
        _ => false,
    };
    if !supported {
        return Err(ImportError::UnsupportedFormat);
    }
    if format == "json" && (value.is_array() || value.get(family).is_some_and(Value::is_array)) {
        return Err(ImportError::BatchNotSupported(match family {
            "groups" => "group",
            "templates" => "email template",
            "pages" => "landing page",
            "smtp" => "sending profile",
            _ => "resource",
        }));
    }
    if !value.is_object() {
        return Err(ImportError::WrongResourceType);
    }
    if format == "json" {
        let matches = match family {
            "groups" => value.get("targets").is_some(),
            "templates" => value.get("text").is_some() || value.get("html").is_some(),
            "pages" | "learning-pages" => value.get("html").is_some(),
            "smtp" => value.get("host").is_some(),
            _ => false,
        };
        if !matches {
            return Err(ImportError::WrongResourceType);
        }
    }
    let mut review = ImportReview {
        notes: vec![ImportNote::Unsaved],
        rejected_rows: Vec::new(),
        requires_acknowledgement: false,
        acknowledged: false,
    };
    if format == "csv" {
        review.rejected_rows = value["errors"].as_array().cloned().unwrap_or_default();
        let targets = value["targets"]
            .as_array()
            .ok_or(ImportError::NoValidRecipients)?;
        let mut seen = HashSet::new();
        let unique: Vec<_> = targets
            .iter()
            .filter(|row| seen.insert(row["email"].as_str().unwrap_or("").to_ascii_lowercase()))
            .cloned()
            .collect();
        let duplicates = targets.len() - unique.len();
        if unique.is_empty() {
            return Err(ImportError::NoValidRecipients);
        }
        review.notes.push(ImportNote::Csv {
            loaded: unique.len(),
            rejected: review.rejected_rows.len(),
            duplicates,
        });
        review.requires_acknowledgement = duplicates > 0 || !review.rejected_rows.is_empty();
        value["targets"] = json!(unique);
    } else if format == "email" {
        review.notes.push(ImportNote::Email);
    } else if matches!(format, "html" | "site") {
        review.notes.push(ImportNote::Html);
    }
    // Drop identity before decoding so an irrelevant old timestamp/ID cannot block an import.
    if let Some(fields) = value.as_object_mut() {
        for key in ["id", "user_id", "modified_date"] {
            fields.remove(key);
        }
    }
    // Canonical serialization also drops nested recipient/header IDs and unknown fields.
    let mut fields = match family {
        "groups" => canonical::<legacy::Group>(value)?,
        "templates" => canonical::<legacy::EmailTemplate>(value)?,
        "learning-pages" => {
            if value.get("name").is_none() {
                value["name"] = json!("");
            }
            canonical::<ruphish_contracts::resources::LearningPageInput>(value)?
        }
        "pages" => canonical::<legacy::LandingPage>(value)?,
        "smtp" => canonical::<legacy::SendingProfile>(value)?,
        _ => return Err(ImportError::WrongResourceType),
    };
    if let Some(object) = fields.as_object_mut() {
        object.remove("modified_date");
        object.insert("id".into(), json!(0));
    }
    Ok(PreparedImport { fields, review })
}

#[cfg(test)]
mod learning_import_tests {
    use super::*;
    #[test]
    fn exported_learning_content_imports_as_a_new_editable_resource_without_identity_or_tracking_policy()
     {
        let imported = prepare_creation("learning-pages", "json", json!({"id":21,"public_id":"old-public-identity","user_id":9,"modified_date":"invalid-old-date","name":"Explanation","html":"<h1>Check the sender</h1>","capture_credentials":true,"redirect_url":"https://outside.invalid"})).unwrap();
        assert_eq!(
            imported.fields,
            json!({"id":0,"name":"Explanation","html":"<h1>Check the sender</h1>"})
        );
        let html =
            prepare_creation("learning-pages", "html", json!({"html":"<p>Explain</p>"})).unwrap();
        assert_eq!(html.fields["html"], "<p>Explain</p>");
        assert_eq!(html.fields["name"], "");
        assert!(
            prepare_creation(
                "learning-pages",
                "json",
                json!({"name":"Wrong family","targets":[]})
            )
            .is_err()
        );
    }
}
