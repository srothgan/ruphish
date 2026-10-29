//! Authorized domain persistence and revision-fenced verification leases.
use crate::postgres::InsertId;
use ruphish_application::{auth::Identity, domains::*, resources::Error};
use sqlx::{PgConnection, PgPool, Row, postgres::PgRow};

#[derive(Clone)]
/// Domain repository sharing the startup-owned pool.
pub struct Domains {
    pool: PgPool,
}
impl Domains {
    /// Attach to the migrated startup database.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
fn storage(error: sqlx::Error) -> Error {
    crate::postgres::resource_error(error)
}
fn token() -> Result<String, Error> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes).map_err(|_| Error::Internal)?;
    Ok(hex::encode(bytes))
}
fn decode(r: PgRow) -> Result<Domain, Error> {
    Ok(Domain {
        verification_required: r.try_get("verification_required").map_err(storage)?,
        eligible_until: r.try_get("eligible_until").map_err(storage)?,
        expires_at: r.try_get("expires_at").map_err(storage)?,
        id: r.try_get("id").map_err(storage)?,
        name: r.try_get("name").map_err(storage)?,
        origin: r.try_get("origin").map_err(storage)?,
        hostname: r.try_get("hostname").map_err(storage)?,
        enabled: r.try_get("enabled").map_err(storage)?,
        revision: r.try_get("revision").map_err(storage)?,
        challenge: r.try_get("challenge").map_err(storage)?,
        challenge_expires: r.try_get("challenge_expires").map_err(storage)?,
        checked_at: r.try_get("checked_at").map_err(storage)?,
        verified_until: r.try_get("verified_until").map_err(storage)?,
        control: r.try_get("control").map_err(storage)?,
        dns: r.try_get("dns").map_err(storage)?,
        routing: r.try_get("routing").map_err(storage)?,
        tls: r.try_get("tls").map_err(storage)?,
        code: r.try_get("code").map_err(storage)?,
        checking: r.try_get::<i64, _>("lease_until").map_err(storage)?
            > ruphish_application::datetime::now().as_second(),
        active_check: if let Some(id) = r.try_get::<Option<i64>, _>("active_id").map_err(storage)? {
            let started_at: Option<i64> = r.try_get("active_started_at").map_err(storage)?;
            Some(VerificationRun {
                id,
                state: if started_at.is_some()
                    && r.try_get::<i64, _>("lease_until").map_err(storage)?
                        > ruphish_application::datetime::now().as_second()
                {
                    "running"
                } else {
                    "queued"
                }
                .into(),
                scheduled_at: r.try_get("active_scheduled_at").map_err(storage)?,
                started_at,
                finished_at: None,
                code: String::new(),
            })
        } else {
            None
        },
        history: Vec::new(),
        next_check_at: r.try_get("due").map_err(storage)?,
        campaigns: r.try_get("campaigns").map_err(storage)?,
    })
}
use super::inventory_sql::DOMAINS as SELECT;
async fn load(c: &mut PgConnection, owner: i64, id: i64) -> Result<Domain, Error> {
    let mut result = decode(
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "{SELECT} WHERE d.owner_id=$1 AND d.id=$2"
        )))
        .bind(owner)
        .bind(id)
        .fetch_optional(&mut *c)
        .await
        .map_err(storage)?
        .ok_or(Error::NotFound("Domain"))?,
    )?;
    result.history = sqlx::query("SELECT id,scheduled_at,started_at,finished_at,outcome,code FROM ruphish_domain_checks WHERE domain_id=$1 AND finished_at IS NOT NULL ORDER BY id DESC LIMIT 10").bind(id).fetch_all(c).await.map_err(storage)?.into_iter().map(|r| Ok(VerificationRun {id:r.try_get("id").map_err(storage)?,state:r.try_get("outcome").map_err(storage)?,scheduled_at:r.try_get("scheduled_at").map_err(storage)?,started_at:r.try_get("started_at").map_err(storage)?,finished_at:r.try_get("finished_at").map_err(storage)?,code:r.try_get("code").map_err(storage)?})).collect::<Result<_,Error>>()?;
    Ok(result)
}
async fn enqueue(c: &mut PgConnection, id: i64, now: i64) -> Result<(), Error> {
    if !super::settings::load(c).await?.policy.verification_enabled {
        return Ok(());
    }
    sqlx::query("INSERT INTO ruphish_domain_checks(domain_id,queued_at,scheduled_at) SELECT id,$1,due FROM ruphish_domains WHERE id=$2 AND enabled=TRUE AND NOT EXISTS(SELECT 1 FROM ruphish_domain_checks WHERE domain_id=$3 AND finished_at IS NULL)").bind(now).bind(id).bind(id).execute(c).await.map_err(storage)?;
    Ok(())
}
async fn prune(c: &mut PgConnection, id: i64) -> Result<(), Error> {
    let boundary:Option<i64> = sqlx::query_scalar("SELECT id FROM ruphish_domain_checks WHERE domain_id=$1 AND finished_at IS NOT NULL ORDER BY id DESC LIMIT 1 OFFSET 9")
        .bind(id).fetch_optional(&mut *c).await.map_err(storage)?;
    if let Some(boundary) = boundary {
        sqlx::query("DELETE FROM ruphish_domain_checks WHERE domain_id=$1 AND finished_at IS NOT NULL AND id<$2").bind(id).bind(boundary).execute(c).await.map_err(storage)?;
    }
    Ok(())
}

/// Fixed origin lookup used inside campaign preparation/launch transactions.
pub(super) async fn eligible(
    c: &mut PgConnection,
    owner: i64,
    value: &str,
    now: i64,
    lock: bool,
) -> Result<i64, Error> {
    let origin = origin(value, true).map_err(|code| Error::Invalid(code.into()))?;
    let statement = if lock {
        "SELECT id,enabled,(SELECT eligible_until FROM ruphish_domain_policy WHERE id=ruphish_domains.id) AS verified_until FROM ruphish_domains WHERE owner_id=$1 AND origin=$2 FOR SHARE"
    } else {
        "SELECT id,enabled,(SELECT eligible_until FROM ruphish_domain_policy WHERE id=ruphish_domains.id) AS verified_until FROM ruphish_domains WHERE owner_id=$1 AND origin=$2"
    };
    let row = sqlx::query(statement)
        .bind(owner)
        .bind(origin.value)
        .fetch_optional(&mut *c)
        .await
        .map_err(storage)?
        .ok_or_else(|| Error::Invalid("domain_not_verified".into()))?;
    eligibility(
        row.try_get("enabled").map_err(storage)?,
        row.try_get("verified_until").map_err(storage)?,
        now,
    )
    .map_err(|code| Error::Invalid(code.into()))?;
    row.try_get("id").map_err(storage)
}

#[async_trait::async_trait]
impl Repository for Domains {
    async fn list(&self, caller: Identity) -> Result<Vec<Domain>, Error> {
        let mut tx = crate::postgres::locks::snapshot(&self.pool)
            .await
            .map_err(storage)?;
        super::resources::authorize(&mut tx, caller, false).await?;
        let result = sqlx::query(sqlx::AssertSqlSafe(format!(
            "{SELECT} WHERE d.owner_id=$1 ORDER BY d.id"
        )))
        .bind(caller.id)
        .fetch_all(&mut *tx)
        .await
        .map_err(storage)?
        .into_iter()
        .map(decode)
        .collect();
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        result
    }
    async fn get(&self, caller: Identity, id: i64) -> Result<Domain, Error> {
        let mut tx = crate::postgres::locks::snapshot(&self.pool)
            .await
            .map_err(storage)?;
        super::resources::authorize(&mut tx, caller, false).await?;
        let result = load(&mut tx, caller.id, id).await?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(result)
    }
    async fn save(
        &self,
        caller: Identity,
        id: Option<i64>,
        edit: Edit,
        now: i64,
    ) -> Result<Domain, Error> {
        let canonical = edit.validate()?;
        let mut tx = crate::postgres::locks::write(&self.pool)
            .await
            .map_err(storage)?;
        super::resources::authorize(&mut tx, caller, true).await?;
        let id = if let Some(id) = id {
            sqlx::query("SELECT id FROM ruphish_domains WHERE id=$1 AND owner_id=$2 FOR UPDATE")
                .bind(id)
                .bind(caller.id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?;
            let old = load(&mut tx, caller.id, id).await?;
            if edit.revision != Some(old.revision) {
                return Err(Error::Changed);
            }
            if canonical.value != old.origin {
                return Err(Error::Invalid("domain_hostname_immutable".into()));
            }
            sqlx::query(
                "DELETE FROM ruphish_domain_checks WHERE domain_id=$1 AND finished_at IS NULL",
            )
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
            sqlx::query("UPDATE ruphish_domains SET name=$1,enabled=$2,revision=revision+1,lease_until=0,generation=generation+1,routing_proof='',due=$3 WHERE id=$4").bind(edit.name).bind(edit.enabled).bind(now).bind(id).execute(&mut *tx).await.map_err(storage)?;
            id
        } else {
            let count: i64 =
                sqlx::query_scalar("SELECT count(*) FROM ruphish_domains WHERE owner_id=$1")
                    .bind(caller.id)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(storage)?;
            if count >= 500 {
                return Err(Error::Invalid("domain_limit_reached".into()));
            }
            let duplicate: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM ruphish_domains WHERE owner_id=$1 AND origin=$2)",
            )
            .bind(caller.id)
            .bind(&canonical.value)
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?;
            if duplicate {
                return Err(Error::Conflict("Domain"));
            }
            sqlx::query("INSERT INTO ruphish_domains(owner_id,name,origin,hostname,enabled,challenge,challenge_expires,due) VALUES($1,$2,$3,$4,$5,$6,$7,$8) RETURNING id").bind(caller.id).bind(edit.name).bind(canonical.value).bind(canonical.host).bind(edit.enabled).bind(token()?).bind(now+CHALLENGE_LIFETIME).bind(now).fetch_one(&mut *tx).await.map_err(storage)?.insert_id().map_err(storage)?
        };
        enqueue(&mut tx, id, now).await?;
        let result = load(&mut tx, caller.id, id).await?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(result)
    }
    async fn delete(&self, caller: Identity, id: i64, revision: i64) -> Result<(), Error> {
        let mut tx = crate::postgres::locks::write(&self.pool)
            .await
            .map_err(storage)?;
        super::resources::authorize(&mut tx, caller, true).await?;
        sqlx::query("SELECT id FROM ruphish_domains WHERE id=$1 AND owner_id=$2 FOR UPDATE")
            .bind(id)
            .bind(caller.id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?;
        let old = load(&mut tx, caller.id, id).await?;
        if old.revision != revision {
            return Err(Error::Changed);
        }
        if old.campaigns > 0 {
            return Err(Error::InUse);
        }
        sqlx::query("DELETE FROM ruphish_domain_checks WHERE domain_id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        sqlx::query("DELETE FROM ruphish_domains WHERE id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        sqlx::query("DELETE FROM ruphish_domain_claims WHERE hostname=$1 AND owner_id=$2 AND NOT EXISTS(SELECT 1 FROM ruphish_domains WHERE hostname=$3 AND owner_id=$4)").bind(&old.hostname).bind(caller.id).bind(old.hostname).bind(caller.id).execute(&mut *tx).await.map_err(storage)?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(())
    }
    async fn request_check(&self, caller: Identity, id: i64, now: i64) -> Result<Domain, Error> {
        let mut tx = crate::postgres::locks::write(&self.pool)
            .await
            .map_err(storage)?;
        super::resources::authorize(&mut tx, caller, true).await?;
        super::settings::require_verification(&mut tx).await?;
        sqlx::query("SELECT id FROM ruphish_domains WHERE id=$1 AND owner_id=$2 FOR UPDATE")
            .bind(id)
            .bind(caller.id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?;
        let old = load(&mut tx, caller.id, id).await?;
        if !old.enabled {
            return Err(Error::Invalid("domain_disabled".into()));
        }
        if old.challenge_expires <= now {
            sqlx::query(
                "DELETE FROM ruphish_domain_checks WHERE domain_id=$1 AND finished_at IS NULL",
            )
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
            sqlx::query("UPDATE ruphish_domains SET challenge=$1,challenge_expires=$2,revision=revision+1,generation=generation+1,lease_until=0,routing_proof='' WHERE id=$3").bind(token()?).bind(now+CHALLENGE_LIFETIME).bind(id).execute(&mut *tx).await.map_err(storage)?;
        } else if old.active_check.is_some() {
            tx.commit()
                .await
                .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
            return Ok(old);
        }
        // Coalesce pending work and enforce a short request cooldown after completion.
        sqlx::query(
            "UPDATE ruphish_domains SET due=LEAST(due,GREATEST($1,checked_at+30)) WHERE id=$2",
        )
        .bind(now)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
        enqueue(&mut tx, id, now).await?;
        let result = load(&mut tx, caller.id, id).await?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(result)
    }
    async fn claim(&self, now: i64) -> Result<Option<Check>, Error> {
        let mut tx = crate::postgres::locks::write(&self.pool)
            .await
            .map_err(storage)?;
        if !super::settings::load(&mut tx)
            .await?
            .policy
            .verification_enabled
        {
            return Ok(None);
        }
        let candidate: Option<i64> = sqlx::query_scalar("SELECT id FROM ruphish_domains WHERE enabled=TRUE AND due<=$1 AND lease_until<=$2 ORDER BY due,id LIMIT 1")
            .bind(now).bind(now).fetch_optional(&mut *tx).await.map_err(storage)?;
        let Some(candidate) = candidate else {
            return Ok(None);
        };
        if !super::locks::domain_owner(&mut tx, candidate, true)
            .await
            .map_err(storage)?
        {
            return Ok(None);
        }
        let row=sqlx::query(sqlx::AssertSqlSafe(format!("{SELECT} WHERE d.enabled=TRUE AND d.due<=$1 AND d.lease_until<=$2 AND d.id=$3 FOR UPDATE OF d SKIP LOCKED"))).bind(now).bind(now).bind(candidate).fetch_optional(&mut *tx).await.map_err(storage)?;
        let Some(row) = row else {
            tx.commit()
                .await
                .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
            return Ok(None);
        };
        let generation = row.try_get::<i64, _>("generation").map_err(storage)? + 1;
        let domain = decode(row)?;
        // An abandoned lease is a completed timeout, not a successful old proof.
        if domain
            .active_check
            .as_ref()
            .is_some_and(|a| a.started_at.is_some())
        {
            sqlx::query("UPDATE ruphish_domain_checks SET finished_at=$1,outcome='timed_out',code='domain_worker_timeout' WHERE domain_id=$2 AND finished_at IS NULL").bind(now).bind(domain.id).execute(&mut *tx).await.map_err(storage)?;
            sqlx::query("UPDATE ruphish_domains SET verified_until=0,checked_at=$1,control=FALSE,dns=FALSE,routing=FALSE,tls=FALSE,code='domain_worker_timeout' WHERE id=$2").bind(now).bind(domain.id).execute(&mut *tx).await.map_err(storage)?;
            prune(&mut tx, domain.id).await?;
        }
        enqueue(&mut tx, domain.id, now).await?;
        sqlx::query("UPDATE ruphish_domain_checks SET started_at=$1 WHERE domain_id=$2 AND finished_at IS NULL").bind(now).bind(domain.id).execute(&mut *tx).await.map_err(storage)?;
        let routing_proof = token()?;
        sqlx::query(
            "UPDATE ruphish_domains SET generation=$1,lease_until=$2,routing_proof=$3 WHERE id=$4",
        )
        .bind(generation)
        .bind(now + 90)
        .bind(&routing_proof)
        .bind(domain.id)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(Some(Check {
            started_at: now,
            domain,
            generation,
            routing_proof,
        }))
    }
    async fn finish(&self, check: &Check, mut o: Observation, now: i64) -> Result<(), Error> {
        let mut tx = crate::postgres::locks::write(&self.pool)
            .await
            .map_err(storage)?;
        let policy = super::settings::load(&mut tx).await?.policy;
        if !policy.verification_enabled {
            return Ok(());
        }
        if !super::locks::domain_owner(&mut tx, check.domain.id, false)
            .await
            .map_err(storage)?
        {
            return Ok(());
        }
        let row=sqlx::query("SELECT owner_id,failures FROM ruphish_domains WHERE id=$1 AND revision=$2 AND generation=$3 AND lease_until>$4 AND enabled=TRUE FOR UPDATE").bind(check.domain.id).bind(check.domain.revision).bind(check.generation).bind(now).fetch_optional(&mut *tx).await.map_err(storage)?;
        let Some(row) = row else { return Ok(()) };
        let owner: i64 = row.try_get("owner_id").map_err(storage)?;
        let mut success = o.control && o.dns && o.routing && o.tls;
        if success {
            sqlx::query(
                "INSERT INTO ruphish_domain_claims(hostname,owner_id) VALUES($1,$2) ON CONFLICT (hostname) DO NOTHING",
            )
            .bind(&check.domain.hostname)
            .bind(owner)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
            let claimed: i64 =
                sqlx::query_scalar("SELECT owner_id FROM ruphish_domain_claims WHERE hostname=$1")
                    .bind(&check.domain.hostname)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(storage)?;
            if claimed != owner {
                success = false;
                o.code = "domain_claim_conflict";
            }
        }
        let failures = if success {
            0
        } else {
            row.try_get::<i64, _>("failures")
                .map_err(storage)?
                .saturating_add(1)
                .min(10)
        };
        let due = now
            + if success {
                policy.refresh_delay(check.domain.id)
            } else {
                retry_delay(failures as u32)
            };
        sqlx::query("UPDATE ruphish_domains SET checked_at=$1,verified_until=CASE WHEN $2 THEN $3 ELSE 0 END,control=$4,dns=$5,routing=$6,tls=$7,code=$8,lease_until=0,routing_proof='',due=$9,failures=$10,challenge_expires=CASE WHEN $11 THEN $12 ELSE challenge_expires END WHERE id=$13").bind(now).bind(success).bind(now+policy.verification_validity_seconds).bind(o.control).bind(o.dns).bind(o.routing).bind(o.tls).bind(if success{"verified"}else{o.code}).bind(due).bind(failures).bind(success).bind(now+CHALLENGE_LIFETIME).bind(check.domain.id).execute(&mut *tx).await.map_err(storage)?;
        sqlx::query("UPDATE ruphish_domain_checks SET finished_at=$1,outcome=$2,code=$3 WHERE domain_id=$4 AND finished_at IS NULL").bind(now).bind(if success {"passed"}else if o.code=="domain_check_timeout" {"timed_out"}else{"failed"}).bind(if success {"verified"}else{o.code}).bind(check.domain.id).execute(&mut *tx).await.map_err(storage)?;
        prune(&mut tx, check.domain.id).await?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(())
    }
    async fn serves(&self, authority: &str) -> Result<bool, Error> {
        let Ok(origin) = origin(&format!("https://{authority}"), false) else {
            return Ok(false);
        };
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ruphish_domains WHERE origin=$1 AND enabled=TRUE AND (SELECT serves FROM ruphish_domain_policy WHERE id=ruphish_domains.id))").bind(origin.value).fetch_one(&self.pool).await.map_err(storage)
    }
    async fn routing_proof(
        &self,
        authority: &str,
        challenge: &str,
        now: i64,
    ) -> Result<Option<String>, Error> {
        if challenge.len() != 64 {
            return Ok(None);
        };
        let Ok(origin) = origin(&format!("https://{authority}"), false) else {
            return Ok(None);
        };
        sqlx::query_scalar("SELECT routing_proof FROM ruphish_domains WHERE origin=$1 AND challenge=$2 AND enabled=TRUE AND lease_until>$3 AND routing_proof!='' LIMIT 1").bind(origin.value).bind(challenge).bind(now).fetch_optional(&self.pool).await.map_err(storage)
    }
}
