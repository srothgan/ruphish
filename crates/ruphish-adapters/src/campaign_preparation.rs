//! Campaign content validation shared by persistence providers.
use ruphish_application::{
    campaign::InteractionPolicy,
    content_work::ContentWork,
    resources::{Content, Error, Resource},
};

/// Validate owned content without moving provider transactions onto blocking workers.
pub(crate) async fn validate_content(
    work: &ContentWork,
    page: Resource,
    policy: InteractionPolicy,
    url: String,
) -> Result<(Resource, InteractionPolicy), Error> {
    work.run(move || {
        let sample = crate::resources::sample();
        let redirect = crate::resources::render(&policy.redirect_url, &sample)?;
        crate::landing::redirect(&redirect, &url)?;
        if let Content::Page(page) = &page.content {
            crate::landing::render(
                &page.html,
                crate::landing::Mode::Landing {
                    context: &sample,
                    endpoint: None,
                },
            )?;
        }
        Ok((page, policy))
    })
    .await
}

pub(crate) fn url(source: &str) -> Result<String, Error> {
    let sample = crate::resources::sample();
    let url = crate::resources::render(source, &sample).map_err(|e| e.at("url"))?;
    // Check the original authority as well: recipient expressions cannot select hosts.
    let configured = ruphish_domain::domains::origin(source, true)
        .map_err(|code| Error::Invalid(code.into()).at("url"))?;
    let rendered = ruphish_domain::domains::origin(&url, true)
        .map_err(|code| Error::Invalid(code.into()).at("url"))?;
    if configured != rendered {
        return Err(Error::Invalid("domain_dynamic_hostname".into()).at("url"));
    }
    Ok(url)
}

/// Supply bounded randomness once; domain owns the actual transition policy.
pub(crate) fn delivery_update(
    current: &str,
    completed: bool,
    retries: u32,
    now: i64,
    outcome: &Result<(), ruphish_domain::campaign::DeliveryFailure>,
) -> Result<Option<ruphish_domain::campaign::DeliveryUpdate>, Error> {
    let mut jitter = [0];
    if outcome
        .as_ref()
        .err()
        .is_some_and(|e| e.kind == ruphish_domain::campaign::FailureKind::Retryable)
        && !completed
    {
        getrandom::fill(&mut jitter).map_err(|_| Error::Internal)?;
    }
    Ok(ruphish_domain::campaign::delivery_update(
        ruphish_domain::campaign::DeliveryState::from_stored(current).ok_or(Error::Internal)?,
        completed,
        retries,
        now,
        outcome,
        jitter[0],
    ))
}
