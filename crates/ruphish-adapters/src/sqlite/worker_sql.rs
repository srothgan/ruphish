//! Worker and integration-history statements shared with physical-work coverage.
pub(crate) const DELIVERY_PREPARING: &str = "UPDATE ruphish_delivery_attempt SET state='abandoned',finished=? WHERE state='preparing' AND (result_id,generation) IN (SELECT result_id,generation FROM ruphish_delivery WHERE state='preparing' AND lease_until<=?)";

pub(crate) const DELIVERY_SUBMITTING: &str = "UPDATE ruphish_delivery_attempt SET state='unknown',finished=? WHERE state='submitting' AND (result_id,generation) IN (SELECT result_id,generation FROM ruphish_delivery WHERE state='submitting' AND lease_until<=?)";

pub(crate) const MAILBOX: &str = r#"
                UPDATE ruphish_mailbox_runs
                SET finished=?,code='interrupted'
                WHERE finished IS NULL
                AND mailbox_id IN (SELECT id
                FROM ruphish_mailboxes
                WHERE lease<=?)
            "#;

pub(crate) const WEBHOOK: &str = r#"
                UPDATE ruphish_webhook_attempts
                SET finished=?,code='outcome_unconfirmed'
                WHERE finished IS NULL
                AND job_id IN (SELECT id
                FROM ruphish_webhook_jobs
                WHERE state='running'
                AND lease<=?)
            "#;

// Evaluate fairness for eligible candidates through the campaign/last-claim index.
// Accepted and other historical states remain part of each campaign maximum.
pub(crate) const CANDIDATE: &str = "SELECT d.*,c.user_id,c.template_id,c.page_id,c.smtp_id,c.url,r.r_id,r.email,r.first_name,r.last_name,r.position FROM ruphish_delivery d JOIN campaigns c ON c.id=d.campaign_id JOIN results r ON r.id=d.result_id WHERE d.state IN ('ready','retry') AND d.due<=? AND c.status!='Completed' AND EXISTS(SELECT 1 FROM ruphish_campaign_domains cd JOIN ruphish_domains domain ON domain.id=cd.domain_id AND domain.owner_id=c.user_id WHERE cd.campaign_id=c.id AND domain.enabled=1 AND (SELECT eligible_until FROM ruphish_domain_policy WHERE id=domain.id)>?) AND NOT EXISTS(SELECT 1 FROM ruphish_delivery busy JOIN campaigns bc ON bc.id=busy.campaign_id WHERE busy.state IN ('preparing','submitting') AND bc.smtp_id=c.smtp_id) AND NOT EXISTS(SELECT 1 FROM ruphish_profile_cooldown cool JOIN smtp p ON p.id=cool.profile_id WHERE cool.profile_id=c.smtp_id AND cool.revision=p.modified_date AND (cool.blocked=1 OR cool.until_time>?)) ORDER BY (SELECT max(history.last_claim) FROM ruphish_delivery history WHERE history.campaign_id=d.campaign_id),d.due,d.result_id LIMIT 1";

pub(crate) const MAILBOX_HISTORY: &str =
    "SELECT * FROM ruphish_mailbox_runs WHERE mailbox_id=? ORDER BY id DESC LIMIT 30";
