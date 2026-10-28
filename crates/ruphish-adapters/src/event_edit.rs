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
/// Only repository-owned literals are translated; no caller data enters SQL text.
pub(crate) fn sql(statement: &str, postgres: bool) -> String {
    if !postgres {
        return statement.to_owned();
    }
    let mut n = 0;
    statement
        .chars()
        .map(|c| {
            if c == '?' {
                n += 1;
                format!("${n}")
            } else {
                c.to_string()
            }
        })
        .collect()
}

macro_rules! repository {
    ($connection:ty, $postgres:expr) => {
        use ruphish_application::{auth::Identity, event_edit::*, resources::Error};
        use sqlx::Row;
        macro_rules! query {
            ($sql:expr) => {
                sqlx::query(sqlx::AssertSqlSafe(crate::event_edit::sql($sql, $postgres)))
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
                "SELECT * FROM ruphish_event_records WHERE campaign_id=? AND recipient_id=? AND id=?"
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
                excluded: row.try_get::<i64, _>("excluded").map_err(storage)? != 0,
                original: crate::event_edit::fact(text("time")?, text("message")?, &text("details")?),
                current: crate::event_edit::fact(
                    text("effective_time")?,
                    text("effective_message")?,
                    &text("effective_details")?,
                ),
            })
        }
        #[async_trait::async_trait]
        impl Repository for super::campaigns::Campaigns {
            async fn inspect_event(
                &self,
                caller: Identity,
                campaign: i64,
                recipient: i64,
                event: i64,
                before: Option<i64>,
            ) -> Result<Inspection, Error> {
                let mut tx = snapshot(&self.pool).await?;
                authorize(&mut tx, caller, campaign, recipient, false).await?;
                let record = load(&mut tx, campaign, recipient, event).await?;
                let mut rows = query!("SELECT revision,actor_id,performed_at,action,before_state,after_state FROM ruphish_event_changes WHERE event_id=? AND revision<? ORDER BY revision DESC LIMIT 51")
                            .bind(event).bind(before.unwrap_or(i64::MAX)).fetch_all(&mut *tx).await.map_err(storage)?;
                let more = rows.len() > 50;
                rows.truncate(50);
                let mut audit = Vec::new();
                for row in rows {
                    let before: Option<String> = row.try_get("before_state").map_err(storage)?;
                    let after: String = row.try_get("after_state").map_err(storage)?;
                    let action: String = row.try_get("action").map_err(storage)?;
                    audit.push(Audit {
                        revision: row.try_get("revision").map_err(storage)?,
                        actor: row.try_get("actor_id").map_err(storage)?,
                        performed_at: row.try_get("performed_at").map_err(storage)?,
                        action: serde_json::from_str(&action).map_err(|_| Error::Internal)?,
                        before: before
                            .map(|v| serde_json::from_str(&v))
                            .transpose()
                            .map_err(|_| Error::Internal)?,
                        after: serde_json::from_str(&after).map_err(|_| Error::Internal)?,
                    });
                }
                tx.commit().await.map_err(storage)?;
                Ok(Inspection {
                    record,
                    next_before: audit.last().map(|a| a.revision).filter(|_| more),
                    audit,
                })
            }
            async fn change_event(
                &self,
                caller: Identity,
                actor: Identity,
                campaign: i64,
                recipient: i64,
                change: Change,
                now: i64,
            ) -> Result<Record, Error> {
                let mut tx = begin(&self.pool).await?;
                let (start, end) = authorize(&mut tx, caller, campaign, recipient, true).await?;
                if actor.id != caller.id {
                    super::resources::authorize(&mut tx, actor, false)
                        .await?
                        .require("modify_system")?;
                }
                let request =
                    serde_json::to_string(&(campaign, recipient, &change)).map_err(|_| Error::Internal)?;
                if let Some(row) = query!("SELECT request_text,after_state,owner_id FROM ruphish_event_changes WHERE actor_id=? AND request_id=?")
                            .bind(actor.id).bind(&change.request_id).fetch_optional(&mut *tx).await.map_err(storage)? {
                            if row.try_get::<String,_>("request_text").map_err(storage)? != request || row.try_get::<i64,_>("owner_id").map_err(storage)? != caller.id { return Err(Error::Changed); }
                            return serde_json::from_str(&row.try_get::<String,_>("after_state").map_err(storage)?).map_err(|_|Error::Internal);
                        }
                let value = validate(&change, start, end, now).map_err(|e| Error::Invalid(e.into()))?;
                let before = if let Some(id) = change.event_id {
                    let record = load(&mut tx, campaign, recipient, id).await?;
                    if !record.editable() {
                        return Err(Error::Invalid("This event is read-only".into()));
                    }
                    if record.revision != change.revision {
                        return Err(Error::Changed);
                    }
                    Some(record)
                } else {
                    None
                };
                let mut after = if let Some(record) = &before {
                    record.clone()
                } else {
                    let fact = value.clone().ok_or(Error::Internal)?;
                    let id = insert(&mut tx, campaign, recipient, &fact).await?;
                    Record {
                        id,
                        source: "operator".into(),
                        revision: 0,
                        original: fact.clone(),
                        current: fact,
                        excluded: false,
                    }
                };
                match change.action {
                    Action::Add => (),
                    Action::Edit => {
                        after.current = value.ok_or(Error::Internal)?;
                    }
                    Action::Exclude => after.excluded = true,
                    Action::Restore => {
                        after.current = after.original.clone();
                        after.excluded = false;
                    }
                }
                after.revision = after.revision.checked_add(1).ok_or(Error::Internal)?;
                let before_json = before
                    .map(|v| serde_json::to_string(&v))
                    .transpose()
                    .map_err(|_| Error::Internal)?;
                let after_json = serde_json::to_string(&after).map_err(|_| Error::Internal)?;
                let action = serde_json::to_string(&change.action).map_err(|_| Error::Internal)?;
                let details = crate::event_edit::details(&after.current)?;
                query!("INSERT INTO ruphish_event_changes(event_id,revision,owner_id,campaign_id,recipient_id,actor_id,request_id,request_text,action,performed_at,before_state,after_state,time,message,details,excluded) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)")
                            .bind(after.id).bind(after.revision).bind(caller.id).bind(campaign).bind(recipient).bind(actor.id).bind(&change.request_id).bind(request).bind(action).bind(now).bind(before_json).bind(after_json)
                            .bind(&after.current.time).bind(after.current.kind.label()).bind(details).bind(i64::from(after.excluded)).execute(&mut *tx).await.map_err(storage)?;
                commit(tx).await?;
                Ok(after)
            }
        }
    };
}
pub(crate) use repository;
