//! Actual SMTP conversations establish the durable submission boundary.
#![allow(
    clippy::unwrap_used,
    reason = "Isolated integration-test fixture setup must fail on errors"
)]
use ruphish_adapters::{outbound::Policy, smtp::Smtp};
use ruphish_application::{
    delivery::{PreparedMail, Transport},
    resources::Profile,
};
use ruphish_domain::campaign::{FailureKind, Stage};
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    sync::mpsc,
    time::Duration,
};

fn server(
    reply: &'static str,
    reject_before_data: bool,
) -> (
    Profile,
    mpsc::Receiver<Vec<u8>>,
    std::thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let profile = Profile {
        host: listener.local_addr().unwrap().to_string(),
        from_address: "training@example.test".into(),
        ..Profile::default()
    };
    let (sent, received) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut io = BufReader::new(stream);
        io.get_mut().write_all(b"220 local ESMTP\r\n").unwrap();
        loop {
            let mut line = String::new();
            if io.read_line(&mut line).unwrap_or(0) == 0 {
                break;
            }
            let response = if line.starts_with("EHLO") {
                "250-local\r\n250 8BITMIME\r\n"
            } else if line.starts_with("MAIL") {
                "250 Sender accepted\r\n"
            } else if line.starts_with("RCPT") {
                if reject_before_data {
                    reply
                } else {
                    "250 Recipient accepted\r\n"
                }
            } else if line.starts_with("DATA") {
                io.get_mut().write_all(b"354 Continue\r\n").unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut part = Vec::new();
                    if io.read_until(b'\n', &mut part).unwrap_or(0) == 0 {
                        return;
                    }
                    if part == b".\r\n" {
                        break;
                    }
                    bytes.extend(part);
                }
                sent.send(bytes).unwrap();
                if reply.is_empty() {
                    return;
                }
                reply
            } else if line.starts_with("QUIT") {
                return;
            } else {
                "250 Reset\r\n"
            };
            if io.get_mut().write_all(response.as_bytes()).is_err() {
                break;
            }
        }
    });
    (profile, received, thread)
}
fn mail() -> PreparedMail {
    PreparedMail {from:"training@example.test".into(),to:"alice@example.test".into(),bytes:b"From: training@example.test\r\nTo: alice@example.test\r\nSubject: Training\r\n\r\nHello\r\n.dot\r\n".to_vec()}
}
#[tokio::test]
async fn explicit_rejections_and_lost_acceptance_have_different_retry_guarantees() {
    for (reply, expected) in [
        ("250 Accepted\r\n", None),
        ("451 Try later\r\n", Some(FailureKind::Retryable)),
        ("550 Rejected\r\n", Some(FailureKind::Permanent)),
        ("", Some(FailureKind::Unknown)),
        (
            "354 Unexpected continuation\r\n",
            Some(FailureKind::Unknown),
        ),
    ] {
        let (profile, received, thread) = server(reply, false);
        let smtp = Smtp::new(Policy::default(), String::new());
        let pending = smtp.prepare(&profile, mail()).await.unwrap();
        assert!(
            received.try_recv().is_err(),
            "preparing must never submit content"
        );
        let outcome = pending.submit().await;
        assert_eq!(outcome.as_ref().err().map(|e| e.kind), expected);
        if let Err(error) = outcome {
            assert_eq!(error.stage, Stage::Acceptance);
        }
        let bytes = received.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(
            String::from_utf8(bytes).unwrap().contains("\r\n..dot\r\n"),
            "Lettre must retain SMTP dot stuffing"
        );
        thread.join().unwrap();
    }
}
#[tokio::test]
async fn recipient_rejection_is_known_before_the_durable_submission_boundary() {
    let (profile, received, thread) = server(
        "450 synthetic-secret should never enter diagnostics\r\n",
        true,
    );
    let smtp = Smtp::new(Policy::default(), String::new());
    let error = match smtp.prepare(&profile, mail()).await {
        Ok(_) => panic!("expected rejection"),
        Err(error) => error,
    };
    assert_eq!(error.kind, FailureKind::Retryable);
    assert_eq!(error.stage, Stage::Envelope);
    assert_eq!(error.smtp_code, Some(450));
    assert!(!error.to_string().contains("synthetic-secret"));
    thread.join().unwrap();
    assert!(received.try_recv().is_err());
}
#[tokio::test]
async fn cancellation_after_preparation_closes_without_sending_content() {
    let (profile, received, thread) = server("250 Accepted\r\n", false);
    let smtp = Smtp::new(Policy::default(), String::new());
    let pending = smtp.prepare(&profile, mail()).await.unwrap();
    drop(pending);
    thread.join().unwrap();
    assert!(received.try_recv().is_err());
}

#[tokio::test]
async fn every_pre_content_smtp_phase_rejects_without_transmitting_a_body() {
    for phase in ["greeting", "EHLO", "MAIL", "RCPT", "DATA"] {
        for code in [451, 550] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let profile = Profile {
                host: listener.local_addr().unwrap().to_string(),
                from_address: "training@example.test".into(),
                ..Default::default()
            };
            let server = std::thread::spawn(move || {
                let (stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut io = BufReader::new(stream);
                let rejected = format!("{code} synthetic-phase-secret\r\n");
                io.get_mut()
                    .write_all(if phase == "greeting" {
                        rejected.as_bytes()
                    } else {
                        b"220 local ESMTP\r\n"
                    })
                    .unwrap();
                let mut commands = Vec::new();
                loop {
                    let mut line = String::new();
                    if io.read_line(&mut line).unwrap_or(0) == 0 {
                        break;
                    }
                    commands.push(line.clone());
                    let reply = if line.starts_with(phase) {
                        rejected.as_str()
                    } else if line.starts_with("EHLO") {
                        "250 local\r\n"
                    } else {
                        "250 OK\r\n"
                    };
                    if io.get_mut().write_all(reply.as_bytes()).is_err() {
                        break;
                    }
                }
                commands
            });
            let smtp = Smtp::new(Policy::default(), String::new());
            let error = match smtp.prepare(&profile, mail()).await {
                Ok(_) => panic!("expected rejection"),
                Err(e) => e,
            };
            assert_eq!(
                error.kind,
                if code == 451 {
                    FailureKind::Retryable
                } else {
                    FailureKind::Permanent
                },
                "{phase}"
            );
            assert_eq!(error.smtp_code, Some(code));
            assert!(!error.to_string().contains("synthetic-phase-secret"));
            assert!(
                server
                    .join()
                    .unwrap()
                    .iter()
                    .all(|line| !line.starts_with("Subject:") && !line.contains("Hello"))
            );
        }
    }
}

#[tokio::test]
async fn unreachable_transport_is_retryable_without_retaining_host_diagnostics() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let host = listener.local_addr().unwrap().to_string();
    drop(listener);
    let smtp = Smtp::new(Policy::default(), String::new());
    let profile = Profile {
        host,
        from_address: "training@example.test".into(),
        ..Default::default()
    };
    let error = match smtp.prepare(&profile, mail()).await {
        Ok(_) => panic!("unexpected connection"),
        Err(e) => e,
    };
    assert_eq!(error.kind, FailureKind::Retryable);
    assert_eq!(error.stage, Stage::Connect);
    assert!(error.smtp_code.is_none());
}
