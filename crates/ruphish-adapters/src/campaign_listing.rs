//! Preflight legacy aggregate lists before loading any payload into application memory.
use ruphish_application::resources::Error;

pub(crate) const QUERY: &str = r#"WITH selected AS (SELECT * FROM campaigns WHERE user_id=?) SELECT CAST(COALESCE(SUM(weight),0) AS BIGINT) FROM (
SELECT 1024+4*(LENGTH(COALESCE(c.name,''))+LENGTH(COALESCE(c.url,''))+LENGTH(COALESCE(c.status,''))+LENGTH(COALESCE(c.created_date,''))+LENGTH(COALESCE(c.launch_date,''))+LENGTH(COALESCE(c.send_by_date,''))+LENGTH(COALESCE(c.completed_date,''))) AS weight FROM selected c
UNION ALL
SELECT 1024+4*(LENGTH(COALESCE(r.r_id,''))+LENGTH(COALESCE(r.email,''))+LENGTH(COALESCE(r.first_name,''))+LENGTH(COALESCE(r.last_name,''))+LENGTH(COALESCE(r.position,''))+LENGTH(COALESCE(r.status,''))+LENGTH(COALESCE(r.send_date,''))+LENGTH(COALESCE(r.modified_date,''))) AS weight FROM results r JOIN selected c ON c.id=r.campaign_id
UNION ALL
SELECT 1024+4*(LENGTH(COALESCE(e.email,''))+LENGTH(COALESCE(e.time,''))+LENGTH(COALESCE(e.message,''))+LENGTH(COALESCE(e.details,''))) AS weight FROM events e JOIN selected c ON c.id=e.campaign_id
UNION ALL
SELECT 1024+4*(LENGTH(COALESCE(t.name,''))+LENGTH(COALESCE(t.modified_date,''))+LENGTH(COALESCE(t.subject,''))+LENGTH(COALESCE(t.text,''))+LENGTH(COALESCE(t.html,''))+LENGTH(COALESCE(t.envelope_sender,''))) AS weight FROM templates t JOIN selected c ON c.template_id=t.id AND c.user_id=t.user_id
UNION ALL
SELECT 1024+4*(LENGTH(COALESCE(p.name,''))+LENGTH(COALESCE(p.modified_date,''))+LENGTH(COALESCE(p.html,''))+LENGTH(COALESCE(p.redirect_url,''))) AS weight FROM pages p JOIN selected c ON c.page_id=p.id AND c.user_id=p.user_id
UNION ALL
SELECT 1024+4*(LENGTH(COALESCE(s.name,''))+LENGTH(COALESCE(s.modified_date,''))+LENGTH(COALESCE(s.interface_type,''))+LENGTH(COALESCE(s.host,''))+LENGTH(COALESCE(s.username,''))+LENGTH(COALESCE(s.password,''))+LENGTH(COALESCE(s.from_address,''))) AS weight FROM smtp s JOIN selected c ON c.smtp_id=s.id AND c.user_id=s.user_id
UNION ALL
SELECT 1024+4*(LENGTH(COALESCE(a.name,''))+LENGTH(COALESCE(a.content,''))+LENGTH(COALESCE(a.type,''))) AS weight FROM attachments a JOIN templates t ON t.id=a.template_id JOIN selected c ON c.template_id=t.id AND c.user_id=t.user_id
UNION ALL
SELECT 1024+4*(LENGTH(COALESCE(h.value,''))+LENGTH(COALESCE(h.key,''))) AS weight FROM headers h JOIN smtp s ON s.id=h.smtp_id JOIN selected c ON c.smtp_id=s.id AND c.user_id=s.user_id
LIMIT 100001) AS budget"#;

pub(crate) fn check(weight: i64) -> Result<(), Error> {
    if weight > 32 * 1024 * 1024 {
        return Err(Error::Invalid("Campaign listing exceeds the aggregate budget; use /api/campaigns/directory and per-campaign routes".into()));
    }
    Ok(())
}
