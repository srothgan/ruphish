//! Executable campaign contracts, real SMTP effects, crash recovery and privacy.
use super::*;
#[path = "campaign_draft_process.rs"]
mod campaign_draft_process;
#[path = "proxy_process.rs"]
mod proxy_process;
use std::{
    io::{BufRead, BufReader},
    net::TcpListener,
    sync::mpsc,
};

fn sql(directory: &Path, query: &str) {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let pool = sqlx::SqlitePool::connect(&format!(
            "sqlite:{}",
            directory.join("installation.sqlite3").display()
        ))
        .await
        .unwrap();
        sqlx::raw_sql(sqlx::AssertSqlSafe(query))
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
    });
}
fn start(directory: &Workspace, options: &[&str]) -> (Process, String) {
    start_verified(directory, options, |_| {
        sql(directory, "UPDATE users SET password_change_required=0");
    })
}

pub(super) struct CampaignNetwork {
    network: domain_network::Network,
    upstream: Arc<Mutex<Option<String>>>,
}

fn start_with_network(directory: &Workspace, options: &[&str]) -> (Process, String) {
    let fixture = directory.network.get_or_init(|| {
        let upstream = Arc::new(Mutex::new(None::<String>));
        let destination = upstream.clone();
        let network = domain_network::Network::new(move |host, path| {
            let address = destination.lock().unwrap().clone()?;
            let response = client()
                .get(format!("http://{address}{path}"))
                .header("x-forwarded-host", host)
                .header("x-forwarded-proto", "https")
                .send()
                .ok()?;
            response
                .status()
                .is_success()
                .then(|| response.text().ok())
                .flatten()
        });
        CampaignNetwork { network, upstream }
    });
    let network = &fixture.network;
    std::fs::write(directory.join("verification-ca.pem"), &network.certificate).unwrap();
    let mut config: Value =
        serde_json::from_slice(&std::fs::read(directory.join("config.json")).unwrap()).unwrap();
    config["domain_verification"] = json!({"resolvers":[network.resolver.to_string()],"allowed_networks":[format!("{}/32",network.address.ip())],"trusted_ca_path":"verification-ca.pem","trusted_proxies":["127.0.0.1/32"]});
    write_config(directory, &config);
    let mut process = Process::with_options(
        directory,
        "all",
        Some("synthetic-bootstrap-password"),
        Some("synthetic-bootstrap-token"),
        options,
    );
    let ready = process.ready();
    let base = format!("http://{}", ready["admin"].as_str().unwrap());
    *fixture.upstream.lock().unwrap() = Some(ready["phish"].as_str().unwrap().into());
    (process, base)
}

pub(super) fn start_verified(
    directory: &Workspace,
    options: &[&str],
    prepare: impl FnOnce(&str),
) -> (Process, String) {
    let (process, base) = start_with_network(directory, options);
    let network = &directory.network.get().unwrap().network;
    prepare(&base);
    let settings = read(&base, "/api/settings");
    let mut policy = settings["policy"].clone();
    policy["verification_enabled"] = json!(true);
    policy["webhooks_enabled"] = json!(true);
    client()
        .put(format!("{base}/api/settings"))
        .bearer_auth("synthetic-bootstrap-token")
        .header("if-match", settings["revision"].to_string())
        .json(&json!({"policy":policy}))
        .send()
        .unwrap()
        .error_for_status()
        .unwrap();
    for host in ["training.example.test", "example.test"] {
        let items = read(&base, "/api/domains/");
        let existing = items
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["hostname"] == host)
            .cloned();
        let domain = existing.unwrap_or_else(|| {
            post(
                &base,
                "/api/domains/",
                &json!({"name":host,"origin":network.origin(host),"enabled":true,"revision":null}),
            )
        });
        network.publish(
            host,
            domain["txt_value"]
                .as_str()
                .unwrap()
                .strip_prefix("ruphish-verification=")
                .unwrap(),
        );
        let id = domain["id"].as_i64().unwrap();
        post(&base, &format!("/api/domains/{id}/verify"), &json!({}));
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            let current = read(&base, &format!("/api/domains/{id}"));
            if current["eligible"] == true {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "Domain verification failed: {current}"
            );
            let guard = process.output.0.lock().unwrap();
            drop(
                process
                    .output
                    .1
                    .wait_timeout(guard, Duration::from_millis(50))
                    .unwrap(),
            );
        }
    }
    (process, base)
}
fn post(base: &str, path: &str, body: &Value) -> Value {
    let response = client()
        .post(format!("{base}{path}"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(body)
        .send()
        .unwrap();
    let status = response.status();
    let value: Value = response.json().unwrap();
    assert!(status.is_success(), "{status}: {value}");
    value
}
fn read(base: &str, path: &str) -> Value {
    let value: Value = client()
        .get(format!("{base}{path}"))
        .bearer_auth("synthetic-bootstrap-token")
        .send()
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .unwrap();
    let route = path.split('?').next().unwrap_or(path);
    if route.ends_with("/delivery") {
        let snapshot: ruphish_contracts::campaign_reads::Delivery =
            serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(snapshot).unwrap(), value);
    } else if route.ends_with("/history") {
        let snapshot: ruphish_contracts::campaign_reads::History =
            serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(snapshot).unwrap(), value);
    }
    value
}
fn normalize(value: &mut Value) {
    match value {
        Value::Array(rows) => rows.iter_mut().for_each(normalize),
        Value::Object(object) => {
            for (key, value) in object.iter_mut() {
                if matches!(key.as_str(), "created_date" | "modified_date" | "time")
                    || (key == "completed_date" && !value.is_null())
                {
                    *value = json!("<time>");
                } else if matches!(key.as_str(), "latitude" | "longitude") {
                    *value = json!(value.as_f64().unwrap());
                } else if key == "id" && value.is_string() {
                    *value = json!("<rid>");
                } else {
                    normalize(value);
                }
            }
        }
        _ => {}
    }
}
#[test]
fn captured_go_campaign_api_cases_run_against_the_new_executable() {
    let directory = workspace();
    let (mut process, base) = start(&directory, &["--disable-mailer"]);
    let cases: Vec<Value> =
        serde_json::from_slice(include_bytes!("fixtures/campaigns/http.json")).unwrap();
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let path = case["path"].as_str().unwrap();
        let mut body = case["body"].as_str().unwrap().to_owned();
        if path == "/api/campaigns/"
            && case["method"] == "POST"
            && let Ok(mut input) = serde_json::from_str::<Value>(&body)
        {
            // DEC-055: translate the historical name-only fixture into native IDs.
            // Fixture resources have independent fixed IDs; missing inputs stay incomplete.
            let reference =
                |value: &Value| json!({"id": if value["name"].is_string() { 1 } else { 0 }});
            let groups = input["groups"]
                .as_array()
                .map(|groups| {
                    groups
                        .iter()
                        .map(|g| json!({"id": if g["name"] == "Overlap" { 2 } else { 1 }}))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            input["selection"] = json!({"groups":groups,"template":reference(&input["template"]),"page":reference(&input["page"]),"smtp":reference(&input["smtp"])});
            if input["name"].is_null() {
                input["name"] = json!("");
            }
            if input["url"].is_null() {
                input["url"] = json!("");
            }
            for key in ["groups", "template", "page", "smtp"] {
                input.as_object_mut().unwrap().remove(key);
            }
            body = serde_json::to_string(&input).unwrap();
        }
        let response = client()
            .request(
                // DEC-057 retains the workflow guarantee through POST only.
                if path.ends_with("/complete") {
                    reqwest::Method::POST
                } else {
                    case["method"].as_str().unwrap().parse().unwrap()
                },
                format!("{base}{path}"),
            )
            .bearer_auth("synthetic-bootstrap-token")
            .header("content-type", "application/json")
            // Recorded response expectations remain independent of native request adaptation.
            .body(body.replace(
                "https://training.example.test",
                &origin(&base, "training.example.test"),
            ))
            .send()
            .unwrap();
        assert_eq!(
            response.status().as_u16(),
            case["status"].as_u64().unwrap() as u16,
            "{id}"
        );
        let mut actual: Value = response.json().unwrap();
        if (200..300).contains(&case["status"].as_u64().unwrap())
            && path.starts_with("/api/campaigns/")
            && actual.get("success").is_none()
        {
            use ruphish_contracts::campaign_details as wire;
            fn round_trip<T: serde::de::DeserializeOwned + serde::Serialize>(value: &Value) {
                let decoded: T = serde_json::from_value(value.clone()).unwrap();
                assert_eq!(serde_json::to_value(decoded).unwrap(), *value);
            }
            match path {
                "/api/campaigns/summary" => round_trip::<wire::Summaries>(&actual),
                path if path.ends_with("/summary") => round_trip::<wire::Summary>(&actual),
                path if path.ends_with("/results") => round_trip::<wire::Results>(&actual),
                "/api/campaigns/" if actual.is_array() => {
                    round_trip::<Vec<wire::Campaign>>(&actual)
                }
                _ => round_trip::<wire::Campaign>(&actual),
            }
        }
        actual = serde_json::from_str(&serde_json::to_string(&actual).unwrap().replace(
            &origin(&base, "training.example.test"),
            "https://training.example.test",
        ))
        .unwrap();
        // Assert the additive error code without changing the recorded Go response.
        let expected_code = match id {
            "missing" | "deleted" => Some("RESOURCE_NOT_FOUND"),
            "invalid-json" => Some("REQUEST_INVALID"),
            "missing-name" | "missing-groups" | "missing-template" | "missing-page"
            | "missing-profile" | "date-order" => Some("RESOURCE_INVALID"),
            _ => None,
        };
        if let Some(expected_code) = expected_code {
            let code = actual.as_object_mut().unwrap().remove("code");
            assert_eq!(code, Some(json!(expected_code)), "Go case {id}");
        }
        // Field reports are additive; preserve every independent Go expectation below.
        let required_fields = [
            "name",
            "selection.groups",
            "selection.template.id",
            "selection.page.id",
            "selection.smtp.id",
        ];
        let expected_fields: &[&str] = match id {
            "missing-name" => &required_fields,
            "missing-groups" => &required_fields[1..],
            "missing-template" => &required_fields[2..],
            "missing-page" => &required_fields[3..],
            "missing-profile" => &required_fields[4..],
            "date-order" => &["send_by_local.local"],
            _ => &[],
        };
        if !expected_fields.is_empty() {
            let reason = if id == "date-order" {
                "schedule_order"
            } else {
                "required"
            };
            let issues: Vec<_> = expected_fields
                .iter()
                .map(|field| json!({"field":field,"code":reason}))
                .collect();
            assert_eq!(
                actual.as_object_mut().unwrap().remove("validation"),
                Some(json!({"issues":issues})),
                "Go case {id} field report"
            );
        }
        let mut expected = case["json"].clone();
        if id == "page-create" {
            // Diagnostics are additive; their behavior has dedicated HTTP coverage.
            assert!(actual["warnings"].is_array());
            actual.as_object_mut().unwrap().remove("warnings");
        }
        // Landing pages retain authored HTML. Go's wrapper insertion is incidental formatting.
        fn exceptions(v: &mut Value) {
            match v {
                Value::Array(rows) => rows.iter_mut().for_each(exceptions),
                Value::Object(o) => {
                    o.remove("capture_credentials");
                    o.remove("capture_passwords");
                    if o.contains_key("host") {
                        let password = o.remove("password");
                        o.insert(
                            "has_password".into(),
                            json!(
                                password
                                    .as_ref()
                                    .and_then(Value::as_str)
                                    .is_some_and(|v| !v.is_empty())
                            ),
                        );
                    }
                    for (k, v) in o {
                        if matches!(k.as_str(), "targets" | "attachments" | "headers")
                            && v.is_null()
                        {
                            *v = json!([]);
                        } else if v == "0001-01-01T00:00:00Z" {
                            *v = Value::Null;
                        } else if k == "html"
                            && v == "<html><head></head><body><p>Training</p></body></html>"
                        {
                            *v = json!("<p>Training</p>");
                        } else if k == "send_date" && v == "2035-01-01T12:03:00Z" {
                            *v = json!("2035-01-01T12:05:00Z");
                        } else {
                            exceptions(v)
                        }
                    }
                }
                _ => {}
            }
        }
        exceptions(&mut expected);
        normalize(&mut expected);
        normalize(&mut actual);
        assert_eq!(actual, expected, "Go case {id}");
    }
    process.stop();
}
pub(super) struct Probe {
    pub address: String,
    pub received: mpsc::Receiver<String>,
    pub reply: mpsc::Sender<Option<&'static str>>,
    pub thread: std::thread::JoinHandle<()>,
}
fn probe() -> Probe {
    probe_many(1)
}
pub(super) fn probe_many(count: usize) -> Probe {
    probe_auth(count, false)
}
fn probe_auth(count: usize, cram: bool) -> Probe {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let (sent, received) = mpsc::channel();
    let (reply, answer) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        'connections: for _ in 0..count {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(15)))
                .unwrap();
            let mut io = BufReader::new(stream);
            let mut challenge = false;
            let mut authenticated = !cram;
            io.get_mut().write_all(b"220 local ESMTP\r\n").unwrap();
            loop {
                let mut line = String::new();
                if io.read_line(&mut line).unwrap_or(0) == 0 {
                    continue 'connections;
                }
                let response = if challenge {
                    assert_eq!(
                        line.trim_end(),
                        "dGltIGI5MTNhNjAyYzdlZGE3YTQ5NWI0ZTZlNzMzNGQzODkw"
                    );
                    challenge = false;
                    authenticated = true;
                    "235 Accepted\r\n"
                } else if line.starts_with("EHLO") {
                    if cram {
                        "250-local\r\n250-8BITMIME\r\n250 AUTH CRAM-MD5 PLAIN\r\n"
                    } else {
                        "250-local\r\n250 8BITMIME\r\n"
                    }
                } else if cram && line == "AUTH CRAM-MD5\r\n" {
                    challenge = true;
                    "334 PDE4OTYuNjk3MTcwOTUyQHBvc3RvZmZpY2UucmVzdG9uLm1jaS5uZXQ+\r\n"
                } else if line.starts_with("DATA") {
                    assert!(authenticated);
                    io.get_mut().write_all(b"354 Continue\r\n").unwrap();
                    let mut body = String::new();
                    loop {
                        let mut line = String::new();
                        if io.read_line(&mut line).unwrap_or(0) == 0 {
                            continue 'connections;
                        }
                        if line == ".\r\n" {
                            break;
                        }
                        body.push_str(&line);
                    }
                    sent.send(body).unwrap();
                    match answer.recv_timeout(Duration::from_secs(20)).unwrap() {
                        Some(reply) => reply,
                        None => continue 'connections,
                    }
                } else if line.starts_with("QUIT") {
                    "221 Bye\r\n"
                } else {
                    "250 OK\r\n"
                };
                if io.get_mut().write_all(response.as_bytes()).is_err() {
                    continue 'connections;
                }
                if line.starts_with("QUIT") {
                    continue 'connections;
                }
            }
        }
    });
    Probe {
        address,
        received,
        reply,
        thread,
    }
}
pub(super) fn origin(base: &str, host: &str) -> String {
    read(base, "/api/domains/")
        .as_array()
        .unwrap()
        .iter()
        .find(|domain| domain["hostname"] == host)
        .unwrap()["origin"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn reviewed_selection(review: &Value) -> Value {
    let select = |r: &Value| json!({"id":r["id"],"revision":r["revision"]});
    let selections = &review["selection"];
    json!({"groups":selections["groups"].as_array().unwrap().iter().map(select).collect::<Vec<_>>(),"template":select(&selections["template"]),"page":select(&selections["page"]),"smtp":select(&selections["smtp"])})
}
fn resources(base: &str, address: &str) -> Value {
    let group = post(
        base,
        "/api/groups/",
        &json!({"name":"Audience","targets":[{"email":"alice@example.test","first_name":"Alice","last_name":"Example"}]}),
    );
    let template = post(
        base,
        "/api/templates/",
        &json!({"name":"Mail","subject":"Hello {{.FirstName}}","text":"Training link: {{.URL}}"}),
    );
    let page = post(
        base,
        "/api/pages/",
        &json!({"name":"Page","html":"<form><input name='username'><input name='password' type='password'></form>"}),
    );
    let smtp = post(
        base,
        "/api/smtp/",
        &json!({"name":"Local","host":address,"from_address":"Training <training@example.test>"}),
    );
    json!({"name":"Training","selection":{"groups":[{"id":group["id"]}],"template":{"id":template["id"]},"page":{"id":page["id"]},"smtp":{"id":smtp["id"]}},"url":origin(base, "training.example.test")})
}
fn wait_state(base: &str, id: i64, state: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let value = read(base, &format!("/api/campaigns/{id}/delivery"));
        if value["counts"][state].as_i64().unwrap_or(0) > 0 {
            return value;
        }
        assert!(Instant::now() < deadline, "Expected {state}: {value}");
        std::thread::sleep(Duration::from_millis(20));
    }
}
#[test]
fn executable_launch_delivers_personalized_mail_once_and_persists_acceptance_across_restart() {
    let directory = workspace();
    let (mut process, base) = start(&directory, &[]);
    let smtp = probe_auth(1, true);
    let mut body = resources(&base, &smtp.address);
    let mut profile = read(&base, "/api/smtp/1");
    profile["username"] = json!("tim");
    profile["password"] = json!("tanstaaftanstaaf");
    assert_eq!(
        client()
            .put(format!("{base}/api/smtp/1"))
            .bearer_auth("synthetic-bootstrap-token")
            .json(&profile)
            .send()
            .unwrap()
            .status(),
        200
    );
    // Exercise managed tracking through the real queued-delivery worker, with no
    // authored tracker and a fragment rather than a pre-wrapped HTML document.
    let response = client().put(format!("{base}/api/templates/1"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(&json!({"id":1,"name":"Mail","subject":"Hello {{.FirstName}}","text":"Training link: {{.URL}}","html":"<p>Hello {{.FirstName}} <a href='{{.URL}}'>Open</a></p>"}))
        .send().unwrap();
    assert_eq!(response.status(), 200);
    // Unknown input must not leak into the persistent launch fingerprint.
    body["unused"] = json!({"password":"synthetic-launch-secret-canary"});
    let rejected = client()
        .post(format!("{base}/api/campaigns/"))
        .bearer_auth("synthetic-bootstrap-token")
        .header("idempotency-key", "process-launch-1")
        .json(&body)
        .send()
        .unwrap();
    assert_eq!(rejected.status(), 400);
    assert!(
        !rejected
            .text()
            .unwrap()
            .contains("synthetic-launch-secret-canary")
    );
    body.as_object_mut().unwrap().remove("unused");
    let launch = || {
        client()
            .post(format!("{base}/api/campaigns/"))
            .bearer_auth("synthetic-bootstrap-token")
            .header("idempotency-key", "process-launch-1")
            .json(&body)
            .send()
            .unwrap()
            .error_for_status()
            .unwrap()
            .json::<Value>()
            .unwrap()
    };
    let campaign = launch();
    let id = campaign["id"].as_i64().unwrap();
    assert_eq!(launch()["id"], id);
    let mail = smtp.received.recv_timeout(Duration::from_secs(15)).unwrap();
    assert!(mail.contains("Subject: Hello Alice"));
    assert!(mail.contains("alice@example.test"));
    assert!(mail.contains(&format!("{}?rid=", origin(&base, "training.example.test"))));
    assert!(!mail.contains("synthetic-launch-secret-canary"));
    let parsed = mail_parser::MessageParser::default()
        .parse(mail.as_bytes())
        .unwrap();
    let html = parsed.body_html(0).unwrap();
    assert!(html.contains("Hello Alice"));
    assert_eq!(html.matches("<img ").count(), 1);
    assert!(html.contains(&format!(
        "src=\"{}/track?rid=",
        origin(&base, "training.example.test")
    )));
    assert!(html.contains("style=\"display:none\""));
    assert!(!html.contains("{{.Tracker}}"));
    smtp.reply.send(Some("250 Accepted\r\n")).unwrap();
    smtp.thread.join().unwrap();
    let state = wait_state(&base, id, "accepted");
    assert_eq!(state["items"][0]["attempts"].as_array().unwrap().len(), 1);
    let recipient = state["items"][0]["id"].as_i64().unwrap();
    let history = read(
        &base,
        &format!("/api/campaigns/{id}/delivery/{recipient}/history"),
    );
    assert_eq!(history["attempts"].as_array().unwrap().len(), 1);
    assert_eq!(history["attempts"][0]["state"], "accepted");
    assert!(history["attempts"][0]["finished"].is_number());
    assert!(mail.contains(history["attempts"][0]["message_id"].as_str().unwrap()));
    assert_eq!(history["events"][0]["message"], "Email Sent");
    assert!(history["events"][0]["at"].is_number());
    assert_eq!(
        read(&base, &format!("/api/campaigns/{id}/summary"))["stats"]["sent"],
        1
    );
    process.stop();
    let (mut restarted, base) = start(&directory, &[]);
    assert_eq!(
        read(&base, &format!("/api/campaigns/{id}/delivery"))["counts"]["accepted"],
        1
    );
    restarted.stop();
    let disk = std::fs::read(directory.path().join("installation.sqlite3")).unwrap();
    assert!(
        !disk
            .windows(b"synthetic-launch-secret-canary".len())
            .any(|w| w == b"synthetic-launch-secret-canary")
    );
}
#[test]
fn killed_after_data_is_unknown_after_restart_and_requires_explicit_reconciliation() {
    let directory = workspace();
    let (mut process, base) = start(&directory, &[]);
    let smtp = probe();
    let body = resources(&base, &smtp.address);
    let campaign = post(&base, "/api/campaigns/", &body);
    let id = campaign["id"].as_i64().unwrap();
    smtp.received.recv_timeout(Duration::from_secs(15)).unwrap();
    process.crash();
    smtp.reply.send(None).unwrap();
    smtp.thread.join().unwrap();
    sql(
        directory.path(),
        "UPDATE ruphish_delivery SET lease_until=0",
    );
    let (mut restarted, base) = start(&directory, &[]);
    let state = wait_state(&base, id, "unknown");
    let row = &state["items"][0];
    assert_eq!(row["code"], "delivery_outcome_unknown");
    post(
        &base,
        &format!("/api/campaigns/{id}/delivery/{}", row["id"]),
        &json!({"generation":row["generation"],"action":"confirm_accepted"}),
    );
    wait_state(&base, id, "accepted");
    restarted.stop();
}
#[test]
fn executable_campaign_reads_never_return_legacy_participant_credentials() {
    let directory = workspace();
    std::fs::copy(
        fixture("campaigns/queued.sqlite3"),
        directory.path().join("installation.sqlite3"),
    )
    .unwrap();
    sql(
        directory.path(),
        // This independently recorded fixture still has the pre-upgrade schema.
        "UPDATE events SET details='{\"payload\":{\"username\":\"private-login-canary\",\"password\":\"private-login-canary\"},\"error\":\"private-login-canary\"}'",
    );
    // Reading migrated campaigns must not require verifying or changing their original URLs.
    let (mut process, base) = start_with_network(&directory, &["--disable-mailer"]);
    sql(&directory, "UPDATE users SET password_change_required=0");
    for path in [
        "/api/campaigns/",
        "/api/campaigns/1",
        "/api/campaigns/1/results",
        "/api/campaigns/1/delivery",
    ] {
        let value = read(&base, path).to_string();
        assert!(!value.contains("private-login-canary"));
        assert!(!value.contains("payload"));
    }
    process.stop();
    assert!(!process.output().contains("private-login-canary"));
    let log = std::fs::read_to_string(directory.path().join("server.log")).unwrap();
    assert!(!log.contains("private-login-canary"));
}

#[test]
fn debug_and_trace_logs_exclude_credentials_from_every_request_location_and_smtp_reply() {
    for level in ["debug", "trace"] {
        let directory = workspace();
        let mut config = configuration();
        config["logging"]["level"] = json!(level);
        write_config(directory.path(), &config);
        let (mut process, base) = start(&directory, &[]);
        let canary = "synthetic-secret-log-canary-994";
        let public = format!("http://{}", process.ready()["phish"].as_str().unwrap());
        for origin in [&base, &public] {
            for path in [
                format!("/{canary}?password={canary}"),
                format!("/api/campaigns/{canary}?api_key={canary}"),
            ] {
                let response = client()
                    .post(format!("{origin}{path}"))
                    .header("authorization", format!("Bearer {canary}"))
                    .header("cookie", format!("session={canary}"))
                    .header("user-agent", canary)
                    .header("referer", format!("https://example.test/{canary}"))
                    .body(format!("username={canary}&password={canary}"))
                    .send()
                    .unwrap();
                assert!(!response.text().unwrap().contains(canary));
            }
        }
        for bytes in [
            format!(r#"{{"name":["{canary}"],"password":"{canary}"}}"#),
            format!(r#"{{"name":"{canary}""#),
        ] {
            let response = client()
                .post(format!("{base}/api/campaigns/"))
                .bearer_auth("synthetic-bootstrap-token")
                .header("content-type", "application/json")
                .body(bytes)
                .send()
                .unwrap();
            assert_eq!(response.status(), 400);
            assert!(!response.text().unwrap().contains(canary));
        }
        // Extended methods are request data too, never a safe log field.
        let _ = client()
            .request(canary.parse().unwrap(), format!("{base}/"))
            .send()
            .unwrap();
        let smtp = probe();
        let body = resources(&base, &smtp.address);
        let created = post(&base, "/api/campaigns/", &body);
        smtp.received.recv_timeout(Duration::from_secs(15)).unwrap();
        smtp.reply
            .send(Some("550 synthetic-secret-log-canary-994 rejected\r\n"))
            .unwrap();
        smtp.thread.join().unwrap();
        let state = wait_state(&base, created["id"].as_i64().unwrap(), "failed");
        assert!(!state.to_string().contains(canary));
        let results = read(&base, &format!("/api/campaigns/{}/results", created["id"]));
        assert!(!results.to_string().contains(canary));
        process.stop();
        let log = std::fs::read_to_string(directory.path().join("server.log")).unwrap();
        assert!(log.contains("http_request"));
        assert!(log.contains("/api/campaigns/{id}"));
        assert!(!log.contains(canary), "secret appeared in {level} log");
        assert!(!process.output().contains(canary));
        let disk = std::fs::read(directory.path().join("installation.sqlite3")).unwrap();
        assert!(
            !disk
                .windows(canary.len())
                .any(|part| part == canary.as_bytes())
        );
    }
}

#[test]
fn startup_database_diagnostics_and_cli_arguments_cannot_leak_credentials() {
    let secret = "synthetic-startup-secret-canary-991";
    let directory = workspace();
    let (mut process, _) = start(&directory, &["--disable-mailer"]);
    process.stop();
    sql(
        directory.path(),
        "UPDATE users SET password_change_required=1; CREATE TRIGGER fail_bootstrap BEFORE UPDATE ON users BEGIN SELECT RAISE(ABORT,'synthetic-startup-secret-canary-991'); END;",
    );
    let mut rejected = Process::start(&directory, "admin");
    rejected.fail();
    assert!(!rejected.output().contains(secret));
    assert!(
        !std::fs::read_to_string(directory.path().join("server.log"))
            .unwrap()
            .contains(secret)
    );
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ruphish"))
        .args(["--mode", secret])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stderr).contains(secret));
}

#[test]
fn reviewed_campaign_preview_test_and_duplicate_use_exact_owned_resources_without_launching() {
    let directory = workspace();
    let (mut process, base) = start(&directory, &["--disable-mailer"]);
    let smtp = probe();
    let mut input = resources(&base, &smtp.address);
    sql(
        directory.path(),
        "UPDATE pages SET redirect_url='https://training.example.test/finish'",
    );
    let inherited = post(&base, "/api/campaigns/prepare", &input);
    let page_url = format!("/api/pages/{}", inherited["selection"]["page"]["id"]);
    let defaults = read(&base, &page_url);
    assert!(defaults.get("capture_credentials").is_none());
    assert!(defaults.get("capture_passwords").is_none());
    assert_eq!(
        defaults["redirect_url"],
        "https://training.example.test/finish"
    );
    assert!(inherited["policy"].get("capture_passwords").is_none());
    for field in ["capture_credentials", "capture_passwords"] {
        let mut retired = input.clone();
        retired["policy"] = json!({field:true});
        let rejected = client()
            .post(format!("{base}/api/campaigns/prepare"))
            .bearer_auth("synthetic-bootstrap-token")
            .json(&retired)
            .send()
            .unwrap();
        assert_eq!(rejected.status(), 400);
    }
    // An explicit empty redirect clears the page default.
    input["policy"] = json!({"redirect_url":""});
    let review = post(&base, "/api/campaigns/prepare", &input);
    assert_eq!(review["policy"], json!({"redirect_url":""}));
    assert!(review["warnings"].is_array());
    assert_eq!(review["total"], 1);
    assert!(
        review["preview"]["subject"]
            .as_str()
            .unwrap()
            .contains("Alice")
    );
    assert!(
        !review.to_string().contains("password")
            || !review.to_string().contains("synthetic-bootstrap-password")
    );
    let mut campaign = input.clone();
    campaign["selection"] = reviewed_selection(&review);
    sql(
        directory.path(),
        "UPDATE templates SET modified_date='2032-01-01T00:00:00Z'",
    );
    let stale = client()
        .post(format!("{base}/api/campaigns/test"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(&json!({"campaign":campaign,"email":"test@example.test"}))
        .send()
        .unwrap();
    assert_eq!(stale.status(), 412);
    let review = post(&base, "/api/campaigns/prepare", &input);
    campaign["selection"] = reviewed_selection(&review);
    let endpoint = base.clone();
    let request = campaign.clone();
    let send = std::thread::spawn(move || {
        post(
            &endpoint,
            "/api/campaigns/test",
            &json!({"campaign":request,"email":"test@example.test"}),
        )
    });
    let received = smtp.received.recv_timeout(Duration::from_secs(15)).unwrap();
    assert!(received.contains("test@example.test"));
    assert!(received.contains("Sample"));
    smtp.reply.send(Some("250 Accepted\r\n")).unwrap();
    smtp.thread.join().unwrap();
    assert_eq!(send.join().unwrap()["success"], true);
    assert!(
        read(&base, "/api/campaigns/")
            .as_array()
            .unwrap()
            .is_empty()
    );
    let created = post(&base, "/api/campaigns/", &campaign);
    assert!(created["send_by_date"].is_null());
    assert!(created["completed_date"].is_null());
    let draft = read(&base, &format!("/api/campaigns/{}/draft", created["id"]));
    assert_eq!(draft["policy"], review["policy"]);
    assert_eq!(read(&base, &page_url), defaults);
    assert_eq!(draft["selection"]["groups"].as_array().unwrap().len(), 1);
    assert!(draft.get("id").is_none());
    assert!(draft.get("launch_date").is_none());
    assert!(draft.get("results").is_none());
    assert!(draft.get("smtp").is_none());
    process.stop();
}

#[test]
fn worker_continues_other_recipients_after_temporary_permanent_and_unknown_failures() {
    for (reply, state) in [
        (Some("451 Later\r\n"), "retry"),
        (Some("550 Rejected\r\n"), "failed"),
        (None, "unknown"),
    ] {
        let directory = workspace();
        let (mut process, base) = start(&directory, &[]);
        let smtp = probe_many(2);
        let body = resources(&base, &smtp.address);
        let update=client().put(format!("{base}/api/groups/1")).bearer_auth("synthetic-bootstrap-token").json(&json!({"id":1,"name":"Audience","targets":[{"email":"alice@example.test"},{"email":"bob@example.test"}]})).send().unwrap();
        assert!(update.status().is_success());
        let c = post(&base, "/api/campaigns/", &body);
        let id = c["id"].as_i64().unwrap();
        let first = smtp.received.recv_timeout(Duration::from_secs(15)).unwrap();
        assert!(first.contains("alice@example.test"));
        smtp.reply.send(reply).unwrap();
        let second = smtp.received.recv_timeout(Duration::from_secs(15)).unwrap();
        assert!(second.contains("bob@example.test"));
        smtp.reply.send(Some("250 Accepted\r\n")).unwrap();
        smtp.thread.join().unwrap();
        let view = wait_state(&base, id, "accepted");
        assert_eq!(view["counts"][state], 1);
        assert_eq!(view["counts"]["accepted"], 1);
        process.stop();
    }
}

#[test]
fn campaign_ownership_read_only_and_locked_owners_hold_at_real_http_boundaries() {
    let directory = workspace();
    let (mut process, base) = start(&directory, &["--disable-mailer"]);
    let body = resources(&base, "127.0.0.1:25");
    let campaign = post(&base, "/api/campaigns/", &body);
    let id = campaign["id"].as_i64().unwrap();
    let before = read(&base, &format!("/api/campaigns/{id}"));
    for method in [reqwest::Method::GET, reqwest::Method::HEAD] {
        let response = client()
            .request(method, format!("{base}/api/campaigns/{id}/complete"))
            .bearer_auth("synthetic-bootstrap-token")
            .send()
            .unwrap();
        assert_eq!(response.status(), 405);
        assert_eq!(read(&base, &format!("/api/campaigns/{id}")), before);
    }
    sql(
        directory.path(),
        "INSERT INTO users(username,hash,api_key,role_id,password_change_required,account_locked) SELECT 'second',hash,'second-owner-key',role_id,0,0 FROM users WHERE id=1;",
    );
    for path in [
        format!("/api/campaigns/{id}"),
        format!("/api/campaigns/{id}/delivery"),
        format!("/api/campaigns/{id}/draft"),
    ] {
        assert_eq!(
            client()
                .get(format!("{base}{path}"))
                .bearer_auth("second-owner-key")
                .send()
                .unwrap()
                .status(),
            404
        );
    }
    let denied = client()
        .post(format!("{base}/api/campaigns/prepare"))
        .bearer_auth("second-owner-key")
        .json(&body)
        .send()
        .unwrap();
    assert_eq!(denied.status(), 400);
    sql(
        directory.path(),
        "INSERT INTO roles(id,slug,name,description) VALUES(99,'read-only','Read only','Synthetic'); INSERT INTO role_permissions(role_id,permission_id) SELECT 99,id FROM permissions WHERE slug='view_objects'; UPDATE users SET role_id=99 WHERE id=1;",
    );
    assert_eq!(read(&base, &format!("/api/campaigns/{id}"))["id"], id);
    for (method, status) in [("GET", 405), ("POST", 403)] {
        assert_eq!(
            client()
                .request(
                    method.parse().unwrap(),
                    format!("{base}/api/campaigns/{id}/complete")
                )
                .bearer_auth("synthetic-bootstrap-token")
                .send()
                .unwrap()
                .status(),
            status
        );
    }
    assert_eq!(
        client()
            .delete(format!("{base}/api/campaigns/{id}"))
            .bearer_auth("synthetic-bootstrap-token")
            .send()
            .unwrap()
            .status(),
        403
    );
    sql(
        directory.path(),
        "UPDATE users SET account_locked=1 WHERE id=1",
    );
    assert_eq!(
        client()
            .get(format!("{base}/api/campaigns/{id}"))
            .bearer_auth("synthetic-bootstrap-token")
            .send()
            .unwrap()
            .status(),
        401
    );
    process.stop();
}

#[test]
fn campaign_commands_reject_ambiguous_json_without_creating_work() {
    let directory = workspace();
    let (mut process, base) = start(&directory, &["--disable-mailer"]);
    let input = resources(&base, "127.0.0.1:25");
    let valid = serde_json::to_string(&input).unwrap();
    let mut bodies = vec![
        "null".into(),
        "[]".into(),
        "{}".into(),
        valid.replacen("\"name\":", "\"Name\":", 1),
        valid.replacen("\"name\":", "\"name\":null,\"name\":", 1),
        valid.replacen("\"id\":", "\"id\":null,\"id\":", 1),
        format!("{valid} {{}}"),
    ];
    for (field, value) in [
        ("groups", json!([{"name":"Audience"}])),
        ("template", json!({"name":"Mail","id":1})),
        ("unexpected", json!(true)),
        ("name", Value::Null),
        ("selection", Value::Null),
    ] {
        let mut invalid = input.clone();
        invalid[field] = value;
        bodies.push(serde_json::to_string(&invalid).unwrap());
    }
    let mut named = input.clone();
    named["selection"]["template"]["name"] = json!("Mail");
    bodies.push(serde_json::to_string(&named).unwrap());
    // Both direct commands and nested commands must retain strict decoding.
    for body in bodies {
        for (path, body) in [
            ("/api/campaigns/", body.clone()),
            ("/api/campaigns/prepare", body.clone()),
            (
                "/api/campaigns/test",
                format!(r#"{{"campaign":{body},"email":"test@example.test"}}"#),
            ),
            (
                "/api/campaign-drafts/strict-command-test/launch",
                format!(r#"{{"revision":1,"campaign":{body}}}"#),
            ),
        ] {
            let response = client()
                .post(format!("{base}{path}"))
                .bearer_auth("synthetic-bootstrap-token")
                .header("content-type", "application/json")
                .body(body)
                .send()
                .unwrap();
            assert_eq!(response.status(), 400, "{path}");
        }
    }
    assert!(
        read(&base, "/api/campaigns/")
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(post(&base, "/api/campaigns/prepare", &input)["total"], 1);
    let created = post(&base, "/api/campaigns/", &input);
    assert_eq!(
        created["template"]["id"],
        input["selection"]["template"]["id"]
    );
    process.stop();
}

#[test]
fn local_campaign_times_resolve_on_server_and_survive_review_launch_and_restart() {
    let directory = workspace();
    let (mut process, base) = start(&directory, &["--disable-mailer"]);
    let input = resources(&base, "127.0.0.1:1");
    let mut saved = Vec::new();
    for (local, end, expected, expected_end) in [
        (
            "2030-03-31T02:30",
            "2030-03-31T03:45",
            "2030-03-31T01:30:00Z",
            "2030-03-31T01:45:00Z",
        ),
        (
            "2030-10-27T02:30",
            "2030-10-27T02:45",
            "2030-10-27T00:30:00Z",
            "2030-10-27T00:45:00Z",
        ),
    ] {
        let mut body = input.clone();
        body["launch_local"] = json!({"local":local,"time_zone":"Europe/Berlin"});
        body["send_by_local"] = json!({"local":end,"time_zone":"Europe/Berlin"});
        let review = post(&base, "/api/campaigns/prepare", &body);
        let utc = |value: &Value| {
            time::OffsetDateTime::from_unix_timestamp(value.as_i64().unwrap())
                .unwrap()
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap()
        };
        assert_eq!(utc(&review["launch_date"]), expected);
        assert_eq!(utc(&review["send_by_date"]), expected_end);
        // Removed prototype fields cannot silently alter or bypass schedule policy.
        body["launch_local"]["ambiguity"] = json!("later");
        let rejected = client()
            .post(format!("{base}/api/campaigns/prepare"))
            .bearer_auth("synthetic-bootstrap-token")
            .json(&body)
            .send()
            .unwrap();
        assert_eq!(rejected.status(), 400);
        body["launch_local"]
            .as_object_mut()
            .unwrap()
            .remove("ambiguity");
        let campaign = post(&base, "/api/campaigns/", &body);
        assert_eq!(campaign["launch_date"], expected);
        assert_eq!(campaign["send_by_date"], expected_end);
        let id = campaign["id"].as_i64().unwrap();
        let queue = read(&base, &format!("/api/campaigns/{id}/delivery"));
        assert_eq!(queue["items"][0]["due"], review["launch_date"]);
        assert_eq!(queue["items"][0]["state"], "ready");
        saved.push((id, expected, expected_end, review["launch_date"].clone()));
    }
    let count = read(&base, "/api/campaigns/").as_array().unwrap().len();
    for local in [
        json!({"local":"2030-02-30T12:00","time_zone":"Europe/Berlin"}),
        json!({"local":"2030-10-27T02:30","time_zone":"Unknown/Place"}),
        json!({"local":"2030-10-27T02:30+01:00","time_zone":"Europe/Berlin"}),
        json!({"local":[],"time_zone":"Europe/Berlin"}),
    ] {
        let mut body = input.clone();
        body["launch_local"] = local;
        for endpoint in ["/api/campaigns/prepare", "/api/campaigns/"] {
            let response = client()
                .post(format!("{base}{endpoint}"))
                .bearer_auth("synthetic-bootstrap-token")
                .json(&body)
                .send()
                .unwrap();
            assert_eq!(response.status(), 400);
            assert_eq!(response.json::<Value>().unwrap()["success"], false);
        }
    }
    // Gap normalization can reverse an apparently ordered wall-clock interval.
    let mut reversed = input.clone();
    reversed["launch_local"] = json!({"local":"2030-03-31T02:30","time_zone":"Europe/Berlin"});
    reversed["send_by_local"] = json!({"local":"2030-03-31T03:15","time_zone":"Europe/Berlin"});
    let response = client()
        .post(format!("{base}/api/campaigns/"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(&reversed)
        .send()
        .unwrap();
    assert_eq!(response.status(), 400);
    assert_eq!(
        read(&base, "/api/campaigns/").as_array().unwrap().len(),
        count
    );
    // Legacy offset-bearing timestamps remain exact, including the later fold occurrence.
    let mut exact = input;
    exact["launch_date"] = json!("2030-10-27T02:30:00+01:00");
    let campaign = post(&base, "/api/campaigns/", &exact);
    assert_eq!(campaign["launch_date"], "2030-10-27T01:30:00Z");
    process.stop();
    let (mut restarted, base) = start(&directory, &["--disable-mailer"]);
    for (id, expected, expected_end, due) in saved {
        let campaign = read(&base, &format!("/api/campaigns/{id}"));
        assert_eq!(campaign["launch_date"], expected);
        assert_eq!(campaign["send_by_date"], expected_end);
        let queue = read(&base, &format!("/api/campaigns/{id}/delivery"));
        assert_eq!(queue["items"][0]["due"], due);
        assert_eq!(queue["items"][0]["state"], "ready");
    }
    restarted.stop();
}

#[test]
fn repeated_campaign_review_recovers_after_validation_failure_without_creating_work() {
    let directory = workspace();
    let (mut process, base) = start(&directory, &["--disable-mailer"]);
    let valid = resources(&base, "127.0.0.1:1");
    for field in ["name", "groups", "template"] {
        let mut invalid = valid.clone();
        match field {
            "groups" => invalid["selection"][field] = json!([]),
            "template" => invalid["selection"][field] = json!({"id":0}),
            _ => invalid[field] = json!(""),
        }
        let response = client()
            .post(format!("{base}/api/campaigns/prepare"))
            .bearer_auth("synthetic-bootstrap-token")
            .json(&invalid)
            .send()
            .unwrap();
        assert_eq!(response.status(), 400, "invalid {field}");
        let error: Value = response.json().unwrap();
        assert_eq!(error["success"], false);
        assert!(!error["message"].as_str().unwrap().is_empty());
        // A corrected retry and a repeated visit both validate the current input.
        for _ in 0..2 {
            let review = post(&base, "/api/campaigns/prepare", &valid);
            assert_eq!(review["total"], 1);
            assert!(
                review["preview"]["subject"]
                    .as_str()
                    .unwrap()
                    .contains("Alice")
            );
        }
    }
    assert!(
        read(&base, "/api/campaigns/")
            .as_array()
            .unwrap()
            .is_empty()
    );
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let pool = sqlx::SqlitePool::connect(&format!("sqlite:{}", directory.path().join("installation.sqlite3").display())).await.unwrap();
        let counts: (i64, i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM campaigns), (SELECT count(*) FROM results), (SELECT count(*) FROM ruphish_delivery)").fetch_one(&pool).await.unwrap();
        assert_eq!(counts, (0, 0, 0), "Review must never schedule delivery or create a campaign");
        pool.close().await;
    });
    process.stop();
}

#[test]
fn campaign_pages_only_offer_next_when_another_owned_row_exists() {
    let directory = workspace();
    let (mut process, base) = start(&directory, &["--disable-mailer"]);
    let empty = read(&base, "/api/campaigns/overview");
    assert_eq!(empty["items"], json!([]));
    assert!(empty["next_after"].is_null());
    let mut body = resources(&base, "127.0.0.1:1");
    let targets: Vec<_> = (0..201)
        .map(|n| json!({"email":format!("recipient-{n}@example.test")}))
        .collect();
    let group = post(
        &base,
        "/api/groups/",
        &json!({"name":"Pagination audience","targets":targets}),
    );
    body["selection"]["groups"] = json!([{"id":group["id"]}]);
    let mut campaigns = Vec::new();
    for n in 0..4 {
        body["name"] = json!(format!("Pagination {n}"));
        campaigns.push(post(&base, "/api/campaigns/", &body));
    }
    let ids: Vec<_> = campaigns
        .iter()
        .map(|c| c["id"].as_i64().unwrap())
        .collect();
    // A later row owned by someone else must not advertise another page.
    sql(
        directory.path(),
        &format!(
            "INSERT INTO users(username,hash,api_key,role_id,password_change_required,account_locked) SELECT 'pagination-owner',hash,'pagination-owner-key',role_id,0,0 FROM users WHERE id=1; UPDATE campaigns SET user_id=2 WHERE id={};",
            ids[3]
        ),
    );
    let first = read(&base, "/api/campaigns/overview?limit=2");
    assert_eq!(
        first["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["id"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        ids[..2]
    );
    assert_eq!(first["next_after"], ids[1]);
    let last = read(
        &base,
        &format!("/api/campaigns/overview?limit=2&after={}", ids[1]),
    );
    assert_eq!(last["items"].as_array().unwrap().len(), 1);
    assert_eq!(last["items"][0]["id"], ids[2]);
    assert!(last["next_after"].is_null());
    for limit in [3, 50, i64::MAX] {
        let page = read(&base, &format!("/api/campaigns/overview?limit={limit}"));
        assert_eq!(page["items"].as_array().unwrap().len(), 3);
        assert!(
            page["next_after"].is_null(),
            "Exact or partial final page at limit {limit}"
        );
    }
    let clamped = read(&base, "/api/campaigns/overview?limit=0");
    assert_eq!(clamped["items"].as_array().unwrap().len(), 1);
    assert_eq!(clamped["next_after"], ids[0]);
    let endpoint = format!("/api/campaigns/{}/delivery", ids[0]);
    for limit in [100, 200, i64::MAX] {
        let mut after = 0;
        let mut seen = Vec::new();
        loop {
            let page = read(&base, &format!("{endpoint}?limit={limit}&after={after}"));
            let items = page["items"].as_array().unwrap();
            assert!(
                !items.is_empty(),
                "A supplied next cursor must lead to rows"
            );
            assert!(items.len() <= limit.min(200) as usize);
            assert_eq!(
                page["counts"]["ready"], 201,
                "Counts cover the whole campaign"
            );
            for row in items {
                let id = row["id"].as_i64().unwrap();
                assert!(id > after);
                assert!(!seen.contains(&id), "Rows must not repeat across pages");
                seen.push(id);
            }
            match page["next_after"].as_i64() {
                Some(next) => {
                    assert_eq!(Some(&next), seen.last());
                    after = next;
                }
                None => break,
            }
        }
        assert_eq!(seen.len(), 201);
        let exact_last = read(&base, &format!("{endpoint}?limit=200&after={}", seen[0]));
        assert_eq!(exact_last["items"].as_array().unwrap().len(), 200);
        assert!(exact_last["next_after"].is_null());
        let two = read(&base, &format!("{endpoint}?after={}", seen[198]));
        assert_eq!(two["items"].as_array().unwrap().len(), 2);
        assert!(two["next_after"].is_null());
        let exhausted = read(&base, &format!("{endpoint}?after={}", seen[200]));
        assert_eq!(exhausted["items"], json!([]));
        assert!(exhausted["next_after"].is_null());
    }
    let foreign = client()
        .get(format!("{base}/api/campaigns/{}/delivery", ids[3]))
        .bearer_auth("synthetic-bootstrap-token")
        .send()
        .unwrap();
    assert_eq!(foreign.status(), 404);
    // Deletion between requests can exhaust a previously valid cursor; reloading
    // the preceding page must now report the new exact end of the collection.
    let deleted = client()
        .delete(format!("{base}/api/campaigns/{}", ids[2]))
        .bearer_auth("synthetic-bootstrap-token")
        .send()
        .unwrap();
    assert!(deleted.status().is_success());
    let empty = read(&base, &format!("/api/campaigns/overview?after={}", ids[1]));
    assert_eq!(empty["items"], json!([]));
    assert!(empty["next_after"].is_null());
    let refreshed = read(&base, "/api/campaigns/overview?limit=2");
    assert_eq!(refreshed["items"].as_array().unwrap().len(), 2);
    assert!(refreshed["next_after"].is_null());
    process.stop();
}

#[test]
fn campaign_context_and_recipient_history_are_complete_bounded_and_owner_scoped() {
    let directory = workspace();
    let (mut process, base) = start(&directory, &["--disable-mailer"]);
    let mut body = resources(&base, "127.0.0.1:1");
    let group = post(
        &base,
        "/api/groups/",
        &json!({"name":"Two recipients","targets":[{"email":"alice@example.test"},{"email":"bob@example.test"}]}),
    );
    body["selection"]["groups"] = json!([{"id":group["id"]}]);
    body["launch_date"] = json!("2030-05-01T09:25:00Z");
    body["send_by_date"] = json!("2030-05-01T10:25:00Z");
    let campaign = post(&base, "/api/campaigns/", &body);
    let id = campaign["id"].as_i64().unwrap();
    let endpoint = format!("/api/campaigns/{id}/delivery");
    let view = read(&base, &endpoint);
    let a = view["items"][0]["id"].as_i64().unwrap();
    let b = view["items"][1]["id"].as_i64().unwrap();
    let history_path = format!("{endpoint}/{a}/history");
    let empty = read(&base, &history_path);
    assert_eq!(empty["attempts"], json!([]));
    assert_eq!(empty["events"], json!([]));
    assert!(empty["next_event"].is_null());
    sql(
        directory.path(),
        "UPDATE smtp SET username='smtp-user-secret-canary',password='smtp-password-secret-canary'; UPDATE templates SET subject='Updated subject {{.FirstName}}',envelope_sender='Notice <notice@example.test>',html='<p>html-body-canary</p>'; ",
    );
    let view = read(&base, &endpoint);
    let metadata = &view["campaign"];
    assert_eq!(metadata["template"]["name"], "Mail");
    assert_eq!(metadata["page"]["name"], "Page");
    assert_eq!(metadata["smtp"]["name"], "Local");
    assert_eq!(metadata["subject"], "Updated subject {{.FirstName}}");
    assert_eq!(
        metadata["profile_sender"],
        "Training <training@example.test>"
    );
    assert_eq!(metadata["template_sender"], "Notice <notice@example.test>");
    assert_eq!(metadata["launch_date"], "2030-05-01T09:25:00Z");
    assert_eq!(metadata["send_by_date"], "2030-05-01T10:25:00Z");
    assert_eq!(metadata["url"], origin(&base, "training.example.test"));
    for value in [&view, &read(&base, "/api/campaigns/overview")] {
        let encoded = value.to_string();
        for secret in [
            "smtp-user-secret-canary",
            "smtp-password-secret-canary",
            "html-body-canary",
        ] {
            assert!(!encoded.contains(secret));
        }
    }
    // Independent persisted history includes more than either page bound and
    // another recipient's newer events, which must not displace Alice's history.
    sql(
        directory.path(),
        &format!(
            r#"
        WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<105)
        INSERT INTO ruphish_delivery_attempt(result_id,generation,started,finished,state,message_id)
        SELECT {a},x,1900000000+x,1900000001+x,'accepted','attempt-'||x||'@example.test' FROM n;
        WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<105)
        INSERT INTO events(campaign_id,email,time,kind,metadata)
        SELECT {id},'alice@example.test','2030-05-01T09:25:00Z','email_sent','{{"password":"participant-secret-canary","password_present":true}}' FROM n;
        WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<150)
        INSERT INTO events(campaign_id,email,time,kind,metadata)
        SELECT {id},'bob@example.test','2030-05-01T10:25:00Z','email_opened','' FROM n;
     UPDATE events SET recipient_id=(SELECT min(r.id) FROM results r WHERE r.campaign_id=events.campaign_id AND r.email=events.email HAVING count(*)=1) WHERE recipient_id IS NULL;"#
        ),
    );
    let first = read(&base, &history_path);
    assert_eq!(first["email"], "alice@example.test");
    assert_eq!(first["attempts"].as_array().unwrap().len(), 100);
    assert_eq!(first["events"].as_array().unwrap().len(), 100);
    assert_eq!(first["attempts"][0]["generation"], 105);
    assert!(!first.to_string().contains("participant-secret-canary"));
    assert!(
        first["events"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["message"] == "Email Sent")
    );
    assert_eq!(
        serde_json::from_str::<Value>(first["events"][0]["details"].as_str().unwrap()).unwrap(),
        json!({"password_present":true})
    );
    let older = read(
        &base,
        &format!(
            "{history_path}?before_attempt={}&before_event={}",
            first["next_attempt"], first["next_event"]
        ),
    );
    assert_eq!(older["attempts"].as_array().unwrap().len(), 5);
    assert_eq!(older["events"].as_array().unwrap().len(), 5);
    assert!(older["next_attempt"].is_null());
    assert!(older["next_event"].is_null());
    let attempt_ids: Vec<_> = first["attempts"]
        .as_array()
        .unwrap()
        .iter()
        .chain(older["attempts"].as_array().unwrap())
        .map(|a| a["generation"].as_i64().unwrap())
        .collect();
    assert_eq!(attempt_ids, (1..=105).rev().collect::<Vec<_>>());
    let event_ids: std::collections::HashSet<_> = first["events"]
        .as_array()
        .unwrap()
        .iter()
        .chain(older["events"].as_array().unwrap())
        .map(|e| e["id"].as_i64().unwrap())
        .collect();
    assert_eq!(event_ids.len(), 105);
    let view = read(&base, &endpoint);
    assert_eq!(view["items"][0]["attempt_count"], 105);
    assert_eq!(view["items"][0]["attempts"].as_array().unwrap().len(), 100);
    let bob = read(&base, &format!("{endpoint}/{b}/history"));
    assert_eq!(bob["attempts"], json!([]));
    assert_eq!(bob["events"].as_array().unwrap().len(), 100);
    assert!(
        bob["events"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["message"] == "Email Opened")
    );
    let exhausted = read(
        &base,
        &format!("{history_path}?before_attempt=0&before_event=0"),
    );
    assert_eq!(exhausted["attempts"], json!([]));
    assert_eq!(exhausted["events"], json!([]));
    let anonymous = client()
        .get(format!("{base}{history_path}"))
        .send()
        .unwrap();
    assert_eq!(anonymous.status(), 401);
    let missing = client()
        .get(format!("{base}{endpoint}/999999/history"))
        .bearer_auth("synthetic-bootstrap-token")
        .send()
        .unwrap();
    assert_eq!(missing.status(), 404);
    body["name"] = json!("Another campaign");
    let second = post(&base, "/api/campaigns/", &body);
    let mismatch = client()
        .get(format!(
            "{base}/api/campaigns/{}/delivery/{a}/history",
            second["id"]
        ))
        .bearer_auth("synthetic-bootstrap-token")
        .send()
        .unwrap();
    assert_eq!(mismatch.status(), 404);
    sql(
        directory.path(),
        "INSERT INTO users(username,hash,api_key,role_id,password_change_required,account_locked) SELECT 'history-owner',hash,'history-owner-key',role_id,0,0 FROM users WHERE id=1; UPDATE templates SET user_id=2; UPDATE pages SET user_id=2; UPDATE smtp SET user_id=2;",
    );
    let foreign = client()
        .get(format!("{base}{history_path}"))
        .bearer_auth("history-owner-key")
        .send()
        .unwrap();
    assert_eq!(foreign.status(), 404);
    let detached = read(&base, &endpoint);
    for resource in ["template", "page", "smtp"] {
        assert!(detached["campaign"][resource]["name"].is_null());
    }
    assert!(detached["campaign"]["subject"].is_null());
    assert!(detached["campaign"]["profile_sender"].is_null());
    sql(
        directory.path(),
        &format!("UPDATE campaigns SET template_id=NULL,page_id=NULL,smtp_id=NULL WHERE id={id}"),
    );
    let missing = read(&base, &endpoint);
    assert_eq!(missing["campaign"]["template"]["id"], 0);
    process.stop();
}
