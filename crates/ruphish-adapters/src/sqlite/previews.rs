//! Preview persistence; owned snapshots leave no connection held by the renderer.
use ruphish_application::{
    auth::Identity,
    delivery::{MessageInput, PreviewPersistence, PreviewSnapshot},
    resources::*,
};
use sqlx::{Row, SqlitePool};

/// Persistence using the existing email_requests table.
pub struct PreviewStore {
    pool: SqlitePool,
}
impl PreviewStore {
    /// Compose the concrete dependencies for this boundary.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}
#[async_trait::async_trait]
impl PreviewPersistence for PreviewStore {
    async fn contains(&self, rid: &str) -> Result<bool, Error> {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM email_requests WHERE r_id=?)")
            .bind(rid)
            .fetch_one(&self.pool)
            .await
            .map_err(crate::sqlite::resource_error)
    }
    async fn store(&self, caller: Identity, m: &MessageInput) -> Result<(), Error> {
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(crate::sqlite::resource_error)?;
        let user = super::accounts::account(&mut tx, caller.id)
            .await
            .map_err(Error::from)?;
        user.authenticate(caller)?;
        user.require("modify_objects")?;
        // Recheck both ownership and continued existence after message composition.
        for (kind, id) in [(Kind::Template, m.template_id), (Kind::Page, m.page_id)] {
            if id != 0 {
                super::resources::load(&mut tx, caller.id, kind, id).await?;
            }
        }
        let visible = if m.template.envelope_sender.is_empty() {
            &m.profile.from_address
        } else {
            &m.template.envelope_sender
        };
        sqlx::query("INSERT INTO email_requests(user_id,template_id,page_id,first_name,last_name,email,position,url,r_id,from_address) VALUES(?,?,?,?,?,?,?,?,?,?)").bind(caller.id).bind(m.template_id).bind(m.page_id).bind(&m.recipient.first_name).bind(&m.recipient.last_name).bind(&m.recipient.email).bind(&m.recipient.position).bind(&m.url).bind(&m.rid).bind(visible).execute(&mut *tx).await.map_err(crate::sqlite::resource_error)?;
        tx.commit().await.map_err(crate::sqlite::resource_error)?;
        Ok(())
    }
    async fn snapshot(&self, rid: &str) -> Result<PreviewSnapshot, Error> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(crate::sqlite::resource_error)?;
        let row = sqlx::query("SELECT * FROM email_requests WHERE r_id=?")
            .bind(rid)
            .fetch_optional(&mut *tx)
            .await
            .map_err(crate::sqlite::resource_error)?
            .ok_or(Error::NotFound("Preview"))?;
        let text = |key: &str| {
            row.try_get::<Option<String>, _>(key)
                .map(|v| v.unwrap_or_default())
                .map_err(crate::sqlite::resource_error)
        };
        let owner = row
            .try_get("user_id")
            .map_err(crate::sqlite::resource_error)?;
        let id = row
            .try_get("page_id")
            .map_err(crate::sqlite::resource_error)?;
        let resource = super::resources::load(&mut tx, owner, Kind::Page, id).await?;
        let Content::Page(page) = resource.content else {
            return Err(Error::Internal);
        };
        let message = MessageInput {
            track_opens: false,
            message_id: None,
            recipient: Recipient {
                email: text("email")?,
                first_name: text("first_name")?,
                last_name: text("last_name")?,
                position: text("position")?,
            },
            template: Template::default(),
            template_id: 0,
            page_id: id,
            profile: Profile {
                from_address: text("from_address")?,
                ..Profile::default()
            },
            url: text("url")?,
            rid: rid.to_owned(),
        };
        tx.commit().await.map_err(crate::sqlite::resource_error)?;
        Ok(PreviewSnapshot { page, message })
    }
}
