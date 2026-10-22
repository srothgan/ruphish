//! Integration orchestration over durable typed claims. Network work never owns a transaction.
use crate::{mailbox_transport, outbound::Policy, webhook_delivery};
use ruphish_application::{integrations::work::*, resources::Error};
fn now() -> i64 {
    ruphish_application::datetime::now().as_second()
}

pub(crate) fn webhook_payload(
    campaign: i64,
    email: String,
    time: String,
    message: String,
    details: &str,
) -> Result<Vec<u8>, Error> {
    serde_json::to_vec(
        &serde_json::json!({"campaign_id":campaign, "email":email, "time":time,
        "message":message, "details":crate::participant_privacy::details(details)}),
    )
    .map_err(|_| Error::Internal)
}
pub(crate) async fn webhooks(store: &dyn WebhookQueue, policy: &Policy) -> Result<(), Error> {
    for result in futures_util::future::join_all((0..4).map(|_| webhook(store, policy))).await {
        result?;
    }
    Ok(())
}
async fn webhook(store: &dyn WebhookQueue, policy: &Policy) -> Result<(), Error> {
    let Some(claim) = store.claim_webhook(now()).await? else {
        return Ok(());
    };
    let outcome = webhook_delivery::send_checked(
        policy,
        &claim.url,
        &claim.secret,
        &claim.body,
        &claim.event.to_string(),
        async {
            store
                .webhook_current(&claim, now())
                .await
                .map_err(|_| "webhook_storage_failed")
        },
    )
    .await;
    let decision = ruphish_domain::integrations::webhook_decision(
        outcome.code,
        outcome.status,
        claim.attempt,
        (claim.id as u32).wrapping_add(claim.attempt as u32),
    );
    let done = now();
    let due = done + decision.delay;
    store
        .finish_webhook(
            &claim,
            WebhookCompletion {
                state: decision.state,
                code: outcome.code,
                status: outcome.status,
                due,
            },
            done,
        )
        .await
}
pub(crate) async fn mailboxes(store: &dyn MailboxQueue, policy: &Policy) -> Result<(), Error> {
    for result in futures_util::future::join_all((0..2).map(|_| mailbox(store, policy))).await {
        result?;
    }
    Ok(())
}
async fn mailbox(store: &dyn MailboxQueue, policy: &Policy) -> Result<(), Error> {
    let Some(claim) = store.claim_mailbox(now()).await? else {
        return Ok(());
    };
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        poll_mailbox(store, policy, &claim),
    )
    .await
    .unwrap_or(Err("mailbox_timeout"));
    let code = result.err().unwrap_or("passed");
    let done = now();
    let delay = ruphish_domain::integrations::mailbox_delay(
        code == "passed",
        claim.frequency,
        claim.failures,
    );
    store.finish_mailbox(&claim, code, done + delay, done).await
}
async fn poll_mailbox(
    store: &dyn MailboxQueue,
    policy: &Policy,
    claim: &MailboxClaim,
) -> Result<(), &'static str> {
    let c = &claim.config;
    let (mut session, mailbox) = mailbox_transport::connect(policy, c).await?;
    store.mailbox_login(c).await?;
    let validity = i64::from(mailbox.uid_validity.ok_or("mailbox_uidvalidity_missing")?);
    let next = mailbox.uid_next.ok_or("mailbox_uidnext_missing")?;
    if !c.initialized {
        store.initialize_mailbox(c, validity, next).await?;
        let _ = session.logout().await;
        return Ok(());
    }
    if c.validity != validity {
        return Err("mailbox_uidvalidity_changed");
    }
    for uid in store.pending_acknowledgements(c, validity).await? {
        store.mailbox_current(c).await?;
        mailbox_transport::acknowledge(&mut session, uid, c.delete).await?;
        store.acknowledge_report(c, validity, uid).await?;
    }
    if next <= c.last.saturating_add(1) {
        let _ = session.logout().await;
        return Ok(());
    }
    let uids = mailbox_transport::search(&mut session, c.last.saturating_add(1), next - 1).await?;
    for uid in uids {
        let identified = match mailbox_transport::fetch(&mut session, uid).await {
            Ok(bytes) => mailbox_transport::identify(&bytes, &c.restrict),
            Err(code @ ("mailbox_message_too_large" | "mailbox_message_missing")) => Err(code),
            Err(code) => return Err(code),
        };
        if store
            .record_report(c, claim.run, validity, uid, identified)
            .await?
        {
            store.mailbox_current(c).await?;
            mailbox_transport::acknowledge(&mut session, uid, c.delete).await?;
            store.acknowledge_report(c, validity, uid).await?;
        }
    }
    let _ = session.logout().await;
    Ok(())
}
pub(crate) async fn validate_mailbox(policy: &Policy, config: &MailboxConfig) -> Result<(), Error> {
    tokio::time::timeout(
        std::time::Duration::from_secs(30),
        mailbox_transport::validate(policy, config),
    )
    .await
    .map_err(|_| Error::Invalid("mailbox_timeout".into()))?
    .map_err(|code| Error::Invalid(code.into()))
}
pub(crate) async fn validate_webhook(
    policy: &Policy,
    url: &str,
    secret: &str,
) -> Result<(), Error> {
    let result =
        webhook_delivery::send(policy, url, secret, b"{\"success\":true}", "validation").await;
    if result.code == "accepted" {
        Ok(())
    } else {
        Err(Error::Invalid(result.code.into()))
    }
}
