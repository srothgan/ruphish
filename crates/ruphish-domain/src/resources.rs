//! Resource values and invariants, independent of transport and persistence.

/// Supported resource families with separate ownership and naming scopes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Recipient collection.
    Group,
    /// Email template and attachments.
    Template,
    /// Landing-page content and capture policy.
    Page,
    /// SMTP connection and message headers.
    Profile,
}
impl Kind {
    /// Legacy singular name used in public errors.
    pub fn label(self) -> &'static str {
        match self {
            Self::Group => "Group",
            Self::Template => "Template",
            Self::Page => "Page",
            Self::Profile => "SMTP",
        }
    }
}
/// Recipient attributes; email spelling is preserved on API writes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Recipient {
    /// Recipient mailbox, preserving the supplied spelling.
    pub email: String,
    /// Recipient given name; an empty value intentionally clears it.
    pub first_name: String,
    /// Recipient family name.
    pub last_name: String,
    /// Recipient position or role.
    pub position: String,
}
/// Attachment data retains the legacy base64 representation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Attachment {
    /// Display name within the owning resource scope.
    pub name: String,
    /// Attachment MIME media type.
    pub mime_type: String,
    /// Resource or attachment content.
    pub content: String,
}
/// An additional rendered message header.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Header {
    /// Custom header name.
    pub key: String,
    /// Custom header value before template expansion.
    pub value: String,
}
/// Authored email content, including the legacy visible-From override.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Template {
    /// Email subject template; empty is supported.
    pub subject: String,
    /// Plain-text alternative.
    pub text: String,
    /// Authoritative authored HTML source.
    pub html: String,
    /// Legacy field providing the visible From override.
    pub envelope_sender: String,
    /// Ordered attachment collection.
    pub attachments: Vec<Attachment>,
}
/// Authored landing page and capture policy; transformations do not mutate the draft.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Page {
    /// Authoritative authored HTML source.
    pub html: String,
    /// Whether submitted non-password fields may be recorded.
    pub capture_credentials: bool,
    /// Whether passwords may be recorded; implies credential capture.
    pub capture_passwords: bool,
    /// Templated destination after submission.
    pub redirect_url: String,
}
/// SMTP credentials stay in backend models and authorized legacy responses only.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Profile {
    /// Legacy sending interface label.
    pub interface_type: String,
    /// SMTP destination, optionally including a port.
    pub host: String,
    /// SMTP authentication identity.
    pub username: String,
    /// SMTP credential; never log or cache in browser storage.
    pub password: String,
    /// SMTP envelope sender and default visible From.
    pub from_address: String,
    /// Explicit legacy exception to SMTP certificate verification.
    pub ignore_cert_errors: bool,
    /// Ordered custom message headers.
    pub headers: Vec<Header>,
}
/// Resource-specific content. No unrelated fields can be mixed between families.
#[derive(Clone, PartialEq, Eq)]
pub enum Content {
    /// Recipient collection.
    Group(Vec<Recipient>),
    /// Email template and attachments.
    Template(Template),
    /// Landing-page content and capture policy.
    Page(Page),
    /// SMTP connection and message headers.
    Profile(Profile),
}
/// Persisted resource without a caller-controlled owner.
#[derive(Clone, PartialEq, Eq)]
pub struct Resource {
    /// Persistent identifier, assigned by storage on creation.
    pub id: i64,
    /// Display name within the owning resource scope.
    pub name: String,
    /// Server-assigned modification timestamp and conditional-write revision.
    pub modified_date: String,
    /// Resource or attachment content.
    pub content: Content,
}
impl Resource {
    /// Resource family, derived from content rather than separately stored state.
    pub fn kind(&self) -> Kind {
        match self.content {
            Content::Group(_) => Kind::Group,
            Content::Template(_) => Kind::Template,
            Content::Page(_) => Kind::Page,
            Content::Profile(_) => Kind::Profile,
        }
    }
    /// Rules independent of parsing libraries; normalization is intentional and minimal.
    pub fn validate(&mut self) -> Result<(), &'static str> {
        match &mut self.content {
            Content::Group(targets) => {
                if self.name.is_empty() {
                    return Err("Group name not specified");
                }
                if targets.is_empty() {
                    return Err("No targets specified");
                }
                if targets.len() > 100_000 {
                    return Err("Group exceeds 100000 recipients");
                }
            }
            Content::Template(t) => {
                if self.name.is_empty() {
                    return Err("Template name not specified");
                }
                if t.text.is_empty() && t.html.is_empty() {
                    return Err("Need to specify at least plaintext or HTML content");
                }
            }
            Content::Page(p) => {
                if self.name.is_empty() {
                    return Err("Page Name not specified");
                }
                p.capture_credentials |= p.capture_passwords;
            }
            Content::Profile(p) => {
                if p.from_address.is_empty() {
                    return Err("No From Address specified");
                }
                if p.host.is_empty() {
                    return Err("No SMTP Host specified");
                }
                for h in &p.headers {
                    if h.key.is_empty()
                        || h.key.contains(['\r', '\n', '\0'])
                        || h.value.contains(['\r', '\n', '\0'])
                    {
                        return Err("Invalid email header");
                    }
                }
            }
        }
        Ok(())
    }
}
/// List metadata deliberately excludes bodies, attachments and credentials.
#[derive(Clone, Debug)]
pub struct Summary {
    /// Persistent identifier, assigned by storage on creation.
    pub id: i64,
    /// Display name within the owning resource scope.
    pub name: String,
    /// Server-assigned modification timestamp and conditional-write revision.
    pub modified_date: String,
    /// Number of group members without loading recipient payloads.
    pub num_targets: i64,
    /// Number of unfinished campaigns referencing this resource.
    pub active_campaigns: i64,
}
