//! Host-bound landing rendering and atomic engagement observations.
use ruphish_application::{
    delivery::MessageInput,
    interactions::*,
    resources::{Content, Error, Kind, Profile, Recipient, Template},
};
use sqlx::{Row, SqlitePool};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

/// Public application adapter shares the startup-owned database pool.
pub struct Interactions {
    pool: SqlitePool,
    rendering: std::sync::Arc<tokio::sync::Semaphore>,
}
impl Interactions {
    /// Attach public operations to the migrated startup pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            rendering: std::sync::Arc::new(tokio::sync::Semaphore::new(8)),
        }
    }
}
fn storage(_: sqlx::Error) -> Error {
    Error::Internal
}
fn missing() -> Error {
    Error::NotFound("Interaction")
}

#[async_trait::async_trait]
impl Public for Interactions {
    async fn interact(
        &self,
        authority: &str,
        rid: &str,
        action: Action,
        now: i64,
    ) -> Result<Output, Error> {
        if rid.is_empty()
            || rid.len() > 128
            || !rid.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        {
            return Err(missing());
        }
        let origin = ruphish_domain::domains::origin(&format!("https://{authority}"), false)
            .map_err(|_| missing())?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let row=sqlx::query("SELECT r.id AS result_id,r.campaign_id,r.user_id,r.email,r.first_name,r.last_name,r.position,r.send_date,c.url,c.page_id,c.smtp_id,c.template_id,cp.capture_credentials,cp.capture_passwords,cp.redirect_url FROM results r JOIN campaigns c ON c.id=r.campaign_id AND c.user_id=r.user_id JOIN ruphish_campaign_domains cd ON cd.campaign_id=c.id JOIN ruphish_domains d ON d.id=cd.domain_id AND d.owner_id=c.user_id JOIN ruphish_campaign_policy cp ON cp.campaign_id=c.id WHERE r.r_id=? AND c.status!='Completed' AND d.origin=? AND d.enabled=1 AND d.verified_until>0").bind(rid).bind(&origin.value).fetch_all(&mut *tx).await.map_err(storage)?;
        if row.len() != 1 {
            return Err(missing());
        }
        let row = &row[0];
        let result_id: i64 = row.try_get("result_id").map_err(storage)?;
        let campaign_id: i64 = row.try_get("campaign_id").map_err(storage)?;
        let owner: i64 = row.try_get("user_id").map_err(storage)?;
        let text = |key: &str| {
            row.try_get::<Option<String>, _>(key)
                .map(|v| v.unwrap_or_default())
                .map_err(storage)
        };
        let email = text("email")?;
        if matches!(action, Action::Inspect) {
            return Ok(Output::Transparency {
                send_date: text("send_date")?,
            });
        }
        if matches!(action, Action::Head) {
            return Ok(Output::Empty);
        }
        let captures: bool = row.try_get("capture_credentials").map_err(storage)?;
        let passwords: bool = row.try_get("capture_passwords").map_err(storage)?;
        let mut rendering = None;
        if matches!(action, Action::View | Action::Submit { .. }) {
            let resource = super::resources::load(
                &mut tx,
                owner,
                Kind::Page,
                row.try_get("page_id").map_err(storage)?,
            )
            .await?;
            let Content::Page(page) = resource.content else {
                return Err(Error::Internal);
            };
            let sender: Option<String> =
                sqlx::query_scalar("SELECT from_address FROM smtp WHERE id=? AND user_id=?")
                    .bind(row.try_get::<i64, _>("smtp_id").map_err(storage)?)
                    .bind(owner)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(storage)?;
            let visible: Option<String> = sqlx::query_scalar(
                "SELECT envelope_sender FROM templates WHERE id=? AND user_id=?",
            )
            .bind(row.try_get::<i64, _>("template_id").map_err(storage)?)
            .bind(owner)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?;
            let message = MessageInput {
                message_id: None,
                recipient: Recipient {
                    email: email.clone(),
                    first_name: text("first_name")?,
                    last_name: text("last_name")?,
                    position: text("position")?,
                },
                template: Template {
                    envelope_sender: visible.unwrap_or_default(),
                    ..Default::default()
                },
                template_id: 0,
                page_id: resource.id,
                profile: Profile {
                    from_address: sender.unwrap_or_else(|| "training@example.invalid".into()),
                    ..Default::default()
                },
                url: text("url")?,
                rid: rid.into(),
            };
            rendering = Some((
                message,
                text("redirect_url")?,
                matches!(action, Action::View).then_some(page.html),
            ));
        }
        tx.commit().await.map_err(storage)?;
        // The snapshot owns its data. Context construction (including URL templates),
        // template execution and final HTML sanitization all run without a connection.
        let (document, destination) = if let Some((message, redirect, source)) = rendering {
            // A cancelled HTTP request must not release capacity while its blocking
            // parser is still running. The task owns the permit until it finishes.
            let permit = self
                .rendering
                .clone()
                .try_acquire_owned()
                .map_err(|_| Error::Internal)?;
            let endpoint = format!("{}/?rid={rid}", origin.value);
            let base = origin.value.clone();
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                let context = crate::smtp::context(&message)?;
                let redirect =
                    crate::resources::render(&redirect, &context).map_err(|_| missing())?;
                let destination = crate::landing::redirect(&redirect, &base)?;
                let document = source
                    .map(|source| {
                        let html =
                            crate::resources::render(&source, &context).map_err(|_| missing())?;
                        crate::landing::render(&html, Some(&endpoint))
                    })
                    .transpose()?;
                Ok::<_, Error>((document, destination))
            })
            .await
            .map_err(|_| Error::Internal)??
        } else {
            (None, None)
        };
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage)?;
        // Completion and disabling must win over a render that started earlier.
        let active:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM campaigns c JOIN ruphish_campaign_domains cd ON cd.campaign_id=c.id JOIN ruphish_domains d ON d.id=cd.domain_id WHERE c.id=? AND c.status!='Completed' AND d.enabled=1 AND d.origin=?)").bind(campaign_id).bind(&origin.value).fetch_one(&mut *tx).await.map_err(storage)?;
        if !active {
            return Err(missing());
        }
        let mut details = String::new();
        let message = match &action {
            Action::View => Some("Clicked Link"),
            Action::Open => Some("Email Opened"),
            Action::Report => Some("Email Reported"),
            Action::Submit {
                event_id,
                password_present,
            } => {
                if event_id.len() != 32 || !event_id.bytes().all(|c| c.is_ascii_hexdigit()) {
                    return Err(Error::Invalid("submission_invalid_id".into()));
                }
                if captures {
                    let count: i64 = sqlx::query_scalar(
                        "SELECT count(*) FROM ruphish_submission_ids WHERE result_id=?",
                    )
                    .bind(result_id)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(storage)?;
                    let duplicate: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ruphish_submission_ids WHERE result_id=? AND event_id=?)")
                        .bind(result_id).bind(event_id).fetch_one(&mut *tx).await.map_err(storage)?;
                    if count >= 1000 && !duplicate {
                        return Err(Error::Invalid("interaction_limit_reached".into()));
                    }
                    let inserted =
                        sqlx::query("INSERT OR IGNORE INTO ruphish_submission_ids VALUES(?,?)")
                            .bind(result_id)
                            .bind(event_id)
                            .execute(&mut *tx)
                            .await
                            .map_err(storage)?
                            .rows_affected()
                            == 1;
                    if passwords {
                        details =
                            serde_json::json!({"password_present":password_present}).to_string();
                    }
                    if inserted {
                        Some("Submitted Data")
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(message) = message {
            // Bound repeated scanner requests without retaining addresses or user agents.
            let count: i64 =
                sqlx::query_scalar("SELECT count(*) FROM events WHERE campaign_id=? AND email=?")
                    .bind(campaign_id)
                    .bind(&email)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(storage)?;
            if count >= 10000 {
                return Err(Error::Invalid("interaction_limit_reached".into()));
            }
            {
                let stamp = OffsetDateTime::from_unix_timestamp(now)
                    .map_err(|_| Error::Internal)?
                    .format(&Rfc3339)
                    .map_err(|_| Error::Internal)?;
                let inserted = sqlx::query(
                    "INSERT INTO events(campaign_id,email,time,message,details) VALUES(?,?,?,?,?)",
                )
                .bind(campaign_id)
                .bind(&email)
                .bind(&stamp)
                .bind(message)
                .bind(details)
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
                sqlx::query("UPDATE ruphish_event_sources SET source='public' WHERE event_id=?")
                    .bind(inserted.last_insert_rowid())
                    .execute(&mut *tx)
                    .await
                    .map_err(storage)?;
                sqlx::query("UPDATE results SET reported=CASE WHEN ?='Email Reported' THEN 1 ELSE reported END,status=CASE WHEN ?='Email Reported' THEN status WHEN status='Submitted Data' THEN status WHEN ?='Submitted Data' THEN ? WHEN status='Clicked Link' THEN status ELSE ? END,modified_date=? WHERE id=?").bind(message).bind(message).bind(message).bind(message).bind(message).bind(stamp).bind(result_id).execute(&mut *tx).await.map_err(storage)?;
            }
        }
        tx.commit().await.map_err(storage)?;
        match action {
            Action::View => {
                let document = document.ok_or(Error::Internal)?;
                Ok(Output::Html {
                    html: document.html,
                    csp: document.csp,
                })
            }
            Action::Open => Ok(Output::Pixel),
            Action::Submit { .. } => Ok(Output::Submitted {
                redirect: destination,
            }),
            _ => Ok(Output::Empty),
        }
    }
}

#[cfg(test)]
mod tests;
