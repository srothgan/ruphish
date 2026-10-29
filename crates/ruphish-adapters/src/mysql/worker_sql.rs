//! Worker and integration-history statements shared with physical-work coverage.
pub(crate) const DELIVERY_PREPARING: &str = "UPDATE ruphish_delivery_attempt SET state='abandoned',finished=? WHERE state='preparing' AND EXISTS(SELECT 1 FROM ruphish_delivery d WHERE d.result_id=ruphish_delivery_attempt.result_id AND d.generation=ruphish_delivery_attempt.generation AND d.state='preparing' AND d.lease_until<=? AND d.campaign_id=?)";

pub(crate) const DELIVERY_SUBMITTING: &str = "UPDATE ruphish_delivery_attempt SET state='unknown',finished=? WHERE state='submitting' AND EXISTS(SELECT 1 FROM ruphish_delivery d WHERE d.result_id=ruphish_delivery_attempt.result_id AND d.generation=ruphish_delivery_attempt.generation AND d.state='submitting' AND d.lease_until<=? AND d.campaign_id=?)";

pub(crate) const MAILBOX: &str = "UPDATE ruphish_mailbox_runs SET finished=?,code='interrupted' WHERE mailbox_id=? AND finished IS NULL";

pub(crate) const WEBHOOK: &str = "UPDATE ruphish_webhook_attempts SET finished=?,code='outcome_unconfirmed' WHERE job_id=? AND finished IS NULL";

// Evaluate fairness for eligible candidates through the campaign/last-claim index.
// Accepted and other historical states remain part of each campaign maximum.
// Keep profile-contention probes on (campaign_id,state): a profile lookup can
// otherwise make MySQL scan all historical deliveries inside the antijoin.
pub(crate) const CANDIDATE: &str = "SELECT d.*,c.user_id,c.template_id,c.page_id,c.smtp_id,c.url,r.r_id,r.email,r.first_name,r.last_name,r.position FROM ruphish_delivery d JOIN campaigns c ON c.id=d.campaign_id JOIN results r ON r.id=d.result_id WHERE (? IS NULL OR d.result_id=?) AND d.state IN ('ready','retry') AND d.due<=? AND c.status!='Completed' AND EXISTS(SELECT 1 FROM ruphish_campaign_domains cd JOIN ruphish_domains domain ON domain.id=cd.domain_id AND domain.owner_id=c.user_id WHERE cd.campaign_id=c.id AND domain.enabled=1 AND (SELECT eligible_until FROM ruphish_domain_policy WHERE id=domain.id)>?) AND NOT EXISTS(SELECT 1 FROM ruphish_delivery busy FORCE INDEX (ruphish_delivery_1) JOIN campaigns bc ON bc.id=busy.campaign_id WHERE busy.state IN ('preparing','submitting') AND bc.smtp_id=c.smtp_id) AND NOT EXISTS(SELECT 1 FROM ruphish_profile_cooldown cool JOIN smtp p ON p.id=cool.profile_id WHERE cool.profile_id=c.smtp_id AND cool.revision=p.modified_date AND (cool.blocked=1 OR cool.until_time>?)) ORDER BY (SELECT max(history.last_claim) FROM ruphish_delivery history WHERE history.campaign_id=d.campaign_id),d.due,d.result_id LIMIT 1";

// Prefer the mailbox/order index: with skewed history MySQL may otherwise scan
// newer runs for unrelated mailboxes backwards through the primary key.
pub(crate) const MAILBOX_HISTORY: &str = "SELECT * FROM ruphish_mailbox_runs FORCE INDEX (ruphish_mailbox_history) WHERE mailbox_id=? ORDER BY id DESC LIMIT 30";
