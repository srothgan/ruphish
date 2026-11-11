//! Shared extraction of observed evidence; progression belongs to the domain.
use ruphish_domain::reporting::{Observations, Progression};
use sqlx::{Row, postgres::PgRow};

/// Aggregate event alias `e` once per unambiguous recipient. Only explicit JSON
/// true on a submission is positive password presence; arbitrary payloads are ignored.
pub(super) const FLAGS: &str = r#"
    COALESCE(bool_or(e.message='Email Sent'),FALSE) AS accepted,
    COALESCE(bool_or(e.message='Email Opened'),FALSE) AS opened,
    COALESCE(bool_or(e.message='Clicked Link'),FALSE) AS clicked,
    COALESCE(bool_or(e.message='Submitted Data'),FALSE) AS submitted,
    COALESCE(bool_or(e.message='Email Reported'),FALSE) AS reported,
    COALESCE(bool_or(CASE WHEN e.message='Submitted Data' AND e.details IS JSON OBJECT THEN
        COALESCE(e.details::jsonb->'password_present'='true'::jsonb,FALSE)
        ELSE FALSE END),FALSE) AS credentials
"#;

/// At most 64 evidence groups, regardless of campaign size or repeated events.
pub(super) const GROUPS: &str = "SELECT accepted,opened,clicked,submitted,reported,credentials,count(*) AS recipients FROM outcomes GROUP BY accepted,opened,clicked,submitted,reported,credentials";

pub(super) fn summarize(rows: Vec<PgRow>) -> Result<Progression, sqlx::Error> {
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
