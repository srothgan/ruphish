//! Real executable, controlled DNS and trusted HTTPS; no external DNS or recipients.
use super::*;
#[path = "learning_pages_process.rs"]
mod learning_pages;
use hickory_proto::{
    op::{Message, OpCode},
    rr::{
        RData, Record, RecordType,
        rdata::{A, TXT},
    },
};
use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr, UdpSocket},
    sync::atomic::{AtomicBool, Ordering},
};

struct Dns {
    address: SocketAddr,
    records: Arc<Mutex<HashMap<String, String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Dns {
    fn start() -> Self {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let address = socket.local_addr().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        let records = Arc::new(Mutex::new(HashMap::<String, String>::new()));
        let values = records.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let thread = std::thread::spawn(move || {
            let mut bytes = [0; 4096];
            while !stopping.load(Ordering::Relaxed) {
                let Ok((size, peer)) = socket.recv_from(&mut bytes) else {
                    continue;
                };
                let Ok(query) = Message::from_vec(&bytes[..size]) else {
                    continue;
                };
                let mut answer = Message::response(query.metadata.id, OpCode::Query);
                answer.metadata.authoritative = true;
                answer.metadata.recursion_available = true;
                answer.metadata.recursion_desired = query.metadata.recursion_desired;
                for question in &query.queries {
                    answer.add_query(question.clone());
                    match question.query_type {
                        RecordType::TXT => {
                            if let Some(value) = values
                                .lock()
                                .unwrap()
                                .get(&question.name.to_utf8().to_lowercase())
                            {
                                // Split one TXT RR into two character strings, as real DNS providers may.
                                let split = value.len() / 2;
                                answer.add_answer(Record::from_rdata(
                                    question.name.clone(),
                                    0,
                                    RData::TXT(TXT::new(vec![
                                        value[..split].into(),
                                        value[split..].into(),
                                    ])),
                                ));
                            }
                        }
                        RecordType::A => {
                            answer.add_answer(Record::from_rdata(
                                question.name.clone(),
                                0,
                                RData::A(A(Ipv4Addr::LOCALHOST)),
                            ));
                        }
                        _ => {}
                    }
                }
                let _ = socket.send_to(&answer.to_vec().unwrap(), peer);
            }
        });
        Self {
            address,
            records,
            stop,
            thread: Some(thread),
        }
    }
    fn publish(&self, domain: &Value) {
        self.records.lock().unwrap().insert(
            format!("{}.", domain["txt_name"].as_str().unwrap()),
            domain["txt_value"].as_str().unwrap().into(),
        );
    }
}
impl Drop for Dns {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            t.join().unwrap();
        }
    }
}
fn post(base: &str, path: &str, body: Value) -> Value {
    let response = client()
        .post(format!("{base}{path}"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(&body)
        .send()
        .unwrap();
    let status = response.status();
    let value: Value = response.json().unwrap();
    assert!(status.is_success(), "{status}: {value}");
    value
}
fn get(base: &str, path: &str) -> Value {
    client()
        .get(format!("{base}{path}"))
        .bearer_auth("synthetic-bootstrap-token")
        .send()
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .unwrap()
}
fn wait_verified(base: &str, id: i64, process: &Process) -> Value {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let value = get(base, &format!("/api/domains/{id}"));
        if value["eligible"] == true {
            return value;
        }
        assert!(
            Instant::now() < deadline,
            "Verification did not succeed: {value}; {}",
            process.output()
        );
        // Wait for observable server activity, with a deadline, rather than assuming DNS timing.
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
fn configure(directory: &Path, dns: &Dns) -> Vec<u8> {
    let material = tls_fixture::server_certificate(&["training.example.test"]);
    let cert = material.cert.pem();
    std::fs::write(directory.join("public.crt"), &cert).unwrap();
    std::fs::write(
        directory.join("public.key"),
        material.signing_key.serialize_pem(),
    )
    .unwrap();
    let mut config = configuration();
    config["phish_server"]["use_tls"] = json!(true);
    config["domain_verification"] = json!({"resolvers":[dns.address.to_string()],"allowed_networks":["127.0.0.1/32"],"trusted_ca_path":"public.crt"});
    write_config(directory, &config);
    cert.into_bytes()
}
#[test]
fn verified_domain_to_campaign_to_private_events_survives_restart_and_rejects_bypasses() {
    let dns = Dns::start();
    let directory = workspace();
    let cert = configure(directory.path(), &dns);
    let static_root = directory.path().join("static/endpoint");
    std::fs::create_dir_all(&static_root).unwrap();
    std::fs::write(static_root.join("training.txt"), "Public training asset").unwrap();
    std::fs::write(static_root.join("page.html"),"<form action='/collect'><input name='password' type='password'></form><script>unsafe()</script>").unwrap();
    let mut process = Process::with_options(
        directory.path(),
        "all",
        Some("synthetic-bootstrap-password"),
        Some("synthetic-bootstrap-token"),
        &["--disable-mailer"],
    );
    let ready = process.ready();
    let admin = format!("http://{}", ready["admin"].as_str().unwrap());
    let public: SocketAddr = ready["phish"].as_str().unwrap().parse().unwrap();
    database(
        directory.path(),
        "UPDATE users SET password_change_required=0; UPDATE ruphish_settings SET verification_enabled=1,webhooks_enabled=1",
    );
    let origin = format!("https://training.example.test:{}", public.port());
    let domain = post(
        &admin,
        "/api/domains/",
        json!({"name":"Training","origin":origin,"enabled":true,"revision":null}),
    );
    let id = domain["id"].as_i64().unwrap();
    assert_eq!(
        client()
            .get(format!("{admin}/api/domains/"))
            .send()
            .unwrap()
            .status(),
        401
    );
    // Password hashing uses the same bounded deadline as the authentication suite.
    let second: Value = client().post(format!("{admin}/api/users/"))
        .bearer_auth("synthetic-bootstrap-token")
        .timeout(Duration::from_secs(20))
        .json(&json!({"username":"domain-other-owner","password":"synthetic-other-owner-password","role":"user"}))
        .send().unwrap().error_for_status().unwrap().json().unwrap();
    let second_key = second["api_key"].as_str().unwrap();
    for path in [
        format!("/api/domains/{id}"),
        format!("/api/domains/{id}/verify"),
    ] {
        let method = if path.ends_with("verify") {
            reqwest::Method::POST
        } else {
            reqwest::Method::GET
        };
        assert_eq!(
            client()
                .request(method, format!("{admin}{path}"))
                .bearer_auth(second_key)
                .send()
                .unwrap()
                .status(),
            404
        );
    }
    let empty: Value = client()
        .get(format!("{admin}/api/domains/"))
        .bearer_auth(second_key)
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(empty, json!([]));
    let duplicate = client()
        .post(format!("{admin}/api/domains/"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(&json!({"name":"Duplicate","origin":origin,"enabled":true}))
        .send()
        .unwrap();
    assert_eq!(duplicate.status(), 409);
    for body in [
        json!([domain.clone(), domain.clone()]),
        json!(["Array", origin, true, null]),
        json!({"name":"Unsafe","origin":origin,"enabled":true,"verified_until":9999999999_i64}),
    ] {
        let response = client()
            .post(format!("{admin}/api/domains/"))
            .bearer_auth("synthetic-bootstrap-token")
            .json(&body)
            .send()
            .unwrap();
        assert_eq!(response.status(), 400);
    }
    let changed = client()
        .put(format!("{admin}/api/domains/{id}"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(&json!({"name":"Changed","origin":origin,"enabled":true,"revision":0}))
        .send()
        .unwrap();
    assert_eq!(changed.status(), 412);
    let immutable=client().put(format!("{admin}/api/domains/{id}")).bearer_auth("synthetic-bootstrap-token").json(&json!({"name":"Changed","origin":"https://different.example.test","enabled":true,"revision":domain["revision"]})).send().unwrap();
    assert_eq!(immutable.status(), 400);
    let missing_revision = client()
        .delete(format!("{admin}/api/domains/{id}"))
        .bearer_auth("synthetic-bootstrap-token")
        .send()
        .unwrap();
    assert_eq!(missing_revision.status(), 428);
    assert_eq!(get(&admin, "/api/domains/validated/"), json!([]));
    assert_eq!(get(&admin, "/api/domains/").as_array().unwrap().len(), 1);
    let group = post(
        &admin,
        "/api/groups/",
        json!({"name":"Audience","targets":[{"email":"alice@example.test","first_name":"Alice"}]}),
    );
    let template = post(
        &admin,
        "/api/templates/",
        json!({"name":"Mail","subject":"Training","text":"Visit {{.URL}}"}),
    );
    let landing = post(
        &admin,
        "/api/pages/",
        json!({"name":"Page","html":"<form><label>Password<input name='password' type='password' autocomplete='current-password'></label><button>Continue</button></form><script>authoredCanary()</script>","capture_credentials":false,"capture_passwords":false,"redirect_url":"http://example.test/finished/{{.FirstName}}"}),
    );
    let smtp = post(
        &admin,
        "/api/smtp/",
        json!({"name":"Local","host":"127.0.0.1:1","from_address":"training@example.test"}),
    );
    let draft = json!({"name":"Private campaign","url":origin,"selection":{"groups":[{"id":group["id"]}],"template":{"id":template["id"]},"page":{"id":landing["id"]},"smtp":{"id":smtp["id"]}}});
    let rejected = client()
        .post(format!("{admin}/api/campaigns/"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(&draft)
        .send()
        .unwrap();
    assert!(!rejected.status().is_success());
    dns.publish(&domain);
    // Advance only scheduling state if the automatic first check preceded DNS publication.
    database(
        directory.path(),
        "UPDATE ruphish_domains SET due=0,checked_at=0 WHERE verified_until=0",
    );
    let verified = wait_verified(&admin, id, &process);
    assert_eq!(verified["status"], "verified");
    assert!(verified["active_check"].is_null());
    assert_eq!(verified["history"][0]["state"], "passed");
    let completed_check_id = verified["history"][0]["id"].clone();
    assert_eq!(
        verified["verified_until"].as_i64().unwrap() - verified["checked_at"].as_i64().unwrap(),
        86_400
    );
    for field in ["control", "dns", "routing", "tls"] {
        assert_eq!(verified[field], true)
    }
    assert_eq!(
        get(&admin, "/api/domains/validated/")
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let campaign = post(&admin, "/api/campaigns/", draft);
    let in_use = client()
        .delete(format!("{admin}/api/domains/{id}"))
        .bearer_auth("synthetic-bootstrap-token")
        .header("if-match", format!("\"{}\"", verified["revision"]))
        .send()
        .unwrap();
    assert_eq!(in_use.status(), 409);
    let rid = campaign["results"][0]["id"].as_str().unwrap();
    let browser = reqwest::blocking::Client::builder()
        .no_proxy()
        .add_root_certificate(reqwest::Certificate::from_pem(&cert).unwrap())
        .resolve("training.example.test", public)
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let url = format!("{origin}/welcome?rid={rid}");
    assert_eq!(
        browser
            .get(format!("{origin}/static/training.txt"))
            .send()
            .unwrap()
            .text()
            .unwrap(),
        "Public training asset"
    );
    let static_page = browser
        .get(format!("{origin}/static/page.html"))
        .send()
        .unwrap();
    assert_eq!(static_page.status(), 200);
    assert!(
        static_page.headers()["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("form-action 'none'")
    );
    assert_eq!(
        browser
            .get(format!("{origin}/static/missing.txt"))
            .send()
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        browser
            .get(format!("{origin}/static/%2e%2e/config.json"))
            .send()
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        browser
            .get(format!("{origin}/static/training.txt"))
            .header("host", "wrong.example.test")
            .send()
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        browser
            .post(format!("{origin}/static/training.txt"))
            .body("unsolicited-canary")
            .send()
            .unwrap()
            .status(),
        405
    );
    for path in ["/", "/track", "/report"] {
        for suffix in [
            "",
            "?rid=unknown",
            "?rid=preview-bogus",
            "?rid=unknown+",
            "?rid=unknown%2b",
        ] {
            let response = browser
                .get(format!("{origin}{path}{suffix}"))
                .send()
                .unwrap();
            assert_eq!(response.status(), 404);
            if path == "/report" {
                assert_eq!(response.headers()["access-control-allow-origin"], "*");
            }
            assert_eq!(response.text().unwrap(), "404 page not found\n");
        }
    }
    assert_eq!(
        browser
            .get(format!("{origin}/robots.txt"))
            .send()
            .unwrap()
            .text()
            .unwrap(),
        "User-agent: *\nDisallow: /\n"
    );
    let before = get(&admin, &format!("/api/campaigns/{}", campaign["id"]));
    // Reserved operational paths must not render or track even a valid recipient.
    for (path, status) in [("live", "live"), ("ready", "ready")] {
        let response = browser
            .get(format!("{origin}/health/{path}?rid={rid}"))
            .send()
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.json::<Value>().unwrap(), json!({"status":status}));
        assert_eq!(
            browser
                .post(format!("{origin}/health/{path}?rid={rid}"))
                .json(&json!({"password":"health-participant-canary"}))
                .send()
                .unwrap()
                .status(),
            405
        );
    }
    for path in ["/", "/track", "/report"] {
        for suffix in ["+", "%2b"] {
            let value: Value = browser
                .get(format!("{origin}{path}?rid={rid}{suffix}"))
                .send()
                .unwrap()
                .json()
                .unwrap();
            assert_eq!(
                value,
                json!({"server":"gophish","contact_address":"","send_date":campaign["results"][0]["send_date"]})
            );
        }
    }
    assert_eq!(browser.head(&url).send().unwrap().status(), 200);
    assert_eq!(
        get(&admin, &format!("/api/campaigns/{}", campaign["id"]))["timeline"],
        before["timeline"]
    );
    assert_eq!(
        browser
            .get(format!("{origin}/track?rid={rid}"))
            .send()
            .unwrap()
            .status(),
        200
    );
    let opened = get(&admin, &format!("/api/campaigns/{}", campaign["id"]));
    assert_eq!(opened["results"][0]["status"], "Email Opened");
    let last = opened["timeline"].as_array().unwrap().last().unwrap();
    assert_eq!(last["message"], "Email Opened");
    assert_eq!(last["time"], opened["results"][0]["modified_date"]);
    let view = browser.get(&url).send().unwrap();
    assert_eq!(view.status(), 200);
    assert!(
        view.headers()["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("form-action 'none'")
    );
    assert_eq!(view.headers()["x-server"], "gophish");
    let clicked = get(&admin, &format!("/api/campaigns/{}", campaign["id"]));
    assert_eq!(clicked["results"][0]["status"], "Clicked Link");
    let last = clicked["timeline"].as_array().unwrap().last().unwrap();
    assert_eq!(last["message"], "Clicked Link");
    assert_eq!(last["time"], clicked["results"][0]["modified_date"]);
    let html = view.text().unwrap();
    assert!(html.contains("name=\"password\""));
    assert!(!html.contains("authoredCanary"));
    assert_eq!(
        browser
            .get(format!("{origin}/track?rid={rid}"))
            .send()
            .unwrap()
            .bytes()
            .unwrap()
            .as_ref(),
        include_bytes!("fixtures/resources/preview-pixel.png")
    );
    assert_eq!(
        browser
            .get(&url)
            .header("host", "wrong.example.test")
            .send()
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        browser
            .get(&url)
            .header("x-forwarded-host", "wrong.example.test")
            .send()
            .unwrap()
            .status(),
        200,
        "untrusted forwarding headers are ignored"
    );
    let canary = "NEVER-RETAIN-PARTICIPANT-731";
    for payload in [
        json!({"event_id":"a".repeat(32),"password_present":true,"password":canary}),
        json!({"event_id":"a".repeat(32),"password_present":canary}),
        json!(["a".repeat(32), true]),
        json!([{ "event_id":"a".repeat(32),"password_present":true },{ "event_id":"b".repeat(32),"password_present":true }]),
    ] {
        let response = browser.post(&url).json(&payload).send().unwrap();
        assert_eq!(response.status(), 400);
        assert!(!response.text().unwrap().contains(canary));
    }
    assert_eq!(
        browser
            .post(&url)
            .form(&[("username", canary), ("password", canary)])
            .send()
            .unwrap()
            .status(),
        415
    );
    assert_eq!(
        browser
            .post(&url)
            .header("content-type", "application/json")
            .body("x".repeat(513))
            .send()
            .unwrap()
            .status(),
        413
    );
    for _ in 0..2 {
        let response: Value = browser
            .post(&url)
            .json(&json!({"event_id":"a".repeat(32),"password_present":true}))
            .send()
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .unwrap();
        assert_eq!(response["redirect"], "http://example.test/finished/Alice");
    }
    assert_eq!(
        browser
            .get(format!("{origin}/report?rid={rid}"))
            .send()
            .unwrap()
            .status(),
        204
    );
    let transparency: Value = browser
        .get(format!("{origin}/?rid={rid}%2b"))
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(transparency["server"], "gophish");
    database(
        directory.path(),
        "UPDATE ruphish_campaign_policy SET redirect_url=''",
    );
    let collector = std::process::Command::new("node")
        .arg(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../ruphish-ui/web/tests/support/public-submissions.mjs"),
        )
        .arg(&url)
        .arg(directory.path().join("public.crt"))
        .output()
        .expect("Node is required for the executable collector workflow");
    assert!(
        collector.status.success(),
        "collector workflow failed: {}",
        String::from_utf8_lossy(&collector.stderr)
    );
    for _ in 0..2 {
        assert_eq!(browser.get(&url).send().unwrap().status(), 200);
        assert_eq!(
            browser
                .get(format!("{origin}/track?rid={rid}"))
                .send()
                .unwrap()
                .status(),
            200
        );
    }
    let current = get(&admin, &format!("/api/campaigns/{}", campaign["id"]));
    assert_eq!(current["results"][0]["status"], "Submitted Data");
    assert_eq!(current["results"][0]["reported"], true);
    assert_eq!(
        current["timeline"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["message"] == "Submitted Data")
            .count(),
        4
    );
    // Actual public requests above must feed the dashboard, without inflating
    // unique counts on repeated opens/clicks/submissions. The disabled mailer
    // means these observations initially have no accepted-email cohort.
    let unsent = get(&admin, "/api/overview/posture?window=7d");
    for field in [
        "delivered",
        "opened",
        "clicked",
        "reported",
        "credentials_entered",
    ] {
        assert_eq!(unsent[field], 0);
    }
    // Supply the independent SMTP acceptance fixture at the delivery boundary.
    database(
        directory.path(),
        "INSERT INTO events(campaign_id,email,kind,time,metadata,recipient_id) SELECT r.campaign_id,r.email,'email_sent',datetime('now','-1 minute'),'',r.id FROM results r JOIN campaigns c ON c.id=r.campaign_id WHERE c.name='Private campaign' AND c.user_id=1",
    );
    let analysis = get(
        &admin,
        &format!("/api/campaigns/{}/analysis?limit=100", campaign["id"]),
    );
    assert_eq!(analysis["unique"][3], 1);
    assert_eq!(analysis["progression"][3], 1);
    assert_eq!(analysis["totals"][3], 4);
    assert_eq!(
        get(
            &admin,
            &format!("/api/campaigns/{}/summary", campaign["id"])
        )["stats"]["submitted_data"],
        1
    );
    let recipient_id = analysis["recipients"][0]["id"].as_i64().unwrap();
    let recipient_analysis = get(
        &admin,
        &format!(
            "/api/campaigns/{}/analysis?recipient={recipient_id}&limit=100",
            campaign["id"]
        ),
    );
    assert_eq!(recipient_analysis["totals"][3], 4);
    assert_eq!(
        recipient_analysis["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["kind"] == "Submitted Data")
            .count(),
        4
    );
    let observed = get(&admin, "/api/overview/posture?window=7d");
    for field in [
        "delivered",
        "opened",
        "clicked",
        "reported",
        "credentials_entered",
    ] {
        assert_eq!(observed[field], 1, "{field}: {observed}");
    }
    assert!(!observed.to_string().contains(canary));
    assert!(!current.to_string().contains(canary));
    assert!(!process.output().contains(canary));
    assert!(
        !std::fs::read_to_string(directory.path().join("server.log"))
            .unwrap()
            .contains(canary)
    );
    let bytes = std::fs::read(directory.path().join("installation.sqlite3")).unwrap();
    assert!(!bytes.windows(canary.len()).any(|w| w == canary.as_bytes()));
    let completed = client()
        .get(format!("{admin}/api/campaigns/{}/complete", campaign["id"]))
        .bearer_auth("synthetic-bootstrap-token")
        .send()
        .unwrap();
    assert_eq!(completed.status(), 200);
    let before = get(&admin, &format!("/api/campaigns/{}", campaign["id"]));
    for path in ["/", "/track", "/report"] {
        assert_eq!(
            browser
                .get(format!("{origin}{path}?rid={rid}"))
                .send()
                .unwrap()
                .status(),
            404
        );
    }
    assert_eq!(
        browser
            .post(&url)
            .json(&json!({"event_id":"b".repeat(32),"password_present":false}))
            .send()
            .unwrap()
            .status(),
        404
    );
    let after = get(&admin, &format!("/api/campaigns/{}", campaign["id"]));
    assert_eq!(before["timeline"], after["timeline"]);
    assert_eq!(before["results"], after["results"]);
    process.stop();
    let mut restarted =
        Process::with_options(directory.path(), "admin", None, None, &["--disable-mailer"]);
    let base = format!("http://{}", restarted.ready()["admin"].as_str().unwrap());
    assert_eq!(get(&base, "/api/domains/")[0]["id"], id);
    let restored = get(&base, &format!("/api/domains/{id}"));
    assert_eq!(restored["history"][0]["id"], completed_check_id);
    assert_eq!(restored["status"], "verified");
    assert_eq!(
        get(&base, &format!("/api/campaigns/{}", campaign["id"]))["results"][0]["status"],
        "Submitted Data"
    );
    restarted.stop();
}

#[test]
fn domain_check_queue_and_history_survive_real_restart_and_latest_failure_revokes_eligibility() {
    let dns = Dns::start();
    let directory = workspace();
    configure(directory.path(), &dns);
    let mut process = Process::with_options(
        directory.path(),
        "all",
        Some("synthetic-bootstrap-password"),
        Some("synthetic-bootstrap-token"),
        &["--disable-mailer"],
    );
    let ready = process.ready();
    let admin = format!("http://{}", ready["admin"].as_str().unwrap());
    let public: SocketAddr = ready["phish"].as_str().unwrap().parse().unwrap();
    database(
        directory.path(),
        "UPDATE users SET password_change_required=0; UPDATE ruphish_settings SET verification_enabled=1,webhooks_enabled=1",
    );
    let origin = format!("https://training.example.test:{}", public.port());
    let domain = post(
        &admin,
        "/api/domains/",
        json!({"name":"History","origin":origin,"enabled":false,"revision":null}),
    );
    let id = domain["id"].as_i64().unwrap();
    assert_eq!(domain["status"], "not_verified");
    assert!(domain["active_check"].is_null());
    dns.publish(&domain);
    let enabled: Value = client()
        .put(format!("{admin}/api/domains/{id}"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(
            &json!({"name":"History","origin":origin,"enabled":true,"revision":domain["revision"]}),
        )
        .send()
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(enabled["active_check"]["state"], "queued");
    let passed = wait_verified(&admin, id, &process);
    let queued = post(&admin, &format!("/api/domains/{id}/verify"), json!(null));
    assert_eq!(queued["active_check"]["state"], "queued");
    assert_eq!(queued["status"], "verified");
    let again = post(&admin, &format!("/api/domains/{id}/verify"), json!(null));
    assert_eq!(again["active_check"]["id"], queued["active_check"]["id"]);
    process.stop();
    // Preserve the public origin while restarting the actual executable.
    let mut config: Value =
        serde_json::from_slice(&std::fs::read(directory.path().join("config.json")).unwrap())
            .unwrap();
    config["phish_server"]["listen_url"] = json!(public.to_string());
    write_config(directory.path(), &config);
    dns.records.lock().unwrap().insert(
        format!("{}.", domain["txt_name"].as_str().unwrap()),
        "wrong-verification-proof".into(),
    );
    let mut restarted =
        Process::with_options(directory.path(), "all", None, None, &["--disable-mailer"]);
    let base = format!("http://{}", restarted.ready()["admin"].as_str().unwrap());
    let restored = get(&base, &format!("/api/domains/{id}"));
    assert_eq!(restored["active_check"]["id"], queued["active_check"]["id"]);
    assert_eq!(restored["history"][0]["id"], passed["history"][0]["id"]);
    database(directory.path(), "UPDATE ruphish_domains SET due=0");
    let deadline = Instant::now() + Duration::from_secs(20);
    let failed = loop {
        let d = get(&base, &format!("/api/domains/{id}"));
        if d["active_check"].is_null() && d["status"] == "failed" {
            break d;
        }
        assert!(
            Instant::now() < deadline,
            "No failed verification result: {d}"
        );
        let guard = restarted.output.0.lock().unwrap();
        drop(
            restarted
                .output
                .1
                .wait_timeout(guard, Duration::from_millis(100))
                .unwrap(),
        );
    };
    assert_eq!(failed["eligible"], false);
    assert_eq!(failed["verified_until"], 0);
    assert_eq!(failed["history"][0]["state"], "failed");
    assert_eq!(failed["history"][0]["id"], queued["active_check"]["id"]);
    assert_eq!(failed["history"][0]["code"], "domain_txt_mismatch");
    assert_eq!(failed["history"][1]["state"], "passed");
    assert_eq!(get(&base, "/api/domains/validated/"), json!([]));
    restarted.stop();
}
