//! Read-only starter catalog. Metadata never belongs to an imported resource.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Independent catalog resource families.
pub enum Kind {
    /// Ordinary email template.
    Email,
    /// Ordinary landing page.
    LandingPage,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
/// Reviewed logo replacement location and supported vendor context.
pub struct CompanyLogo {
    /// Unique selector in this catalog entry, never stored on the created resource.
    pub selector: String,
    /// Maximum displayed image height in CSS pixels.
    pub height: u16,
    /// Vendor configuration represented by the branded variant.
    pub context: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
/// Lightweight selection metadata without HTML content.
pub struct Summary {
    /// Catalog lookup key, never a persistent resource relationship.
    pub id: String,
    /// Suggested ordinary resource name.
    pub name: String,
    /// Searchable brand name.
    pub brand: String,
    /// Creation API family.
    pub kind: Kind,
    /// Whether recipient email prefill is supported; selection defaults off.
    pub prefill: bool,
    /// Available only for reviewed branding variants; selection defaults off.
    pub company_logo: Option<CompanyLogo>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
/// Read-only content and capabilities for one selection.
pub struct Entry {
    /// Catalog-only information.
    pub summary: Summary,
    /// An ordinary create payload; contains no catalog identifier or options.
    pub resource: serde_json::Value,
}
