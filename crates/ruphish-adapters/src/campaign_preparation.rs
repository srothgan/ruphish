//! Campaign URL template validation shared by persistence providers.
use ruphish_application::resources::Error;
pub(crate) fn url(source: &str) -> Result<String, Error> {
    let sample = crate::resources::sample();
    let url = crate::resources::render(source, &sample)?;
    // Check the original authority as well: recipient expressions cannot select hosts.
    let configured = ruphish_domain::domains::origin(source, true)
        .map_err(|code| Error::Invalid(code.into()))?;
    let rendered =
        ruphish_domain::domains::origin(&url, true).map_err(|code| Error::Invalid(code.into()))?;
    if configured != rendered {
        return Err(Error::Invalid("domain_dynamic_hostname".into()));
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
        current, completed, retries, now, outcome, jitter[0],
    ))
}
