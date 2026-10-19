//! Bounded IMAP transport and transient MIME correlation. Never log protocol data.
use crate::{
    imap_limit::{Limited, MAX_MESSAGE},
    outbound::Policy,
    sqlite::mailbox_worker::Config,
};
use async_imap::imap_proto::{AttributeValue, MailboxDatum, Response, Status};
use mail_parser::MimeHeaders;
use std::{collections::BTreeSet, sync::Arc};
type Session = async_imap::Session<Limited<tokio_rustls::client::TlsStream<tokio::net::TcpStream>>>;
pub(crate) async fn connect(
    policy: &Policy,
    c: &Config,
) -> Result<(Session, async_imap::types::Mailbox), &'static str> {
    let addresses = policy
        .resolve(&c.host, c.port)
        .await
        .map_err(|_| "mailbox_destination_denied")?;
    let mut socket = None;
    for address in addresses {
        if let Ok(Ok(s)) = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio::net::TcpStream::connect(address),
        )
        .await
        {
            socket = Some(s);
            break;
        }
    }
    let socket = socket.ok_or("mailbox_unreachable")?;
    let socket = if c.tls {
        socket
    } else {
        let mut client = async_imap::Client::new(Limited::new(socket));
        client
            .read_response()
            .await
            .map_err(|_| "mailbox_protocol_error")?
            .ok_or("mailbox_protocol_error")?;
        client
            .run_command_and_check_ok("STARTTLS", None)
            .await
            .map_err(|_| "mailbox_tls_required")?;
        client.into_inner().into_inner()
    };
    let mut roots = tokio_rustls::rustls::RootCertStore::from_iter(
        webpki_roots::TLS_SERVER_ROOTS.iter().cloned(),
    );
    for certificate in rustls_native_certs::load_native_certs().certs {
        roots
            .add(certificate)
            .map_err(|_| "mailbox_invalid_trust_store")?;
    }
    let config = tokio_rustls::rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let name = tokio_rustls::rustls::pki_types::ServerName::try_from(c.host.clone())
        .map_err(|_| "mailbox_invalid_host")?;
    let tls = tokio_rustls::TlsConnector::from(Arc::new(config))
        .connect(name, socket)
        .await
        .map_err(|_| "mailbox_tls_failed")?;
    let mut client = async_imap::Client::new(Limited::new(tls));
    if c.tls {
        client
            .read_response()
            .await
            .map_err(|_| "mailbox_protocol_error")?
            .ok_or("mailbox_protocol_error")?;
    }
    let mut session = client
        .login(&c.username, &c.password)
        .await
        .map_err(|_| "mailbox_authentication_failed")?;
    let mailbox = session
        .select(&c.folder)
        .await
        .map_err(|_| "mailbox_folder_unavailable")?;
    Ok((session, mailbox))
}
pub(crate) async fn validate(policy: &Policy, c: &Config) -> Result<(), &'static str> {
    let (mut s, _) = connect(policy, c).await?;
    s.logout().await.map_err(|_| "mailbox_protocol_error")?;
    Ok(())
}

/// Scan bounded decoded text and nested email parts; never fetch referenced URLs.
pub(crate) fn identify(bytes: &[u8], restrict: &str) -> Result<(String, String), &'static str> {
    if bytes.len() > MAX_MESSAGE {
        return Err("mailbox_message_too_large");
    }
    let message = mail_parser::MessageParser::default()
        .parse(bytes)
        .ok_or("mailbox_invalid_message")?;
    if !restrict.is_empty() {
        let allowed = message.from().is_some_and(|a| {
            a.iter().any(|a| {
                a.address
                    .as_ref()
                    .and_then(|s| s.rsplit_once('@'))
                    .is_some_and(|(_, d)| d.eq_ignore_ascii_case(restrict))
            })
        });
        if !allowed {
            return Err("mailbox_sender_filtered");
        }
    }
    let mut candidates = BTreeSet::new();
    let mut parts = 0;
    scan(&message, 0, &mut parts, &mut candidates)?;
    if candidates.len() != 1 {
        return Err(if candidates.is_empty() {
            "mailbox_unmatched"
        } else {
            "mailbox_ambiguous"
        });
    }
    candidates.into_iter().next().ok_or("mailbox_unmatched")
}
fn scan(
    m: &mail_parser::Message<'_>,
    depth: usize,
    count: &mut usize,
    found: &mut BTreeSet<(String, String)>,
) -> Result<(), &'static str> {
    if depth > 8 {
        return Err("mailbox_nesting_limit");
    }
    for part in &m.parts {
        *count += 1;
        if *count > 256 {
            return Err("mailbox_part_limit");
        }
        match &part.body {
            mail_parser::PartType::Text(t) | mail_parser::PartType::Html(t) => {
                let mut decoded = t.replace("&amp;", "&");
                for _ in 0..3 {
                    let next = percent_encoding::percent_decode_str(&decoded)
                        .decode_utf8_lossy()
                        .into_owned();
                    if next == decoded {
                        break;
                    }
                    decoded = next;
                }
                // Wrapped security URLs may contain an encoded original URL as a query value.
                for token in decoded
                    .split(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\''))
                {
                    for (start, _) in token.match_indices("http") {
                        let candidate = token[start..].trim_end_matches(['.', ',', ')', ']', ';']);
                        if let Ok(url) = url::Url::parse(candidate) {
                            if !matches!(url.scheme(), "https" | "http") {
                                continue;
                            }
                            for (key, value) in url.query_pairs() {
                                if key == "rid"
                                    && !value.is_empty()
                                    && value.len() <= 128
                                    && value
                                        .bytes()
                                        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                                {
                                    found.insert((
                                        value.into_owned(),
                                        url.origin().ascii_serialization(),
                                    ));
                                    if found.len() > 16 {
                                        return Err("mailbox_identifier_limit");
                                    }
                                }
                            }
                        }
                    }
                }
            }
            mail_parser::PartType::Message(child) => scan(child, depth + 1, count, found)?,
            mail_parser::PartType::Binary(bytes) | mail_parser::PartType::InlineBinary(bytes)
                if part
                    .attachment_name()
                    .is_some_and(|name| name.to_ascii_lowercase().ends_with(".eml")) =>
            {
                let child = mail_parser::MessageParser::default()
                    .parse(bytes.as_ref())
                    .ok_or("mailbox_invalid_message")?;
                scan(&child, depth + 1, count, found)?;
            }
            _ => {}
        }
    }
    Ok(())
}
/// Use the parser's typed responses, but explicitly verify tagged completion.
/// async-imap 0.12's streaming FETCH/SEARCH helpers discard the final NO/BAD status.
async fn checked_command(
    session: &mut Session,
    command: String,
    mut observe: impl FnMut(&Response<'_>) -> Result<(), &'static str>,
) -> Result<(), &'static str> {
    let expected = session
        .run_command(command)
        .await
        .map_err(|_| "mailbox_command_failed")?;
    loop {
        let response = session
            .read_response()
            .await
            .map_err(|_| "mailbox_protocol_error")?
            .ok_or("mailbox_disconnected")?;
        match response.parsed() {
            Response::Done { tag, status, .. } if tag == &expected => {
                return if *status == Status::Ok {
                    Ok(())
                } else {
                    Err("mailbox_command_rejected")
                };
            }
            Response::Done { .. } => return Err("mailbox_protocol_error"),
            value => observe(value)?,
        }
    }
}
pub(crate) async fn search(
    session: &mut Session,
    first: u32,
    last: u32,
) -> Result<Vec<u32>, &'static str> {
    let mut result = std::collections::BTreeSet::new();
    checked_command(
        session,
        format!("UID SEARCH UID {first}:{last}"),
        |response| {
            if let Response::MailboxData(MailboxDatum::Search(ids)) = response {
                for id in ids {
                    if (first..=last).contains(id) {
                        result.insert(*id);
                    }
                    if result.len() > 25 {
                        result.pop_last();
                    }
                }
            }
            Ok(())
        },
    )
    .await?;
    Ok(result.into_iter().collect())
}
pub(crate) async fn fetch(session: &mut Session, uid: u32) -> Result<Vec<u8>, &'static str> {
    let mut size = None;
    checked_command(
        session,
        format!("UID FETCH {uid} (UID RFC822.SIZE)"),
        |response| {
            if let Response::Fetch(_, attributes) = response
                && attributes
                    .iter()
                    .any(|a| matches!(a, AttributeValue::Uid(id) if *id == uid))
            {
                size = attributes.iter().find_map(|a| {
                    if let AttributeValue::Rfc822Size(size) = a {
                        Some(*size)
                    } else {
                        None
                    }
                });
            }
            Ok(())
        },
    )
    .await?;
    if size.ok_or("mailbox_message_missing")? as usize > MAX_MESSAGE {
        return Err("mailbox_message_too_large");
    }
    let mut body = None;
    checked_command(
        session,
        format!("UID FETCH {uid} (UID BODY.PEEK[])"),
        |response| {
            if let Response::Fetch(_, attributes) = response
                && attributes
                    .iter()
                    .any(|a| matches!(a, AttributeValue::Uid(id) if *id == uid))
            {
                for attribute in attributes {
                    if let AttributeValue::BodySection {
                        section: None,
                        data: Some(bytes),
                        ..
                    } = attribute
                    {
                        if body.is_some() {
                            return Err("mailbox_protocol_error");
                        }
                        body = Some(bytes.to_vec());
                    }
                }
            }
            Ok(())
        },
    )
    .await?;
    body.ok_or("mailbox_message_missing")
}
pub(crate) async fn acknowledge(
    s: &mut Session,
    uid: u32,
    delete: bool,
) -> Result<(), &'static str> {
    if delete {
        let caps = s
            .capabilities()
            .await
            .map_err(|_| "mailbox_acknowledgement_failed")?;
        if !caps.has_str("UIDPLUS") {
            return Err("mailbox_safe_delete_unavailable");
        }
    }
    let flags = if delete {
        "+FLAGS.SILENT (\\Seen \\Deleted)"
    } else {
        "+FLAGS.SILENT (\\Seen)"
    };
    session_command(s, format!("UID STORE {uid} {flags}")).await?;
    if delete {
        session_command(s, format!("UID EXPUNGE {uid}")).await?;
    }
    Ok(())
}

async fn session_command(session: &mut Session, command: String) -> Result<(), &'static str> {
    session
        .run_command_and_check_ok(command)
        .await
        .map_err(|_| "mailbox_acknowledgement_failed")
}

#[cfg(test)]
mod tests {
    use super::identify;
    #[test]
    fn mime_correlation_decodes_forwarded_messages_and_rejects_ambiguity() {
        let plain = b"From: User <employee@example.test>\r\nContent-Type: text/plain\r\n\r\nReported https://training.test/?rid=abc123\r\nhttps://training.test/track?rid=abc123";
        assert_eq!(
            identify(plain, "EXAMPLE.TEST"),
            Ok(("abc123".into(), "https://training.test".into()))
        );
        assert_eq!(
            identify(plain, "other.test"),
            Err("mailbox_sender_filtered")
        );
        let attachment = b"From: employee@example.test\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=report.EML\r\n\r\nFrom: training@example.test\r\n\r\nhttps://training.test/?rid=abc123";
        assert_eq!(
            identify(attachment, ""),
            Ok(("abc123".into(), "https://training.test".into()))
        );
        let nested = b"From: employee@example.test\r\nContent-Type: message/rfc822\r\n\r\nFrom: training@example.test\r\nContent-Type: text/html\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\n<a href=3D=22https://training.test/?rid=3Dabc123=22>Report</a>";
        assert_eq!(
            identify(nested, ""),
            Ok(("abc123".into(), "https://training.test".into()))
        );
        assert_eq!(identify(b"Subject: ambiguous\r\n\r\nhttps://training.test/?rid=abc https://training.test/?rid=def", ""), Err("mailbox_ambiguous"));
        assert_eq!(
            identify(b"Subject: no correlation\r\n\r\npassword=PRIVATE-VALUE", ""),
            Err("mailbox_unmatched")
        );
    }
}
