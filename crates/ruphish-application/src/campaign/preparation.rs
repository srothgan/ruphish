//! Campaign preparation rules shared by all persistence providers.
use crate::{
    campaign::{Create, InteractionPolicy, PreparedGroup},
    resources::{Content, Error, Recipient, Resource},
};
use std::collections::HashMap;
/// Validate required selections and launch ordering.
pub fn validate(input: &Create, now: i64) -> Result<i64, Error> {
    let launch = input.launch.unwrap_or(now);
    ruphish_domain::campaign::Fields {
        name: &input.name,
        groups: !input.groups.is_empty(),
        template: input.template.id > 0,
        page: input.page.id > 0,
        smtp: input.profile.id > 0,
        launch,
        send_by: input.send_by,
    }
    .validation()
    .finish()?;
    Ok(launch)
}
/// Deduplicated audience and conflicting attributes from the selected groups.
#[derive(Default)]
pub struct Audience {
    /// First occurrence of each exact email identity.
    pub recipients: Vec<Recipient>,
    /// First hundred conflicting attribute pairs for review.
    pub conflicts: Vec<(Recipient, Recipient)>,
    /// Total conflicts, including those beyond the review limit.
    pub conflict_count: usize,
}
// Campaign and per-group limits are separate business rules.
const MAX_CAMPAIGN_RECIPIENTS: usize = 100_000;

/// Aggregate resolved groups incrementally, rejecting before later groups are loaded.
#[derive(Default)]
pub struct AudienceBuilder {
    audience: Audience,
    seen: HashMap<String, usize>,
}

impl AudienceBuilder {
    /// Consume a group, retaining only its review metadata and first unique recipients.
    pub fn add_group(&mut self, group: Resource) -> Result<PreparedGroup, Error> {
        let Content::Group(rows) = group.content else {
            return Err(Error::Internal);
        };
        let recipient_count = rows.len();
        self.extend(rows)?;
        Ok(PreparedGroup {
            id: group.id,
            name: group.name,
            modified_date: group.modified_date,
            recipient_count,
        })
    }

    fn extend(&mut self, rows: impl IntoIterator<Item = Recipient>) -> Result<(), Error> {
        for recipient in rows {
            if let Some(&index) = self.seen.get(&recipient.email) {
                let first = &self.audience.recipients[index];
                if first != &recipient {
                    self.audience.conflict_count += 1;
                    if self.audience.conflicts.len() < 100 {
                        self.audience.conflicts.push((first.clone(), recipient));
                    }
                }
            } else {
                if self.audience.recipients.len() == MAX_CAMPAIGN_RECIPIENTS {
                    return Err(Error::Invalid(format!(
                        "Campaign exceeds {MAX_CAMPAIGN_RECIPIENTS} recipients"
                    ))
                    .at("selection.groups"));
                }
                self.seen
                    .insert(recipient.email.clone(), self.audience.recipients.len());
                self.audience.recipients.push(recipient);
            }
        }
        Ok(())
    }

    /// Require a nonempty final audience and release its temporary identity index.
    pub fn finish(self) -> Result<Audience, Error> {
        if self.audience.recipients.is_empty() {
            return Err(Error::Invalid("No targets specified".into()).at("selection.groups"));
        }
        Ok(self.audience)
    }
}
/// Resolve optional page defaults while enforcing presence-only tracking dependencies.
pub fn policy(input: &Create, page: &Resource) -> Result<InteractionPolicy, Error> {
    let Content::Page(defaults) = &page.content else {
        return Err(Error::Internal);
    };
    ruphish_domain::campaign::policy_validation(
        input.policy.learning_page_id,
        input.policy.redirect_url.as_deref().unwrap_or(""),
    )
    .finish()?;
    let policy = InteractionPolicy {
        track_opens: input.policy.track_opens,
        learning_page_id: input.policy.learning_page_id,
        redirect_url: input.policy.redirect_url.clone().unwrap_or_else(|| {
            if input.policy.learning_page_id.is_some() {
                String::new()
            } else {
                defaults.redirect_url.clone()
            }
        }),
    };
    Ok(policy)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipient(id: usize) -> Recipient {
        Recipient {
            email: format!("recipient-{id}@example.test"),
            first_name: format!("First {id}"),
            last_name: String::new(),
            position: String::new(),
        }
    }

    fn group(id: i64, rows: Vec<Recipient>) -> Resource {
        Resource {
            id,
            name: format!("Group {id}"),
            modified_date: "2026-10-28T12:00:00Z".into(),
            content: Content::Group(rows),
        }
    }

    #[test]
    fn exact_cap_accepts_duplicates_and_preserves_group_review_counts() {
        let mut builder = AudienceBuilder::default();
        let first = builder
            .add_group(group(7, (0..100_000).map(recipient).collect()))
            .unwrap();
        let duplicate = builder
            .add_group(group(8, vec![recipient(0), recipient(99_999)]))
            .unwrap();
        assert_eq!(
            (first.id, first.name.as_str(), first.recipient_count),
            (7, "Group 7", 100_000)
        );
        assert_eq!(first.modified_date, "2026-10-28T12:00:00Z");
        assert_eq!(duplicate.recipient_count, 2);
        let audience = builder.finish().unwrap();
        assert_eq!(audience.recipients.len(), 100_000);
        assert_eq!(audience.recipients[0].email, "recipient-0@example.test");
        assert_eq!(
            audience.recipients[99_999].email,
            "recipient-99999@example.test"
        );
        assert_eq!(audience.conflict_count, 0);
        assert!(audience.conflicts.is_empty());
    }

    #[test]
    fn oversized_tail_stops_at_the_first_excess_unique_recipient() {
        let visited = std::cell::Cell::new(0);
        let rows = (0..1_000_000)
            .map(recipient)
            .inspect(|_| visited.set(visited.get() + 1));
        let mut builder = AudienceBuilder::default();
        let failure = builder.extend(rows).unwrap_err();
        assert_eq!(failure.to_string(), "Campaign exceeds 100000 recipients");
        assert_eq!(visited.get(), 100_001);
        assert_eq!(builder.audience.recipients.len(), 100_000);
    }

    #[test]
    fn conflicts_keep_first_occurrence_order_and_only_the_first_hundred_pairs() {
        let mut builder = AudienceBuilder::default();
        builder
            .add_group(group(1, vec![recipient(2), recipient(1)]))
            .unwrap();
        let rows = (0..121)
            .map(|index| {
                let mut row = recipient(if index % 2 == 0 { 1 } else { 2 });
                row.first_name = format!("Conflict {index}");
                row
            })
            .collect();
        builder.add_group(group(2, rows)).unwrap();
        let audience = builder.finish().unwrap();
        assert_eq!(audience.recipients, vec![recipient(2), recipient(1)]);
        assert_eq!(audience.conflict_count, 121);
        assert_eq!(audience.conflicts.len(), 100);
        assert_eq!(
            audience.conflicts[0],
            (
                recipient(1),
                Recipient {
                    first_name: "Conflict 0".into(),
                    ..recipient(1)
                }
            )
        );
        assert_eq!(
            audience.conflicts[99],
            (
                recipient(2),
                Recipient {
                    first_name: "Conflict 99".into(),
                    ..recipient(2)
                }
            )
        );
    }

    #[test]
    fn empty_selected_groups_still_reject_without_an_audience() {
        let mut builder = AudienceBuilder::default();
        assert_eq!(
            builder.add_group(group(1, vec![])).unwrap().recipient_count,
            0
        );
        assert!(
            matches!(builder.finish(), Err(Error::Field { field, source })
            if field == "selection.groups" && source.to_string() == "No targets specified")
        );
    }
}
