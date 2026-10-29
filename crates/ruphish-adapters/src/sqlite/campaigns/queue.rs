//! Durable claims distinguish safe preparation recovery from uncertain submission.
use super::*;
use ruphish_application::delivery::MessageInput;

async fn owner_enabled(c: &mut SqliteConnection, owner: i64) -> Result<bool, Error> {
    match super::super::accounts::account(c, owner).await {
        Ok(user) => Ok(!user.account_locked && user.require("modify_objects").is_ok()),
        Err(ruphish_application::auth::Error::NotFound) => Ok(false),
        Err(_) => Err(Error::Internal),
    }
}
#[async_trait::async_trait]
impl Queue for Campaigns {
    async fn release(&self, claim: &Claim, now: i64) -> Result<bool, Error> {
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage)?;
        let changed=sqlx::query("UPDATE ruphish_delivery SET state='ready',generation=generation+1,lease_until=0 WHERE result_id=? AND generation=? AND state='preparing'").bind(claim.result_id).bind(claim.generation).execute(&mut *tx).await.map_err(storage)?.rows_affected()==1;
        if changed {
            sqlx::query("UPDATE ruphish_delivery_attempt SET state='cancelled',finished=? WHERE result_id=? AND generation=?").bind(now).bind(claim.result_id).bind(claim.generation).execute(&mut *tx).await.map_err(storage)?;
        }
        tx.commit().await.map_err(storage)?;
        Ok(changed)
    }
    async fn claim(&self, now: i64) -> Result<Option<Claim>, Error> {
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage)?;
        sqlx::query(crate::sqlite::worker_sql::DELIVERY_PREPARING)
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        sqlx::query("UPDATE ruphish_delivery SET state='ready',lease_until=0 WHERE state='preparing' AND lease_until<=?").bind(now).execute(&mut *tx).await.map_err(storage)?;
        sqlx::query(crate::sqlite::worker_sql::DELIVERY_SUBMITTING)
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        sqlx::query("INSERT INTO events(campaign_id,email,time,kind,metadata,recipient_id) SELECT r.campaign_id,r.email,?,'email_error','{\"code\":\"delivery_outcome_unknown\"}',r.id FROM ruphish_delivery d JOIN results r ON r.id=d.result_id WHERE d.state='submitting' AND d.lease_until<=?").bind(stamp(now)?).bind(now).execute(&mut *tx).await.map_err(storage)?;
        sqlx::query("UPDATE results SET status=CASE WHEN status IN ('Email Opened','Clicked Link','Submitted Data') THEN status ELSE 'Error' END,modified_date=? WHERE id IN (SELECT result_id FROM ruphish_delivery WHERE state='submitting' AND lease_until<=?)").bind(stamp(now)?).bind(now).execute(&mut *tx).await.map_err(storage)?;
        sqlx::query("UPDATE ruphish_delivery SET state='unknown',code='delivery_outcome_unknown' WHERE state='submitting' AND lease_until<=?").bind(now).execute(&mut *tx).await.map_err(storage)?;
        let active: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM ruphish_delivery WHERE state IN ('preparing','submitting')",
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(storage)?;
        if active >= 4 {
            tx.commit().await.map_err(storage)?;
            return Ok(None);
        }
        let row = sqlx::query(crate::sqlite::worker_sql::CANDIDATE)
            .bind(now)
            .bind(now)
            .bind(now)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?;
        let Some(row) = row else {
            tx.commit().await.map_err(storage)?;
            return Ok(None);
        };
        let owner: i64 = row.try_get("user_id").map_err(storage)?;
        let id: i64 = row.try_get("result_id").map_err(storage)?;
        if !owner_enabled(&mut tx, owner).await? {
            sqlx::query("UPDATE ruphish_delivery SET state='blocked',code='campaign_owner_unavailable' WHERE result_id=?").bind(id).execute(&mut *tx).await.map_err(storage)?;
            tx.commit().await.map_err(storage)?;
            return Ok(None);
        }
        let template_id: i64 = row.try_get("template_id").map_err(storage)?;
        let profile_id: i64 = row.try_get("smtp_id").map_err(storage)?;
        let selected = async {
            Ok::<_, Error>((
                super::super::resources::load(&mut tx, owner, Kind::Template, template_id).await?,
                super::super::resources::load(&mut tx, owner, Kind::Profile, profile_id).await?,
            ))
        }
        .await;
        let (template, profile) = match selected {
            Ok(v) => v,
            Err(Error::NotFound(_)) => {
                sqlx::query("UPDATE ruphish_delivery SET state='blocked',code='campaign_resource_missing' WHERE result_id=?").bind(id).execute(&mut *tx).await.map_err(storage)?;
                tx.commit().await.map_err(storage)?;
                return Ok(None);
            }
            Err(e) => return Err(e),
        };
        let revisions = [
            template.modified_date.clone(),
            profile.modified_date.clone(),
        ];
        let Content::Template(template) = template.content else {
            return Err(Error::Internal);
        };
        let Content::Profile(profile) = profile.content else {
            return Err(Error::Internal);
        };
        let generation = row.try_get::<i64, _>("generation").map_err(storage)? + 1;
        let campaign_id = row.try_get("campaign_id").map_err(storage)?;
        let message_id = format!("{}@ruphish.local", random()?);
        sqlx::query("UPDATE ruphish_delivery SET state='preparing',generation=?,lease_until=?,last_claim=?,profile_revision=(SELECT s.modified_date FROM smtp s JOIN campaigns c ON c.smtp_id=s.id WHERE c.id=ruphish_delivery.campaign_id) WHERE result_id=?").bind(generation).bind(now+900).bind(now).bind(id).execute(&mut *tx).await.map_err(storage)?;
        sqlx::query("INSERT INTO ruphish_delivery_attempt(result_id,generation,started,state,message_id,resource_revision) VALUES(?,?,?,'preparing',?,?)").bind(id).bind(generation).bind(now).bind(&message_id).bind(format!("template:{template_id}@{};profile:{profile_id}@{}",revisions[0],revisions[1])).execute(&mut *tx).await.map_err(storage)?;
        sqlx::query("UPDATE campaigns SET status='In progress' WHERE id=? AND status='Queued'")
            .bind(campaign_id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        let message = MessageInput {
            message_id: Some(message_id),
            recipient: Recipient {
                email: text(&row, "email")?,
                first_name: text(&row, "first_name")?,
                last_name: text(&row, "last_name")?,
                position: text(&row, "position")?,
            },
            template,
            template_id,
            page_id: row.try_get("page_id").map_err(storage)?,
            profile,
            url: text(&row, "url")?,
            rid: text(&row, "r_id")?,
        };
        tx.commit().await.map_err(storage)?;
        Ok(Some(Claim {
            result_id: id,
            campaign_id,
            generation,
            revisions,
            message,
        }))
    }
    async fn arm(&self, claim: &Claim, now: i64) -> Result<bool, Error> {
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage)?;
        let owner: Option<i64> =
            sqlx::query_scalar("SELECT user_id FROM campaigns WHERE id=? AND status!='Completed'")
                .bind(claim.campaign_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?;
        let Some(owner) = owner else { return Ok(false) };
        let domain_ready:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ruphish_campaign_domains cd JOIN ruphish_domains d ON d.id=cd.domain_id WHERE cd.campaign_id=? AND d.owner_id=? AND d.enabled=1 AND (SELECT eligible_until FROM ruphish_domain_policy WHERE id=d.id)>?)").bind(claim.campaign_id).bind(owner).bind(now).fetch_one(&mut *tx).await.map_err(storage)?;
        if !domain_ready {
            return Ok(false);
        }

        if !owner_enabled(&mut tx, owner).await? {
            return Ok(false);
        }
        let refs = sqlx::query("SELECT template_id,smtp_id FROM campaigns WHERE id=?")
            .bind(claim.campaign_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?;
        let mut valid = true;
        for (index, kind, column) in [
            (0, Kind::Template, "template_id"),
            (1, Kind::Profile, "smtp_id"),
        ] {
            let resource = super::super::resources::load(
                &mut tx,
                owner,
                kind,
                refs.try_get(column).map_err(storage)?,
            )
            .await?;
            valid &= resource.modified_date == claim.revisions[index];
        }
        if !valid {
            sqlx::query("UPDATE ruphish_delivery_attempt SET state='cancelled',finished=? WHERE result_id=? AND generation=? AND state='preparing'").bind(now).bind(claim.result_id).bind(claim.generation).execute(&mut *tx).await.map_err(storage)?;
            sqlx::query("UPDATE ruphish_delivery SET state='ready',generation=generation+1,lease_until=0 WHERE result_id=? AND generation=? AND state='preparing'").bind(claim.result_id).bind(claim.generation).execute(&mut *tx).await.map_err(storage)?;
            tx.commit().await.map_err(storage)?;
            return Ok(false);
        }
        let changed=sqlx::query("UPDATE ruphish_delivery SET state='submitting',lease_until=? WHERE result_id=? AND generation=? AND state='preparing' AND lease_until>?").bind(now+1200).bind(claim.result_id).bind(claim.generation).bind(now).execute(&mut *tx).await.map_err(storage)?.rows_affected()==1;
        if changed {
            sqlx::query("UPDATE ruphish_delivery_attempt SET state='submitting' WHERE result_id=? AND generation=?").bind(claim.result_id).bind(claim.generation).execute(&mut *tx).await.map_err(storage)?;
        }
        tx.commit().await.map_err(storage)?;
        Ok(changed)
    }
    async fn finish(
        &self,
        claim: &Claim,
        outcome: Result<(), DeliveryFailure>,
        now: i64,
    ) -> Result<(), Error> {
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage)?;
        let row=sqlx::query("SELECT d.*,c.status AS campaign_status,c.smtp_id FROM ruphish_delivery d JOIN campaigns c ON c.id=d.campaign_id WHERE d.result_id=? AND d.generation=? AND d.state IN ('preparing','submitting','unknown')").bind(claim.result_id).bind(claim.generation).fetch_optional(&mut *tx).await.map_err(storage)?;
        let Some(row) = row else { return Ok(()) };
        let current = text(&row, "state")?;
        let retries: u32 = row.try_get("retries").map_err(storage)?;
        let Some(DeliveryUpdate {
            state,
            next,
            count,
            code,
            stage,
            smtp_code,
            cooldown,
            status,
        }) = crate::campaign_preparation::delivery_update(
            &current,
            text(&row, "campaign_status")? == "Completed",
            retries,
            now,
            &outcome,
        )?
        else {
            return Ok(());
        };
        sqlx::query("UPDATE ruphish_delivery SET state=?,due=?,retries=?,lease_until=0,code=?,stage=?,smtp_code=? WHERE result_id=? AND generation=?").bind(state.as_str()).bind(next).bind(count).bind(code).bind(&stage).bind(smtp_code).bind(claim.result_id).bind(claim.generation).execute(&mut *tx).await.map_err(storage)?;
        sqlx::query("UPDATE ruphish_delivery_attempt SET state=?,finished=?,stage=?,smtp_code=? WHERE result_id=? AND generation=?").bind(state.as_str()).bind(now).bind(stage).bind(smtp_code).bind(claim.result_id).bind(claim.generation).execute(&mut *tx).await.map_err(storage)?;
        if cooldown {
            sqlx::query("INSERT INTO ruphish_profile_cooldown VALUES(?,?,?,?) ON CONFLICT(profile_id) DO UPDATE SET until_time=max(until_time,excluded.until_time),revision=excluded.revision,blocked=excluded.blocked").bind(row.try_get::<i64,_>("smtp_id").map_err(storage)?).bind(next).bind(text(&row,"profile_revision")?).bind(state == DeliveryState::Blocked).execute(&mut *tx).await.map_err(storage)?;
        }
        sqlx::query("UPDATE results SET status=CASE WHEN status IN ('Email Opened','Clicked Link','Submitted Data') THEN status ELSE ? END,send_date=CASE WHEN ? IN ('accepted','retry') THEN ? ELSE send_date END,modified_date=? WHERE id=?").bind(status).bind(state.as_str()).bind(stamp(if state == DeliveryState::Retry {next} else {now})?).bind(stamp(now)?).bind(claim.result_id).execute(&mut *tx).await.map_err(storage)?;
        let details = outcome
            .as_ref()
            .err()
            .map(|e| serde_json::json!({"error":e.to_string(),"code":code}).to_string())
            .unwrap_or_default();
        sqlx::query("INSERT INTO events(campaign_id,email,time,kind,metadata,recipient_id) VALUES(?,?,?,?,?,?)")
            .bind(claim.campaign_id)
            .bind(&claim.message.recipient.email)
            .bind(stamp(now)?)
            .bind(if outcome.is_ok() {
                "email_sent"
            } else {
                "email_error"
            })
            .bind(crate::participant_privacy::details(&details))
        .bind(claim.result_id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(())
    }
}
