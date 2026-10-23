//! Campaign preparation rules shared by all persistence providers.
use crate::{
    campaign::{Create, InteractionPolicy},
    resources::{Content, Error, Recipient, Resource},
};
use std::collections::HashMap;
/// Validate required selections and launch ordering.
pub fn validate(input: &Create, now: i64) -> Result<i64, Error> {
    for (invalid, message) in [
        (input.name.is_empty(), "Campaign name not specified"),
        (input.groups.is_empty(), "No groups specified"),
        (
            input.template.name.is_empty() && input.template.id.is_none(),
            "No email template specified",
        ),
        (
            input.page.name.is_empty() && input.page.id.is_none(),
            "No landing page specified",
        ),
        (
            input.profile.name.is_empty() && input.profile.id.is_none(),
            "No sending profile specified",
        ),
    ] {
        if invalid {
            return Err(Error::Invalid(message.into()));
        }
    }
    let launch = input.launch.unwrap_or(now);
    if input.send_by.is_some_and(|end| end < launch) {
        return Err(Error::Invalid(
            "The launch date must be before the \"send emails by\" date".into(),
        ));
    }
    Ok(launch)
}
/// Deduplicated audience and conflicting attributes from the selected groups.
pub struct Audience {
    /// First occurrence of each exact email identity.
    pub recipients: Vec<Recipient>,
    /// First hundred conflicting attribute pairs for review.
    pub conflicts: Vec<(Recipient, Recipient)>,
    /// Total conflicts, including those beyond the review limit.
    pub conflict_count: usize,
}
/// Preserve selection order and first-record precedence; enforce audience bounds.
pub fn audience(groups: &[Resource]) -> Result<Audience, Error> {
    let mut recipients = Vec::new();
    let mut seen = HashMap::<String, Recipient>::new();
    let mut conflicts = Vec::new();
    let mut conflict_count = 0;
    for group in groups {
        if let Content::Group(rows) = &group.content {
            for r in rows {
                if let Some(first) = seen.get(&r.email) {
                    if first != r {
                        conflict_count += 1;
                        if conflicts.len() < 100 {
                            conflicts.push((first.clone(), r.clone()));
                        }
                    }
                } else {
                    seen.insert(r.email.clone(), r.clone());
                    recipients.push(r.clone());
                }
            }
        }
    }
    if recipients.is_empty() {
        return Err(Error::Invalid("No targets specified".into()));
    }
    if recipients.len() > 100_000 {
        return Err(Error::Invalid("Campaign exceeds 100000 recipients".into()));
    }
    Ok(Audience {
        recipients,
        conflicts,
        conflict_count,
    })
}
/// Resolve optional page defaults while enforcing presence-only tracking dependencies.
pub fn policy(input: &Create, page: &Resource) -> Result<InteractionPolicy, Error> {
    let Content::Page(defaults) = &page.content else {
        return Err(Error::Internal);
    };
    let policy = InteractionPolicy {
        redirect_url: input
            .policy
            .redirect_url
            .clone()
            .unwrap_or_else(|| defaults.redirect_url.clone()),
    };
    Ok(policy)
}
