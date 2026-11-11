//! Shared inventory and domain usage reads.
pub(crate) const TEMPLATES: &str = "SELECT count(*) FROM templates";
pub(crate) const PAGES: &str = "SELECT count(*) FROM pages";
pub(crate) const GROUPS: &str = "SELECT count(*) FROM groups";
pub(crate) const PROFILES: &str = "SELECT count(*) FROM smtp";
pub(crate) const DOMAINS: &str = "SELECT d.*,(SELECT verification_required FROM ruphish_domain_policy WHERE id=d.id) AS verification_required,(SELECT eligible_until FROM ruphish_domain_policy WHERE id=d.id) AS eligible_until,(SELECT expires_at FROM ruphish_domain_policy WHERE id=d.id) AS expires_at,a.id AS active_id,a.started_at AS active_started_at,a.scheduled_at AS active_scheduled_at,(SELECT count(*) FROM ruphish_campaign_domains cd JOIN campaigns c ON c.id=cd.campaign_id WHERE cd.domain_id=d.id) AS campaigns FROM ruphish_domains d LEFT JOIN ruphish_domain_checks a ON a.domain_id=d.id AND a.finished_at IS NULL";
