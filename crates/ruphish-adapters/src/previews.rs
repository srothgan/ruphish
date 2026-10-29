//! Shared test-preview identity and rendering behavior.
use ruphish_application::{
    auth::Identity,
    delivery::{MessageInput, PreviewPersistence, Previews},
    resources::Error,
};
use std::sync::Arc;
pub(crate) struct PreviewService {
    store: Arc<dyn PreviewPersistence>,
    rendering: Arc<tokio::sync::Semaphore>,
}
impl PreviewService {
    pub(crate) fn new(
        store: Arc<dyn PreviewPersistence>,
        rendering: Arc<tokio::sync::Semaphore>,
    ) -> Self {
        Self { store, rendering }
    }
}
fn valid(rid: &str) -> bool {
    rid.starts_with("preview-") && rid.len() <= 128
}
#[async_trait::async_trait]
impl Previews for PreviewService {
    fn id(&self) -> Result<String, Error> {
        let mut bytes = [0; 24];
        getrandom::fill(&mut bytes).map_err(|_| Error::Internal)?;
        Ok(format!("preview-{}", hex::encode(bytes)))
    }
    async fn contains(&self, rid: &str) -> Result<bool, Error> {
        if !valid(rid) {
            return Ok(false);
        }
        self.store.contains(rid).await
    }
    async fn store(&self, caller: Identity, m: &MessageInput) -> Result<(), Error> {
        self.store.store(caller, m).await
    }
    async fn render(&self, rid: &str) -> Result<String, Error> {
        if !valid(rid) {
            return Err(Error::NotFound("Preview"));
        }
        let snapshot = self.store.snapshot(rid).await?;
        ruphish_application::content_work::ContentWork::new(self.rendering.clone())
            .run(move || {
                let context = crate::smtp::context(&snapshot.message)?;
                crate::landing::render(
                    &snapshot.page.html,
                    crate::landing::Mode::Landing {
                        context: &context,
                        endpoint: None,
                    },
                )
                .map(|document| document.html)
                .map_err(|error| match error {
                    crate::landing::RenderError::Template(_) => {
                        crate::resources::invalid("Preview could not be rendered")
                    }
                    crate::landing::RenderError::Document(error) => error,
                })
            })
            .await
    }
}
