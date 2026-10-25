//! Shared public rendering between a provider snapshot and a fenced event commit.
use ruphish_application::{interactions::*, resources::Error};
use std::sync::Arc;
pub(crate) struct Interactions {
    store: Arc<dyn Persistence>,
    rendering: Arc<tokio::sync::Semaphore>,
}
impl Interactions {
    pub(crate) fn new(store: Arc<dyn Persistence>, rendering: Arc<tokio::sync::Semaphore>) -> Self {
        Self { store, rendering }
    }
}
fn missing() -> Error {
    Error::NotFound("Interaction")
}
#[async_trait::async_trait]
impl Public for Interactions {
    async fn learning_page(&self, authority: &str, public_id: &str) -> Result<Output, Error> {
        if public_id.len() != 32 || !public_id.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(missing());
        }
        let origin = ruphish_domain::domains::origin(&format!("https://{authority}"), false)
            .map_err(|_| missing())?;
        let source = self.store.learning_page(&origin.value, public_id).await?;
        let permit = self
            .rendering
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Internal)?;
        let document = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            crate::landing::learning(&source)
        })
        .await
        .map_err(|_| Error::Internal)??;
        Ok(Output::Html {
            html: document.html,
            csp: document.csp,
        })
    }
    async fn interact(
        &self,
        authority: &str,
        rid: &str,
        action: Action,
        now: i64,
    ) -> Result<Output, Error> {
        action.validate()?;
        if rid.is_empty()
            || rid.len() > 128
            || !rid.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        {
            return Err(missing());
        }
        let origin = ruphish_domain::domains::origin(&format!("https://{authority}"), false)
            .map_err(|_| missing())?;
        let mut snapshot = self.store.snapshot(&origin.value, rid, &action).await?;
        if matches!(action, Action::Inspect) {
            return Ok(Output::Transparency {
                send_date: snapshot.send_date,
            });
        }
        if matches!(action, Action::Head) {
            return Ok(Output::Empty);
        }
        // The snapshot owns its data. Context construction (including URL templates),
        // template execution and final HTML sanitization all run without a connection.
        let (document, destination) = if let Some(RenderInput {
            message,
            redirect,
            source,
        }) = snapshot.rendering.take()
        {
            // A cancelled HTTP request must not release capacity while its blocking
            // parser is still running. The task owns the permit until it finishes.
            let permit = self
                .rendering
                .clone()
                .try_acquire_owned()
                .map_err(|_| Error::Internal)?;
            let endpoint = format!("{}/?rid={rid}", origin.value);
            let base = origin.value.clone();
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                let context = crate::smtp::context(&message)?;
                let redirect =
                    crate::resources::render(&redirect, &context).map_err(|_| missing())?;
                let destination = crate::landing::redirect(&redirect, &base)?;
                let document = source
                    .map(|source| {
                        let html =
                            crate::resources::render(&source, &context).map_err(|_| missing())?;
                        crate::landing::render(&html, Some(&endpoint))
                    })
                    .transpose()?;
                Ok::<_, Error>((document, destination))
            })
            .await
            .map_err(|_| Error::Internal)??
        } else {
            (None, None)
        };

        self.store
            .record(&snapshot, &origin.value, &action, now)
            .await?;
        match action {
            Action::View => {
                let document = document.ok_or(Error::Internal)?;
                Ok(Output::Html {
                    html: document.html,
                    csp: document.csp,
                })
            }
            Action::Open => Ok(Output::Pixel),
            Action::Submit { .. } => Ok(Output::Submitted {
                redirect: destination,
            }),
            _ => Ok(Output::Empty),
        }
    }
}
