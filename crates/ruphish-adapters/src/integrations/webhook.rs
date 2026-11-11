//! Signed, destination-checked HTTP delivery with closed diagnostics.

#[cfg(test)]
#[path = "../tests/webhook_delivery.rs"]
pub(crate) mod tests;

use crate::outbound::Policy;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
pub(crate) struct Outcome {
    pub status: Option<u16>,
    pub code: &'static str,
}
pub(crate) fn signature(secret: &str, bytes: &[u8]) -> Result<String, ()> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).map_err(|_| ())?;
    mac.update(bytes);
    Ok(format!(
        "sha256={}",
        hex::encode(mac.finalize().into_bytes())
    ))
}
pub(crate) async fn send(
    policy: &Policy,
    url: &str,
    secret: &str,
    body: &[u8],
    event: &str,
) -> Outcome {
    send_checked(policy, url, secret, body, event, async { Ok(true) }).await
}

pub(crate) async fn send_checked(
    policy: &Policy,
    url: &str,
    secret: &str,
    body: &[u8],
    event: &str,
    authorize: impl std::future::Future<Output = Result<bool, &'static str>>,
) -> Outcome {
    let attempt = async {
        let url = url::Url::parse(url).map_err(|_| "webhook_invalid_url")?;
        let host = url.host_str().ok_or("webhook_invalid_url")?;
        let port = url.port_or_known_default().ok_or("webhook_invalid_url")?;
        let addresses = policy
            .resolve(host, port)
            .await
            .map_err(|_| "webhook_destination_denied")?;
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(10))
            .resolve_to_addrs(host, &addresses)
            .build()
            .map_err(|_| "webhook_transport_unavailable")?;
        let signature = signature(secret, body).map_err(|_| "webhook_signing_failed")?;
        if !authorize.await? {
            return Err("configuration_changed");
        }
        let response = client
            .post(url)
            .header("content-type", "application/json")
            .header("X-Gophish-Signature", signature)
            .header("X-Ruphish-Event-ID", event)
            .body(body.to_vec())
            .send()
            .await
            .map_err(|_| "webhook_transport_unavailable")?;
        let status = response.status().as_u16();
        Ok::<_, &'static str>(Outcome {
            status: Some(status),
            code: if (200..300).contains(&status) {
                "accepted"
            } else {
                "webhook_http_rejected"
            },
        })
    };
    match tokio::time::timeout(std::time::Duration::from_secs(10), attempt).await {
        Ok(Ok(v)) => v,
        Ok(Err(code)) => Outcome { status: None, code },
        Err(_) => Outcome {
            status: None,
            code: "webhook_timeout",
        },
    }
}
