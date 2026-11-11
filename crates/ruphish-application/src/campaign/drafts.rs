//! Durable drafts and the single transition into campaign execution.
use crate::{auth::Identity, campaign::Create, resources::Error};
pub use ruphish_domain::campaign::draft::Configuration;
pub use ruphish_domain::campaign::draft::Status;

/// A saved draft; a campaign reference fences all subsequent edits and launches.
#[derive(Clone)]
pub struct Draft {
    /// Server-assigned creator and last-editor attribution.
    pub attribution: ruphish_domain::attribution::Attribution,

    /// Database-assigned identity in the shared workspace.
    pub id: i64,
    /// Conditional-write revision.
    pub revision: i64,
    /// Incomplete, bounded input.
    pub configuration: Configuration,
    /// Last editing step, zero through three.
    pub step: i64,
    /// Creation UTC seconds.
    pub created: i64,
    /// Last saved UTC seconds.
    pub modified: i64,
    /// Completed transition, retained for retry reconciliation.
    pub campaign_id: Option<i64>,
}

/// Validate persistence independently of launch readiness.
pub fn validate(config: &Configuration, step: i64) -> Result<(), Error> {
    if !(0..=3).contains(&step) || !config.validate() {
        return Err(Error::Invalid("Invalid campaign draft".into()));
    }
    Ok(())
}

/// Decide whether a save is a mutation or an acknowledged retry of the same save.
pub fn should_write(
    existing: &Draft,
    expected: Option<i64>,
    configuration: &Configuration,
    step: i64,
) -> Result<bool, Error> {
    if existing.campaign_id.is_some() {
        return Err(Error::Changed);
    }
    match crate::resources::require_revision(existing.revision, expected) {
        Ok(()) => Ok(true),
        Err(Error::Changed)
            if expected.is_some_and(|revision| {
                revision > 0 && revision.checked_add(1) == Some(existing.revision)
            }) && &existing.configuration == configuration
                && existing.step == step =>
        {
            Ok(false)
        }
        Err(error) => Err(error),
    }
}

/// Resolve the saved settings and reject any launch that changes the saved draft.
/// Reviewed resource revisions remain authoritative in the normal launch transaction.
pub fn validate_launch(
    draft: &Draft,
    revision: i64,
    input: &Create,
    now: i64,
) -> Result<(), Error> {
    if draft.revision != revision {
        return Err(Error::Changed);
    }
    if input
        .groups
        .iter()
        .chain([&input.template, &input.page, &input.profile])
        .any(|s| s.revision.as_ref().is_none_or(String::is_empty))
    {
        return Err(Error::Invalid(
            "Review the current resources before launching".into(),
        ));
    }
    if input.policy.learning_page_id.is_some()
        && input
            .learning_revision
            .as_ref()
            .is_none_or(String::is_empty)
    {
        return Err(Error::Invalid(
            "Review the learning page before launching".into(),
        ));
    }
    let c = &draft.configuration;
    let resolve = |value: &Option<ruphish_domain::campaign::draft::LocalTime>| {
        value
            .as_ref()
            .map(|value| {
                crate::datetime::resolve_local(&value.local, &value.time_zone)
                    .map(|time| time.as_second())
                    .map_err(|e| Error::Invalid(e.to_string()))
            })
            .transpose()
    };
    let launch = resolve(&c.launch_local)?;
    let end = resolve(&c.send_by_local)?;
    if launch.is_some_and(|time| time < now) {
        return Err(Error::DraftSchedulePast);
    }
    let Some(policy) = &c.policy else {
        return Err(Error::Invalid("Draft policy is incomplete".into()));
    };
    let Some(redirect_url) = &policy.redirect_url else {
        return Err(Error::Invalid("Draft policy is incomplete".into()));
    };
    if c.name != input.name
        || c.url != input.url
        || launch != input.launch
        || end != input.send_by
        || c.selection.groups.iter().map(|s| s.id).collect::<Vec<_>>()
            != input.groups.iter().map(|s| s.id).collect::<Vec<_>>()
        || c.selection.template.id != input.template.id
        || c.selection.page.id != input.page.id
        || c.selection.smtp.id != input.profile.id
        || policy.track_opens != input.policy.track_opens
        || policy.learning_page_id != input.policy.learning_page_id
        || Some(redirect_url) != input.policy.redirect_url.as_ref()
    {
        return Err(Error::Changed);
    }
    Ok(())
}

/// All operations share the configured database and current account authorization.
#[async_trait::async_trait]
pub trait Repository: Send + Sync {
    /// Create incomplete inputs once per caller/key, without execution state.
    async fn create_draft(
        &self,
        caller: Identity,
        idempotency: (String, String),
        configuration: Configuration,
        step: i64,
        now: i64,
    ) -> Result<crate::attribution::Saved<Draft>, Error>;
    /// Save an existing draft using its expected revision.
    async fn save_draft(
        &self,
        caller: Identity,
        id: i64,
        revision: Option<i64>,
        configuration: Configuration,
        step: i64,
        now: i64,
    ) -> Result<crate::attribution::Saved<Draft>, Error>;
    /// Read a shared draft, including a completed launch reference for recovery.
    async fn draft(&self, caller: Identity, id: i64) -> Result<Draft, Error>;
    /// Stable bounded page of editable drafts; fetch limit+1 to establish continuation.
    async fn drafts(&self, caller: Identity, after: i64, limit: i64) -> Result<Vec<Draft>, Error>;
    /// Delete only an unchanged, unlaunched draft.
    async fn delete_draft(&self, caller: Identity, id: i64, revision: i64) -> Result<(), Error>;
    /// Atomically launch the stored revision or return its already-created campaign.
    async fn launch_draft(
        &self,
        caller: Identity,
        id: i64,
        revision: i64,
        input: Create,
        now: i64,
    ) -> Result<crate::attribution::Saved<super::Campaign>, Error>;
}
