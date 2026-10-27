//! Resource acceptance against the executable, its real authentication and SQLite.
use super::*;
use reqwest::blocking::Client;

fn ready(process: &mut Process) -> String {
    format!("http://{}", process.ready()["admin"].as_str().unwrap())
}
fn client() -> Client {
    Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap()
}

#[test]
fn demo_csv_acceptance_cases_match_the_real_import_review() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = ready(&mut process);
    unlock(directory.path());
    let client = client();
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("../demo/recipient-csv-cases.json")).unwrap();
    for case in cases {
        let mut body = case["request"].clone();
        if case["template"] == true {
            body["content"] = json!(include_str!(
                "../crates/ruphish-ui/assets/recipient-template.csv"
            ));
        }
        let response = client
            .post(format!("{base}/api/import/group/review"))
            .bearer_auth("synthetic-bootstrap-token")
            .json(&body)
            .send()
            .unwrap();
        assert_eq!(
            u64::from(response.status().as_u16()),
            case["status"].as_u64().unwrap(),
            "{}",
            case["name"]
        );
        if response.status().is_success() {
            let value: Value = response.json().unwrap();
            assert_eq!(value["targets"], case["targets"], "{}", case["name"]);
            let errors: Vec<_> = value["errors"]
                .as_array()
                .unwrap()
                .iter()
                .map(|error| json!({"row":error["row"],"code":error["code"]}))
                .collect();
            assert_eq!(json!(errors), case["errors"], "{}", case["name"]);
        }
    }
    process.stop();
}

#[test]
fn recipient_csv_template_imports_reviews_and_persists_without_accepting_bad_rows() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = ready(&mut process);
    unlock(directory.path());
    let client = client();
    let sample = include_str!("../crates/ruphish-ui/assets/recipient-template.csv");
    let review = |content: &str, columns: Value| {
        client
            .post(format!("{base}/api/import/group/review"))
            .bearer_auth("synthetic-bootstrap-token")
            .json(&json!({"content":content,"columns":columns}))
            .send()
            .unwrap()
    };
    let response = review(sample, Value::Null);
    assert_eq!(response.status(), 200);
    let reviewed: Value = response.json().unwrap();
    assert_eq!(reviewed["errors"], json!([]));
    let targets = &reviewed["targets"];
    assert_eq!(targets.as_array().unwrap().len(), 2);
    let upload = |bytes: &[u8]| {
        let mut body = b"--recipient-template-test\r\nContent-Disposition: form-data; name=\"file\"; filename=\"recipient-template.csv\"\r\nContent-Type: text/csv\r\n\r\n".to_vec();
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n--recipient-template-test--\r\n");
        client
            .post(format!("{base}/api/import/group"))
            .bearer_auth("synthetic-bootstrap-token")
            .header(
                "Content-Type",
                "multipart/form-data; boundary=recipient-template-test",
            )
            .body(body)
            .send()
            .unwrap()
    };
    let uploaded = upload(sample.as_bytes());
    assert_eq!(uploaded.status(), 200);
    assert_eq!(uploaded.json::<Value>().unwrap(), *targets);
    let mut invalid_encoding = sample.as_bytes().to_vec();
    invalid_encoding.extend_from_slice(b"\xff,Invalid,bad@example.test,Role\n");
    assert_eq!(upload(&invalid_encoding).status(), 400);
    for (index, first, last, email, position) in [
        (0, "Alex", "Rivera", "alex@example.test", "Operations"),
        (1, "Sam", "Lee", "sam@example.test", "Support"),
    ] {
        assert_eq!(targets[index]["first_name"], first);
        assert_eq!(targets[index]["last_name"], last);
        assert_eq!(targets[index]["email"], email);
        assert_eq!(targets[index]["position"], position);
    }
    // A BOM, CRLF, Unicode and explicit reordered column mapping survive real parsing.
    let mapped = review(
        "\u{feff}Address,Role,Surname,Given\r\nelise@example.test,\"=1+1\",Müller,\"Élise, E\"\r\n",
        json!(["Address", "Given", "Surname", "Role"]),
    );
    assert_eq!(mapped.status(), 200);
    let mapped: Value = mapped.json().unwrap();
    assert_eq!(mapped["errors"], json!([]));
    assert_eq!(mapped["targets"][0]["first_name"], "Élise, E");
    assert_eq!(mapped["targets"][0]["last_name"], "Müller");
    assert_eq!(mapped["targets"][0]["position"], "=1+1");
    let hostile = format!("{sample}Bad,Address,not-an-email,Role\nShort,row\n");
    assert_eq!(upload(hostile.as_bytes()).status(), 400);
    let rejected: Value = review(&hostile, Value::Null).json().unwrap();
    assert_eq!(rejected["targets"], *targets);
    assert_eq!(rejected["errors"].as_array().unwrap().len(), 2);
    assert_eq!(
        review("First Name,Position\nAlex,Role\n", Value::Null).status(),
        400
    );
    assert_eq!(
        client
            .post(format!("{base}/api/import/group/review"))
            .json(&json!({"content":sample}))
            .send()
            .unwrap()
            .status(),
        401
    );
    let groups: Value = client
        .get(format!("{base}/api/groups/"))
        .bearer_auth("synthetic-bootstrap-token")
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(groups, json!([]), "review must never persist a group");
    let saved = client
        .post(format!("{base}/api/groups/"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(&json!({"name":"CSV template audience","targets":targets}))
        .send()
        .unwrap();
    assert_eq!(saved.status(), 201);
    let saved: Value = saved.json().unwrap();
    let id = saved["id"].as_i64().unwrap();
    process.stop();
    let mut process = Process::start(directory.path(), "admin");
    let base = ready(&mut process);
    let persisted: Value = client
        .get(format!("{base}/api/groups/{id}"))
        .bearer_auth("synthetic-bootstrap-token")
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(persisted["targets"], saved["targets"]);
    assert_eq!(persisted["name"], "CSV template audience");
    process.stop();
}

#[test]
fn smtp_cram_negotiates_and_delivers_without_fallback_or_secret_diagnostics() {
    use std::io::{BufRead, BufReader};
    let directory = workspace();
    let mut config = configuration();
    config["logging"]["level"] = json!("trace");
    write_config(directory.path(), &config);
    let mut process = Process::start(directory.path(), "admin");
    let base = ready(&mut process);
    unlock(directory.path());
    // RFC 2195's independent vector; peers never call the production HMAC helper.
    const RESPONSE: &str = "dGltIGI5MTNhNjAyYzdlZGE3YTQ5NWI0ZTZlNzMzNGQzODkw";
    for (capabilities, challenge, result, send, success) in [
        (
            "CRAM-MD5",
            "PDE4OTYuNjk3MTcwOTUyQHBvc3RvZmZpY2UucmVzdG9uLm1jaS5uZXQ+",
            "235 Accepted",
            false,
            true,
        ),
        (
            "LOGIN PLAIN CRAM-MD5",
            "PDE4OTYuNjk3MTcwOTUyQHBvc3RvZmZpY2UucmVzdG9uLm1jaS5uZXQ+",
            "235 Accepted",
            true,
            true,
        ),
        ("PLAIN LOGIN", "", "235 Accepted", false, true),
        ("LOGIN", "", "235 Accepted", false, true),
        ("CRAM-MD5 PLAIN", "***", "235 Accepted", true, false),
        ("CRAM-MD5 PLAIN", "", "235 Accepted", false, false),
        (
            "CRAM-MD5 LOGIN",
            "PDE4OTYuNjk3MTcwOTUyQHBvc3RvZmZpY2UucmVzdG9uLm1jaS5uZXQ+",
            "535 rejected-tanstaaftanstaaf",
            true,
            false,
        ),
        (
            "CRAM-MD5",
            "PDE4OTYuNjk3MTcwOTUyQHBvc3RvZmZpY2UucmVzdG9uLm1jaS5uZXQ+",
            "334 unexpected challenge",
            false,
            false,
        ),
    ] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let peer = std::thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut peer = BufReader::new(socket);
            peer.get_mut().write_all(b"220 fixture ESMTP\r\n").unwrap();
            let mut authenticating = 0;
            let mut authenticated = false;
            let mut data = Vec::new();
            loop {
                let mut line = String::new();
                if peer.read_line(&mut line).unwrap_or(0) == 0 {
                    break;
                }
                let reply = if authenticating == 1 {
                    assert_eq!(line.trim_end(), RESPONSE);
                    authenticating = 0;
                    authenticated = result.starts_with("235");
                    result.to_owned()
                } else if authenticating == 2 {
                    assert_eq!(line.trim_end(), "dGlt");
                    authenticating = 3;
                    "334 UGFzc3dvcmQ6".into()
                } else if authenticating == 3 {
                    assert_eq!(line.trim_end(), "dGFuc3RhYWZ0YW5zdGFhZg==");
                    authenticating = 0;
                    authenticated = true;
                    "235 Accepted".into()
                } else if line.starts_with("EHLO ") {
                    format!("250-fixture\r\n250 AUTH {capabilities}")
                } else if line == "AUTH CRAM-MD5\r\n" {
                    authenticating = 1;
                    format!("334 {challenge}")
                } else if capabilities == "PLAIN LOGIN" && line.starts_with("AUTH PLAIN ") {
                    assert_eq!(line.trim_end(), "AUTH PLAIN AHRpbQB0YW5zdGFhZnRhbnN0YWFm");
                    authenticated = true;
                    "235 Accepted".into()
                } else if capabilities == "LOGIN" && line == "AUTH LOGIN\r\n" {
                    authenticating = 2;
                    "334 VXNlcm5hbWU6".into()
                } else if line == "QUIT\r\n" {
                    peer.get_mut().write_all(b"221 Bye\r\n").unwrap();
                    break;
                } else {
                    assert!(
                        authenticated,
                        "no fallback AUTH or delivery after failed authentication"
                    );
                    if line == "DATA\r\n" {
                        peer.get_mut().write_all(b"354 Content\r\n").unwrap();
                        loop {
                            let mut line = String::new();
                            assert!(peer.read_line(&mut line).unwrap() > 0);
                            if line == ".\r\n" {
                                break;
                            }
                            data.extend_from_slice(line.as_bytes());
                        }
                    } else {
                        assert!(
                            line == "NOOP\r\n"
                                || line.starts_with("MAIL FROM:")
                                || line.starts_with("RCPT TO:")
                        );
                    }
                    "250 OK".into()
                };
                peer.get_mut()
                    .write_all(format!("{reply}\r\n").as_bytes())
                    .unwrap();
            }
            data
        });
        let profile = json!({"host":address.to_string(),"from_address":"sender@example.test","username":"tim","password":"tanstaaftanstaaf"});
        let (path, body) = if send {
            (
                "send_test_email",
                json!({"smtp":profile,"email":"recipient@example.test","first_name":"Alice"}),
            )
        } else {
            ("test_connection", profile)
        };
        let response = client()
            .post(format!("{base}/api/util/{path}"))
            .bearer_auth("synthetic-bootstrap-token")
            .json(&body)
            .send()
            .unwrap();
        let status = response.status();
        let body = response.text().unwrap();
        assert_eq!(status.is_success(), success, "{body}");
        assert!(!body.contains("tanstaaftanstaaf") && !body.contains(RESPONSE));
        let message = peer.join().unwrap();
        assert_eq!(!message.is_empty(), send && success);
        if send && success {
            assert!(
                String::from_utf8(message)
                    .unwrap()
                    .contains("recipient@example.test")
            );
        }
    }
    process.stop();
    let log = std::fs::read_to_string(directory.path().join("server.log")).unwrap();
    assert!(!log.contains("tanstaaftanstaaf") && !log.contains(RESPONSE));
}

#[test]
fn smtp_starttls_verifies_certificates_before_authentication_and_reports_rejection() {
    use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
    use std::io::{BufRead, BufReader};
    let directory = workspace();
    let material = ruphish_adapters::tls::load_or_generate(
        &directory.path().join("smtp.crt"),
        &directory.path().join("smtp.key"),
        "localhost",
    )
    .unwrap();
    let tls = std::sync::Arc::new(
        rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from_pem_slice(&material.certificate).unwrap()],
                PrivateKeyDer::from_pem_slice(&material.key).unwrap(),
            )
            .unwrap(),
    );
    let mut process = Process::start(directory.path(), "admin");
    let base = ready(&mut process);
    unlock(directory.path());
    for (ignore_cert_errors, accepted_auth) in [(false, true), (true, true), (true, false)] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let tls = tls.clone();
        let thread = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(12)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(12)))
                .unwrap();
            let mut plain = BufReader::new(stream);
            plain
                .get_mut()
                .write_all(b"220 loopback ESMTP\r\n")
                .unwrap();
            let mut line = String::new();
            plain.read_line(&mut line).unwrap();
            assert!(line.starts_with("EHLO "));
            plain
                .get_mut()
                .write_all(b"250-loopback\r\n250 STARTTLS\r\n")
                .unwrap();
            line.clear();
            plain.read_line(&mut line).unwrap();
            assert_eq!(line, "STARTTLS\r\n");
            plain.get_mut().write_all(b"220 Begin TLS\r\n").unwrap();
            let connection = rustls::ServerConnection::new(tls).unwrap();
            let mut encrypted =
                BufReader::new(rustls::StreamOwned::new(connection, plain.into_inner()));
            line.clear();
            if encrypted.read_line(&mut line).is_err() || line.is_empty() {
                return false;
            }
            assert!(line.starts_with("EHLO "));
            encrypted
                .get_mut()
                .write_all(b"250-loopback\r\n250 AUTH PLAIN\r\n")
                .unwrap();
            encrypted.get_mut().flush().unwrap();
            line.clear();
            encrypted.read_line(&mut line).unwrap();
            assert!(line.starts_with("EHLO "));
            encrypted
                .get_mut()
                .write_all(b"250-loopback\r\n250 AUTH PLAIN\r\n")
                .unwrap();
            encrypted.get_mut().flush().unwrap();
            line.clear();
            encrypted.read_line(&mut line).unwrap();
            assert!(line.starts_with("AUTH PLAIN "));
            encrypted
                .get_mut()
                .write_all(if accepted_auth {
                    b"235 Authenticated\r\n"
                } else {
                    b"535 Invalid credentials\r\n"
                })
                .unwrap();
            encrypted.get_mut().flush().unwrap();
            if accepted_auth {
                line.clear();
                encrypted.read_line(&mut line).unwrap();
                assert_eq!(line, "NOOP\r\n");
                encrypted.get_mut().write_all(b"250 OK\r\n").unwrap();
                encrypted.get_mut().flush().unwrap();
                line.clear();
                encrypted.read_line(&mut line).unwrap();
                assert_eq!(line, "QUIT\r\n");
                encrypted.get_mut().write_all(b"221 Bye\r\n").unwrap();
                encrypted.get_mut().flush().unwrap();
            }
            true
        });
        let response = client().post(format!("{base}/api/util/test_connection")).bearer_auth("synthetic-bootstrap-token").json(&json!({"host":address.to_string(),"from_address":"training@example.test","username":"synthetic-user","password":"synthetic-password","ignore_cert_errors":ignore_cert_errors})).send().unwrap();
        let success = response.status().is_success();
        let body = response.text().unwrap();
        assert!(!body.contains("synthetic-password"));
        assert_eq!(success, ignore_cert_errors && accepted_auth, "{body}");
        if !ignore_cert_errors {
            assert!(
                body.contains("TLS negotiation/certificate validation"),
                "{body}"
            );
        } else if !accepted_auth {
            assert!(
                body.contains("authentication") && body.contains("SMTP 535"),
                "{body}"
            );
        }
        assert_eq!(thread.join().unwrap(), ignore_cert_errors);
    }
    process.stop();
}

struct MailSink {
    address: std::net::SocketAddr,
    received: std::sync::mpsc::Receiver<(Vec<String>, Vec<u8>)>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl MailSink {
    fn start(rejection: Option<&'static str>) -> Self {
        Self::scenario(rejection, false)
    }
    fn scenario(rejection: Option<&'static str>, lose_acknowledgement: bool) -> Self {
        use std::io::{BufRead, BufReader};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (tx, received) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(12)))
                .unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut commands = Vec::new();
            let mut mail = Vec::new();
            let _ = stream.write_all(b"220 loopback ESMTP\r\n");
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    _ => {}
                }
                commands.push(line.trim_end().to_owned());
                if line.starts_with("EHLO") {
                    let _ = stream.write_all(b"250-loopback\r\n250 SIZE 67108864\r\n");
                } else if line.starts_with("DATA") {
                    if let Some(reply) = rejection {
                        let _ = stream.write_all(reply.as_bytes());
                        break;
                    }
                    let _ = stream.write_all(b"354 Send message\r\n");
                    loop {
                        let mut part = String::new();
                        if reader.read_line(&mut part).unwrap_or(0) == 0 {
                            break;
                        }
                        if part == ".\r\n" {
                            break;
                        }
                        if part.starts_with("..") {
                            part.remove(0);
                        }
                        mail.extend_from_slice(part.as_bytes());
                    }
                    if lose_acknowledgement {
                        break;
                    }
                    let _ = stream.write_all(b"250 Accepted\r\n");
                } else if line.starts_with("QUIT") {
                    let _ = stream.write_all(b"221 Bye\r\n");
                    break;
                } else {
                    let _ = stream.write_all(b"250 OK\r\n");
                }
            }
            let _ = tx.send((commands, mail));
        });
        Self {
            address,
            received,
            thread: Some(thread),
        }
    }
    fn result(&self) -> (Vec<String>, Vec<u8>) {
        self.received.recv_timeout(Duration::from_secs(15)).unwrap()
    }
}
impl Drop for MailSink {
    fn drop(&mut self) {
        let _ = std::net::TcpStream::connect_timeout(&self.address, Duration::from_millis(200));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[test]
fn actual_test_delivery_composes_mime_persists_preview_and_never_records_campaign_events() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "all");
    let addresses = process.ready();
    let base = format!("http://{}", addresses["admin"].as_str().unwrap());
    let public = format!("http://{}", addresses["phish"].as_str().unwrap());
    unlock(directory.path());
    let client = client();
    let sink = MailSink::start(None);
    for (family, value) in [
        (
            "templates",
            json!({"name":"Test mail","subject":"Hello {{.FirstName}}","text":"Text {{.Email}} {{.URL}}","html":"<p>HTML {{.Email}}</p>","envelope_sender":"Training Team <visible@example.test>","attachments":[{"name":"sample.txt","type":"text/plain","content":"SGVsbG8ge3suRmlyc3ROYW1lfX0="}]}),
        ),
        (
            "pages",
            json!({"name":"Preview page","html":"<h1>Hello {{.FirstName}}</h1><form action='/other'><input name='email'></form>"}),
        ),
        (
            "smtp",
            json!({"name":"Loopback","host":sink.address.to_string(),"from_address":"envelope@example.test","headers":[{"key":"X-Example","value":"{{.FirstName}}"}]}),
        ),
    ] {
        let response = client
            .post(format!("{base}/api/{family}/"))
            .bearer_auth("synthetic-bootstrap-token")
            .json(&value)
            .send()
            .unwrap();
        assert_eq!(response.status(), 201, "{}", response.text().unwrap());
    }
    let response=client.post(format!("{base}/api/util/send_test_email")).bearer_auth("synthetic-bootstrap-token").json(&json!({"template":{"name":"Test mail"},"page":{"name":"Preview page"},"smtp":{"name":"Loopback","host":"bad:port","from_address":"not-mailbox"},"url":format!("{public}/landing"),"email":"recipient@example.test","first_name":"Alice","last_name":"Example"})).send().unwrap();
    assert_eq!(response.status(), 200, "{}", response.text().unwrap());
    let (commands, bytes) = sink.result();
    assert!(
        commands
            .iter()
            .any(|c| c.starts_with("MAIL FROM:<envelope@example.test>"))
    );
    assert!(
        commands
            .iter()
            .any(|c| c == "RCPT TO:<recipient@example.test>")
    );
    let raw = String::from_utf8(bytes).unwrap();
    assert!(raw.contains("Subject: Hello Alice"));
    assert!(raw.contains("visible@example.test"));
    assert!(raw.contains("X-Example: Alice"));
    assert!(raw.contains("multipart/alternative"));
    assert!(raw.contains("sample.txt"));
    assert!(raw.contains("Hello Alice"));
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let rid = runtime.block_on(async {
        let pool = sqlx::SqlitePool::connect(&format!(
            "sqlite:{}",
            directory.path().join("installation.sqlite3").display()
        ))
        .await
        .unwrap();
        let rid: String = sqlx::query_scalar(
            "SELECT r_id FROM email_requests WHERE email='recipient@example.test'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        for table in ["results", "events", "mail_logs"] {
            let count: i64 =
                sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert_eq!(count, 0);
        }
        pool.close().await;
        rid
    });
    let mail = mail_parser::MessageParser::default()
        .parse(raw.as_bytes())
        .unwrap();
    let sent_html = mail.body_html(0).unwrap();
    let tracking_url = format!("{public}/landing/track?rid={rid}");
    assert_eq!(sent_html.matches("<img ").count(), 1);
    assert!(sent_html.contains(&format!("src=\"{tracking_url}\"")));
    assert!(sent_html.contains("style=\"display:none\""));
    let response = client
        .get(format!("{public}/landing?rid={rid}"))
        .send()
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(
        response.headers()["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("sandbox")
    );
    let html = response.text().unwrap();
    assert!(html.contains("Hello Alice"));
    assert!(!html.contains("name='email'"));
    let pixel = client.get(&tracking_url).send().unwrap();
    assert_eq!(pixel.status(), 200);
    assert_eq!(pixel.headers()["content-type"], "image/png");
    assert_eq!(
        pixel.bytes().unwrap().as_ref(),
        include_bytes!("fixtures/resources/preview-pixel.png")
    );
    assert_eq!(
        client
            .get(format!("{public}/report?rid={rid}"))
            .send()
            .unwrap()
            .status(),
        204
    );
    for path in ["/", "/track", "/report"] {
        assert_eq!(
            client
                .get(format!("{public}{path}?rid=preview-unknown"))
                .send()
                .unwrap()
                .status(),
            404
        );
    }
    assert_eq!(
        client
            .post(format!("{public}/landing?rid={rid}"))
            .body("password=must-not-store")
            .send()
            .unwrap()
            .status(),
        404
    );
    process.stop();
    let mut restarted = Process::start(directory.path(), "all");
    let addresses = restarted.ready();
    let public = format!("http://{}", addresses["phish"].as_str().unwrap());
    let reopened = client
        .get(format!("{public}/landing?rid={rid}"))
        .send()
        .unwrap()
        .text()
        .unwrap();
    assert_eq!(reopened, html);
    runtime.block_on(async {
        let pool = sqlx::SqlitePool::connect(&format!(
            "sqlite:{}",
            directory.path().join("installation.sqlite3").display()
        ))
        .await
        .unwrap();
        for table in ["events", "results", "mail_logs"] {
            let count: i64 =
                sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert_eq!(count, 0);
        }
        pool.close().await;
    });
    restarted.stop();
}

#[test]
fn email_create_edit_and_preview_share_validation_without_mutating_source() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = ready(&mut process);
    unlock(directory.path());
    let client = client();
    let source = "<p>Hello {{.FirstName}}</p><!--[if mso]><p>Outlook</p><![endif]-->";
    let valid = json!({"name":"Managed tracking", "html":source});
    let response = client
        .post(format!("{base}/api/templates/"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(&valid)
        .send()
        .unwrap();
    assert_eq!(response.status(), 201);
    let saved: Value = response.json().unwrap();
    assert_eq!(saved["html"], source);
    let id = saved["id"].as_i64().unwrap();
    let saved: Value = client
        .get(format!("{base}/api/templates/{id}"))
        .bearer_auth("synthetic-bootstrap-token")
        .send()
        .unwrap()
        .json()
        .unwrap();
    for html in [
        "<script>alert(1)</script>",
        "<img src=x onerror='alert(1)'>",
        "<form><input></form>",
    ] {
        let invalid = json!({"name":"Rejected", "html":html});
        let response = client
            .post(format!("{base}/api/templates/"))
            .bearer_auth("synthetic-bootstrap-token")
            .json(&invalid)
            .send()
            .unwrap();
        assert_eq!(response.status(), 400);
        let invalid = json!({"id":id,"name":"Managed tracking", "html":html});
        let response = client
            .put(format!("{base}/api/templates/{id}"))
            .bearer_auth("synthetic-bootstrap-token")
            .json(&invalid)
            .send()
            .unwrap();
        assert_eq!(response.status(), 400);
    }
    let unchanged: Value = client
        .get(format!("{base}/api/templates/{id}"))
        .bearer_auth("synthetic-bootstrap-token")
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(unchanged, saved);
    let edited_source = "<body><p>Edited {{.Email | html}}</p></body>";
    let response = client
        .put(format!("{base}/api/templates/{id}"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(&json!({"id":id,"name":"Managed tracking", "html":edited_source}))
        .send()
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.json::<Value>().unwrap()["html"], edited_source);
    let response = client
        .post(format!("{base}/api/util/preview"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(&json!({"html":"<p>Hello</p>{{.Tracker}}<img src='{{.TrackingURL}}'>"}))
        .send()
        .unwrap();
    assert_eq!(response.status(), 200);
    let preview: Value = response.json().unwrap();
    assert_eq!(preview["html"], "<p>Hello</p>");
    let response = client.put(format!("{base}/api/templates/{id}"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(&json!({"id":id,"name":"Managed tracking", "html":"{{if eq .FirstName \"Unsafe\"}}<img src=x onerror='bad()'>{{else}}<p>Hello</p>{{end}}"}))
        .send().unwrap();
    assert_eq!(response.status(), 200);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let response = client.post(format!("{base}/api/util/send_test_email"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(&json!({"template":{"name":"Managed tracking"},"smtp":{"host":listener.local_addr().unwrap().to_string(),"from_address":"sender@example.test"},"email":"recipient@example.test","first_name":"Unsafe","url":"http://127.0.0.1/"}))
        .send().unwrap();
    assert_eq!(response.status(), 400);
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let pool = sqlx::SqlitePool::connect(&format!(
            "sqlite:{}",
            directory.path().join("installation.sqlite3").display()
        ))
        .await
        .unwrap();
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM email_requests")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            count, 0,
            "failed composition must not persist preview state"
        );
        pool.close().await;
    });
    process.stop();
}

#[test]
fn smtp_rejections_and_read_only_permissions_do_not_claim_success_or_queue_retries() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = ready(&mut process);
    unlock(directory.path());
    let client = client();
    for reply in ["451 Temporary rejection\r\n", "550 Permanent rejection\r\n"] {
        let sink = MailSink::start(Some(reply));
        let response=client.post(format!("{base}/api/util/send_test_email")).bearer_auth("synthetic-bootstrap-token").json(&json!({"smtp":{"host":sink.address.to_string(),"from_address":"sender@example.test"},"email":"recipient@example.test"})).send().unwrap();
        assert!(!response.status().is_success());
        let body: Value = response.json().unwrap();
        assert_eq!(body["success"], false);
        let (_, bytes) = sink.result();
        assert!(bytes.is_empty());
    }
    let sink = MailSink::scenario(None, true);
    let response = client.post(format!("{base}/api/util/send_test_email")).bearer_auth("synthetic-bootstrap-token").json(&json!({"smtp":{"host":sink.address.to_string(),"from_address":"sender@example.test"},"email":"recipient@example.test"})).send().unwrap();
    assert!(!response.status().is_success());
    assert!(response.text().unwrap().contains("outcome is unknown"));
    let (commands, bytes) = sink.result();
    assert!(!bytes.is_empty());
    assert_eq!(
        commands
            .iter()
            .filter(|line| line.as_str() == "DATA")
            .count(),
        1
    );
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async{let pool=sqlx::SqlitePool::connect(&format!("sqlite:{}",directory.path().join("installation.sqlite3").display())).await.unwrap();let queued:i64=sqlx::query_scalar("SELECT count(*) FROM mail_logs").fetch_one(&pool).await.unwrap();assert_eq!(queued,0);sqlx::query("DELETE FROM role_permissions WHERE permission_id=(SELECT id FROM permissions WHERE slug='modify_objects')").execute(&pool).await.unwrap();pool.close().await;});
    for path in [
        "/api/groups/",
        "/api/templates/",
        "/api/pages/",
        "/api/smtp/",
        "/api/import/email",
        "/api/import/site",
        "/api/import/group/review",
        "/api/util/send_test_email",
        "/api/util/test_connection",
    ] {
        assert_eq!(
            client
                .post(format!("{base}{path}"))
                .bearer_auth("synthetic-bootstrap-token")
                .json(&json!({}))
                .send()
                .unwrap()
                .status(),
            403,
            "{path}"
        );
    }
    process.stop();
}
fn unlock(directory: &Path) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let pool = sqlx::SqlitePool::connect(&format!(
            "sqlite:{}",
            directory.join("installation.sqlite3").display()
        ))
        .await
        .unwrap();
        sqlx::query("UPDATE users SET password_change_required=0")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
    });
}
fn group(name: &str, first: &str) -> Value {
    json!({"name":name,"targets":[{"email":"alice@example.test","first_name":first,"last_name":"Example","position":"Analyst"}]})
}

fn normalize(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            if fields.contains_key("modified_date") {
                fields.insert("modified_date".into(), json!("<modified>"));
            }
            for value in fields.values_mut() {
                normalize(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                normalize(value);
            }
        }
        _ => {}
    }
}

#[test]
fn independent_go_resource_observations_run_against_the_executable() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = ready(&mut process);
    unlock(directory.path());
    let client = client();
    let cases: Vec<Value> =
        serde_json::from_slice(include_bytes!("fixtures/resources/http.json")).unwrap();
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let method =
            reqwest::Method::from_bytes(case["method"].as_str().unwrap().as_bytes()).unwrap();
        let response = client
            .request(method, format!("{base}{}", case["path"].as_str().unwrap()))
            .bearer_auth("synthetic-bootstrap-token")
            .header("content-type", "application/json")
            .body(case["body"].as_str().unwrap().to_owned())
            .send()
            .unwrap();
        let expected_status = if id == "templates-unknown-field" {
            400
        } else {
            case["status"].as_u64().unwrap() as u16
        };
        assert_eq!(
            response.status().as_u16(),
            expected_status,
            "{id}: {}",
            response.text().unwrap()
        );
        let mut observed: Value = response.json().unwrap();
        // Assert the additive error code without changing the recorded Go response.
        let expected_code = match id {
            "groups-no-name"
            | "groups-no-targets"
            | "templates-no-content"
            | "templates-unknown-field" => Some("RESOURCE_INVALID"),
            "groups-duplicate" | "templates-duplicate" | "profiles-duplicate" => {
                Some("RESOURCE_NAME_TAKEN")
            }
            "groups-mismatch" => Some("RESOURCE_ID_MISMATCH"),
            _ => None,
        };
        if let Some(expected_code) = expected_code {
            let code = observed.as_object_mut().unwrap().remove("code");
            assert_eq!(code, Some(json!(expected_code)), "{id}");
        }
        // Field reports are additive; retain every independent Go expectation below.
        let expected_fields: &[&str] = match id {
            "groups-no-name"
            | "groups-duplicate"
            | "templates-duplicate"
            | "profiles-duplicate" => &["name"],
            "groups-no-targets" => &["targets"],
            "templates-no-content" => &["html", "text"],
            "templates-unknown-field" => &["text"],
            _ => &[],
        };
        if !expected_fields.is_empty() {
            let report: ruphish_contracts::validation::ValidationReport = serde_json::from_value(
                observed
                    .as_object_mut()
                    .unwrap()
                    .remove("validation")
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(
                report
                    .issues
                    .iter()
                    .map(|issue| issue.field.as_str())
                    .collect::<Vec<_>>(),
                expected_fields,
                "{id}"
            );
        }
        normalize(&mut observed);
        let mut expected = case["json"].clone();
        match id {
            // Approved recipient isolation: the independently captured Go result demonstrates the defect.
            "groups-shared-after-edit" => {
                assert_eq!(expected["targets"][0]["first_name"], "");
                expected["targets"][0]["first_name"] = json!("Alice");
                expected["targets"][0]["position"] = json!("Analyst");
            }
            // Authored page source stays reversible; served policy is tested independently.
            "pages-empty-html" | "pages-capture" | "pages-no-capture" => {
                let input: Value = serde_json::from_str(case["body"].as_str().unwrap()).unwrap();
                expected["html"] = input.get("html").cloned().unwrap_or(json!(""));
                // Retired capture flags are absent from the native response.
                expected
                    .as_object_mut()
                    .unwrap()
                    .remove("capture_credentials");
                expected
                    .as_object_mut()
                    .unwrap()
                    .remove("capture_passwords");
                assert!(observed["warnings"].is_array());
                observed.as_object_mut().unwrap().remove("warnings");
            }
            // Engine diagnostics are not identical; both reject the invalid expression before persistence.
            "templates-unknown-field" => {
                assert_eq!(observed["success"], false);
                assert!(observed["message"].as_str().unwrap().contains("Template"));
                continue;
            }
            _ => {}
        }
        assert_eq!(observed, expected, "{id}");
    }
    process.stop();
}

#[test]
fn migrated_shared_recipients_and_duplicate_names_are_preserved_then_isolated() {
    let directory = workspace();
    std::fs::write(
        directory.path().join("installation.sqlite3"),
        include_bytes!("fixtures/resources/shared-recipients.sqlite3"),
    )
    .unwrap();
    let mut process = Process::start(directory.path(), "admin");
    let base = ready(&mut process);
    let client = client();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let pool = sqlx::SqlitePool::connect(&format!(
            "sqlite:{}",
            directory.path().join("installation.sqlite3").display()
        ))
        .await
        .unwrap();
        let count: i64 = sqlx::query_scalar("SELECT count(DISTINCT target_id) FROM group_targets")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
        sqlx::query("UPDATE groups SET name='Duplicate'")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
    });
    let mut first: Value = client
        .get(format!("{base}/api/groups/1"))
        .bearer_auth("synthetic-bootstrap-token")
        .send()
        .unwrap()
        .json()
        .unwrap();
    first["targets"][0]["first_name"] = json!("Changed");
    assert_eq!(
        client
            .put(format!("{base}/api/groups/1"))
            .bearer_auth("synthetic-bootstrap-token")
            .json(&first)
            .send()
            .unwrap()
            .status(),
        200
    );
    let other: Value = client
        .get(format!("{base}/api/groups/2"))
        .bearer_auth("synthetic-bootstrap-token")
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(other["targets"][0]["first_name"], "Alice");
    assert_eq!(other["name"], "Duplicate");
    assert_eq!(
        client
            .post(format!("{base}/api/groups/"))
            .bearer_auth("synthetic-bootstrap-token")
            .json(&group("Duplicate", "New"))
            .send()
            .unwrap()
            .status(),
        409
    );
    process.stop();
}

#[test]
fn resource_replacements_roll_back_children_and_block_live_campaign_deletion() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = ready(&mut process);
    unlock(directory.path());
    let client = client();
    for (family, body, child, foreign, column) in [
        (
            "templates",
            json!({"name":"Template","text":"Hello {{.FirstName}}","attachments":[{"name":"a.txt","type":"text/plain","content":"aGVsbG8="}]}),
            "attachments",
            "template_id",
            "template_id",
        ),
        (
            "smtp",
            json!({"name":"Profile","host":"127.0.0.1:2525","from_address":"training@example.test","headers":[{"key":"X-Test","value":"original"}]}),
            "headers",
            "smtp_id",
            "smtp_id",
        ),
    ] {
        let response = client
            .post(format!("{base}/api/{family}/"))
            .bearer_auth("synthetic-bootstrap-token")
            .json(&body)
            .send()
            .unwrap();
        assert_eq!(response.status(), 201, "{}", response.text().unwrap());
        let original: Value = response.json().unwrap();
        let id = original["id"].as_i64().unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async{let pool=sqlx::SqlitePool::connect(&format!("sqlite:{}",directory.path().join("installation.sqlite3").display())).await.unwrap();sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE TRIGGER fail_child BEFORE INSERT ON {child} BEGIN SELECT RAISE(ABORT,'injected child failure'); END;"))).execute(&pool).await.unwrap();pool.close().await;});
        let mut changed = original.clone();
        changed["name"] = json!("Changed");
        assert_eq!(
            client
                .put(format!("{base}/api/{family}/{id}"))
                .bearer_auth("synthetic-bootstrap-token")
                .json(&changed)
                .send()
                .unwrap()
                .status(),
            500
        );
        assert_eq!(
            client
                .get(format!("{base}/api/{family}/{id}"))
                .bearer_auth("synthetic-bootstrap-token")
                .send()
                .unwrap()
                .json::<Value>()
                .unwrap(),
            original
        );
        runtime.block_on(async{let pool=sqlx::SqlitePool::connect(&format!("sqlite:{}",directory.path().join("installation.sqlite3").display())).await.unwrap();sqlx::raw_sql("DROP TRIGGER fail_child").execute(&pool).await.unwrap();let count:i64=sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {child} WHERE {foreign}=?"))).bind(id).fetch_one(&pool).await.unwrap();assert_eq!(count,1);sqlx::query(sqlx::AssertSqlSafe(format!("INSERT INTO campaigns(name,user_id,status,{column}) VALUES('Imported active',1,'Queued',?)"))).bind(id).execute(&pool).await.unwrap();pool.close().await;});
        assert_eq!(
            client
                .delete(format!("{base}/api/{family}/{id}"))
                .bearer_auth("synthetic-bootstrap-token")
                .send()
                .unwrap()
                .status(),
            409
        );
    }
    process.stop();
}
fn create(client: &Client, base: &str, value: &Value) -> Value {
    let response = client
        .post(format!("{base}/api/groups/"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(value)
        .send()
        .unwrap();
    assert_eq!(response.status(), 201, "{}", response.text().unwrap());
    response.json().unwrap()
}

#[test]
fn groups_round_trip_isolate_recipients_clear_fields_and_survive_restart() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = ready(&mut process);
    unlock(directory.path());
    let client = client();
    let mut first = create(&client, &base, &group("First", "Alice"));
    let second = create(&client, &base, &group("Second", "Alice"));
    let id = first["id"].as_i64().unwrap();
    first["targets"][0]["first_name"] = json!("");
    first["targets"][0]["position"] = json!("");
    let response = client
        .put(format!("{base}/api/groups/{id}"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(&first)
        .send()
        .unwrap();
    assert_eq!(response.status(), 200, "{}", response.text().unwrap());
    let saved: Value = response.json().unwrap();
    assert_eq!(saved["targets"][0]["first_name"], "");
    let other: Value = client
        .get(format!("{base}/api/groups/{}", second["id"]))
        .bearer_auth("synthetic-bootstrap-token")
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(other["targets"][0]["first_name"], "Alice");
    assert_eq!(other["targets"][0]["position"], "Analyst");
    let listed: Value = client
        .get(format!("{base}/api/groups/"))
        .bearer_auth("synthetic-bootstrap-token")
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(listed.as_array().unwrap().len(), 2);
    assert_eq!(listed[0], saved);
    assert_eq!(listed[1], other);
    // Go's name lookup has no dedicated HTTP route. Exercise the actual repository
    // against the executable-created database, including missing names and SQL metacharacters.
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        use ruphish_application::{auth::Identity, resources::{Kind,Error,Content}};
        let pool = sqlx::SqlitePool::connect(&format!("sqlite:{}",directory.path().join("installation.sqlite3").display())).await.unwrap();
        let epoch: i64 = sqlx::query_scalar("SELECT COALESCE((SELECT epoch FROM ruphish_auth_state WHERE user_id=1),0)").fetch_one(&pool).await.unwrap();
        let provider = ruphish_adapters::database::open(ruphish_adapters::database::Config::Sqlite {
            path: directory.path().join("installation.sqlite3"), migrations_prefix: String::new(),
        }, ruphish_adapters::outbound::Policy::default()).await.unwrap();
        let repository = provider.resources();
        let caller = Identity{id:1,epoch};
        let named = repository.get_by_name(caller,Kind::Group,"Second").await.unwrap();
        assert_eq!(named.id,second["id"].as_i64().unwrap());
        assert!(matches!(named.content,Content::Group(ref rows) if rows.len()==1 && rows[0].first_name=="Alice"));
        for name in ["Missing", "' OR 1=1 --"] {
            assert!(matches!(repository.get_by_name(caller,Kind::Group,name).await,Err(Error::NotFound(_))));
        }
        pool.close().await;
        provider.close().await;
    });
    process.stop();
    let mut process = Process::start(directory.path(), "admin");
    let base = ready(&mut process);
    let stored: Value = client
        .get(format!("{base}/api/groups/{id}"))
        .bearer_auth("synthetic-bootstrap-token")
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(stored, saved);
    let summary: Value = client
        .get(format!("{base}/api/groups/summary"))
        .bearer_auth("synthetic-bootstrap-token")
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(summary["total"], 2);
    assert_eq!(summary["groups"][0]["num_targets"], 1);
    assert!(summary["groups"][0].get("targets").is_none());
    assert_eq!(
        client
            .delete(format!("{base}/api/groups/{id}"))
            .bearer_auth("synthetic-bootstrap-token")
            .send()
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        client
            .get(format!("{base}/api/groups/{id}"))
            .bearer_auth("synthetic-bootstrap-token")
            .send()
            .unwrap()
            .status(),
        404
    );
    process.stop();
}

#[test]
fn groups_reject_invalid_replacements_and_stale_revisions_without_losing_data() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = ready(&mut process);
    unlock(directory.path());
    let client = client();
    let original = create(&client, &base, &group("Reviewed", "Alice"));
    let path = format!("{base}/api/groups/{}", original["id"]);
    let response = client
        .get(&path)
        .bearer_auth("synthetic-bootstrap-token")
        .send()
        .unwrap();
    let revision = response.headers()["etag"].to_str().unwrap().to_owned();
    let mut invalid = original.clone();
    invalid["targets"][0]["email"] = json!("invalid");
    assert_eq!(
        client
            .put(&path)
            .bearer_auth("synthetic-bootstrap-token")
            .json(&invalid)
            .send()
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        client
            .get(&path)
            .bearer_auth("synthetic-bootstrap-token")
            .send()
            .unwrap()
            .json::<Value>()
            .unwrap(),
        original
    );
    let mut changed = original.clone();
    changed["name"] = json!("Changed");
    assert_eq!(
        client
            .put(&path)
            .bearer_auth("synthetic-bootstrap-token")
            .header("if-match", &revision)
            .json(&changed)
            .send()
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        client
            .put(&path)
            .bearer_auth("synthetic-bootstrap-token")
            .header("if-match", &revision)
            .json(&original)
            .send()
            .unwrap()
            .status(),
        412
    );
    assert_eq!(
        client
            .post(format!("{base}/api/groups/"))
            .bearer_auth("synthetic-bootstrap-token")
            .json(&group("Changed", "Other"))
            .send()
            .unwrap()
            .status(),
        409
    );
    assert_eq!(client.get(&path).send().unwrap().status(), 401);
    process.stop();
}

#[test]
fn legacy_resource_validation_and_page_policy_survive_round_trip_and_restart() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = ready(&mut process);
    unlock(directory.path());
    let client = client();
    for (payload, expected) in [
        (
            json!({"smtp":{"host":"127.0.0.1:1","from_address":"from@example.com"}}),
            "No email address specified",
        ),
        (
            json!({"email":"test@example.com","smtp":{"host":"127.0.0.1:1"}}),
            "No From Address specified",
        ),
    ] {
        let response = client
            .post(format!("{base}/api/util/send_test_email"))
            .bearer_auth("synthetic-bootstrap-token")
            .json(&payload)
            .send()
            .unwrap();
        assert_eq!(response.status(), 400);
        assert_eq!(response.json::<Value>().unwrap()["message"], expected);
    }
    for field in ["template", "page"] {
        let mut payload = json!({"email":"test@example.com"});
        payload[field] = json!({"name":"Missing resource"});
        assert_eq!(
            client
                .post(format!("{base}/api/util/send_test_email"))
                .bearer_auth("synthetic-bootstrap-token")
                .json(&payload)
                .send()
                .unwrap()
                .status(),
            400
        );
    }
    for (family, value) in [
        (
            "smtp",
            json!({"name":"Test SMTP","from_address":"foo@example.com"}),
        ),
        ("smtp", json!({"name":"Test SMTP","host":"1.1.1.1:25"})),
        (
            "smtp",
            json!({"name":"Test SMTP","host":"1.1.1.1:25","from_address":"example.com"}),
        ),
        (
            "smtp",
            json!({"name":"Test SMTP","host":"1.1.1.1:25","from_address":"foo@example.com","headers":[{"key":"X-Test","value":"ok\r\nInjected: bad"}]}),
        ),
        ("pages", json!({"html":"{{.BaseURL}}"})),
        (
            "pages",
            json!({"name":"Invalid HTML","html":"{{.INVALIDTAG}}"}),
        ),
        (
            "pages",
            json!({"name":"Invalid redirect","html":"valid data","redirect_url":"http://example.com/{{.INVALIDTAG}}"}),
        ),
    ] {
        let response = client
            .post(format!("{base}/api/{family}/"))
            .bearer_auth("synthetic-bootstrap-token")
            .json(&value)
            .send()
            .unwrap();
        assert_eq!(
            response.status(),
            400,
            "{family}: {}",
            response.text().unwrap()
        );
    }
    // Approved address-syntax improvement over Go's TestPostInvalidFrom.
    let profile = json!({"name":"Display name and IPv6","host":"[::1]:25","from_address":"Foo Bar <foo@example.com>","headers":[{"key":"Reply-To","value":"test@example.com"},{"key":"X-Mailer","value":"gophish"}]});
    let response = client
        .post(format!("{base}/api/smtp/"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(&profile)
        .send()
        .unwrap();
    assert_eq!(response.status(), 201, "{}", response.text().unwrap());
    let stored: Value = response.json().unwrap();
    assert_eq!(stored["from_address"], profile["from_address"]);
    assert_eq!(stored["headers"], profile["headers"]);
    let response = client
        .get(format!("{base}/api/smtp/-1"))
        .bearer_auth("synthetic-bootstrap-token")
        .send()
        .unwrap();
    assert_eq!(response.status(), 404);
    let denied = client
        .post(format!("{base}/api/util/test_connection"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(&json!({"host":"169.254.169.254:25","from_address":"foo@example.com"}))
        .send()
        .unwrap();
    assert_eq!(denied.status(), 400);
    assert!(
        denied
            .text()
            .unwrap()
            .to_lowercase()
            .contains("smtp configuration")
    );
    let html = "<html><head></head><body><form action='example.com'><input name='username'><input name='password' type='password'></form></body></html>";
    let mut page: Value = client
        .post(format!("{base}/api/pages/"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(
            &json!({"name":"Reversible settings","html":html,"redirect_url":"http://example.com"}),
        )
        .send()
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .unwrap();
    assert!(page.get("capture_credentials").is_none());
    let id = page["id"].as_i64().unwrap();
    for redirect in ["", "https://example.test/finished", ""] {
        page["redirect_url"] = json!(redirect);
        page = client
            .put(format!("{base}/api/pages/{id}"))
            .bearer_auth("synthetic-bootstrap-token")
            .json(&page)
            .send()
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .unwrap();
        assert_eq!(page["html"], html);
        assert_eq!(page["redirect_url"], redirect);
        assert!(page.get("capture_credentials").is_none());
        assert!(page.get("capture_passwords").is_none());
    }
    process.stop();
    let mut restarted = Process::start(directory.path(), "admin");
    let base = ready(&mut restarted);
    let reopened: Value = client
        .get(format!("{base}/api/pages/{id}"))
        .bearer_auth("synthetic-bootstrap-token")
        .send()
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(reopened, page);
    restarted.stop();
}

#[test]
fn landing_diagnostics_warn_without_blocking_save_and_preserve_correct_form_semantics() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = ready(&mut process);
    unlock(directory.path());
    let client = client();
    assert_eq!(
        client
            .post(format!("{base}/api/pages/validate"))
            .json(&json!({"html":""}))
            .send()
            .unwrap()
            .status(),
        401
    );
    for (index, html, expected, absent) in [
        (
            0,
            "<!doctype html><form><input type=password name=p disabled value=prefilled><button>Submit</button></form>",
            "password_disabled",
            "password_without_form",
        ),
        (
            1,
            "<!doctype html><form><fieldset disabled><legend><input type=password name=allowed></legend><input type=password name=blocked></fieldset><button>Submit</button></form>",
            "password_disabled",
            "password_unnamed",
        ),
        (
            2,
            "<!doctype html><form><input type=password><button>Submit</button></form>",
            "password_unnamed",
            "password_disabled",
        ),
        (
            3,
            "<!doctype html><input type=password name=p>",
            "password_without_form",
            "password_disabled",
        ),
        (
            4,
            "<!doctype html><form><input name=u><input type=password name=p></form>",
            "form_without_submit",
            "password_without_form",
        ),
    ] {
        let read_warnings = |path: &str| -> Value {
            client
                .post(format!("{base}{path}"))
                .bearer_auth("synthetic-bootstrap-token")
                .json(&json!({"html":html}))
                .send()
                .unwrap()
                .error_for_status()
                .unwrap()
                .json()
                .unwrap()
        };
        let validation = read_warnings("/api/pages/validate");
        let codes: Vec<_> = validation["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| w["code"].as_str().unwrap())
            .collect();
        assert!(codes.contains(&expected), "{index}: {validation}");
        assert!(!codes.contains(&absent), "{index}: {validation}");
        if index == 1 {
            assert_eq!(
                codes
                    .iter()
                    .filter(|code| **code == "password_disabled")
                    .count(),
                1
            );
        }
        let saved: Value = client
            .post(format!("{base}/api/pages/"))
            .bearer_auth("synthetic-bootstrap-token")
            .json(&json!({"name":format!("Warning {index}"),"html":html}))
            .send()
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .unwrap();
        assert_eq!(saved["html"], html);
        assert_eq!(saved["warnings"], validation["warnings"]);
        assert!(saved.get("capture_passwords").is_none());
        assert_eq!(
            read_warnings("/api/util/preview")["warnings"],
            validation["warnings"]
        );
    }
    let good: Value = client.post(format!("{base}/api/pages/validate")).bearer_auth("synthetic-bootstrap-token")
        .json(&json!({"html":"<!doctype html><html><head><title>Sign in</title></head><body><form id=f><fieldset disabled><legend><input type=password name=p value=prefilled readonly></legend></fieldset><button>Submit</button></form><input form=f type=password name=external autocomplete=current-password></body></html>"}))
        .send().unwrap().error_for_status().unwrap().json().unwrap();
    assert!(good["warnings"].as_array().unwrap().is_empty(), "{good}");
    process.stop();
}

#[test]
fn ownership_and_concurrent_writes_hold_through_real_http_requests() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = ready(&mut process);
    unlock(directory.path());
    let client = client();
    let response = client
        .post(format!("{base}/api/users/"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(
            &json!({"username":"other-owner","password":"other-owner-test-password","role":"user"}),
        )
        .send()
        .unwrap();
    assert_eq!(response.status(), 200);
    let user: Value = response.json().unwrap();
    let key = user["api_key"].as_str().unwrap();
    for (family, value) in [
        ("groups", group("Private", "Alice")),
        ("templates", json!({"name":"Private","text":"Hello"})),
        ("pages", json!({"name":"Private","html":"<p>private</p>"})),
        (
            "smtp",
            json!({"name":"Private","host":"localhost:25","from_address":"sender@example.test","password":"must-not-leak"}),
        ),
    ] {
        let created = client
            .post(format!("{base}/api/{family}/"))
            .bearer_auth("synthetic-bootstrap-token")
            .json(&value)
            .send()
            .unwrap();
        assert_eq!(created.status(), 201);
        let saved: Value = created.json().unwrap();
        let path = format!("{base}/api/{family}/{}", saved["id"]);
        assert_eq!(
            client.get(&path).bearer_auth(key).send().unwrap().status(),
            404
        );
        assert_eq!(
            client
                .put(&path)
                .bearer_auth(key)
                .json(&saved)
                .send()
                .unwrap()
                .status(),
            404
        );
        assert_eq!(
            client
                .delete(&path)
                .bearer_auth(key)
                .send()
                .unwrap()
                .status(),
            404
        );
        let list: Value = client
            .get(format!("{base}/api/{family}/"))
            .bearer_auth(key)
            .send()
            .unwrap()
            .json()
            .unwrap();
        assert_eq!(list, json!([]));
        // Naming scopes are owner-specific, including credentials and memberships.
        assert_eq!(
            client
                .post(format!("{base}/api/{family}/"))
                .bearer_auth(key)
                .json(&value)
                .send()
                .unwrap()
                .status(),
            201
        );
    }
    let statuses = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let client = client.clone();
                let base = &base;
                scope.spawn(move || {
                    client
                        .post(format!("{base}/api/groups/"))
                        .bearer_auth("synthetic-bootstrap-token")
                        .json(&group("Concurrent unique name", "Alice"))
                        .send()
                        .unwrap()
                        .status()
                        .as_u16()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(statuses.iter().filter(|s| **s == 201).count(), 1);
    assert_eq!(statuses.iter().filter(|s| **s == 409).count(), 7);
    let response = client
        .get(format!("{base}/api/groups/1"))
        .bearer_auth("synthetic-bootstrap-token")
        .send()
        .unwrap();
    let revision = response.headers()["etag"].to_str().unwrap().to_owned();
    let value: Value = response.json().unwrap();
    let statuses = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..2)
            .map(|index| {
                let client = client.clone();
                let base = &base;
                let revision = &revision;
                let mut value = value.clone();
                value["name"] = json!(format!("Concurrent edit {index}"));
                scope.spawn(move || {
                    client
                        .put(format!("{base}/api/groups/1"))
                        .bearer_auth("synthetic-bootstrap-token")
                        .header("if-match", revision)
                        .json(&value)
                        .send()
                        .unwrap()
                        .status()
                        .as_u16()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(statuses.iter().filter(|s| **s == 200).count(), 1);
    assert_eq!(statuses.iter().filter(|s| **s == 412).count(), 1);
    process.stop();
}

#[test]
fn site_import_rechecks_redirect_destinations_and_rejects_untrusted_tls() {
    let directory = workspace();
    let mut config = configuration();
    config["admin_server"]["use_tls"] = json!(true);
    std::fs::write(
        directory.path().join("config.json"),
        serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();
    let mut process = Process::start(directory.path(), "admin");
    let base = format!("https://{}", process.ready()["admin"].as_str().unwrap());
    unlock(directory.path());
    let client = Client::builder()
        .no_proxy()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap();
    let response = client
        .post(format!("{base}/api/import/site"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(&json!({"url":format!("{base}/login")}))
        .send()
        .unwrap();
    assert_eq!(response.status(), 400);
    assert!(response.text().unwrap().contains("TLS"));
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let thread = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut request = [0; 4096];
        let _ = stream.read(&mut request);
        stream.write_all(b"HTTP/1.1 302 Found\r\nLocation: http://169.254.169.254/metadata\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
    });
    let response = client
        .post(format!("{base}/api/import/site"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(&json!({"url":format!("http://{address}/start")}))
        .send()
        .unwrap();
    assert_eq!(response.status(), 400);
    assert!(response.text().unwrap().contains("denied"));
    thread.join().unwrap();
    process.stop();
}

#[test]
fn legacy_site_import_allow_deny_and_base_href_cases_use_controlled_servers() {
    for hosts in [vec![], vec!["192.168.1.1"], vec!["127.0.0.1"]] {
        let directory = workspace();
        let mut config = configuration();
        config["admin_server"]["allowed_internal_hosts"] = json!(hosts);
        std::fs::write(
            directory.path().join("config.json"),
            serde_json::to_vec(&config).unwrap(),
        )
        .unwrap();
        let mut process = Process::start(directory.path(), "admin");
        let base = ready(&mut process);
        unlock(directory.path());
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let allowed = hosts.first() == Some(&"127.0.0.1");
        let target = format!("http://{address}/fixture");
        let body = "<html><head></head><body><img src=\"/test.png\"/></body></html>";
        let handle = allowed.then(||std::thread::spawn(move||{
            let (mut stream,_) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
            let mut request=[0;4096];
            assert!(stream.read(&mut request).unwrap() > 0);
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        }));
        let response = client()
            .post(format!("{base}/api/import/site"))
            .bearer_auth("synthetic-bootstrap-token")
            .json(&json!({"url":target,"include_resources":true}))
            .send()
            .unwrap();
        if allowed {
            assert_eq!(response.status(), 200);
            let result: Value = response.json().unwrap();
            let html = result["html"].as_str().unwrap();
            assert!(html.contains(&format!("<head><base href=\"{target}\">")));
            assert!(html.contains("src=\"/test.png\""));
            handle.unwrap().join().unwrap();
        } else {
            assert_eq!(response.status(), 400);
            assert!(
                response
                    .text()
                    .unwrap()
                    .to_lowercase()
                    .contains("upstream connection denied")
            );
        }
        let denied = client()
            .post(format!("{base}/api/import/site"))
            .bearer_auth("synthetic-bootstrap-token")
            .json(&json!({"url":"http://169.254.169.254/latest/meta-data/"}))
            .send()
            .unwrap();
        assert_eq!(denied.status(), 400);
        assert!(
            denied
                .text()
                .unwrap()
                .to_lowercase()
                .contains("upstream connection denied")
        );
        process.stop();
    }
}
