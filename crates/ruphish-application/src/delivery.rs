//! Synchronous test-email orchestration, independent of SMTP and HTTP.
use crate::{
    auth::{Identity, Service as Accounts},
    resources::*,
};
use std::sync::Arc;

/// Connected and negotiated SMTP transaction that has not transmitted content.
/// Dropping it cancels safely. Durable callers must commit their submission marker
/// before calling `submit`; uncertain outcomes must never be retried automatically.
#[async_trait::async_trait]
pub trait Submission: Send {
    /// Transmit content once and return only confirmed acceptance or typed failure.
    async fn submit(self: Box<Self>) -> Result<(), ruphish_domain::campaign::DeliveryFailure>;
}
/// Protocol preparation and the externally visible submission boundary.
#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    /// Negotiate connection, authentication, envelope and DATA without sending content.
    async fn prepare(
        &self,
        profile: &Profile,
        mail: PreparedMail,
    ) -> Result<Box<dyn Submission>, ruphish_domain::campaign::DeliveryFailure>;
}

/// Explicit profile selection, separate from validation success or failure.
pub enum ProfileSelection {
    /// Load the complete owned profile, including its backend-only credential.
    Saved(i64),
    /// Use unsaved settings; an existing identity permits retaining its saved credential.
    Draft {
        /// Existing profile identity, absent for a new profile.
        id: Option<i64>,
        /// Draft SMTP settings; an omitted password retains the saved value.
        profile: Profile<Option<String>>,
    },
}
/// A user-requested test using explicit SMTP selection and saved message content.
pub struct TestRequest {
    /// Recipient and sample personalisation fields.
    pub recipient: Recipient,
    /// Saved template name, or empty for the default test message.
    pub template_name: String,
    /// Optional saved landing-page name.
    pub page_name: String,
    /// Explicit saved or draft SMTP selection.
    pub profile: ProfileSelection,
    /// Base test URL, before recipient identifier insertion.
    pub url: String,
}
/// Fully selected input for MIME composition and persistence.
#[derive(Clone)]
pub struct MessageInput {
    /// Optional durable attempt Message-ID; test messages let Lettre generate one.
    pub message_id: Option<String>,
    /// Recipient and sample personalisation fields.
    pub recipient: Recipient,
    /// Selected saved message content.
    pub template: Template,
    /// Persistent template reference, or zero for the default test.
    pub template_id: i64,
    /// Persistent preview page reference, or zero if absent.
    pub page_id: i64,
    /// Selected SMTP configuration and credentials.
    pub profile: Profile,
    /// Base test URL, before recipient identifier insertion.
    pub url: String,
    /// Unpredictable preview identifier; never a campaign result.
    pub rid: String,
}
/// Prepared bytes prevent template failures after the external send has started.
pub struct PreparedMail {
    /// SMTP envelope sender mailbox.
    pub from: String,
    /// SMTP envelope recipient mailbox.
    pub to: String,
    /// Fully composed MIME bytes, ready for one submission.
    pub bytes: Vec<u8>,
}
/// Rendered review content; contains no transport credentials or participant input.
pub struct ReviewMail {
    /// Rendered subject.
    pub subject: String,
    /// Rendered HTML, to be displayed in a restricted preview frame.
    pub html: String,
    /// Rendered plain-text alternative.
    pub text: String,
    /// Actual SMTP envelope sender.
    pub transport_sender: String,
    /// Configured visible From header.
    pub visible_sender: String,
}
/// Bounded SMTP transport; it never retries ambiguous submissions automatically.
#[async_trait::async_trait]
pub trait Sender: Send + Sync {
    /// Decode the composed message for review without sending or persisting it.
    fn review(&self, request: &MessageInput) -> Result<ReviewMail, Error>;
    /// Complete template expansion and MIME construction before network submission.
    fn compose(&self, request: &MessageInput) -> Result<PreparedMail, Error>;
    /// Submit one composed message without automatically retrying ambiguity.
    async fn send(&self, profile: &Profile, mail: PreparedMail) -> Result<(), Error>;
    /// Verify connectivity and authentication without sending an email.
    async fn connect(&self, profile: &Profile) -> Result<(), Error>;
}
/// Stored preview identity shared by emailed links and the public preview handler.
#[async_trait::async_trait]
pub trait Previews: Send + Sync {
    /// Generate an unpredictable public preview identifier.
    fn id(&self) -> Result<String, Error>;
    /// Recognize a persisted test identifier without producing campaign events.
    async fn contains(&self, rid: &str) -> Result<bool, Error>;
    /// Persist preview state with current owner authorization.
    async fn store(&self, caller: Identity, message: &MessageInput) -> Result<(), Error>;
    /// Resolve and render a stored preview without recording campaign events.
    async fn render(&self, rid: &str) -> Result<String, Error>;
}
/// Select owned resources, compose, persist preview state, then perform one send.
#[derive(Clone)]
pub struct Service {
    accounts: Accounts,
    resources: crate::resources::Service,
    sender: Arc<dyn Sender>,
    previews: Arc<dyn Previews>,
}
impl Service {
    fn campaign_message(
        &self,
        prepared: crate::campaign::Prepared,
        recipient: Recipient,
        url: String,
    ) -> Result<MessageInput, Error> {
        let Content::Template(template) = prepared.template.content else {
            return Err(Error::Internal);
        };
        let Content::Profile(profile) = prepared.profile.content else {
            return Err(Error::Internal);
        };
        Ok(MessageInput {
            message_id: None,
            recipient,
            template,
            template_id: prepared.template.id,
            page_id: prepared.page.id,
            profile,
            url,
            rid: self.previews.id()?,
        })
    }
    /// Review a coherently resolved campaign without creating public preview state.
    pub async fn review_campaign(
        &self,
        prepared: crate::campaign::Prepared,
        url: String,
    ) -> Result<ReviewMail, Error> {
        let recipient = prepared
            .recipients
            .first()
            .cloned()
            .ok_or_else(|| Error::Invalid("Campaign audience is empty".into()))?;
        let message = self.campaign_message(prepared, recipient, url)?;
        let sender = self.sender.clone();
        self.resources
            .content_work
            .run(move || sender.review(&message))
            .await
    }
    /// Explicitly send one test from owned, revision-checked campaign selections.
    pub async fn test_campaign(
        &self,
        caller: Identity,
        prepared: crate::campaign::Prepared,
        recipient: Recipient,
        url: String,
    ) -> Result<(), Error> {
        self.authorize(caller).await?;
        let message = self.campaign_message(prepared, recipient, url)?;
        self.submit_test(caller, message).await
    }
    async fn submit_test(&self, caller: Identity, message: MessageInput) -> Result<(), Error> {
        let sender = self.sender.clone();
        let (message, mail) = self
            .resources
            .content_work
            .run(move || {
                let mail = sender.compose(&message)?;
                Ok((message, mail))
            })
            .await?;
        if message.template_id != 0 {
            self.previews.store(caller, &message).await?;
        }
        self.authorize(caller).await?;
        self.sender
            .send(&message.profile, mail)
            .await
            .map_err(|error| Error::Delivery(error.to_string()))
    }
    /// Compose the concrete dependencies for this boundary.
    pub fn new(
        accounts: Accounts,
        resources: crate::resources::Service,
        sender: Arc<dyn Sender>,
        previews: Arc<dyn Previews>,
    ) -> Self {
        Self {
            accounts,
            resources,
            sender,
            previews,
        }
    }
    async fn authorize(&self, caller: Identity) -> Result<(), Error> {
        self.accounts
            .session(caller)
            .await?
            .require("modify_objects")?;
        Ok(())
    }
    async fn named(&self, caller: Identity, kind: Kind, name: &str) -> Result<Resource, Error> {
        self.resources.get_by_name(caller, kind, name).await
    }
    async fn profile(
        &self,
        caller: Identity,
        selection: ProfileSelection,
    ) -> Result<Profile, Error> {
        let selected = match selection {
            ProfileSelection::Saved(id) => self.resources.get(caller, Kind::Profile, id).await?,
            ProfileSelection::Draft { id, profile } => {
                let previous = if let Some(id) = id {
                    Some(self.resources.get(caller, Kind::Profile, id).await?)
                } else {
                    None
                };
                let draft = Input {
                    id: id.unwrap_or_default(),
                    name: String::new(),
                    modified_date: String::new(),
                    content: Content::Profile(profile),
                };
                self.resources
                    .validate(draft)
                    .await?
                    .resolve_password(previous.as_ref())
            }
        };
        match selected.content {
            Content::Profile(p) => Ok(p),
            _ => Err(Error::Internal),
        }
    }
    /// Verify connectivity and authentication without sending an email or saving draft settings.
    pub async fn connect(
        &self,
        caller: Identity,
        selection: ProfileSelection,
    ) -> Result<(), Error> {
        self.authorize(caller).await?;
        let profile = self.profile(caller, selection).await?;
        self.sender.connect(&profile).await
    }
    /// Submit one composed message without automatically retrying ambiguity.
    pub async fn send(&self, caller: Identity, request: TestRequest) -> Result<(), Error> {
        self.authorize(caller).await?;
        let (template_id, template) = if request.template_name.is_empty() {
            (0,Template{subject:"Default Email from Gophish".into(),text:"It works!\n\nThis test confirms that your sending profile can submit email.\n\nFrom: {{.From}}\nRecipient: {{.Email}}\n{{.FirstName}} {{.LastName}}\n{{.Position}}".into(),..Template::default()})
        } else {
            let resource = self
                .named(caller, Kind::Template, &request.template_name)
                .await?;
            match resource.content {
                Content::Template(t) => (resource.id, t),
                _ => return Err(Error::Internal),
            }
        };
        let page_id = if request.page_name.is_empty() {
            0
        } else {
            self.named(caller, Kind::Page, &request.page_name).await?.id
        };
        let profile = self.profile(caller, request.profile).await?;
        if request.recipient.email.is_empty() {
            return Err(Error::Invalid("No email address specified".into()));
        }
        let message = MessageInput {
            message_id: None,
            recipient: request.recipient,
            template,
            template_id,
            page_id,
            profile,
            url: request.url,
            rid: if template_id == 0 {
                String::new()
            } else {
                self.previews.id()?
            },
        };
        self.submit_test(caller, message).await
    }
    /// Public preview lookup is authorized by an unpredictable preview identifier.
    pub async fn preview(&self, rid: &str) -> Result<String, Error> {
        self.previews.render(rid).await
    }
}

/// Stored preview data, fully owned and independent of a SQL connection.
pub struct PreviewSnapshot {
    /// Authored landing page.
    pub page: crate::resources::Page,
    /// Saved message context.
    pub message: MessageInput,
}
/// Preview persistence, separate from identifier allocation and HTML rendering.
#[async_trait::async_trait]
pub trait PreviewPersistence: Send + Sync {
    /// Check whether a valid preview identifier exists.
    async fn contains(&self, rid: &str) -> Result<bool, Error>;
    /// Recheck ownership and store preview state atomically.
    async fn store(&self, caller: Identity, message: &MessageInput) -> Result<(), Error>;
    /// Load the saved context and current owned page consistently.
    async fn snapshot(&self, rid: &str) -> Result<PreviewSnapshot, Error>;
}
