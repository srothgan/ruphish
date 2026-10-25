//! Owner-scoped resource transactions over the preserved legacy tables.
use crate::mysql::InsertId;
use ruphish_application::datetime;
use ruphish_application::{auth::Identity, resources::*};
use sqlx::{MySqlConnection, MySqlPool, Row, mysql::MySqlRow};

#[derive(Clone)]
/// Resource repository over the startup-owned MySQL pool.
pub struct Resources {
    pool: MySqlPool,
}
impl Resources {
    /// Compose the concrete dependencies for this boundary.
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}
fn storage(error: sqlx::Error) -> Error {
    crate::mysql::resource_error(error)
}
fn table(kind: Kind) -> &'static str {
    match kind {
        Kind::Group => "`groups`",
        Kind::Template => "templates",
        Kind::Page => "pages",
        Kind::LearningPage => "ruphish_learning_pages",
        Kind::Profile => "smtp",
    }
}
pub(super) async fn authorize(
    c: &mut MySqlConnection,
    caller: Identity,
    write: bool,
) -> Result<ruphish_application::auth::Account, Error> {
    if write {
        super::locks::owner(c, caller.id).await.map_err(storage)?;
    }
    let user = super::accounts::account(c, caller.id)
        .await
        .map_err(Error::from)?;
    user.authenticate(caller)?;
    user.require(if write {
        "modify_objects"
    } else {
        "view_objects"
    })?;
    Ok(user)
}
fn text(row: &MySqlRow, field: &str) -> Result<String, Error> {
    Ok(row
        .try_get::<Option<String>, _>(field)
        .map_err(storage)?
        .unwrap_or_default())
}
fn boolean(row: &MySqlRow, field: &str) -> Result<bool, Error> {
    Ok(row
        .try_get::<Option<bool>, _>(field)
        .map_err(storage)?
        .unwrap_or_default())
}
fn timestamp(row: &MySqlRow) -> Result<String, Error> {
    let value = text(row, "modified_date")?;
    if value.is_empty() {
        return Ok("0001-01-01T00:00:00Z".into());
    }
    datetime::parse_stored(&value)
        .map(datetime::utc)
        .map_err(|_| Error::Internal)
}
pub(super) async fn load(
    c: &mut MySqlConnection,
    owner: i64,
    kind: Kind,
    id: i64,
) -> Result<Resource, Error> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT * FROM {} WHERE id=? AND user_id=?",
        table(kind)
    )))
    .bind(id)
    .bind(owner)
    .fetch_optional(&mut *c)
    .await
    .map_err(storage)?
    .ok_or(Error::NotFound(kind.label()))?;
    let content = match kind {
        Kind::Group => {
            let rows = sqlx::query("SELECT t.* FROM targets t JOIN group_targets g ON g.target_id=t.id WHERE g.group_id=? ORDER BY t.id").bind(id).fetch_all(&mut *c).await.map_err(storage)?;
            Content::Group(
                rows.iter()
                    .map(|r| {
                        Ok(Recipient {
                            email: text(r, "email")?,
                            first_name: text(r, "first_name")?,
                            last_name: text(r, "last_name")?,
                            position: text(r, "position")?,
                        })
                    })
                    .collect::<Result<_, Error>>()?,
            )
        }
        Kind::Template => {
            let rows = sqlx::query("SELECT * FROM attachments WHERE template_id=? ORDER BY id")
                .bind(id)
                .fetch_all(&mut *c)
                .await
                .map_err(storage)?;
            Content::Template(Template {
                subject: text(&row, "subject")?,
                text: text(&row, "text")?,
                html: text(&row, "html")?,
                envelope_sender: text(&row, "envelope_sender")?,
                attachments: rows
                    .iter()
                    .map(|r| {
                        Ok(Attachment {
                            name: text(r, "name")?,
                            mime_type: text(r, "type")?,
                            content: text(r, "content")?,
                        })
                    })
                    .collect::<Result<_, Error>>()?,
            })
        }
        Kind::LearningPage => Content::LearningPage(LearningPage {
            html: text(&row, "html")?,
            public_id: text(&row, "public_id")?,
        }),
        Kind::Page => Content::Page(Page {
            html: text(&row, "html")?,
            redirect_url: text(&row, "redirect_url")?,
        }),
        Kind::Profile => {
            let rows = sqlx::query("SELECT * FROM headers WHERE smtp_id=? ORDER BY id")
                .bind(id)
                .fetch_all(&mut *c)
                .await
                .map_err(storage)?;
            Content::Profile(Profile {
                interface_type: text(&row, "interface_type")?,
                host: text(&row, "host")?,
                username: text(&row, "username")?,
                password: text(&row, "password")?,
                from_address: text(&row, "from_address")?,
                ignore_cert_errors: boolean(&row, "ignore_cert_errors")?,
                headers: rows
                    .iter()
                    .map(|r| {
                        Ok(Header {
                            key: text(r, "key")?,
                            value: text(r, "value")?,
                        })
                    })
                    .collect::<Result<_, Error>>()?,
            })
        }
    };
    Ok(Resource {
        id,
        name: text(&row, "name")?,
        modified_date: timestamp(&row)?,
        content,
    })
}
async fn usage(c: &mut MySqlConnection, kind: Kind, id: i64) -> Result<i64, Error> {
    if kind == Kind::LearningPage {
        return sqlx::query_scalar("SELECT count(*) FROM campaigns c JOIN ruphish_campaign_policy p ON p.campaign_id=c.id WHERE p.learning_page_id=? AND COALESCE(c.status,'') != 'Completed'")
            .bind(id).fetch_one(c).await.map_err(storage);
    }
    let column = match kind {
        Kind::Group | Kind::LearningPage => return Ok(0),
        Kind::Template => "template_id",
        Kind::Page => "page_id",
        Kind::Profile => "smtp_id",
    };
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) FROM campaigns WHERE {column}=? AND COALESCE(status,'') != 'Completed'"
    )))
    .bind(id)
    .fetch_one(c)
    .await
    .map_err(storage)
}
fn condition(old: &Resource, expected: Option<&str>) -> Result<(), Error> {
    if expected.is_some_and(|v| v != old.modified_date) {
        return Err(Error::Changed);
    }
    Ok(())
}
async fn clear_children(c: &mut MySqlConnection, kind: Kind, id: i64) -> Result<(), Error> {
    match kind {
        Kind::Group => {
            // Delete only rows formerly owned by this group and unreferenced elsewhere.
            let targets: Vec<i64> =
                sqlx::query_scalar("SELECT target_id FROM group_targets WHERE group_id=?")
                    .bind(id)
                    .fetch_all(&mut *c)
                    .await
                    .map_err(storage)?;
            sqlx::query("DELETE FROM group_targets WHERE group_id=?")
                .bind(id)
                .execute(&mut *c)
                .await
                .map_err(storage)?;
            for target in targets {
                sqlx::query("DELETE FROM targets WHERE id=? AND NOT EXISTS(SELECT 1 FROM group_targets WHERE target_id=?)").bind(target).bind(target).execute(&mut *c).await.map_err(storage)?;
            }
        }
        Kind::Template => {
            sqlx::query("DELETE FROM attachments WHERE template_id=?")
                .bind(id)
                .execute(c)
                .await
                .map_err(storage)?;
        }
        Kind::Profile => {
            sqlx::query("DELETE FROM headers WHERE smtp_id=?")
                .bind(id)
                .execute(c)
                .await
                .map_err(storage)?;
        }
        Kind::Page | Kind::LearningPage => {}
    }
    Ok(())
}
#[async_trait::async_trait]
impl Repository for Resources {
    async fn list(&self, caller: Identity, kind: Kind) -> Result<Vec<Resource>, Error> {
        let mut tx = crate::mysql::locks::snapshot(&self.pool)
            .await
            .map_err(storage)?;
        authorize(&mut tx, caller, false).await?;
        let ids: Vec<i64> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT id FROM {} WHERE user_id=? ORDER BY id",
            table(kind)
        )))
        .bind(caller.id)
        .fetch_all(&mut *tx)
        .await
        .map_err(storage)?;
        let mut result = Vec::with_capacity(ids.len());
        for id in ids {
            result.push(load(&mut tx, caller.id, kind, id).await?);
        }
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::mysql::commit_error(e)))?;
        Ok(result)
    }
    async fn summaries(&self, caller: Identity, kind: Kind) -> Result<Vec<Summary>, Error> {
        let mut tx = crate::mysql::locks::snapshot(&self.pool)
            .await
            .map_err(storage)?;
        authorize(&mut tx, caller, false).await?;
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT id,name,modified_date FROM {} WHERE user_id=? ORDER BY id",
            table(kind)
        )))
        .bind(caller.id)
        .fetch_all(&mut *tx)
        .await
        .map_err(storage)?;
        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            let id = row.try_get("id").map_err(storage)?;
            let num_targets = if kind == Kind::Group {
                sqlx::query_scalar("SELECT count(*) FROM group_targets WHERE group_id=?")
                    .bind(id)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(storage)?
            } else {
                0
            };
            result.push(Summary {
                id,
                name: text(&row, "name")?,
                modified_date: timestamp(&row)?,
                num_targets,
                active_campaigns: usage(&mut tx, kind, id).await?,
            });
        }
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::mysql::commit_error(e)))?;
        Ok(result)
    }
    async fn get(&self, caller: Identity, kind: Kind, id: i64) -> Result<Resource, Error> {
        let mut tx = crate::mysql::locks::snapshot(&self.pool)
            .await
            .map_err(storage)?;
        authorize(&mut tx, caller, false).await?;
        let result = load(&mut tx, caller.id, kind, id).await?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::mysql::commit_error(e)))?;
        Ok(result)
    }
    async fn get_by_name(
        &self,
        caller: Identity,
        kind: Kind,
        name: &str,
    ) -> Result<Resource, Error> {
        let mut tx = crate::mysql::locks::snapshot(&self.pool)
            .await
            .map_err(storage)?;
        authorize(&mut tx, caller, false).await?;
        let id: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT id FROM {} WHERE user_id=? AND name=? ORDER BY id LIMIT 1",
            table(kind)
        )))
        .bind(caller.id)
        .bind(name)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage)?
        .ok_or(Error::NotFound(kind.label()))?;
        let result = load(&mut tx, caller.id, kind, id).await?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::mysql::commit_error(e)))?;
        Ok(result)
    }
    async fn save(
        &self,
        caller: Identity,
        mut value: Resource,
        existing: Option<i64>,
        expected: Option<String>,
    ) -> Result<Resource, Error> {
        let mut tx = crate::mysql::locks::write(&self.pool)
            .await
            .map_err(storage)?;
        authorize(&mut tx, caller, true).await?;
        crate::database::text::resource(&value)?;
        let kind = value.kind();
        let old = if let Some(id) = existing {
            let old = load(&mut tx, caller.id, kind, id).await?;
            condition(&old, expected.as_deref())?;
            Some(old)
        } else {
            if expected.is_some() {
                return Err(Error::Changed);
            }
            None
        };
        if old.as_ref().is_none_or(|o| o.name != value.name) {
            let duplicate: bool = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "SELECT EXISTS(SELECT 1 FROM {} WHERE user_id=? AND name=?)",
                table(kind)
            )))
            .bind(caller.id)
            .bind(&value.name)
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?;
            if duplicate {
                return Err(Error::Conflict(kind.label()));
            }
        }
        value.modified_date = datetime::next_revision(
            old.as_ref().map(|old| old.modified_date.as_str()),
            datetime::now(),
        )
        .map_err(|_| Error::Internal)?;
        value.id = if let Some(id) = existing {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "UPDATE {} SET name=?,modified_date=? WHERE id=? AND user_id=?",
                table(kind)
            )))
            .bind(&value.name)
            .bind(&value.modified_date)
            .bind(id)
            .bind(caller.id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
            clear_children(&mut tx, kind, id).await?;
            id
        } else {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "INSERT INTO {}(user_id,name,modified_date) VALUES(?,?,?)",
                table(kind)
            )))
            .bind(caller.id)
            .bind(&value.name)
            .bind(&value.modified_date)
            .execute(&mut *tx)
            .await
            .map_err(storage)?
            .insert_id()
            .map_err(storage)?
        };
        if let Content::LearningPage(page) = &mut value.content {
            page.public_id = match old.as_ref().map(|resource| &resource.content) {
                Some(Content::LearningPage(previous)) => previous.public_id.clone(),
                _ => {
                    let mut bytes = [0; 16];
                    getrandom::fill(&mut bytes).map_err(|_| Error::Internal)?;
                    hex::encode(bytes)
                }
            };
        }
        let id = value.id;
        match &value.content {
            Content::Group(targets) => {
                for t in targets {
                    let target = sqlx::query(
                        "INSERT INTO targets(email,first_name,last_name,position) VALUES(?,?,?,?)",
                    )
                    .bind(&t.email)
                    .bind(&t.first_name)
                    .bind(&t.last_name)
                    .bind(&t.position)
                    .execute(&mut *tx)
                    .await
                    .map_err(storage)?
                    .insert_id()
                    .map_err(storage)?;
                    sqlx::query("INSERT INTO group_targets(group_id,target_id) VALUES(?,?)")
                        .bind(id)
                        .bind(target)
                        .execute(&mut *tx)
                        .await
                        .map_err(storage)?;
                }
            }
            Content::Template(t) => {
                sqlx::query(
                    "UPDATE templates SET subject=?,text=?,html=?,envelope_sender=? WHERE id=?",
                )
                .bind(&t.subject)
                .bind(&t.text)
                .bind(&t.html)
                .bind(&t.envelope_sender)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
                for a in &t.attachments {
                    sqlx::query(
                        "INSERT INTO attachments(template_id,name,type,content) VALUES(?,?,?,?)",
                    )
                    .bind(id)
                    .bind(&a.name)
                    .bind(&a.mime_type)
                    .bind(&a.content)
                    .execute(&mut *tx)
                    .await
                    .map_err(storage)?;
                }
            }
            Content::LearningPage(p) => {
                sqlx::query("UPDATE ruphish_learning_pages SET html=?,public_id=? WHERE id=?")
                    .bind(&p.html)
                    .bind(&p.public_id)
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(storage)?;
            }
            Content::Page(p) => {
                sqlx::query("UPDATE pages SET html=?,redirect_url=? WHERE id=?")
                    .bind(&p.html)
                    .bind(&p.redirect_url)
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(storage)?;
            }
            Content::Profile(p) => {
                sqlx::query("UPDATE smtp SET interface_type=?,host=?,username=?,password=?,from_address=?,ignore_cert_errors=? WHERE id=?").bind(&p.interface_type).bind(&p.host).bind(&p.username).bind(&p.password).bind(&p.from_address).bind(p.ignore_cert_errors).bind(id).execute(&mut *tx).await.map_err(storage)?;
                for h in &p.headers {
                    sqlx::query("INSERT INTO headers(smtp_id,`key`,`value`) VALUES(?,?,?)")
                        .bind(id)
                        .bind(&h.key)
                        .bind(&h.value)
                        .execute(&mut *tx)
                        .await
                        .map_err(storage)?;
                }
            }
        }
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::mysql::commit_error(e)))?;
        Ok(value)
    }
    async fn delete(
        &self,
        caller: Identity,
        kind: Kind,
        id: i64,
        expected: Option<String>,
    ) -> Result<(), Error> {
        let mut tx = crate::mysql::locks::write(&self.pool)
            .await
            .map_err(storage)?;
        authorize(&mut tx, caller, true).await?;
        let old = load(&mut tx, caller.id, kind, id).await?;
        condition(&old, expected.as_deref())?;
        if usage(&mut tx, kind, id).await? > 0 {
            return Err(Error::InUse);
        }
        clear_children(&mut tx, kind, id).await?;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DELETE FROM {} WHERE id=? AND user_id=?",
            table(kind)
        )))
        .bind(id)
        .bind(caller.id)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::mysql::commit_error(e)))?;
        Ok(())
    }
}
