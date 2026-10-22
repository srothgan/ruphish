//! Atomic campaign persistence and durable queue adapters.
use crate::postgres::InsertId;
mod directory;
mod drafts;
mod queue;
use ruphish_application::datetime;
use ruphish_application::{
    auth::Identity,
    campaign::*,
    resources::{Content, Error, Kind, Page, Profile, Recipient, Resource, Template},
};
use sqlx::{PgConnection, PgPool, Row, postgres::PgRow};

/// Campaign adapter shares the startup-owned database pool.
#[derive(Clone)]
pub struct Campaigns {
    pub(super) pool: PgPool,
}
impl Campaigns {
    /// Use the existing startup/migration lifetime.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
fn storage(error: sqlx::Error) -> Error {
    crate::postgres::resource_error(error)
}
fn text(row: &PgRow, key: &str) -> Result<String, Error> {
    Ok(row
        .try_get::<Option<String>, _>(key)
        .map_err(storage)?
        .unwrap_or_default())
}
fn stamp(seconds: i64) -> Result<String, Error> {
    datetime::utc_seconds(seconds).map_err(|_| Error::Invalid("Invalid campaign date".into()))
}
const ZERO: &str = "0001-01-01T00:00:00Z";
fn random() -> Result<String, Error> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|_| Error::Internal)?;
    Ok(hex::encode(bytes))
}
async fn selection(
    c: &mut PgConnection,
    owner: i64,
    kind: Kind,
    s: &Selection,
) -> Result<Resource, Error> {
    let table = match kind {
        Kind::Group => "groups",
        Kind::Template => "templates",
        Kind::Page => "pages",
        Kind::Profile => "smtp",
    };
    let id = if let Some(id) = s.id {
        id
    } else {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT id FROM {table} WHERE user_id=$1 AND name=$2 ORDER BY id LIMIT 1"
        )))
        .bind(owner)
        .bind(&s.name)
        .fetch_optional(&mut *c)
        .await
        .map_err(storage)?
        .ok_or(Error::NotFound(kind.label()))?
    };
    let resource = super::resources::load(c, owner, kind, id).await?;
    if s.revision
        .as_ref()
        .is_some_and(|v| v != &resource.modified_date)
    {
        return Err(Error::Changed);
    }
    Ok(resource)
}
async fn prepare_authorized(
    c: &mut PgConnection,
    caller: Identity,
    input: &Create,
    now: i64,
    lock_domain: bool,
) -> Result<Prepared, Error> {
    crate::database::text::bounded(&input.name, 255)?;
    let launch = ruphish_application::campaign::preparation::validate(input, now)?;
    let mut groups = Vec::new();
    for s in &input.groups {
        groups.push(selection(c, caller.id, Kind::Group, s).await?);
    }
    let ruphish_application::campaign::preparation::Audience {
        recipients,
        conflicts,
        conflict_count,
    } = ruphish_application::campaign::preparation::audience(&groups)?;
    let template = selection(c, caller.id, Kind::Template, &input.template).await?;
    let page = selection(c, caller.id, Kind::Page, &input.page).await?;
    let profile = selection(c, caller.id, Kind::Profile, &input.profile).await?;
    let policy = ruphish_application::campaign::preparation::policy(input, &page)?;
    // Validate syntax using the same renderer; recipient-dependent failures are still
    // checked before every individual submission.
    let sample = crate::resources::sample();
    let url = crate::campaign_preparation::url(&input.url)?;
    let domain_id = super::domains::eligible(c, caller.id, &url, now, lock_domain).await?;
    crate::resources::render(&policy.redirect_url, &sample)?;
    Ok(Prepared {
        domain_id,
        conflicts,
        conflict_count,
        groups,
        template,
        page,
        profile,
        recipients,
        policy,
        launch,
    })
}
async fn missing_resource(
    c: &mut PgConnection,
    owner: i64,
    kind: Kind,
    id: i64,
) -> Result<Resource, Error> {
    match super::resources::load(c, owner, kind, id).await {
        Ok(r) => Ok(r),
        Err(Error::NotFound(_)) => Ok(Resource {
            id: 0,
            name: "[Deleted]".into(),
            modified_date: ZERO.into(),
            content: match kind {
                Kind::Template => Content::Template(Template::default()),
                Kind::Page => Content::Page(Page::default()),
                Kind::Profile => Content::Profile(Profile::default()),
                Kind::Group => Content::Group(vec![]),
            },
        }),
        Err(e) => Err(e),
    }
}
async fn load(c: &mut PgConnection, owner: i64, id: i64) -> Result<Campaign, Error> {
    load_aggregate(c, owner, id, true).await
}
async fn load_aggregate(
    c: &mut PgConnection,
    owner: i64,
    id: i64,
    include_groups: bool,
) -> Result<Campaign, Error> {
    let row = sqlx::query("SELECT * FROM campaigns WHERE id=$1 AND user_id=$2")
        .bind(id)
        .bind(owner)
        .fetch_optional(&mut *c)
        .await
        .map_err(storage)?
        .ok_or(Error::NotFound("Campaign"))?;
    let mut resources = Vec::new();
    for (kind, column) in [
        (Kind::Template, "template_id"),
        (Kind::Page, "page_id"),
        (Kind::Profile, "smtp_id"),
    ] {
        resources.push(
            missing_resource(
                c,
                owner,
                kind,
                row.try_get::<Option<i64>, _>(column)
                    .map_err(storage)?
                    .unwrap_or(0),
            )
            .await?,
        );
    }
    let mut results = Vec::new();
    for r in sqlx::query("SELECT * FROM results WHERE campaign_id=$1 ORDER BY id")
        .bind(id)
        .fetch_all(&mut *c)
        .await
        .map_err(storage)?
    {
        results.push(RecipientResult {
            rid: text(&r, "r_id")?,
            recipient: Recipient {
                email: text(&r, "email")?,
                first_name: text(&r, "first_name")?,
                last_name: text(&r, "last_name")?,
                position: text(&r, "position")?,
            },
            status: text(&r, "status")?,
            ip: String::new(),
            latitude: 0.0,
            longitude: 0.0,
            reported: r
                .try_get::<Option<bool>, _>("reported")
                .map_err(storage)?
                .unwrap_or(false),
            send_date: text(&r, "send_date")?,
            modified_date: text(&r, "modified_date")?,
        });
    }
    let mut events = Vec::new();
    for e in sqlx::query("SELECT * FROM events WHERE campaign_id=$1 ORDER BY id")
        .bind(id)
        .fetch_all(&mut *c)
        .await
        .map_err(storage)?
    {
        events.push(Event {
            id: e.try_get("id").map_err(storage)?,
            email: text(&e, "email")?,
            time: text(&e, "time")?,
            message: text(&e, "message")?,
            details: crate::participant_privacy::details(&text(&e, "details")?),
        });
    }
    let policy = sqlx::query("SELECT * FROM ruphish_campaign_policy WHERE campaign_id=$1")
        .bind(id)
        .fetch_optional(&mut *c)
        .await
        .map_err(storage)?
        .map(|p| {
            Ok::<_, Error>(CapturePolicy {
                capture_credentials: p.try_get("capture_credentials").map_err(storage)?,
                capture_passwords: p.try_get("capture_passwords").map_err(storage)?,
                redirect_url: text(&p, "redirect_url")?,
            })
        })
        .transpose()?;
    let mut groups = Vec::new();
    if include_groups {
        let group_ids: Vec<i64> = sqlx::query_scalar(
            "SELECT group_id FROM ruphish_campaign_groups WHERE campaign_id=$1 ORDER BY ordinal",
        )
        .bind(id)
        .fetch_all(&mut *c)
        .await
        .map_err(storage)?;
        for group_id in group_ids {
            match super::resources::load(c, owner, Kind::Group, group_id).await {
                Ok(r) => groups.push(r),
                Err(Error::NotFound(_)) => {}
                Err(e) => return Err(e),
            }
        }
    }
    Ok(Campaign {
        groups,
        id,
        name: text(&row, "name")?,
        status: text(&row, "status")?,
        created: text(&row, "created_date")?,
        launch: text(&row, "launch_date")?,
        send_by: text(&row, "send_by_date")?,
        completed: text(&row, "completed_date")?,
        url: text(&row, "url")?,
        resources,
        results,
        events,
        policy,
    })
}
async fn header(c: &mut PgConnection, owner: i64, id: i64) -> Result<CampaignHeader, Error> {
    let row=sqlx::query("SELECT c.id,c.name,c.status,c.launch_date,c.send_by_date,c.url,c.template_id,c.page_id,c.smtp_id,t.name AS template_name,t.subject,t.envelope_sender,s.name AS profile_name,s.from_address,l.name AS page_name,(SELECT count(*) FROM results r WHERE r.campaign_id=c.id) AS total,p.capture_credentials,p.capture_passwords,p.redirect_url FROM campaigns c LEFT JOIN ruphish_campaign_policy p ON p.campaign_id=c.id LEFT JOIN templates t ON t.id=c.template_id AND t.user_id=c.user_id LEFT JOIN pages l ON l.id=c.page_id AND l.user_id=c.user_id LEFT JOIN smtp s ON s.id=c.smtp_id AND s.user_id=c.user_id WHERE c.id=$1 AND c.user_id=$2").bind(id).bind(owner).fetch_optional(&mut *c).await.map_err(storage)?.ok_or(Error::NotFound("Campaign"))?;
    let credentials: Option<bool> = row.try_get("capture_credentials").map_err(storage)?;
    let domain: Option<(i64,bool,i64)> = sqlx::query_as("SELECT d.id,d.enabled,d.verified_until FROM ruphish_campaign_domains cd JOIN ruphish_domains d ON d.id=cd.domain_id AND d.owner_id=$1 WHERE cd.campaign_id=$2").bind(owner).bind(id).fetch_optional(&mut *c).await.map_err(storage)?;
    let domain_id = domain.map(|d| d.0);
    let domain_block = domain
        .map_or(Err("domain_not_verified"), |(_, enabled, until)| {
            ruphish_domain::domains::eligibility(
                enabled,
                until,
                ruphish_application::datetime::now().as_second(),
            )
        })
        .err()
        .map(str::to_owned);
    Ok(CampaignHeader {
        domain_id,
        domain_block,
        id,
        name: text(&row, "name")?,
        status: text(&row, "status")?,
        launch: text(&row, "launch_date")?,
        send_by: text(&row, "send_by_date")?,
        url: text(&row, "url")?,
        template: CampaignResource {
            id: row
                .try_get::<Option<i64>, _>("template_id")
                .map_err(storage)?
                .unwrap_or(0),
            name: row.try_get("template_name").map_err(storage)?,
        },
        page: CampaignResource {
            id: row
                .try_get::<Option<i64>, _>("page_id")
                .map_err(storage)?
                .unwrap_or(0),
            name: row.try_get("page_name").map_err(storage)?,
        },
        profile: CampaignResource {
            id: row
                .try_get::<Option<i64>, _>("smtp_id")
                .map_err(storage)?
                .unwrap_or(0),
            name: row.try_get("profile_name").map_err(storage)?,
        },
        subject: row.try_get("subject").map_err(storage)?,
        profile_sender: row.try_get("from_address").map_err(storage)?,
        template_sender: row.try_get("envelope_sender").map_err(storage)?,
        total: row.try_get("total").map_err(storage)?,
        policy: credentials
            .map(|capture_credentials| {
                Ok::<_, Error>(CapturePolicy {
                    capture_credentials,
                    capture_passwords: row.try_get("capture_passwords").map_err(storage)?,
                    redirect_url: text(&row, "redirect_url")?,
                })
            })
            .transpose()?,
    })
}
#[async_trait::async_trait]
impl Repository for Campaigns {
    async fn directory(
        &self,
        caller: Identity,
        filter: ruphish_application::campaign::directory::Filter,
        after: ruphish_application::campaign::directory::Cursor,
        limit: i64,
    ) -> Result<ruphish_application::campaign::directory::Page, Error> {
        self.management_directory(caller, filter, after, limit)
            .await
    }
    async fn overview_filtered(
        &self,
        caller: Identity,
        after: i64,
        limit: i64,
        completed: Option<bool>,
    ) -> Result<CampaignOverview, Error> {
        let mut tx = crate::postgres::locks::snapshot(&self.pool)
            .await
            .map_err(storage)?;
        super::resources::authorize(&mut tx, caller, false).await?;
        let limit = limit.clamp(1, 200);
        let mut ids: Vec<i64> = sqlx::query_scalar(
            "SELECT id FROM campaigns WHERE user_id=$1 AND id>$2 AND ($3=0 OR ($4=1 AND status='Completed') OR ($5=2 AND (status IS NULL OR status<>'Completed'))) ORDER BY id LIMIT $6",
        )
        .bind(caller.id)
        .bind(after)
        .bind(completed.map(|v|if v{1i32}else{2}).unwrap_or(0))
        .bind(completed.map(|v|if v{1i32}else{2}).unwrap_or(0))
        .bind(completed.map(|v|if v{1i32}else{2}).unwrap_or(0))
        .bind(limit + 1)
        .fetch_all(&mut *tx)
        .await
        .map_err(storage)?;
        let has_more = ids.len() > limit as usize;
        ids.truncate(limit as usize);
        let next_after = ids.last().copied().filter(|_| has_more);
        let mut result = Vec::new();
        for id in ids {
            result.push(header(&mut tx, caller.id, id).await?);
        }
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(CampaignOverview {
            items: result,
            next_after,
        })
    }

    async fn prepare(&self, caller: Identity, input: &Create, now: i64) -> Result<Prepared, Error> {
        let mut tx = crate::postgres::locks::snapshot(&self.pool)
            .await
            .map_err(storage)?;
        super::resources::authorize_review(&mut tx, caller).await?;
        let value = prepare_authorized(&mut tx, caller, input, now, false).await?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(value)
    }
    async fn create(&self, caller: Identity, input: Create, now: i64) -> Result<Campaign, Error> {
        self.create_inner(caller, input, now, None).await
    }
    async fn get(&self, caller: Identity, id: i64) -> Result<Campaign, Error> {
        let mut tx = crate::postgres::locks::snapshot(&self.pool)
            .await
            .map_err(storage)?;
        super::resources::authorize(&mut tx, caller, false).await?;
        let v = load(&mut tx, caller.id, id).await?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(v)
    }
    async fn list(&self, caller: Identity) -> Result<Vec<Campaign>, Error> {
        let mut tx = crate::postgres::locks::snapshot(&self.pool)
            .await
            .map_err(storage)?;
        super::resources::authorize(&mut tx, caller, false).await?;
        let weight: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(
            crate::campaign_listing::QUERY.replace('?', "$1"),
        ))
        .bind(caller.id)
        .fetch_one(&mut *tx)
        .await
        .map_err(storage)?;
        crate::campaign_listing::check(weight)?;
        let ids: Vec<i64> =
            sqlx::query_scalar("SELECT id FROM campaigns WHERE user_id=$1 ORDER BY id")
                .bind(caller.id)
                .fetch_all(&mut *tx)
                .await
                .map_err(storage)?;
        let mut values = Vec::new();
        for id in ids {
            values.push(load_aggregate(&mut tx, caller.id, id, false).await?);
        }
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(values)
    }
    async fn complete(&self, caller: Identity, id: i64, now: i64) -> Result<(), Error> {
        let mut tx = crate::postgres::locks::write(&self.pool)
            .await
            .map_err(storage)?;
        super::resources::authorize(&mut tx, caller, true).await?;
        sqlx::query("SELECT id FROM campaigns WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?;
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM campaigns WHERE id=$1 AND user_id=$2)")
                .bind(id)
                .bind(caller.id)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage)?;
        if !exists {
            return Err(Error::NotFound("Campaign"));
        }
        sqlx::query("UPDATE campaigns SET status='Completed',completed_date=$1 WHERE id=$2 AND status!='Completed'").bind(stamp(now)?).bind(id).execute(&mut *tx).await.map_err(storage)?;
        sqlx::query("UPDATE ruphish_delivery_attempt SET state='cancelled',finished=$1 WHERE state='preparing' AND result_id IN (SELECT result_id FROM ruphish_delivery WHERE campaign_id=$2 AND state='preparing')").bind(now).bind(id).execute(&mut *tx).await.map_err(storage)?;
        sqlx::query("UPDATE ruphish_delivery SET state='cancelled',generation=generation+1,lease_until=0 WHERE campaign_id=$1 AND state IN ('ready','retry','preparing','blocked')").bind(id).execute(&mut *tx).await.map_err(storage)?;
        sqlx::query("DELETE FROM mail_logs WHERE campaign_id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(())
    }
    async fn delete(&self, caller: Identity, id: i64) -> Result<(), Error> {
        let mut tx = crate::postgres::locks::write(&self.pool)
            .await
            .map_err(storage)?;
        super::resources::authorize(&mut tx, caller, true).await?;
        sqlx::query("SELECT id FROM campaigns WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?;
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM campaigns WHERE id=$1 AND user_id=$2)")
                .bind(id)
                .bind(caller.id)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage)?;
        if !exists {
            return Err(Error::NotFound("Campaign"));
        }
        super::events::remove_campaign(&mut tx, id)
            .await
            .map_err(storage)?;
        for sql in [
            "DELETE FROM ruphish_legacy_queue_issues WHERE mail_log_id IN (SELECT id FROM mail_logs WHERE campaign_id=$1)",
            "DELETE FROM ruphish_delivery_attempt WHERE result_id IN (SELECT id FROM results WHERE campaign_id=$1)",
            "DELETE FROM ruphish_delivery WHERE campaign_id=$1",
            "DELETE FROM ruphish_submission_ids WHERE result_id IN (SELECT id FROM results WHERE campaign_id=$1)",
            "DELETE FROM ruphish_campaign_domains WHERE campaign_id=$1",
            "DELETE FROM ruphish_campaign_policy WHERE campaign_id=$1",
            "DELETE FROM ruphish_campaign_groups WHERE campaign_id=$1",
            "DELETE FROM events WHERE campaign_id=$1",
            "DELETE FROM results WHERE campaign_id=$1",
            "DELETE FROM mail_logs WHERE campaign_id=$1",
            "DELETE FROM campaigns WHERE id=$1",
        ] {
            sqlx::query(sql)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
        }
        // Idempotency tombstones intentionally survive deletion: replay cannot relaunch.
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(())
    }
    async fn deliveries(
        &self,
        caller: Identity,
        id: i64,
        after: i64,
        limit: i64,
    ) -> Result<DeliveryView, Error> {
        let mut tx = crate::postgres::locks::snapshot(&self.pool)
            .await
            .map_err(storage)?;
        super::resources::authorize(&mut tx, caller, false).await?;
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM campaigns WHERE id=$1 AND user_id=$2)")
                .bind(id)
                .bind(caller.id)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage)?;
        if !exists {
            return Err(Error::NotFound("Campaign"));
        }
        let campaign = header(&mut tx, caller.id, id).await?;
        let counts = sqlx::query_as(
            "SELECT state,count(*) FROM ruphish_delivery WHERE campaign_id=$1 GROUP BY state",
        )
        .bind(id)
        .fetch_all(&mut *tx)
        .await
        .map_err(storage)?;
        let limit = limit.clamp(1, 200);
        let mut rows=sqlx::query("SELECT d.*,r.r_id,r.email,(SELECT count(*) FROM ruphish_delivery_attempt a WHERE a.result_id=d.result_id) AS attempt_count FROM ruphish_delivery d JOIN results r ON r.id=d.result_id WHERE d.campaign_id=$1 AND d.result_id>$2 ORDER BY d.result_id LIMIT $3").bind(id).bind(after).bind(limit + 1).fetch_all(&mut *tx).await.map_err(storage)?;
        let has_more = rows.len() > limit as usize;
        rows.truncate(limit as usize);
        let mut items = Vec::new();
        for row in rows {
            let result_id = row.try_get("result_id").map_err(storage)?;
            let mut attempts = Vec::new();
            for a in sqlx::query("SELECT * FROM ruphish_delivery_attempt WHERE result_id=$1 ORDER BY generation DESC LIMIT 100").bind(result_id).fetch_all(&mut *tx).await.map_err(storage)? {
                attempts.push(decode_attempt(a)?);
            }
            items.push(DeliveryItem {
                attempt_count: row.try_get("attempt_count").map_err(storage)?,
                id: result_id,
                rid: text(&row, "r_id")?,
                email: text(&row, "email")?,
                state: text(&row, "state")?,
                due: row.try_get("due").map_err(storage)?,
                retries: row.try_get("retries").map_err(storage)?,
                generation: row.try_get("generation").map_err(storage)?,
                code: text(&row, "code")?,
                attempts,
            });
        }
        let mut activity = Vec::new();
        for e in sqlx::query("SELECT * FROM events WHERE campaign_id=$1 ORDER BY id DESC LIMIT 50")
            .bind(id)
            .fetch_all(&mut *tx)
            .await
            .map_err(storage)?
        {
            activity.push(Event {
                id: e.try_get("id").map_err(storage)?,
                email: text(&e, "email")?,
                time: text(&e, "time")?,
                message: text(&e, "message")?,
                details: crate::participant_privacy::details(&text(&e, "details")?),
            });
        }
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(DeliveryView {
            next_after: items.last().map(|r| r.id).filter(|_| has_more),
            activity,
            campaign,
            counts,
            items,
        })
    }
    async fn recipient_history(
        &self,
        caller: Identity,
        campaign: i64,
        result: i64,
        before_attempt: Option<i64>,
        before_event: Option<i64>,
    ) -> Result<RecipientHistory, Error> {
        let mut tx = crate::postgres::locks::snapshot(&self.pool)
            .await
            .map_err(storage)?;
        super::resources::authorize(&mut tx, caller, false).await?;
        let email: String = sqlx::query_scalar("SELECT r.email FROM results r JOIN campaigns c ON c.id=r.campaign_id WHERE c.user_id=$1 AND c.id=$2 AND r.id=$3")
            .bind(caller.id).bind(campaign).bind(result).fetch_optional(&mut *tx).await.map_err(storage)?.ok_or(Error::NotFound("Recipient"))?;
        let mut rows = sqlx::query("SELECT * FROM ruphish_delivery_attempt WHERE result_id=$1 AND ($2::bigint IS NULL OR generation<$3) ORDER BY generation DESC LIMIT 101")
            .bind(result).bind(before_attempt).bind(before_attempt).fetch_all(&mut *tx).await.map_err(storage)?;
        let more_attempts = rows.len() > 100;
        rows.truncate(100);
        let mut attempts = Vec::new();
        for a in rows {
            attempts.push(decode_attempt(a)?);
        }
        let mut rows = sqlx::query("SELECT * FROM events WHERE campaign_id=$1 AND email=$2 AND ($3::bigint IS NULL OR id<$4) ORDER BY id DESC LIMIT 101")
            .bind(campaign).bind(&email).bind(before_event).bind(before_event).fetch_all(&mut *tx).await.map_err(storage)?;
        let more_events = rows.len() > 100;
        rows.truncate(100);
        let mut events = Vec::new();
        for e in rows {
            events.push(Event {
                id: e.try_get("id").map_err(storage)?,
                email: text(&e, "email")?,
                time: text(&e, "time")?,
                message: text(&e, "message")?,
                details: crate::participant_privacy::details(&text(&e, "details")?),
            });
        }
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(RecipientHistory {
            result_id: result,
            email,
            next_attempt: attempts
                .last()
                .map(|a| a.generation)
                .filter(|_| more_attempts),
            next_event: events.last().map(|e| e.id).filter(|_| more_events),
            attempts,
            events,
        })
    }
    async fn resolve(
        &self,
        caller: Identity,
        campaign: i64,
        result: i64,
        generation: i64,
        action: &str,
        now: i64,
    ) -> Result<(), Error> {
        let mut tx = crate::postgres::locks::write(&self.pool)
            .await
            .map_err(storage)?;
        super::resources::authorize(&mut tx, caller, true).await?;
        sqlx::query("SELECT id FROM campaigns WHERE id=$1 AND user_id=$2 FOR UPDATE")
            .bind(campaign)
            .bind(caller.id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?;
        let row=sqlx::query("SELECT d.state,d.generation,c.smtp_id FROM ruphish_delivery d JOIN campaigns c ON c.id=d.campaign_id WHERE d.result_id=$1 AND d.campaign_id=$2 AND c.user_id=$3 AND c.status!='Completed'").bind(result).bind(campaign).bind(caller.id).fetch_optional(&mut *tx).await.map_err(storage)?.ok_or(Error::NotFound("Delivery"))?;
        if row.try_get::<i64, _>("generation").map_err(storage)? != generation {
            return Err(Error::Changed);
        }
        let state = text(&row, "state")?;
        let target =
            resolve_delivery(&state, action).map_err(|error| Error::Invalid(error.to_string()))?;
        sqlx::query("UPDATE ruphish_delivery SET state=$1,generation=generation+1,lease_until=0,due=$2,retries=0,code='' WHERE result_id=$3").bind(target).bind(now).bind(result).execute(&mut *tx).await.map_err(storage)?;
        sqlx::query("DELETE FROM ruphish_profile_cooldown WHERE profile_id=$1")
            .bind(row.try_get::<i64, _>("smtp_id").map_err(storage)?)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        sqlx::query("UPDATE results SET status=CASE WHEN status IN ('Email Opened','Clicked Link','Submitted Data') THEN status ELSE $1 END,send_date=$2,modified_date=$3 WHERE id=$4").bind(if target=="accepted" {"Email Sent"} else {"Sending"}).bind(stamp(now)?).bind(stamp(now)?).bind(result).execute(&mut *tx).await.map_err(storage)?;
        if target == "accepted" {
            let inserted = sqlx::query("INSERT INTO events(campaign_id,email,time,message,details) SELECT r.campaign_id,r.email,$1,'Email Sent',$2 FROM results r WHERE r.id=$3 AND NOT EXISTS(SELECT 1 FROM events e WHERE e.campaign_id=r.campaign_id AND e.email=r.email AND e.message='Email Sent') RETURNING id")
                .bind(stamp(now)?)
                .bind(serde_json::json!({"action":action,"actor":caller.id}).to_string())
                .bind(result).fetch_optional(&mut *tx).await.map_err(storage)?;
            if let Some(inserted) = inserted {
                super::events::publish(
                    &mut tx,
                    inserted.insert_id().map_err(storage)?,
                    "application",
                )
                .await
                .map_err(storage)?;
            }
        }
        let inserted=sqlx::query("INSERT INTO events(campaign_id,email,time,message,details) SELECT campaign_id,email,$1,'Delivery reconciled',$2 FROM results WHERE id=$3 RETURNING id").bind(stamp(now)?).bind(serde_json::json!({"action":action,"actor":caller.id,"previous_state":state}).to_string()).bind(result).fetch_one(&mut *tx).await.map_err(storage)?;
        super::events::publish(
            &mut tx,
            inserted.insert_id().map_err(storage)?,
            "application",
        )
        .await
        .map_err(storage)?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(())
    }
}

fn decode_attempt(a: PgRow) -> Result<Attempt, Error> {
    Ok(Attempt {
        generation: a.try_get("generation").map_err(storage)?,
        started: a.try_get("started").map_err(storage)?,
        finished: a.try_get("finished").map_err(storage)?,
        state: text(&a, "state")?,
        stage: text(&a, "stage")?,
        smtp_code: a.try_get("smtp_code").map_err(storage)?,
        message_id: text(&a, "message_id")?,
    })
}

impl Campaigns {
    async fn create_inner(
        &self,
        caller: Identity,
        mut input: Create,
        now: i64,
        draft: Option<(&str, i64)>,
    ) -> Result<Campaign, Error> {
        let mut tx = crate::postgres::locks::write(&self.pool)
            .await
            .map_err(storage)?;
        super::resources::authorize(&mut tx, caller, true).await?;
        if draft.is_some() {
            input.idempotency = None;
        }
        if let Some((id, revision)) = draft {
            let saved = drafts::launching(&mut tx, caller.id, id).await?;
            if let Some(campaign_id) = saved.campaign_id {
                return load(&mut tx, caller.id, campaign_id).await;
            }
            ruphish_application::campaign::drafts::validate_launch(&saved, revision, &input, now)?;
        }
        if let Some((key, fingerprint)) = &input.idempotency {
            if key.is_empty() || key.len() > 128 {
                return Err(Error::Invalid("Invalid launch idempotency key".into()));
            }
            if let Some(row)=sqlx::query("SELECT fingerprint,campaign_id FROM ruphish_campaign_launch WHERE owner_id=$1 AND key=$2").bind(caller.id).bind(key).fetch_optional(&mut *tx).await.map_err(storage)? {
                if text(&row,"fingerprint")?!=*fingerprint {return Err(Error::Conflict("Launch key"));}
                return load(&mut tx,caller.id,row.try_get("campaign_id").map_err(storage)?).await;
            }
        }
        let p = prepare_authorized(&mut tx, caller, &input, now, true).await?;
        let created = stamp(now)?;
        let launch = stamp(p.launch)?;
        let end = input.send_by.map(stamp).transpose()?.unwrap_or(ZERO.into());
        let status = if p.launch <= now {
            "In progress"
        } else {
            "Queued"
        };
        let id=sqlx::query("INSERT INTO campaigns(user_id,name,created_date,launch_date,send_by_date,completed_date,template_id,page_id,status,smtp_id,url) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) RETURNING id").bind(caller.id).bind(&input.name).bind(&created).bind(&launch).bind(end).bind(ZERO).bind(p.template.id).bind(p.page.id).bind(status).bind(p.profile.id).bind(&input.url).fetch_one(&mut *tx).await.map_err(storage)?.insert_id().map_err(storage)?;
        sqlx::query("INSERT INTO ruphish_campaign_domains VALUES($1,$2)")
            .bind(id)
            .bind(p.domain_id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        sqlx::query("INSERT INTO ruphish_campaign_policy VALUES($1,$2,$3,$4)")
            .bind(id)
            .bind(p.policy.capture_credentials)
            .bind(p.policy.capture_passwords)
            .bind(&p.policy.redirect_url)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        let inserted=sqlx::query("INSERT INTO events(campaign_id,email,time,message,details) VALUES($1,'',$2,'Campaign Created','') RETURNING id").bind(id).bind(&created).fetch_one(&mut *tx).await.map_err(storage)?;
        super::events::publish(
            &mut tx,
            inserted.insert_id().map_err(storage)?,
            "application",
        )
        .await
        .map_err(storage)?;
        for (index, r) in p.recipients.iter().enumerate() {
            let due = send_date(p.launch, input.send_by, index, p.recipients.len())
                .ok_or_else(|| Error::Invalid("Campaign schedule overflow".into()))?;
            let rid = loop {
                let candidate = random()?;
                let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM results WHERE r_id=$1 UNION ALL SELECT 1 FROM email_requests WHERE r_id=$2)").bind(&candidate).bind(&candidate).fetch_one(&mut *tx).await.map_err(storage)?;
                if !exists {
                    break candidate;
                }
            };
            let result=sqlx::query("INSERT INTO results(campaign_id,user_id,r_id,email,first_name,last_name,position,status,ip,latitude,longitude,send_date,reported,modified_date) VALUES($1,$2,$3,$4,$5,$6,$7,$8, '',0,0,$9,FALSE,$10) RETURNING id").bind(id).bind(caller.id).bind(rid).bind(&r.email).bind(&r.first_name).bind(&r.last_name).bind(&r.position).bind(if due<=now {"Sending"} else {"Scheduled"}).bind(stamp(due)?).bind(&created).fetch_one(&mut *tx).await.map_err(storage)?.insert_id().map_err(storage)?;
            sqlx::query("INSERT INTO ruphish_delivery(result_id,campaign_id,state,due) VALUES($1,$2,'ready',$3)").bind(result).bind(id).bind(due).execute(&mut *tx).await.map_err(storage)?;
        }
        if let Some((key, fingerprint)) = input.idempotency {
            sqlx::query("INSERT INTO ruphish_campaign_launch VALUES($1,$2,$3,$4)")
                .bind(caller.id)
                .bind(key)
                .bind(fingerprint)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
        }
        for (index, g) in p.groups.iter().enumerate() {
            sqlx::query("INSERT INTO ruphish_campaign_groups VALUES($1,$2,$3)")
                .bind(id)
                .bind(index as i64)
                .bind(g.id)
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
        }
        if let Some((draft_id, _)) = draft {
            sqlx::query("UPDATE ruphish_campaign_drafts SET campaign_id=$1,revision=revision+1,modified=$2 WHERE id=$3 AND owner_id=$4").bind(id).bind(now).bind(draft_id).bind(caller.id).execute(&mut *tx).await.map_err(storage)?;
        }
        let campaign = load(&mut tx, caller.id, id).await?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(campaign)
    }
}
