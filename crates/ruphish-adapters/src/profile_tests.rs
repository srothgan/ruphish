//! One transactional attempt algorithm shared by the three SQL providers.
macro_rules! repository {
    ($connection:ty, $pool:ty, $row:ty, $postgres:expr) => {
        use ruphish_application::{auth::Identity, profile_tests::*, resources::Error};
        use sqlx::Row;
        macro_rules! query {
            ($sql:expr) => { sqlx::query(sqlx::AssertSqlSafe(crate::sql::placeholders($sql, $postgres))) };
        }
        /// Authorized test operations sharing the startup-owned pool.
        pub struct Tests { pool: $pool }
        impl Tests {
            /// Attach to the migrated startup database.
            pub fn new(pool: $pool) -> Self { Self { pool } }
        }
        fn storage(error: sqlx::Error) -> Error { super::resource_error(error) }
        fn encoded<T: serde::Serialize>(value: T) -> Result<String, Error> {
            serde_json::to_value(value).map_err(|_| Error::Internal)?.as_str().map(str::to_owned).ok_or(Error::Internal)
        }
        fn decode(row: $row) -> Result<Attempt, Error> {
            let parse = |column| -> Result<serde_json::Value, Error> { Ok(serde_json::Value::String(row.try_get::<String,_>(column).map_err(storage)?)) };
            Ok(Attempt {
                id: row.try_get("id").map_err(storage)?, generation: row.try_get("generation").map_err(storage)?,
                kind: serde_json::from_value(parse("kind")?).map_err(|_| Error::Internal)?,
                operator: row.try_get("operator").map_err(storage)?, email: row.try_get("email").map_err(storage)?,
                started_at: row.try_get("started_at").map_err(storage)?, finished_at: row.try_get("finished_at").map_err(storage)?,
                outcome: serde_json::from_value(parse("outcome")?).map_err(|_| Error::Internal)?,
                code: row.try_get("code").map_err(storage)?,
                smtp_code: row.try_get::<Option<i32>,_>("smtp_code").map_err(storage)?.map(|v| u16::try_from(v).map_err(|_| Error::Internal)).transpose()?,
            })
        }
        async fn recover(c: &mut $connection, profile: i64, now: i64) -> Result<(), Error> {
            query!("UPDATE ruphish_profile_tests SET outcome=CASE WHEN outcome='submitting' THEN 'unknown' ELSE 'interrupted' END,code='profile_test_interrupted',finished_at=? WHERE profile_id=? AND finished_at IS NULL AND deadline<=?")
                .bind(now).bind(profile).bind(now).execute(c).await.map_err(storage)?;
            Ok(())
        }
        /// Read launch readiness within the caller's already authorized transaction.
        pub(super) async fn eligible(c: &mut $connection, profile: i64) -> Result<bool, Error> {
            let row = query!("SELECT s.test_generation,p.sending_profile_test_required FROM smtp s CROSS JOIN ruphish_settings p WHERE s.id=? AND p.id=1")
                .bind(profile).fetch_one(&mut *c).await.map_err(storage)?;
            if !row.try_get::<bool,_>("sending_profile_test_required").map_err(storage)? { return Ok(true); }
            let generation: i64 = row.try_get("test_generation").map_err(storage)?;
            let active = query!("SELECT id FROM ruphish_profile_tests WHERE profile_id=? AND finished_at IS NULL LIMIT 1").bind(profile).fetch_optional(&mut *c).await.map_err(storage)?;
            if active.is_some() { return Ok(false); }
            let decisive = query!("SELECT * FROM ruphish_profile_tests WHERE profile_id=? AND generation=? AND outcome IN ('accepted','failed','interrupted','unknown') ORDER BY sequence DESC LIMIT 1")
                .bind(profile).bind(generation).fetch_optional(c).await.map_err(storage)?.map(decode).transpose()?;
            Ok(decisive.is_some_and(|attempt| ruphish_domain::profile_tests::qualifies([&attempt], generation)))
        }
        #[async_trait::async_trait]
        impl Repository for Tests {
            async fn read(&self, caller: Identity, profile: i64, now: i64) -> Result<Summary, Error> {
                let mut tx = write(&self.pool).await.map_err(storage)?;
                super::resources::authorize(&mut tx, caller, false).await?;
                guard(&mut tx).await.map_err(storage)?;
                super::resources::load(&mut tx, ruphish_application::resources::Kind::Profile, profile).await?;
                recover(&mut tx, profile, now).await?;
                let eligible = eligible(&mut tx, profile).await?;
                let row = query!("SELECT s.test_generation,p.sending_profile_test_required FROM smtp s CROSS JOIN ruphish_settings p WHERE s.id=? AND p.id=1").bind(profile).fetch_one(&mut *tx).await.map_err(storage)?;
                let history = query!("SELECT * FROM ruphish_profile_tests WHERE profile_id=? AND finished_at IS NOT NULL ORDER BY sequence DESC LIMIT 10").bind(profile).fetch_all(&mut *tx).await.map_err(storage)?.into_iter().map(decode).collect::<Result<Vec<_>,_>>()?;
                let active = query!("SELECT * FROM ruphish_profile_tests WHERE profile_id=? AND finished_at IS NULL ORDER BY sequence DESC LIMIT 1").bind(profile).fetch_optional(&mut *tx).await.map_err(storage)?.map(decode).transpose()?;
                let mut latest = Vec::new();
                for kind in ["connection", "email"] {
                    latest.push(query!("SELECT * FROM ruphish_profile_tests WHERE profile_id=? AND kind=? AND finished_at IS NOT NULL ORDER BY sequence DESC LIMIT 1").bind(profile).bind(kind).fetch_optional(&mut *tx).await.map_err(storage)?.map(decode).transpose()?);
                }
                let result = Summary { generation: row.try_get("test_generation").map_err(storage)?, required: row.try_get("sending_profile_test_required").map_err(storage)?, eligible, active, connection: latest.remove(0), email: latest.remove(0), history };
                tx.commit().await.map_err(storage)?;
                Ok(result)
            }
            async fn begin(&self, caller: Identity, profile: i64, request: Request, now: i64) -> Result<Start, Error> {
                let mut tx = write(&self.pool).await.map_err(storage)?;
                super::resources::authorize(&mut tx, caller, true).await?;
                recover(&mut tx, profile, now).await?;
                let resource = super::resources::load(&mut tx, ruphish_application::resources::Kind::Profile, profile).await?;
                if let Some(row) = query!("SELECT * FROM ruphish_profile_tests WHERE id=?").bind(&request.id).fetch_optional(&mut *tx).await.map_err(storage)? {
                    let profile_id: i64 = row.try_get("profile_id").map_err(storage)?;
                    let revision: i64 = row.try_get("profile_revision").map_err(storage)?;
                    let attempt = decode(row)?;
                    if profile_id != profile || attempt.operator != caller.id || attempt.kind != request.kind || attempt.email != request.email || revision != request.revision { return Err(Error::Changed); }
                    tx.commit().await.map_err(storage)?;
                    return Ok(Start::Existing(attempt));
                }
                ruphish_application::resources::require_revision(resource.revision, Some(request.revision))?;
                if query!("SELECT id FROM ruphish_profile_tests WHERE profile_id=? AND finished_at IS NULL LIMIT 1").bind(profile).fetch_optional(&mut *tx).await.map_err(storage)?.is_some() { return Err(Error::ProfileTestBusy); }
                let generation: i64 = query!("SELECT test_generation FROM smtp WHERE id=?").bind(profile).fetch_one(&mut *tx).await.map_err(storage)?.try_get("test_generation").map_err(storage)?;
                query!("INSERT INTO ruphish_profile_tests(id,profile_id,generation,profile_revision,kind,operator,email,started_at,deadline,outcome) VALUES(?,?,?,?,?,?,?,?,?,'running')")
                    .bind(&request.id).bind(profile).bind(generation).bind(request.revision).bind(encoded(request.kind)?).bind(caller.id).bind(&request.email).bind(now).bind(now.saturating_add(120)).execute(&mut *tx).await.map_err(storage)?;
                let attempt = Attempt { id: request.id, generation, kind: request.kind, operator: caller.id, email: request.email, started_at: now, finished_at: None, outcome: Outcome::Running, code: String::new(), smtp_code: None };
                let ruphish_application::resources::Content::Profile(settings) = resource.content else { return Err(Error::Internal); };
                tx.commit().await.map_err(storage)?;
                Ok(Start::Started(attempt, settings))
            }
            async fn submitting(&self, id: &str, now: i64) -> Result<(), Error> {
                let changed = query!("UPDATE ruphish_profile_tests SET outcome='submitting' WHERE id=? AND kind='email' AND outcome='running' AND finished_at IS NULL AND deadline>?").bind(id).bind(now).execute(&self.pool).await.map_err(storage)?.rows_affected();
                if changed != 1 { return Err(Error::Changed); }
                Ok(())
            }
            async fn finish(&self, id: &str, outcome: Outcome, code: &str, smtp_code: Option<u16>, now: i64) -> Result<Attempt, Error> {
                let mut tx = write(&self.pool).await.map_err(storage)?;
                guard(&mut tx).await.map_err(storage)?;
                query!("UPDATE ruphish_profile_tests SET outcome=?,code=?,smtp_code=?,finished_at=? WHERE id=? AND finished_at IS NULL")
                    .bind(encoded(outcome)?).bind(code).bind(smtp_code.map(i32::from)).bind(now).bind(id).execute(&mut *tx).await.map_err(storage)?;
                let result = decode(query!("SELECT * FROM ruphish_profile_tests WHERE id=?").bind(id).fetch_optional(&mut *tx).await.map_err(storage)?.ok_or(Error::NotFound("Profile test"))?)?;
                tx.commit().await.map_err(storage)?;
                Ok(result)
            }
        }
    };
}
pub(crate) use repository;
