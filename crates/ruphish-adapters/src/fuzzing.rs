//! Narrow access to private production boundaries for the standalone fuzz harness.

use ruphish_application::resources::Error;

/// Identify a reported message without opening an IMAP connection.
pub fn identify_mailbox(bytes: &[u8], restrict: &str) -> Result<(String, String), &'static str> {
    crate::integrations::mailbox::identify(bytes, restrict)
}

/// Validate and normalize rendered email markup.
pub fn email_document(source: &str) -> Result<String, Error> {
    crate::email_html::document(source)
}

/// Apply the production managed-image policy.
pub fn email_tracked(source: &str, url: &str, enabled: bool) -> Result<String, Error> {
    crate::email_html::tracked(source, url, enabled)
}

/// Remove legacy tracking images, including conditional-comment content.
pub fn email_without_tracker(source: &str, url: &str) -> Result<String, Error> {
    crate::email_html::without_tracker(source, url)
}
