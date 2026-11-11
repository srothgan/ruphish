//! Bounded address fallback ends before AUTH or any mail transaction.

#[cfg(test)]
#[path = "tests/connection.rs"]
pub(crate) mod tests;

use super::transport::{failure, timed_out};
use futures_util::{StreamExt, stream::FuturesUnordered};
use lettre::transport::smtp::{
    client::{AsyncSmtpConnection, TlsParameters},
    extension::ClientId,
};
use ruphish_application::resources::Profile;
use ruphish_domain::campaign::{DeliveryFailure, FailureKind, Stage};
use std::{collections::VecDeque, net::SocketAddr, time::Duration};
use tokio::time::{Instant, sleep_until, timeout_at};

const ATTEMPT_DELAY: Duration = Duration::from_millis(250);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const SETUP_TIMEOUT: Duration = Duration::from_secs(300);

/// Interleave families while preserving resolver preference within each family.
fn interleaved(addresses: Vec<SocketAddr>) -> VecDeque<SocketAddr> {
    let first_is_v6 = addresses.first().is_some_and(SocketAddr::is_ipv6);
    let (mut first, mut second): (VecDeque<_>, VecDeque<_>) = addresses
        .into_iter()
        .partition(|address| address.is_ipv6() == first_is_v6);
    let mut ordered = VecDeque::new();
    while !first.is_empty() || !second.is_empty() {
        if let Some(address) = first.pop_front() {
            ordered.push_back(address);
        }
        if let Some(address) = second.pop_front() {
            ordered.push_back(address);
        }
    }
    ordered
}

pub(super) async fn connect(
    addresses: Vec<SocketAddr>,
    host: String,
    port: u16,
    profile: &Profile,
) -> Result<AsyncSmtpConnection, DeliveryFailure> {
    let tls = TlsParameters::builder(host)
        .dangerous_accept_invalid_certs(profile.ignore_cert_errors)
        .build()
        .map_err(|_| DeliveryFailure::new(FailureKind::Blocked, Stage::Configuration, None))?;
    connect_checked(addresses, port == 465, tls).await
}

async fn connect_checked(
    addresses: Vec<SocketAddr>,
    implicit_tls: bool,
    tls: TlsParameters,
) -> Result<AsyncSmtpConnection, DeliveryFailure> {
    let mut addresses = interleaved(addresses);
    let mut pending = FuturesUnordered::new();
    let mut next = Instant::now();
    let deadline = next + SETUP_TIMEOUT;
    let mut last = timed_out(Stage::Connect);
    loop {
        // The destination policy caps this set at 16. Stagger all candidates:
        // a pair of stalled greetings must not starve a later working address.
        if pending.is_empty() || Instant::now() >= next {
            if let Some(address) = addresses.pop_front() {
                let require_tls = !crate::outbound::canonical_ip(address.ip()).is_loopback();
                pending.push(endpoint(
                    address,
                    implicit_tls,
                    tls.clone(),
                    require_tls,
                    deadline,
                ));
                next = Instant::now() + ATTEMPT_DELAY;
            } else if pending.is_empty() {
                return Err(last);
            }
        }
        tokio::select! {
            result = pending.next() => {
                match result {
                    Some(Ok(connection)) => return Ok(connection),
                    // A certificate or explicit protocol rejection is not a
                    // connectivity failure. Never hide it by trying credentials
                    // or submitting the message through another endpoint.
                    Some(Err(error)) if error.kind != FailureKind::Retryable => return Err(error),
                    Some(Err(error)) => {
                        // Prefer an explicit server response / TLS-stage failure
                        // to a generic unreachable address, independent of timing.
                        let priority = |e: &DeliveryFailure| (e.smtp_code.is_some(), e.stage == Stage::Tls, e.smtp_code);
                        if priority(&error) >= priority(&last) {
                            last = error;
                        }
                    }
                    None => return Err(last),
                }
            }
            () = sleep_until(next), if !addresses.is_empty() => {}
        }
    }
    // Returning or cancelling drops every losing connection future here.
}

async fn endpoint(
    address: SocketAddr,
    implicit_tls: bool,
    tls: TlsParameters,
    require_tls: bool,
    deadline: Instant,
) -> Result<AsyncSmtpConnection, DeliveryFailure> {
    let hello = ClientId::Domain("ruphish.local".into());
    let mut connection = timeout_at(
        deadline,
        AsyncSmtpConnection::connect_tokio1(
            address,
            Some(CONNECT_TIMEOUT),
            &hello,
            implicit_tls.then(|| tls.clone()),
            None,
        ),
    )
    .await
    // Lettre combines TCP, implicit TLS and the greeting in this operation;
    // a deadline alone cannot identify which of those stalled.
    .map_err(|_| timed_out(Stage::Connect))?
    .map_err(|error| failure(error, Stage::Connect))?;
    if !connection.is_encrypted() && connection.can_starttls() {
        timeout_at(deadline, connection.starttls(tls, &hello))
            .await
            .map_err(|_| timed_out(Stage::Tls))?
            .map_err(|error| failure(error, Stage::Tls))?;
    }
    // Explicitly allowed loopback relays remain available for local test mail.
    // Remote submissions require TLS even when no credentials were configured.
    if !connection.is_encrypted() && require_tls {
        return Err(DeliveryFailure::new(FailureKind::Blocked, Stage::Tls, None));
    }
    Ok(connection)
}
