//! Real local SMTP/TLS peers; no provider credentials or external recipients.
#![allow(
    clippy::unwrap_used,
    reason = "Isolated protocol fixture setup and assertions"
)]
use super::*;
use lettre::{address::Envelope, transport::smtp::client::Certificate};
use std::sync::Arc;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader},
    net::TcpListener,
    task::JoinHandle,
    time::timeout,
};
use tokio_rustls::{TlsAcceptor, rustls};

#[path = "../../../../../tests/support/tls.rs"]
mod tls_fixture;

enum Mode {
    Plain,
    Close,
    Race(Arc<tokio::sync::Barrier>),
    Stall,
    StartTls(Arc<rustls::ServerConfig>),
    ImplicitTls(Arc<rustls::ServerConfig>),
}

struct Peer {
    address: SocketAddr,
    task: JoinHandle<Vec<String>>,
}
impl Peer {
    async fn start(bind: &str, mode: Mode, auth_reply: &'static str) -> Self {
        let listener = TcpListener::bind(bind).await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            timeout(Duration::from_secs(5), async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                match mode {
                    Mode::Close => Vec::new(),
                    Mode::Stall => {
                        let mut bytes = Vec::new();
                        socket.read_to_end(&mut bytes).await.unwrap();
                        assert!(
                            bytes.is_empty(),
                            "cancelled greeting must receive no commands"
                        );
                        Vec::new()
                    }
                    Mode::Plain => dialogue(socket, true, auth_reply, None).await,
                    Mode::Race(barrier) => dialogue(socket, true, auth_reply, Some(barrier)).await,
                    Mode::ImplicitTls(config) => {
                        match TlsAcceptor::from(config).accept(socket).await {
                            Ok(tls) => dialogue(tls, true, auth_reply, None).await,
                            Err(_) => Vec::new(),
                        }
                    }
                    Mode::StartTls(config) => {
                        let mut plain = BufReader::new(socket);
                        plain
                            .get_mut()
                            .write_all(b"220 fixture ESMTP\r\n")
                            .await
                            .unwrap();
                        let mut line = String::new();
                        plain.read_line(&mut line).await.unwrap();
                        assert!(line.starts_with("EHLO "));
                        plain
                            .get_mut()
                            .write_all(b"250-fixture\r\n250 STARTTLS\r\n")
                            .await
                            .unwrap();
                        line.clear();
                        plain.read_line(&mut line).await.unwrap();
                        assert_eq!(line, "STARTTLS\r\n");
                        plain
                            .get_mut()
                            .write_all(b"220 Begin TLS\r\n")
                            .await
                            .unwrap();
                        match TlsAcceptor::from(config).accept(plain.into_inner()).await {
                            Ok(tls) => dialogue(tls, false, auth_reply, None).await,
                            Err(_) => Vec::new(),
                        }
                    }
                }
            })
            .await
            .unwrap()
        });
        Self { address, task }
    }
    async fn finished(mut self) -> Vec<String> {
        (&mut self.task).await.unwrap()
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn dialogue<S: AsyncRead + AsyncWrite + Unpin>(
    stream: S,
    greeting: bool,
    auth_reply: &str,
    mut barrier: Option<Arc<tokio::sync::Barrier>>,
) -> Vec<String> {
    let mut io = BufReader::new(stream);
    let mut commands = Vec::new();
    if greeting {
        io.get_mut()
            .write_all(b"220 fixture ESMTP\r\n")
            .await
            .unwrap();
    }
    loop {
        let mut line = String::new();
        if io.read_line(&mut line).await.unwrap_or(0) == 0 {
            return commands;
        }
        commands.push(line.clone());
        let reply = if line.starts_with("EHLO ") {
            if let Some(barrier) = barrier.take() {
                barrier.wait().await;
            }
            "250-fixture\r\n250 AUTH PLAIN\r\n"
        } else if line.starts_with("AUTH ") {
            auth_reply
        } else if line == "DATA\r\n" {
            io.get_mut().write_all(b"354 Continue\r\n").await.unwrap();
            loop {
                let mut line = String::new();
                let read = io.read_line(&mut line).await.unwrap();
                assert_ne!(read, 0, "message body must complete");
                if line == ".\r\n" {
                    break;
                }
            }
            "250 Accepted\r\n"
        } else if line == "QUIT\r\n" {
            "221 Bye\r\n"
        } else {
            "250 OK\r\n"
        };
        if io.get_mut().write_all(reply.as_bytes()).await.is_err() {
            return commands;
        }
        if line == "QUIT\r\n" {
            return commands;
        }
    }
}

fn tls_material(expired: bool) -> (Arc<rustls::ServerConfig>, Certificate) {
    let generated = if expired {
        tls_fixture::server_certificate_expiry(&["smtp.fixture.test"], true)
    } else {
        tls_fixture::server_certificate(&["smtp.fixture.test"])
    };
    let certificate = Certificate::from_der(generated.cert.der().to_vec()).unwrap();
    let key = rustls::pki_types::PrivatePkcs8KeyDer::from(generated.signing_key.serialize_der());
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![generated.cert.der().clone()], key.into())
        .unwrap();
    (Arc::new(config), certificate)
}

fn public_tls() -> TlsParameters {
    TlsParameters::new("smtp.fixture.test".into()).unwrap()
}

#[tokio::test]
async fn either_family_and_unreachable_first_address_connect_without_authenticating() {
    for (bind, fallback) in [
        ("127.0.0.1:0", false),
        ("[::1]:0", false),
        ("127.0.0.1:0", true),
        ("[::1]:0", true),
    ] {
        let peer = Peer::start(bind, Mode::Plain, "235 Authenticated\r\n").await;
        let other_bind = if peer.address.is_ipv6() {
            "127.0.0.1:0"
        } else {
            "[::1]:0"
        };
        let unused = TcpListener::bind(other_bind).await.unwrap();
        let unreachable = unused.local_addr().unwrap();
        drop(unused);
        let addresses = if fallback {
            vec![unreachable, peer.address]
        } else {
            vec![peer.address]
        };
        let mut connection = timeout(
            Duration::from_secs(2),
            connect_checked(addresses, false, public_tls()),
        )
        .await
        .unwrap()
        .unwrap();
        connection.quit().await.unwrap();
        let commands = peer.finished().await;
        assert!(commands.iter().any(|c| c.starts_with("EHLO ")));
        assert!(
            !commands
                .iter()
                .any(|c| c.starts_with("AUTH ") || c.starts_with("MAIL "))
        );
    }
}

#[tokio::test]
async fn stalled_candidates_do_not_starve_later_addresses_and_only_one_message_is_submitted() {
    for reverse in [false, true] {
        let slow_v6 = Peer::start("[::1]:0", Mode::Stall, "").await;
        let slow_v4 = Peer::start("127.0.0.1:0", Mode::Stall, "").await;
        let good = Peer::start("127.0.0.1:0", Mode::Plain, "235 Authenticated\r\n").await;
        let mut addresses = vec![slow_v6.address, slow_v4.address];
        if reverse {
            addresses.reverse();
        }
        addresses.push(good.address);
        let mut connection = timeout(
            Duration::from_secs(3),
            connect_checked(addresses, false, public_tls()),
        )
        .await
        .unwrap()
        .unwrap();
        let profile = Profile {
            username: "fixture-user".into(),
            password: "fixture-secret".into(),
            ..Default::default()
        };
        super::super::authentication::authenticate(
            &mut connection,
            &ClientId::Domain("fixture.test".into()),
            &profile,
        )
        .await
        .unwrap();
        let envelope = Envelope::new(
            Some("sender@example.test".parse().unwrap()),
            vec!["recipient@example.test".parse().unwrap()],
        )
        .unwrap();
        connection
            .send(&envelope, b"Subject: fixture\r\n\r\nbody\r\n")
            .await
            .unwrap();
        connection.quit().await.unwrap();
        let commands = good.finished().await;
        assert_eq!(
            commands.iter().filter(|c| c.starts_with("AUTH ")).count(),
            1
        );
        assert_eq!(commands.iter().filter(|c| *c == "DATA\r\n").count(), 1);
        assert!(slow_v4.finished().await.is_empty());
        assert!(slow_v6.finished().await.is_empty());
    }
}

#[tokio::test]
async fn cancellation_closes_every_pending_socket_without_auth_or_submission() {
    let a = Peer::start("127.0.0.1:0", Mode::Stall, "").await;
    let b = Peer::start("[::1]:0", Mode::Stall, "").await;
    assert!(
        timeout(
            Duration::from_millis(750),
            connect_checked(vec![a.address, b.address], false, public_tls())
        )
        .await
        .is_err()
    );
    assert!(a.finished().await.is_empty());
    assert!(b.finished().await.is_empty());
}

#[tokio::test]
async fn both_tls_modes_validate_trust_hostname_and_expiry_before_authentication() {
    for implicit in [false, true] {
        for scenario in ["valid", "untrusted", "wrong-host", "expired"] {
            let (config, certificate) = tls_material(scenario == "expired");
            let mode = if implicit {
                Mode::ImplicitTls(config)
            } else {
                Mode::StartTls(config)
            };
            let peer = Peer::start("127.0.0.1:0", mode, "235 Authenticated\r\n").await;
            let hostname = if scenario == "wrong-host" {
                "wrong.fixture.test"
            } else {
                "smtp.fixture.test"
            };
            let mut builder = TlsParameters::builder(hostname.into());
            if scenario != "untrusted" {
                builder = builder.add_root_certificate(certificate);
            }
            let result = timeout(
                Duration::from_secs(3),
                connect_checked(vec![peer.address], implicit, builder.build().unwrap()),
            )
            .await
            .unwrap();
            if scenario == "valid" {
                let mut connection = result.unwrap();
                assert!(connection.is_encrypted());
                let profile = Profile {
                    username: "fixture-user".into(),
                    password: "fixture-secret".into(),
                    ..Default::default()
                };
                super::super::authentication::authenticate(
                    &mut connection,
                    &ClientId::Domain("fixture.test".into()),
                    &profile,
                )
                .await
                .unwrap();
                let envelope = Envelope::new(
                    Some("sender@example.test".parse().unwrap()),
                    vec!["recipient@example.test".parse().unwrap()],
                )
                .unwrap();
                connection
                    .send(&envelope, b"Subject: TLS fixture\r\n\r\nbody\r\n")
                    .await
                    .unwrap();
                connection.quit().await.unwrap();
            } else {
                let error = result.err().unwrap();
                assert_eq!(
                    error.kind,
                    FailureKind::Blocked,
                    "implicit={implicit}, scenario={scenario}: {error}"
                );
                assert_eq!(
                    error.stage,
                    Stage::Tls,
                    "implicit={implicit}, scenario={scenario}: {error}"
                );
            }
            let commands = peer.finished().await;
            let expected = usize::from(scenario == "valid");
            assert_eq!(
                commands.iter().filter(|c| c.starts_with("AUTH ")).count(),
                expected
            );
            assert_eq!(
                commands.iter().filter(|c| c.starts_with("MAIL ")).count(),
                expected
            );
            assert_eq!(
                commands.iter().filter(|c| *c == "DATA\r\n").count(),
                expected
            );
        }
    }
}

#[tokio::test]
async fn connection_loss_during_implicit_tls_is_retryable_without_a_certificate_failure() {
    for implicit in [false, true] {
        let peer = Peer::start("127.0.0.1:0", Mode::Close, "").await;
        let error = timeout(
            Duration::from_secs(3),
            connect_checked(vec![peer.address], implicit, public_tls()),
        )
        .await
        .unwrap()
        .err()
        .unwrap();
        assert_eq!(
            error.kind,
            FailureKind::Retryable,
            "implicit={implicit}: {error}"
        );
        assert_eq!(error.stage, Stage::Connect);
        assert!(error.smtp_code.is_none());
        assert!(peer.finished().await.is_empty());
    }
}

#[tokio::test]
async fn required_tls_cannot_downgrade_even_without_credentials() {
    let peer = Peer::start("127.0.0.1:0", Mode::Plain, "235 Authenticated\r\n").await;
    let error = endpoint(
        peer.address,
        false,
        public_tls(),
        true,
        Instant::now() + SETUP_TIMEOUT,
    )
    .await
    .err()
    .unwrap();
    assert_eq!(error.kind, FailureKind::Blocked);
    assert_eq!(error.stage, Stage::Tls);
    assert!(
        !peer
            .finished()
            .await
            .iter()
            .any(|c| c.starts_with("AUTH ") || c.starts_with("MAIL "))
    );
}

#[tokio::test]
async fn authentication_rejection_is_not_replayed_on_another_address() {
    let slow = Peer::start("[::1]:0", Mode::Stall, "").await;
    let rejected = Peer::start(
        "127.0.0.1:0",
        Mode::Plain,
        "535 fixture-secret rejected\r\n",
    )
    .await;
    let mut connection = connect_checked(vec![slow.address, rejected.address], false, public_tls())
        .await
        .unwrap();
    let profile = Profile {
        username: "fixture-user".into(),
        password: "fixture-secret".into(),
        ..Default::default()
    };
    let error = super::super::authentication::authenticate(
        &mut connection,
        &ClientId::Domain("fixture.test".into()),
        &profile,
    )
    .await
    .unwrap_err();
    assert_eq!(error.kind, FailureKind::Blocked);
    assert_eq!(error.smtp_code, Some(535));
    assert!(!error.to_string().contains("fixture-secret"));
    drop(connection);
    assert!(slow.finished().await.is_empty());
    let commands = rejected.finished().await;
    assert_eq!(
        commands.iter().filter(|c| c.starts_with("AUTH ")).count(),
        1
    );
    assert!(!commands.iter().any(|c| c.starts_with("MAIL ")));
}

#[tokio::test]
async fn simultaneous_ready_connections_authenticate_and_submit_on_only_one_socket() {
    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    let a = Peer::start(
        "127.0.0.1:0",
        Mode::Race(barrier.clone()),
        "235 Authenticated\r\n",
    )
    .await;
    let b = Peer::start("[::1]:0", Mode::Race(barrier), "235 Authenticated\r\n").await;
    let mut connection = timeout(
        Duration::from_secs(3),
        connect_checked(vec![a.address, b.address], false, public_tls()),
    )
    .await
    .unwrap()
    .unwrap();
    let profile = Profile {
        username: "fixture-user".into(),
        password: "fixture-secret".into(),
        ..Default::default()
    };
    super::super::authentication::authenticate(
        &mut connection,
        &ClientId::Domain("fixture.test".into()),
        &profile,
    )
    .await
    .unwrap();
    let envelope = Envelope::new(
        Some("sender@example.test".parse().unwrap()),
        vec!["recipient@example.test".parse().unwrap()],
    )
    .unwrap();
    connection
        .send(&envelope, b"Subject: fixture\r\n\r\nbody\r\n")
        .await
        .unwrap();
    connection.quit().await.unwrap();
    let commands: Vec<_> = a
        .finished()
        .await
        .into_iter()
        .chain(b.finished().await)
        .collect();
    assert_eq!(
        commands.iter().filter(|c| c.starts_with("AUTH ")).count(),
        1
    );
    assert_eq!(
        commands.iter().filter(|c| c.starts_with("MAIL ")).count(),
        1
    );
    assert_eq!(commands.iter().filter(|c| *c == "DATA\r\n").count(), 1);
}

#[tokio::test]
async fn all_unreachable_addresses_fail_retryably_and_setup_deadlines_close_stalled_sockets() {
    let a = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let b = TcpListener::bind("[::1]:0").await.unwrap();
    let addresses = vec![a.local_addr().unwrap(), b.local_addr().unwrap()];
    drop((a, b));
    let error = timeout(
        Duration::from_secs(3),
        connect_checked(addresses, false, public_tls()),
    )
    .await
    .unwrap()
    .err()
    .unwrap();
    assert_eq!(error.kind, FailureKind::Retryable);
    assert_eq!(error.stage, Stage::Connect);
    let stalled = Peer::start("127.0.0.1:0", Mode::Stall, "").await;
    let error = endpoint(
        stalled.address,
        false,
        public_tls(),
        true,
        Instant::now() + Duration::from_millis(100),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(error.kind, FailureKind::Retryable);
    assert_eq!(error.stage, Stage::Connect);
    assert!(stalled.finished().await.is_empty());
}
