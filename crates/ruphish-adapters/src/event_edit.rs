//! One transaction algorithm shared by the three SQL providers.
use ruphish_domain::event_edit::{Fact, Presence};

pub(crate) fn fact(time: String, message: String, details: &str) -> Fact {
    let safe: serde_json::Value =
        serde_json::from_str(&crate::participant_privacy::details(details)).unwrap_or_default();
    Fact {
        time,
        kind: ruphish_domain::events::RecordedKind::from_label(message),
        presence: Presence {
            username_present: safe["username_present"].as_bool(),
            password_present: safe["password_present"].as_bool(),
        },
    }
}
pub(crate) fn details(fact: &Fact) -> Result<String, ruphish_application::resources::Error> {
    serde_json::to_string(&ruphish_domain::events::Metadata {
        username_present: fact.presence.username_present,
        password_present: fact.presence.password_present,
        ..Default::default()
    })
    .map_err(|_| ruphish_application::resources::Error::Internal)
}
macro_rules! repository {
    ($connection:ty, $postgres:expr) => {
        use ruphish_application::{auth::Identity, event_edit::*, resources::Error};
        use ruphish_domain::events::RecordedKind;
        use sqlx::Row;
        macro_rules! query {
            ($sql:expr) => {
                sqlx::query(sqlx::AssertSqlSafe(crate::sql::placeholders($sql, $postgres)))
            };
        }
        async fn authorize(
            c: &mut $connection,
            caller: Identity,
            campaign: i64,
            recipient: i64,
            write: bool,
        ) -> Result<
            (
                Option<ruphish_application::datetime::Timestamp>,
                Option<ruphish_application::datetime::Timestamp>,
            ),
            Error,
        > {
            super::resources::authorize(c, caller, write).await?;
            let row = query!("SELECT c.launch_date,c.created_date,c.completed_date FROM campaigns c JOIN results r ON r.campaign_id=c.id WHERE c.user_id=? AND c.id=? AND r.id=?")
                        .bind(caller.id).bind(campaign).bind(recipient).fetch_optional(c).await.map_err(storage)?.ok_or(Error::NotFound("Recipient"))?;
            let parse = |column| -> Option<ruphish_application::datetime::Timestamp> {
                row.try_get::<Option<String>, _>(column)
                    .ok()
                    .flatten()
                    .and_then(|v| ruphish_application::datetime::parse_stored(&v).ok())
                    .filter(|v| *v != ruphish_application::datetime::ZERO_TIMESTAMP)
            };
            Ok((
                parse("launch_date").or_else(|| parse("created_date")),
                parse("completed_date"),
            ))
        }
        pub(super) async fn load(
            c: &mut $connection,
            campaign: i64,
            recipient: i64,
            id: i64,
        ) -> Result<Record, Error> {
            let row = query!(
                "SELECT * FROM event_history WHERE campaign_id=? AND recipient_id=? AND id=?"
            )
            .bind(campaign)
            .bind(recipient)
            .bind(id)
            .fetch_optional(c)
            .await
            .map_err(storage)?
            .ok_or(Error::NotFound("Event"))?;
            let text = |column| -> Result<String, Error> {
                Ok(row
                    .try_get::<Option<String>, _>(column)
                    .map_err(storage)?
                    .unwrap_or_default())
            };
            Ok(Record {
                id,
                source: text("source")?,
                revision: row.try_get("revision").map_err(storage)?,
                edited: row.try_get::<bool, _>("edited").map_err(storage)?,
                current: crate::event_edit::fact(text("time")?, text("message")?, &text("details")?),
            })
        }
        #[async_trait::async_trait]
        impl Repository for super::campaigns::Campaigns {
            async fn change_event(
                &self, caller: Identity, campaign: i64, recipient: i64, change: Change, now: i64,
            ) -> Result<Outcome, Error> {
                let mut tx = begin(&self.pool).await?;
                let (start, end) = authorize(&mut tx, caller, campaign, recipient, true).await?;
                let value = validate(&change, start, end, now).map_err(|e| Error::Invalid(e.into()))?;
                if change.action == Action::Add {
                    let fact = value.ok_or(Error::Internal)?;
                    if let Some(id) = query!("SELECT id FROM events WHERE campaign_id=? AND recipient_id=? AND create_key=?")
                        .bind(campaign).bind(recipient).bind(&change.request_id).fetch_optional(&mut *tx).await.map_err(storage)? {
                        let record = load(&mut tx, campaign, recipient, id.try_get("id").map_err(storage)?).await?;
                        if record.revision != 0 || !record.current.same_values(&fact) { return Err(Error::Changed); }
                        return Ok(Outcome { id: record.id, event: Some(record) });
                    }
                    let id = insert(&mut tx, campaign, recipient, &fact, &change.request_id).await?;
                    let record = Record { id, source: "operator".into(), revision: 0, edited: false, current: fact };
                    commit(tx).await?;
                    return Ok(Outcome { id, event: Some(record) });
                }
                let id = change.event_id.ok_or(Error::Internal)?;
                let mut record = load(&mut tx, campaign, recipient, id).await?;
                if !record.editable() { return Err(Error::Invalid("This event is read-only".into())); }
                if record.revision != change.revision {
                    // An acknowledgement may have been lost; matching the requested next
                    // version is safe to acknowledge without writing anything again.
                    if change.action == Action::Edit && change.revision.checked_add(1) == Some(record.revision)
                        && value.as_ref().is_some_and(|v| record.current.same_values(v)) {
                        return Ok(Outcome { id, event: Some(record) });
                    }
                    return Err(Error::Changed);
                }
                if change.action == Action::Delete {
                    query!("DELETE FROM ruphish_webhook_attempts WHERE job_id IN (SELECT id FROM ruphish_webhook_jobs WHERE event_id=?)")
                        .bind(id).execute(&mut *tx).await.map_err(storage)?;
                    query!("DELETE FROM ruphish_webhook_jobs WHERE event_id=?")
                        .bind(id).execute(&mut *tx).await.map_err(storage)?;
                    query!("DELETE FROM ruphish_event_receipts WHERE event_id=?")
                        .bind(id).execute(&mut *tx).await.map_err(storage)?;
                    query!("DELETE FROM events WHERE id=? AND campaign_id=? AND recipient_id=?")
                        .bind(id).bind(campaign).bind(recipient).execute(&mut *tx).await.map_err(storage)?;
                    commit(tx).await?;
                    return Ok(Outcome { id, event: None });
                }
                let fact = value.ok_or(Error::Internal)?;
                if record.current.same_values(&fact) { return Ok(Outcome { id, event: Some(record) }); }
                let revision = record.revision.checked_add(1).ok_or(Error::Internal)?;
                let RecordedKind::Known(kind) = fact.kind else { return Err(Error::Internal); };
                let metadata = crate::event_edit::details(&fact)?;
                query!("UPDATE events SET time=?,kind=?,metadata=?,edited=TRUE,revision=? WHERE id=? AND campaign_id=? AND recipient_id=?")
                    .bind(&fact.time).bind(kind.key()).bind(metadata).bind(revision).bind(id).bind(campaign).bind(recipient)
                    .execute(&mut *tx).await.map_err(storage)?;
                // Do not send edited values through an as-yet-undelivered original job.
                query!("DELETE FROM ruphish_webhook_attempts WHERE job_id IN (SELECT id FROM ruphish_webhook_jobs WHERE event_id=?)")
                    .bind(id).execute(&mut *tx).await.map_err(storage)?;
                query!("DELETE FROM ruphish_webhook_jobs WHERE event_id=?")
                    .bind(id).execute(&mut *tx).await.map_err(storage)?;
                record.current = fact;
                record.revision = revision;
                record.edited = true;
                commit(tx).await?;
                Ok(Outcome { id, event: Some(record) })
            }
        }
    };
}
pub(crate) use repository;
