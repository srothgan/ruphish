//! Host-bound landing rendering and atomic engagement observations.
use crate::postgres::InsertId;
use ruphish_application::datetime;
use ruphish_application::{
    delivery::MessageInput,
    interactions::*,
    resources::{Content, Error, Kind, Profile, Recipient, Template},
};
use sqlx::{PgPool, Row};

pub struct Interactions {
    pool: PgPool,
}
impl Interactions {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
fn storage(error: sqlx::Error) -> Error {
    crate::postgres::resource_error(error)
}
fn missing() -> Error {
    Error::NotFound("Interaction")
}
#[async_trait::async_trait]
impl Persistence for Interactions {
    async fn snapshot(&self, origin: &str, rid: &str, action: &Action) -> Result<Snapshot, Error> {
        let mut tx = crate::postgres::locks::snapshot(&self.pool)
            .await
            .map_err(storage)?;
        let row=sqlx::query("SELECT r.id AS result_id,r.campaign_id,r.user_id,r.email,r.first_name,r.last_name,r.position,r.send_date,c.url,c.page_id,c.smtp_id,c.template_id,cp.redirect_url FROM results r JOIN campaigns c ON c.id=r.campaign_id AND c.user_id=r.user_id JOIN ruphish_campaign_domains cd ON cd.campaign_id=c.id JOIN ruphish_domains d ON d.id=cd.domain_id AND d.owner_id=c.user_id JOIN ruphish_campaign_policy cp ON cp.campaign_id=c.id WHERE r.r_id=$1 AND c.status!='Completed' AND d.origin=$2 AND d.enabled=TRUE AND d.verified_until>0").bind(rid).bind(origin).fetch_all(&mut *tx).await.map_err(storage)?;
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
                sqlx::query_scalar("SELECT from_address FROM smtp WHERE id=$1 AND user_id=$2")
                    .bind(row.try_get::<i64, _>("smtp_id").map_err(storage)?)
                    .bind(owner)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(storage)?;
            let visible: Option<String> = sqlx::query_scalar(
                "SELECT envelope_sender FROM templates WHERE id=$1 AND user_id=$2",
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
            rendering = Some(RenderInput {
                message,
                redirect: text("redirect_url")?,
                source: matches!(action, Action::View).then_some(page.html),
            });
        }
        let send_date = text("send_date")?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(Snapshot {
            result_id,
            campaign_id,
            email,
            send_date,
            rendering,
        })
    }
    async fn record(
        &self,
        snapshot: &Snapshot,
        origin: &str,
        action: &Action,
        now: i64,
    ) -> Result<(), Error> {
        let Snapshot {
            result_id,
            campaign_id,
            email,
            ..
        } = snapshot;
        let (result_id, campaign_id) = (*result_id, *campaign_id);
        let mut tx = crate::postgres::locks::write(&self.pool)
            .await
            .map_err(storage)?;
        if !super::locks::campaign(&mut tx, campaign_id, false)
            .await
            .map_err(storage)?
        {
            return Err(missing());
        }
        sqlx::query("SELECT c.id,d.id FROM campaigns c JOIN ruphish_campaign_domains cd ON cd.campaign_id=c.id JOIN ruphish_domains d ON d.id=cd.domain_id WHERE c.id=$1 FOR SHARE")
            .bind(campaign_id).fetch_optional(&mut *tx).await.map_err(storage)?;
        sqlx::query("SELECT id FROM results WHERE id=$1 FOR UPDATE")
            .bind(result_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?;
        // Completion and disabling must win over a render that started earlier.
        let active:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM campaigns c JOIN ruphish_campaign_domains cd ON cd.campaign_id=c.id JOIN ruphish_domains d ON d.id=cd.domain_id WHERE c.id=$1 AND c.status!='Completed' AND d.enabled=TRUE AND d.verified_until>0 AND d.origin=$2)").bind(campaign_id).bind(origin).fetch_one(&mut *tx).await.map_err(storage)?;
        if !active {
            return Err(missing());
        }
        let Some(kind) = action.kind() else {
            return Ok(());
        };
        let message = kind.label();
        let request_id = match action {
            Action::Submit { event_id, .. } => Some(event_id.as_str()),
            _ => None,
        };
        if let Some(request_id) = request_id {
            let duplicate: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ruphish_event_receipts WHERE result_id=$1 AND request_id=$2)")
                .bind(result_id).bind(request_id).fetch_one(&mut *tx).await.map_err(storage)?;
            if duplicate {
                return Ok(());
            }
        }
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM events WHERE recipient_id=$1")
            .bind(result_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?;
        check_event_capacity(count, false)?;
        let details = match action {
            Action::Submit {
                password_present, ..
            } => serde_json::to_string(&ruphish_domain::events::Metadata {
                password_present: Some(*password_present),
                ..Default::default()
            })
            .map_err(|_| Error::Internal)?,
            _ => String::new(),
        };
        let stamp = datetime::utc_seconds(now).map_err(|_| Error::Internal)?;
        let inserted = sqlx::query("INSERT INTO events(campaign_id,email,time,kind,metadata,recipient_id,source) VALUES($1,$2,$3,$4,$5,$6,'public') RETURNING id")
            .bind(campaign_id).bind(email).bind(&stamp).bind(kind.key()).bind(details).bind(result_id)
            .fetch_one(&mut *tx).await.map_err(storage)?;
        let event_id = inserted.insert_id().map_err(storage)?;
        super::events::publish(&mut tx, event_id, "public")
            .await
            .map_err(storage)?;
        if let Some(request_id) = request_id {
            sqlx::query("INSERT INTO ruphish_event_receipts(result_id,request_id,event_id) VALUES($1,$2,$3)")
                .bind(result_id).bind(request_id).bind(event_id).execute(&mut *tx).await.map_err(storage)?;
        }
        sqlx::query("UPDATE results SET reported=CASE WHEN $1='Email Reported' THEN TRUE ELSE reported END,status=CASE WHEN $2='Email Reported' THEN status WHEN status='Submitted Data' THEN status WHEN $3='Submitted Data' THEN $4 WHEN status='Clicked Link' THEN status ELSE $5 END,modified_date=$6 WHERE id=$7").bind(message).bind(message).bind(message).bind(message).bind(message).bind(stamp).bind(result_id).execute(&mut *tx).await.map_err(storage)?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(())
    }
}
