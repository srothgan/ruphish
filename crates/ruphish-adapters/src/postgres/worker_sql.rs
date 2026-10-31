//! Worker and integration-history statements shared with physical-work coverage.
pub(crate) const DELIVERY_PREPARING: &str = "UPDATE ruphish_delivery_attempt SET state='abandoned',finished=$1 WHERE state='preparing' AND EXISTS(SELECT 1 FROM ruphish_delivery d WHERE d.result_id=ruphish_delivery_attempt.result_id AND d.generation=ruphish_delivery_attempt.generation AND d.state='preparing' AND d.lease_until<=$2 AND d.campaign_id=$3)";

pub(crate) const DELIVERY_SUBMITTING: &str = "UPDATE ruphish_delivery_attempt SET state='unknown',finished=$1 WHERE state='submitting' AND EXISTS(SELECT 1 FROM ruphish_delivery d WHERE d.result_id=ruphish_delivery_attempt.result_id AND d.generation=ruphish_delivery_attempt.generation AND d.state='submitting' AND d.lease_until<=$2 AND d.campaign_id=$3)";

pub(crate) const MAILBOX: &str = "UPDATE ruphish_mailbox_runs SET finished=$1,code='interrupted' WHERE mailbox_id=$2 AND finished IS NULL";

pub(crate) const WEBHOOK: &str = "UPDATE ruphish_webhook_attempts SET finished=$1,code='outcome_unconfirmed' WHERE job_id=$2 AND finished IS NULL";

// Evaluate fairness for eligible candidates through the campaign/last-claim index.
// Accepted and other historical states remain part of each campaign maximum.
pub(crate) const CANDIDATE: &str = "SELECT d.*,c.user_id,c.template_id,c.page_id,c.smtp_id,c.url,(SELECT track_opens FROM ruphish_campaign_policy WHERE campaign_id=c.id) AS track_opens,r.r_id,r.email,r.first_name,r.last_name,r.position FROM ruphish_delivery d JOIN campaigns c ON c.id=d.campaign_id JOIN results r ON r.id=d.result_id WHERE ($1::bigint IS NULL OR d.result_id=$2) AND d.state IN ('ready','retry') AND d.due<=$3 AND c.status!='Completed' AND EXISTS(SELECT 1 FROM ruphish_campaign_domains cd JOIN ruphish_domains domain ON domain.id=cd.domain_id AND domain.owner_id=c.user_id WHERE cd.campaign_id=c.id AND domain.enabled=TRUE AND (SELECT eligible_until FROM ruphish_domain_policy WHERE id=domain.id)>$4) AND NOT EXISTS(SELECT 1 FROM ruphish_delivery busy JOIN campaigns bc ON bc.id=busy.campaign_id WHERE busy.state IN ('preparing','submitting') AND bc.smtp_id=c.smtp_id) AND NOT EXISTS(SELECT 1 FROM ruphish_profile_cooldown cool JOIN smtp p ON p.id=cool.profile_id WHERE cool.profile_id=c.smtp_id AND cool.revision=p.modified_date AND (cool.blocked=TRUE OR cool.until_time>$5)) ORDER BY (SELECT max(history.last_claim) FROM ruphish_delivery history WHERE history.campaign_id=d.campaign_id),d.due,d.result_id LIMIT 1";

pub(crate) const MAILBOX_HISTORY: &str =
    "SELECT * FROM ruphish_mailbox_runs WHERE mailbox_id=$1 ORDER BY id DESC LIMIT 30";
