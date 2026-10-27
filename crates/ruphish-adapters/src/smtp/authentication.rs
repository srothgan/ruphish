//! Compatibility authentication on the existing Lettre connection; never log exchanges.
use base64::{Engine, prelude::BASE64_STANDARD};
use hmac::{Hmac, KeyInit, Mac};
use lettre::transport::smtp::{
    authentication::{Credentials, Mechanism},
    client::AsyncSmtpConnection,
    commands::Ehlo,
    extension::ClientId,
};
use md5::Md5;
use ruphish_application::resources::Profile;
use ruphish_domain::campaign::{DeliveryFailure, FailureKind, Stage};

fn invalid() -> DeliveryFailure {
    DeliveryFailure::new(FailureKind::Blocked, Stage::Authentication, None)
}

pub(super) async fn authenticate(
    connection: &mut AsyncSmtpConnection,
    hello: &ClientId,
    profile: &Profile,
) -> Result<(), DeliveryFailure> {
    let failure = |error| super::transport::failure(error, Stage::Authentication);
    // Lettre discards unknown AUTH mechanisms, including CRAM-MD5. Obtain the
    // raw capabilities on this same, already upgraded connection, before AUTH.
    let capabilities = connection
        .command(Ehlo::new(hello.clone()))
        .await
        .map_err(failure)?;
    if !capabilities.has_code(250) {
        return Err(invalid());
    }
    let mut cram = false;
    let mut plain = false;
    let mut login = false;
    for line in capabilities.message() {
        let mut words = line.split_ascii_whitespace();
        if !words
            .next()
            .is_some_and(|word| word.eq_ignore_ascii_case("AUTH"))
        {
            continue;
        }
        for word in words {
            cram |= word.eq_ignore_ascii_case("CRAM-MD5");
            plain |= word.eq_ignore_ascii_case("PLAIN");
            login |= word.eq_ignore_ascii_case("LOGIN");
        }
    }
    if cram {
        let challenge = connection
            .command("AUTH CRAM-MD5\r\n")
            .await
            .map_err(failure)?;
        if !challenge.has_code(334) {
            return Err(invalid());
        }
        let lines: Vec<_> = challenge.message().collect();
        if lines.len() != 1 {
            return Err(invalid());
        }
        let response = cram_response(&profile.username, &profile.password, lines[0])?;
        let accepted = connection
            .command(format!("{response}\r\n"))
            .await
            .map_err(failure)?;
        if !accepted.has_code(235) {
            return Err(invalid());
        }
    } else {
        let mechanism = if plain {
            Mechanism::Plain
        } else if login {
            Mechanism::Login
        } else {
            return Err(invalid());
        };
        let accepted = connection
            .auth(
                &[mechanism],
                &Credentials::new(profile.username.clone(), profile.password.clone()),
            )
            .await
            .map_err(failure)?;
        if !accepted.has_code(235) {
            return Err(invalid());
        }
    }
    Ok(())
}

fn cram_response(username: &str, password: &str, encoded: &str) -> Result<String, DeliveryFailure> {
    // A CRAM identity is followed by one space and the digest, not a quoted string.
    if username.is_empty()
        || username
            .bytes()
            .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
        || encoded.len() > 4096
    {
        return Err(invalid());
    }
    let challenge = BASE64_STANDARD.decode(encoded).map_err(|_| invalid())?;
    if challenge.is_empty() {
        return Err(invalid());
    }
    let mut mac = Hmac::<Md5>::new_from_slice(password.as_bytes()).map_err(|_| invalid())?;
    mac.update(&challenge);
    let digest = hex::encode(mac.finalize().into_bytes());
    Ok(BASE64_STANDARD.encode(format!("{username} {digest}")))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Independent RFC vector assertions")]
mod tests {
    use super::*;

    #[test]
    fn rfc_2195_challenge_and_invalid_inputs() {
        // RFC 2195 section 2: independent published challenge/response.
        assert_eq!(
            cram_response(
                "tim",
                "tanstaaftanstaaf",
                "PDE4OTYuNjk3MTcwOTUyQHBvc3RvZmZpY2UucmVzdG9uLm1jaS5uZXQ+"
            )
            .unwrap(),
            "dGltIGI5MTNhNjAyYzdlZGE3YTQ5NWI0ZTZlNzMzNGQzODkw"
        );
        for challenge in ["", "***", "YQ=", "YQ==\r\nMAIL FROM:bad"] {
            assert!(cram_response("tim", "secret", challenge).is_err());
        }
        assert!(cram_response("two words", "secret", "YQ==").is_err());
        assert!(cram_response("tim", "secret", &"A".repeat(4097)).is_err());
    }
}
