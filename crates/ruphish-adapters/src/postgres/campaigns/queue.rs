//! Durable claims distinguish safe preparation recovery from uncertain submission.
use super::*;
use ruphish_application::delivery::MessageInput;

async fn owner_enabled(c: &mut PgConnection, owner: i64) -> Result<bool, Error> {
    match super::super::accounts::account(c, owner).await {
        Ok(user) => Ok(!user.account_locked && user.require("modify_objects").is_ok()),
        Err(ruphish_application::auth::Error::NotFound) => Ok(false),
        Err(_) => Err(Error::Internal),
    }
}
#[async_trait::async_trait]
impl Queue for Campaigns {
    async fn release(&self, claim: &Claim, now: i64) -> Result<bool, Error> {
        let mut tx = crate::postgres::locks::write(&self.pool)
            .await
            .map_err(storage)?;
        if !super::super::locks::campaign(&mut tx, claim.campaign_id, true)
            .await
            .map_err(storage)?
        {
            return Ok(false);
        }
        let changed=sqlx::query("UPDATE ruphish_delivery SET state='ready',generation=generation+1,lease_until=0 WHERE result_id=$1 AND generation=$2 AND state='preparing'").bind(claim.result_id).bind(claim.generation).execute(&mut *tx).await.map_err(storage)?.rows_affected()==1;
        if changed {
            sqlx::query("UPDATE ruphish_delivery_attempt SET state='cancelled',finished=$1 WHERE result_id=$2 AND generation=$3").bind(now).bind(claim.result_id).bind(claim.generation).execute(&mut *tx).await.map_err(storage)?;
        }
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(changed)
    }
    async fn claim(&self, now: i64) -> Result<Option<Claim>, Error> {
        recover(&self.pool, now).await?;
        let mut tx = crate::postgres::locks::write(&self.pool)
            .await
            .map_err(storage)?;
        super::super::locks::guard(&mut tx, "delivery-claims")
            .await
            .map_err(storage)?;
        let active: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM ruphish_delivery WHERE state IN ('preparing','submitting')",
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(storage)?;
        if active >= 4 {
            tx.commit()
                .await
                .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
            return Ok(None);
        }
        let row = candidate(&mut tx, now, None).await?;
        let Some(row) = row else {
            tx.commit()
                .await
                .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
            return Ok(None);
        };
        let candidate_id: i64 = row.try_get("result_id").map_err(storage)?;
        let campaign_id: i64 = row.try_get("campaign_id").map_err(storage)?;
        if !super::super::locks::campaign(&mut tx, campaign_id, true)
            .await
            .map_err(storage)?
        {
            return Ok(None);
        }
        let Some(row) = candidate(&mut tx, now, Some(candidate_id)).await? else {
            return Ok(None);
        };
        let owner: i64 = row.try_get("user_id").map_err(storage)?;
        let id: i64 = row.try_get("result_id").map_err(storage)?;
        if !owner_enabled(&mut tx, owner).await? {
            sqlx::query("UPDATE ruphish_delivery SET state='blocked',code='campaign_owner_unavailable' WHERE result_id=$1").bind(id).execute(&mut *tx).await.map_err(storage)?;
            tx.commit()
                .await
                .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
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
                sqlx::query("UPDATE ruphish_delivery SET state='blocked',code='campaign_resource_missing' WHERE result_id=$1").bind(id).execute(&mut *tx).await.map_err(storage)?;
                tx.commit()
                    .await
                    .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
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
        sqlx::query("UPDATE ruphish_delivery SET state='preparing',generation=$1,lease_until=$2,last_claim=$3,profile_revision=(SELECT s.modified_date FROM smtp s JOIN campaigns c ON c.smtp_id=s.id WHERE c.id=ruphish_delivery.campaign_id) WHERE result_id=$4").bind(generation).bind(now+900).bind(now).bind(id).execute(&mut *tx).await.map_err(storage)?;
        sqlx::query("INSERT INTO ruphish_delivery_attempt(result_id,generation,started,state,message_id,resource_revision) VALUES($1,$2,$3,'preparing',$4,$5)").bind(id).bind(generation).bind(now).bind(&message_id).bind(format!("template:{template_id}@{};profile:{profile_id}@{}",revisions[0],revisions[1])).execute(&mut *tx).await.map_err(storage)?;
        sqlx::query("UPDATE campaigns SET status='In progress' WHERE id=$1 AND status='Queued'")
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
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(Some(Claim {
            result_id: id,
            campaign_id,
            generation,
            revisions,
            message,
        }))
    }
    async fn arm(&self, claim: &Claim, now: i64) -> Result<bool, Error> {
        let mut tx = crate::postgres::locks::write(&self.pool)
            .await
            .map_err(storage)?;
        if !super::super::locks::campaign(&mut tx, claim.campaign_id, true)
            .await
            .map_err(storage)?
        {
            return Ok(false);
        }
        let owner: Option<i64> =
            sqlx::query_scalar("SELECT user_id FROM campaigns WHERE id=$1 AND status!='Completed'")
                .bind(claim.campaign_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?;
        let Some(owner) = owner else { return Ok(false) };
        let domain_ready:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ruphish_campaign_domains cd JOIN ruphish_domains d ON d.id=cd.domain_id WHERE cd.campaign_id=$1 AND d.owner_id=$2 AND d.enabled=TRUE AND (SELECT eligible_until FROM ruphish_domain_policy WHERE id=d.id)>$3)").bind(claim.campaign_id).bind(owner).bind(now).fetch_one(&mut *tx).await.map_err(storage)?;
        if !domain_ready {
            return Ok(false);
        }

        if !owner_enabled(&mut tx, owner).await? {
            return Ok(false);
        }
        let refs = sqlx::query("SELECT template_id,smtp_id FROM campaigns WHERE id=$1")
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
            sqlx::query("UPDATE ruphish_delivery_attempt SET state='cancelled',finished=$1 WHERE result_id=$2 AND generation=$3 AND state='preparing'").bind(now).bind(claim.result_id).bind(claim.generation).execute(&mut *tx).await.map_err(storage)?;
            sqlx::query("UPDATE ruphish_delivery SET state='ready',generation=generation+1,lease_until=0 WHERE result_id=$1 AND generation=$2 AND state='preparing'").bind(claim.result_id).bind(claim.generation).execute(&mut *tx).await.map_err(storage)?;
            tx.commit()
                .await
                .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
            return Ok(false);
        }
        let changed=sqlx::query("UPDATE ruphish_delivery SET state='submitting',lease_until=$1 WHERE result_id=$2 AND generation=$3 AND state='preparing' AND lease_until>$4").bind(now+1200).bind(claim.result_id).bind(claim.generation).bind(now).execute(&mut *tx).await.map_err(storage)?.rows_affected()==1;
        if changed {
            sqlx::query("UPDATE ruphish_delivery_attempt SET state='submitting' WHERE result_id=$1 AND generation=$2").bind(claim.result_id).bind(claim.generation).execute(&mut *tx).await.map_err(storage)?;
        }
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(changed)
    }
    async fn finish(
        &self,
        claim: &Claim,
        outcome: Result<(), DeliveryFailure>,
        now: i64,
    ) -> Result<(), Error> {
        let mut tx = crate::postgres::locks::write(&self.pool)
            .await
            .map_err(storage)?;
        if !super::super::locks::campaign(&mut tx, claim.campaign_id, true)
            .await
            .map_err(storage)?
        {
            return Ok(());
        }
        let row=sqlx::query("SELECT d.*,c.status AS campaign_status,c.smtp_id FROM ruphish_delivery d JOIN campaigns c ON c.id=d.campaign_id WHERE d.result_id=$1 AND d.generation=$2 AND d.state IN ('preparing','submitting','unknown') FOR UPDATE").bind(claim.result_id).bind(claim.generation).fetch_optional(&mut *tx).await.map_err(storage)?;
        let Some(row) = row else { return Ok(()) };
        let current = text(&row, "state")?;
        let retries = u32::try_from(row.try_get::<i64, _>("retries").map_err(storage)?)
            .map_err(|_| Error::Internal)?;
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
        sqlx::query("UPDATE ruphish_delivery SET state=$1,due=$2,retries=$3,lease_until=0,code=$4,stage=$5,smtp_code=$6 WHERE result_id=$7 AND generation=$8").bind(state.as_str()).bind(next).bind(i64::from(count)).bind(code).bind(&stage).bind(smtp_code.map(i64::from)).bind(claim.result_id).bind(claim.generation).execute(&mut *tx).await.map_err(storage)?;
        sqlx::query("UPDATE ruphish_delivery_attempt SET state=$1,finished=$2,stage=$3,smtp_code=$4 WHERE result_id=$5 AND generation=$6").bind(state.as_str()).bind(now).bind(stage).bind(smtp_code.map(i64::from)).bind(claim.result_id).bind(claim.generation).execute(&mut *tx).await.map_err(storage)?;
        if cooldown {
            sqlx::query("INSERT INTO ruphish_profile_cooldown VALUES($1,$2,$3,$4) ON CONFLICT (profile_id) DO UPDATE SET until_time=GREATEST(ruphish_profile_cooldown.until_time,excluded.until_time),revision=excluded.revision,blocked=excluded.blocked").bind(row.try_get::<i64,_>("smtp_id").map_err(storage)?).bind(next).bind(text(&row,"profile_revision")?).bind(state == DeliveryState::Blocked).execute(&mut *tx).await.map_err(storage)?;
        }
        sqlx::query("UPDATE results SET status=CASE WHEN status IN ('Email Opened','Clicked Link','Submitted Data') THEN status ELSE $1 END,send_date=CASE WHEN $2 IN ('accepted','retry') THEN $3 ELSE send_date END,modified_date=$4 WHERE id=$5").bind(status).bind(state.as_str()).bind(stamp(if state == DeliveryState::Retry {next} else {now})?).bind(stamp(now)?).bind(claim.result_id).execute(&mut *tx).await.map_err(storage)?;
        let details = outcome
            .as_ref()
            .err()
            .map(|e| serde_json::json!({"error":e.to_string(),"code":code}).to_string())
            .unwrap_or_default();
        let inserted = sqlx::query(
            "INSERT INTO events(campaign_id,email,time,kind,metadata,recipient_id) VALUES($1,$2,$3,$4,$5,$6) RETURNING id",
        )
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
        .fetch_one(&mut *tx)
        .await
        .map_err(storage)?;
        super::super::events::publish(
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

async fn recover(pool: &PgPool, now: i64) -> Result<(), Error> {
    let campaigns:Vec<i64>=sqlx::query_scalar("SELECT DISTINCT campaign_id FROM ruphish_delivery WHERE state IN ('preparing','submitting') AND lease_until<=$1 ORDER BY campaign_id LIMIT 100").bind(now).fetch_all(pool).await.map_err(storage)?;
    for campaign in campaigns {
        let mut tx = crate::postgres::locks::write(pool).await.map_err(storage)?;
        if !super::super::locks::campaign(&mut tx, campaign, true)
            .await
            .map_err(storage)?
        {
            continue;
        }
        sqlx::query(crate::postgres::worker_sql::DELIVERY_PREPARING)
            .bind(now)
            .bind(now)
            .bind(campaign)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        sqlx::query("UPDATE ruphish_delivery SET state='ready',lease_until=0 WHERE state='preparing' AND lease_until<=$1 AND campaign_id=$2").bind(now).bind(campaign).execute(&mut *tx).await.map_err(storage)?;
        sqlx::query(crate::postgres::worker_sql::DELIVERY_SUBMITTING)
            .bind(now)
            .bind(now)
            .bind(campaign)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        let recipients:Vec<(i64,String)>=sqlx::query_as("SELECT r.id,r.email FROM ruphish_delivery d JOIN results r ON r.id=d.result_id WHERE d.state='submitting' AND d.lease_until<=$1 AND d.campaign_id=$2").bind(now).bind(campaign).fetch_all(&mut *tx).await.map_err(storage)?;
        for (result_id, email) in recipients {
            let inserted=sqlx::query("INSERT INTO events(campaign_id,email,time,kind,metadata,recipient_id) VALUES($1,$2,$3,'email_error','{\"code\":\"delivery_outcome_unknown\"}',$4) RETURNING id").bind(campaign).bind(email).bind(stamp(now)?).bind(result_id).fetch_one(&mut *tx).await.map_err(storage)?;
            super::super::events::publish(
                &mut tx,
                inserted.insert_id().map_err(storage)?,
                "application",
            )
            .await
            .map_err(storage)?;
        }
        sqlx::query("UPDATE results SET status=CASE WHEN status IN ('Email Opened','Clicked Link','Submitted Data') THEN status ELSE 'Error' END,modified_date=$1 WHERE id IN (SELECT result_id FROM ruphish_delivery WHERE state='submitting' AND lease_until<=$2 AND campaign_id=$3)").bind(stamp(now)?).bind(now).bind(campaign).execute(&mut *tx).await.map_err(storage)?;
        sqlx::query("UPDATE ruphish_delivery SET state='unknown',code='delivery_outcome_unknown' WHERE state='submitting' AND lease_until<=$1 AND campaign_id=$2").bind(now).bind(campaign).execute(&mut *tx).await.map_err(storage)?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
    }
    Ok(())
}

async fn candidate(
    c: &mut PgConnection,
    now: i64,
    id: Option<i64>,
) -> Result<Option<PgRow>, Error> {
    sqlx::query(crate::postgres::worker_sql::CANDIDATE)
        .bind(id)
        .bind(id)
        .bind(now)
        .bind(now)
        .bind(now)
        .fetch_optional(c)
        .await
        .map_err(storage)
}
