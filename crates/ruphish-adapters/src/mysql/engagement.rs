//! Shared extraction of observed evidence; progression belongs to the domain.
use ruphish_domain::reporting::{Observations, Progression};
use sqlx::{Row, mysql::MySqlRow};

/// Aggregate event alias `e` once per unambiguous recipient. Only explicit JSON
/// true on a submission is positive password presence; arbitrary payloads are ignored.
pub(super) const FLAGS: &str = r#"
    max(CASE WHEN e.message='Email Sent' THEN 1 ELSE 0 END) AS accepted,
    max(CASE WHEN e.message='Email Opened' THEN 1 ELSE 0 END) AS opened,
    max(CASE WHEN e.message='Clicked Link' THEN 1 ELSE 0 END) AS clicked,
    max(CASE WHEN e.message='Submitted Data' THEN 1 ELSE 0 END) AS submitted,
    max(CASE WHEN e.message='Email Reported' THEN 1 ELSE 0 END) AS reported,
    max(CASE WHEN e.message='Submitted Data' THEN
        CASE WHEN json_valid(e.details) THEN
            CASE WHEN JSON_TYPE(JSON_EXTRACT(e.details,'$.password_present')) COLLATE utf8mb4_bin='BOOLEAN' AND JSON_EXTRACT(e.details,'$.password_present')=CAST('true' AS JSON) THEN 1 ELSE 0 END
        ELSE 0 END ELSE 0 END) AS credentials
"#;

/// At most 64 evidence groups, regardless of campaign size or repeated events.
pub(super) const GROUPS: &str = "SELECT accepted,opened,clicked,submitted,reported,credentials,count(*) AS recipients FROM outcomes GROUP BY accepted,opened,clicked,submitted,reported,credentials";

pub(super) fn summarize(rows: Vec<MySqlRow>) -> Result<Progression, sqlx::Error> {
    let mut summary = Progression::default();
    for row in rows {
        summary.include(
            Observations {
                accepted: row.try_get("accepted")?,
                opened: row.try_get("opened")?,
                clicked: row.try_get("clicked")?,
                submitted: row.try_get("submitted")?,
                reported: row.try_get("reported")?,
                credentials: row.try_get("credentials")?,
            },
            row.try_get("recipients")?,
        );
    }
    Ok(summary)
}
