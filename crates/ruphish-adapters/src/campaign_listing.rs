//! Preflight legacy aggregate lists before loading any payload into application memory.
use ruphish_application::resources::Error;

pub(crate) const QUERY: &str = r#"WITH selected AS (SELECT * FROM campaigns WHERE user_id=?) SELECT CAST(COALESCE(SUM(weight),0) AS BIGINT) FROM (
SELECT 1024+4*(LENGTH(COALESCE(c.name,''))+LENGTH(COALESCE(c.url,''))+LENGTH(COALESCE(c.status,''))+LENGTH(COALESCE(c.created_date,''))+LENGTH(COALESCE(c.launch_date,''))+LENGTH(COALESCE(c.send_by_date,''))+LENGTH(COALESCE(c.completed_date,''))+LENGTH(COALESCE(c.frozen_content,''))) AS weight FROM selected c
UNION ALL
SELECT 1024+4*(LENGTH(COALESCE(r.r_id,''))+LENGTH(COALESCE(r.email,''))+LENGTH(COALESCE(r.first_name,''))+LENGTH(COALESCE(r.last_name,''))+LENGTH(COALESCE(r.position,''))+LENGTH(COALESCE(r.status,''))+LENGTH(COALESCE(r.send_date,''))+LENGTH(COALESCE(r.modified_date,''))) AS weight FROM results r JOIN selected c ON c.id=r.campaign_id
UNION ALL
SELECT 1024+4*(LENGTH(COALESCE(e.email,''))+LENGTH(COALESCE(e.time,''))+LENGTH(COALESCE(e.message,''))+LENGTH(COALESCE(e.details,''))) AS weight FROM event_history e JOIN selected c ON c.id=e.campaign_id
LIMIT 100001) AS budget"#;

pub(crate) fn check(weight: i64) -> Result<(), Error> {
    if weight > 32 * 1024 * 1024 {
        return Err(Error::Invalid("Campaign listing exceeds the aggregate budget; use /api/campaigns/directory and per-campaign routes".into()));
    }
    Ok(())
}
