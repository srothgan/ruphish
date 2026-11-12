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
    /// Reusable static educational content.
    LearningPage,
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
            Self::LearningPage => "Learning page",
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
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Attachment {
    /// Display name within the owning resource scope.
    pub name: String,
    /// Attachment MIME media type.
    pub mime_type: String,
    /// Resource or attachment content.
    pub content: String,
}
/// An additional rendered message header.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Header {
    /// Custom header name.
    pub key: String,
    /// Custom header value before template expansion.
    pub value: String,
}
/// Authored email content, including the legacy visible-From override.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Page {
    /// Authoritative authored HTML source.
    pub html: String,
    /// Templated destination after submission.
    pub redirect_url: String,
}
/// Static content with a server-assigned public identity.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LearningPage {
    /// Authored HTML; never interpreted as a recipient template.
    pub html: String,
    /// Stable opaque public identity, assigned by persistence.
    pub public_id: String,
}
/// SMTP configuration with a credential representation chosen by its boundary.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Profile<P = String> {
    /// Legacy sending interface label.
    pub interface_type: String,
    /// SMTP destination, optionally including a port.
    pub host: String,
    /// SMTP authentication identity.
    pub username: String,
    /// SMTP credential; never log or cache in browser storage.
    pub password: P,
    /// SMTP envelope sender and default visible From.
    pub from_address: String,
    /// Explicit legacy exception to SMTP certificate verification.
    pub ignore_cert_errors: bool,
    /// Ordered custom message headers.
    pub headers: Vec<Header>,
}
/// Resource-specific content. No unrelated fields can be mixed between families.
#[derive(Clone, PartialEq, Eq)]
pub enum Content<P = String, L = LearningPage> {
    /// Recipient collection.
    Group(Vec<Recipient>),
    /// Email template and attachments.
    Template(Template),
    /// Landing-page content and capture policy.
    Page(Page),
    /// Static educational content.
    LearningPage(L),
    /// SMTP connection and message headers.
    Profile(Profile<P>),
}
/// Resource data without a caller-controlled owner, using stored credentials by default.
#[derive(Clone, PartialEq, Eq)]
pub struct Resource<P = String> {
    /// Server-assigned creator and last-editor attribution.
    pub attribution: crate::attribution::Attribution,
    /// Monotonic configuration revision, independent of timestamps.
    pub revision: i64,

    /// Persistent identifier, assigned by storage on creation.
    pub id: i64,
    /// Display name within the owning resource scope.
    pub name: String,
    /// Server-assigned modification timestamp.
    pub modified_date: String,
    /// Resource or attachment content.
    pub content: Content<P>,
}
/// Authored learning content, before a public identity is assigned.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LearningPageInput {
    /// Authored static HTML.
    pub html: String,
}

/// Writable resource content; identity, attribution and revisions are server owned.
#[derive(Clone, PartialEq, Eq)]
pub struct Input {
    /// Shared display name.
    pub name: String,
    /// Replacement content and optional replacement credential.
    pub content: Content<Option<String>, LearningPageInput>,
}
impl<P, L> Content<P, L> {
    /// Derive the resource family from its content.
    pub fn kind(&self) -> Kind {
        match self {
            Self::Group(_) => Kind::Group,
            Self::Template(_) => Kind::Template,
            Self::Page(_) => Kind::Page,
            Self::LearningPage(_) => Kind::LearningPage,
            Self::Profile(_) => Kind::Profile,
        }
    }
    /// Change only credentials and learning metadata at a representation boundary.
    fn map<Q, M>(
        self,
        password: impl FnOnce(P) -> Q,
        learning: impl FnOnce(L) -> M,
    ) -> Content<Q, M> {
        match self {
            Self::Group(v) => Content::Group(v),
            Self::Template(v) => Content::Template(v),
            Self::Page(v) => Content::Page(v),
            Self::LearningPage(v) => Content::LearningPage(learning(v)),
            Self::Profile(p) => Content::Profile(Profile {
                interface_type: p.interface_type,
                host: p.host,
                username: p.username,
                password: password(p.password),
                from_address: p.from_address,
                ignore_cert_errors: p.ignore_cert_errors,
                headers: p.headers,
            }),
        }
    }
    /// Shared validation for authored content and saved records.
    pub fn validation(&self, name: &str) -> crate::validation::Report {
        let fields = match self {
            Content::Group(targets) => Fields::Group {
                name,
                count: targets.len(),
            },
            Content::Template(t) => Fields::Template {
                name,
                text: &t.text,
                html: &t.html,
            },
            Content::Page(_) | Content::LearningPage(_) => Fields::Page { name },
            Content::Profile(p) => Fields::Profile {
                from: &p.from_address,
                host: &p.host,
                headers: p
                    .headers
                    .iter()
                    .map(|h| (h.key.as_str(), h.value.as_str()))
                    .collect(),
            },
        };
        fields.validate()
    }
}
impl<P> Resource<P> {
    /// Change only the credential representation, retaining persisted metadata.
    pub fn map_password<Q>(self, map: impl FnOnce(P) -> Q) -> Resource<Q> {
        Resource {
            attribution: self.attribution,
            revision: self.revision,
            id: self.id,
            name: self.name,
            modified_date: self.modified_date,
            content: self.content.map(map, |page| page),
        }
    }
    /// Resource family, derived from content.
    pub fn kind(&self) -> Kind {
        self.content.kind()
    }
    /// Validate saved content using the same rules as input.
    pub fn validate(&mut self) -> Result<(), &'static str> {
        self.validation().finish().map_err(|e| e.message())
    }
    /// Independent field failures, excluding authored values.
    pub fn validation(&self) -> crate::validation::Report {
        self.content.validation(&self.name)
    }
}
impl<P: Into<Option<String>>> From<Resource<P>> for Input {
    fn from(value: Resource<P>) -> Self {
        Self {
            name: value.name,
            content: value
                .content
                .map(Into::into, |page| LearningPageInput { html: page.html }),
        }
    }
}
impl Input {
    /// Resource family, derived from writable content.
    pub fn kind(&self) -> Kind {
        self.content.kind()
    }
    /// Independent field failures, excluding authored values.
    pub fn validation(&self) -> crate::validation::Report {
        self.content.validation(&self.name)
    }
    /// Resolve credentials from an authorized saved record without changing their connection boundary.
    pub fn resolve_content(
        self,
        previous: Option<&Resource>,
    ) -> Result<Content, crate::validation::Report> {
        let password = match (&self.content, previous.map(|r| &r.content)) {
            (Content::Profile(profile), Some(Content::Profile(saved)))
                if profile.password.is_none() && !saved.password.is_empty() =>
            {
                use crate::mail_credentials::{Connection, smtp_address};
                let address = |host: &str| {
                    smtp_address(host).map_err(|message| {
                        let mut report = crate::validation::Report::default();
                        report.reject("host", crate::validation::Code::Invalid, message);
                        report
                    })
                };
                let (host, port) = address(&profile.host)?;
                let (old_host, old_port) = address(&saved.host)?;
                Connection {
                    host: &host,
                    port,
                    username: &profile.username,
                    implicit_tls: port == 465,
                    ignore_cert_errors: profile.ignore_cert_errors,
                }
                .retain_password(
                    &Connection {
                        host: &old_host,
                        port: old_port,
                        username: &saved.username,
                        implicit_tls: old_port == 465,
                        ignore_cert_errors: saved.ignore_cert_errors,
                    },
                    &saved.password,
                )?
            }
            (Content::Profile(profile), _) => profile.password.clone().unwrap_or_default(),
            _ => String::new(),
        };
        Ok(self.content.map(
            |_| password,
            |page| LearningPage {
                html: page.html,
                public_id: match previous.map(|r| &r.content) {
                    Some(Content::LearningPage(previous)) => previous.public_id.clone(),
                    _ => String::new(),
                },
            },
        ))
    }
}
/// List metadata deliberately excludes bodies, attachments and credentials.
#[derive(Clone, Debug)]
pub struct Summary {
    /// Server-assigned creator and last-editor attribution.
    pub attribution: crate::attribution::Attribution,
    /// Monotonic configuration revision, independent of timestamps.
    pub revision: i64,

    /// Persistent identifier, assigned by storage on creation.
    pub id: i64,
    /// Display name within the owning resource scope.
    pub name: String,
    /// Server-assigned modification timestamp.
    pub modified_date: String,
    /// Number of group members without loading recipient payloads.
    pub num_targets: i64,
    /// Number of unfinished campaigns referencing this resource.
    pub active_campaigns: i64,
}

/// Safe advisory code and optional one-based input position, never authored field values.
#[derive(Clone, Debug, serde::Serialize)]
pub struct PageWarning {
    /// Stable code translated by the admin UI.
    pub code: &'static str,
    /// Input position in the processed document, when applicable.
    pub control: Option<usize>,
}

/// Borrowed editable fields shared by wire projections and authoritative resources.
pub enum Fields<'a> {
    /// Group name and audience size.
    Group {
        /// Display name.
        name: &'a str,
        /// Number of recipients.
        count: usize,
    },
    /// Email name and alternative bodies.
    Template {
        /// Display name.
        name: &'a str,
        /// Plain-text alternative.
        text: &'a str,
        /// HTML alternative.
        html: &'a str,
    },
    /// Landing or learning page name.
    Page {
        /// Display name.
        name: &'a str,
    },
    /// Sending profile fields; credentials are not needed for these checks.
    Profile {
        /// Envelope sender.
        from: &'a str,
        /// SMTP endpoint.
        host: &'a str,
        /// Ordered custom header names and values.
        headers: Vec<(&'a str, &'a str)>,
    },
}
impl Fields<'_> {
    /// Apply the same rules regardless of create, edit, import or rendering surface.
    pub fn validate(&self) -> crate::validation::Report {
        use crate::validation::{Code, Report};
        let mut report = Report::default();
        match self {
            Self::Group { name, count } => {
                if name.is_empty() {
                    report.reject("name", Code::Required, "Group name not specified");
                }
                if *count == 0 {
                    report.reject("targets", Code::Required, "No targets specified");
                }
                if *count > 100_000 {
                    report.reject("targets", Code::TooLong, "Group exceeds 100000 recipients");
                }
            }
            Self::Template { name, text, html } => {
                if name.is_empty() {
                    report.reject("name", Code::Required, "Template name not specified");
                }
                if text.is_empty() && html.is_empty() {
                    report.reject(
                        "html",
                        Code::EmailContent,
                        "Need to specify at least plaintext or HTML content",
                    );
                    report.reject(
                        "text",
                        Code::EmailContent,
                        "Need to specify at least plaintext or HTML content",
                    );
                }
            }
            Self::Page { name } => {
                if name.is_empty() {
                    report.reject("name", Code::Required, "Page Name not specified");
                }
            }
            Self::Profile {
                from,
                host,
                headers,
            } => {
                if from.is_empty() {
                    report.reject("from_address", Code::Required, "No From Address specified");
                }
                if host.is_empty() {
                    report.reject("host", Code::Required, "No SMTP Host specified");
                }
                for (index, (key, value)) in headers.iter().enumerate() {
                    if key.is_empty() || key.contains(['\r', '\n', '\0']) {
                        report.reject(
                            format!("headers.{index}.key"),
                            Code::Invalid,
                            "Invalid email header",
                        );
                    }
                    if value.contains(['\r', '\n', '\0']) {
                        report.reject(
                            format!("headers.{index}.value"),
                            Code::Invalid,
                            "Invalid email header",
                        );
                    }
                }
            }
        }
        report
    }
}
