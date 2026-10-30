//! Shared executable contract for server database providers.
use super::mailbox;
use super::*;

fn request(
    client: &reqwest::blocking::Client,
    base: &str,
    method: reqwest::Method,
    path: &str,
    body: Option<&Value>,
) -> reqwest::blocking::Response {
    let mut r = client
        .request(method, format!("{base}{path}"))
        .bearer_auth("synthetic-bootstrap-token");
    if let Some(body) = body {
        r = r.json(body);
    }
    r.send().unwrap()
}
fn get(client: &reqwest::blocking::Client, base: &str, path: &str) -> Value {
    let r = request(client, base, reqwest::Method::GET, path, None);
    assert_eq!(r.status(), 200, "{}", r.text().unwrap());
    r.json().unwrap()
}
fn wait_mailbox(client: &reqwest::blocking::Client, base: &str, id: i64, after: i64) -> Value {
    loop {
        let value = get(client, base, &format!("/api/reporting-mailboxes/{id}"));
        if value["history"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"].as_i64().unwrap() > after && !r["finished"].is_null())
        {
            return value;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}
pub(super) fn exercise(mut database_config: Value, mut fault: impl FnMut(&Path, Fault<'_>)) {
    let directory = workspace();
    let provider = database_config["db_name"]
        .as_str()
        .unwrap_or("sqlite3")
        .to_owned();
    let network = if provider == "sqlite3" {
        None
    } else {
        let port = database_config[&provider]["port"].as_u64().unwrap() as u16;
        let network = database_network::Network::new(([127, 0, 0, 1], port).into());
        database_config[&provider]["port"] = json!(network.address.port());
        database_config[&provider]["acquire_timeout_seconds"] = json!(2);
        database_config[&provider]["connect_timeout_seconds"] = json!(2);
        Some(network)
    };
    let peer = mailbox::Mailbox::new();
    std::fs::write(directory.path().join("mailbox-ca.pem"), &peer.certificate).unwrap();
    let mut config = configuration();
    for (key, value) in database_config.as_object().unwrap() {
        config[key] = value.clone();
    }
    config["logging"]["level"] = json!("trace");
    write_config(directory.path(), &config);
    let browser = auth_process::browser();
    let (mut process, mut base) = campaign_process::start_verified(&directory, &[], |base| {
        auth_process::setup(&browser, base);
    });
    let client = client();
    let settings = get(&client, &base, "/api/settings");
    assert_eq!(settings["policy"]["reporting_mailboxes_enabled"], false);
    let mut policy = settings["policy"].clone();
    policy["reporting_mailboxes_enabled"] = json!(true);
    let response = client
        .put(format!("{base}/api/settings"))
        .bearer_auth("synthetic-bootstrap-token")
        .header("if-match", settings["revision"].to_string())
        .json(&json!({"policy":policy}))
        .send()
        .unwrap();
    assert_eq!(response.status(), 200, "{}", response.text().unwrap());
    let r = request(
        &client,
        &base,
        reqwest::Method::POST,
        "/api/groups/",
        Some(
            &json!({"name":"Provider group","targets":[{"email":"person@example.test","first_name":"Elise"}]}),
        ),
    );
    assert_eq!(r.status(), 201, "{}", r.text().unwrap());
    let group: Value = r.json().unwrap();
    let group_id = group["id"].as_i64().unwrap();
    assert_eq!(
        get(&client, &base, &format!("/api/groups/{group_id}"))["targets"][0]["email"],
        "person@example.test"
    );
    assert_eq!(
        request(
            &client,
            &base,
            reqwest::Method::POST,
            "/api/groups/",
            Some(&json!({"name":"Provider group","targets":[{"email":"person@example.test"}]}))
        )
        .status(),
        409
    );
    let hooks = webhook_receiver::Receiver::new();
    let hook = post(
        &base,
        "/api/webhooks/",
        &json!({"name":"Local receiver","url":hooks.url,"secret":"HOOK-SECRET-CANARY","is_active":true}),
    );
    let (campaign_id, rid) = campaign_workflow(&directory, &mut process, &base, &mut fault);
    let first_hook = hooks.requests.recv().unwrap();

    loop {
        let detail = get(&client, &base, &format!("/api/webhooks/{}", hook["id"]));
        if detail["history"]
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h["status"] == 503)
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let r = request(
        &client,
        &base,
        reqwest::Method::POST,
        "/api/reporting-mailboxes/",
        Some(
            &json!({"name":"Reports","enabled":true,"host":"127.0.0.1","port":peer.address.port(),"username":"service","password":"MAILBOX-SECRET-CANARY","tls":true,"folder":"INBOX","frequency":60}),
        ),
    );
    assert_eq!(r.status(), 201, "{}", r.text().unwrap());
    let saved: Value = r.json().unwrap();
    assert!(saved.get("password").is_none());
    let mailbox_id = saved["id"].as_i64().unwrap();
    let initial = wait_mailbox(&client, &base, mailbox_id, 0);
    assert_eq!(initial["code"], "passed");
    let run = initial["history"][0]["id"].as_i64().unwrap();
    {
        let mut state = peer.state.lock().unwrap();
        state.messages.insert(
            1,
            format!(
                "Subject: report\r\n\r\n{}/?rid={rid}\r\npassword=PARTICIPANT-CANARY",
                campaign_process::origin(&base, "training.example.test")
            )
            .into_bytes(),
        );
        state.messages.insert(
            2,
            format!("Subject: forged origin\r\n\r\nhttps://other.test/?rid={rid}").into_bytes(),
        );
        state.reject_ack = true;
    }
    assert_eq!(
        request(
            &client,
            &base,
            reqwest::Method::POST,
            &format!("/api/reporting-mailboxes/{mailbox_id}/check"),
            None
        )
        .status(),
        202
    );
    let failed = wait_mailbox(&client, &base, mailbox_id, run);
    assert_eq!(failed["code"], "mailbox_acknowledgement_failed");
    assert_eq!(
        get(
            &client,
            &base,
            &format!("/api/campaigns/{campaign_id}/results")
        )["totals"]["reported"],
        1
    );
    process.stop();
    assert!(!process.output().contains("PARTICIPANT-CANARY"));
    assert!(!process.output().contains("MAILBOX-SECRET-CANARY"));
    peer.state.lock().unwrap().reject_ack = false;
    while hooks.requests.try_recv().is_ok() {}
    hooks.status.store(204, std::sync::atomic::Ordering::SeqCst);
    fault(directory.path(), Fault::DueWebhooks);
    (process, base) = campaign_process::start_verified(&directory, &[], |_| {});

    loop {
        let received = hooks.requests.recv().unwrap();
        if received.0 == first_hook.0 {
            assert_eq!(
                received.1, first_hook.1,
                "retry after restart must retain signed event bytes"
            );
            break;
        }
    }
    assert_eq!(
        auth_process::sign_in(
            &auth_process::browser(),
            &base,
            "admin",
            "established-admin-password"
        )
        .status(),
        200,
        "established password survives process restart"
    );
    assert_eq!(
        get(&client, &base, &format!("/api/groups/{group_id}"))["name"],
        "Provider group"
    );
    let run = failed["history"][0]["id"].as_i64().unwrap();
    assert_eq!(
        request(
            &client,
            &base,
            reqwest::Method::POST,
            &format!("/api/reporting-mailboxes/{mailbox_id}/check"),
            None
        )
        .status(),
        202
    );
    let recovered = wait_mailbox(&client, &base, mailbox_id, run);
    assert_eq!(recovered["code"], "passed");
    let analysis = get(
        &client,
        &base,
        &format!("/api/campaigns/{campaign_id}/results"),
    );
    assert_eq!(
        analysis["totals"]["reported"], 1,
        "acknowledgement retry must not duplicate reports"
    );
    assert!(!analysis.to_string().contains("PARTICIPANT-CANARY"));
    assert_eq!(
        analysis["totals"],
        json!({"accepted":2,"opened":2,"clicked":1,"submitted":1,"reported":1})
    );
    assert_eq!(
        get(
            &client,
            &base,
            &format!("/api/campaigns/{campaign_id}/delivery")
        )["counts"]["accepted"],
        2
    );
    if let Some(network) = &network {
        network.available(false);
        let response = request(
            &auth_process::browser(),
            &base,
            reqwest::Method::GET,
            "/api/groups/",
            None,
        );
        assert!(
            response.status().is_server_error(),
            "outage must not look like an empty list or invalid credentials: {}",
            response.status()
        );

        assert!(
            !response
                .text()
                .unwrap()
                .contains("synthetic-process-password")
        );
        assert!(
            process.child.try_wait().unwrap().is_none(),
            "database outage must not stop the application: {}",
            process.output()
        );
        network.available(true);

        loop {
            let response = request(&client, &base, reqwest::Method::GET, "/api/groups/", None);
            if response.status().is_success() {
                let rows: Value = response.json().unwrap();
                assert!(rows.as_array().unwrap().iter().any(|r| r["id"] == group_id));
                break;
            }
        }
        let domain = get(&client, &base, "/api/domains/")[0].clone();
        let old =
            get(&client, &base, &format!("/api/domains/{}", domain["id"]))["history"][0]["id"]
                .as_i64()
                .unwrap();
        fault(directory.path(), Fault::DueDomains);

        loop {
            let checked = get(&client, &base, &format!("/api/domains/{}", domain["id"]));
            if checked["history"]
                .as_array()
                .unwrap()
                .iter()
                .any(|h| h["id"].as_i64().unwrap() > old && h["state"] == "passed")
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    uncertain_delivery(&directory, &mut process, &mut base, &mut fault);
    assert_eq!(
        request(
            &client,
            &base,
            reqwest::Method::DELETE,
            &format!("/api/groups/{group_id}"),
            None
        )
        .status(),
        200
    );
    assert_eq!(
        request(
            &client,
            &base,
            reqwest::Method::GET,
            &format!("/api/groups/{group_id}"),
            None
        )
        .status(),
        404
    );
    process.stop();
    let log = std::fs::read_to_string(directory.path().join("server.log")).unwrap();
    for canary in [
        "PARTICIPANT-CANARY",
        "MAILBOX-SECRET-CANARY",
        "HOOK-SECRET-CANARY",
        "changed-process-password",
        "ruphish-mysql-fixture-only",
        "synthetic-process-password",
        "established-admin-password",
    ] {
        assert!(!log.contains(canary), "logs exposed a synthetic secret");
    }
    assert!(
        provider == "sqlite3" || !directory.path().join("installation.sqlite3").exists(),
        "Server database selection must not fall back to SQLite"
    );
}

#[derive(Clone, Copy)]
pub(super) enum Fault<'a> {
    RejectInsert(&'a str),
    RestoreInsert(&'a str),
    AssertEmptyLaunch,
    DueWebhooks,
    DueDomains,
    ExpireDelivery,
}

pub(super) fn fault_statements(provider: &str, fault: Fault<'_>) -> Vec<String> {
    match fault {
        Fault::DueDomains => vec!["UPDATE ruphish_domains SET due=0".into()],
        Fault::ExpireDelivery => vec!["UPDATE ruphish_delivery SET lease_until=0 WHERE state='submitting'".into()],
        Fault::DueWebhooks => vec!["UPDATE ruphish_webhook_jobs SET due=0 WHERE state='retry'".into()],
        Fault::RejectInsert(table) => match provider {
            "postgres" => vec!["CREATE FUNCTION fixture_reject() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected write failure'; END $$".into(), format!("CREATE TRIGGER fixture_reject BEFORE INSERT ON {table} FOR EACH ROW EXECUTE FUNCTION fixture_reject()")],
            "mysql" => vec![format!("CREATE TRIGGER fixture_reject BEFORE INSERT ON {table} FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='injected write failure'")],
            _ => vec![format!("CREATE TRIGGER fixture_reject BEFORE INSERT ON {table} BEGIN SELECT RAISE(ABORT,'injected write failure'); END")],
        },
        Fault::RestoreInsert(table) if provider == "postgres" => vec![format!("DROP TRIGGER fixture_reject ON {table}"), "DROP FUNCTION fixture_reject()".into()],
        Fault::RestoreInsert(_) => vec!["DROP TRIGGER fixture_reject".into()],
        Fault::AssertEmptyLaunch => ["campaigns","ruphish_campaign_policy","ruphish_campaign_groups","events","results","ruphish_delivery","ruphish_campaign_launch","ruphish_webhook_jobs"].into_iter().map(|t|format!("SELECT count(*) FROM {t}")).collect(),
    }
}

#[test]
fn sqlite_process_full_production_workflow() {
    exercise(json!({}), |path, fault| {
        use sqlx::Connection;
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let mut connection = sqlx::SqliteConnection::connect_with(
                &sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(path.join("installation.sqlite3")),
            )
            .await
            .unwrap();
            for statement in fault_statements("sqlite3", fault) {
                if matches!(fault, Fault::AssertEmptyLaunch) {
                    assert_eq!(
                        sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(statement))
                            .fetch_one(&mut connection)
                            .await
                            .unwrap(),
                        0
                    );
                } else {
                    sqlx::raw_sql(sqlx::AssertSqlSafe(statement))
                        .execute(&mut connection)
                        .await
                        .unwrap();
                }
            }
        });
    });
}

fn post(base: &str, path: &str, value: &Value) -> Value {
    let response = request(&client(), base, reqwest::Method::POST, path, Some(value));
    let status = response.status();
    let body: Value = response.json().unwrap();
    assert!(status.is_success(), "{path}: {status}: {body}");
    body
}

fn uncertain_delivery(
    directory: &Workspace,
    process: &mut Process,
    base: &mut String,
    fault: &mut impl FnMut(&Path, Fault<'_>),
) {
    let smtp = campaign_process::probe_many(1);
    let group = post(
        base,
        "/api/groups/",
        &json!({"name":"Recovery audience","targets":[{"email":"alice@example.test"}]}),
    );
    let profile = post(
        base,
        "/api/smtp/",
        &json!({"name":"Recovery SMTP","host":smtp.address,"from_address":"training@example.test"}),
    );
    let templates = get(&client(), base, "/api/templates/");
    let template = templates
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "Personalized")
        .unwrap();
    let pages = get(&client(), base, "/api/pages/");
    let page = pages
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "Private page")
        .unwrap();
    let campaign = post(
        base,
        "/api/campaigns/",
        &json!({"name":"Uncertain acceptance","url":campaign_process::origin(base, "training.example.test"),"selection":{"groups":[{"id":group["id"]}],"template":{"id":template["id"]},"page":{"id":page["id"]},"smtp":{"id":profile["id"]}}}),
    );
    let id = campaign["id"].as_i64().unwrap();
    let message = smtp.received.recv().unwrap();
    assert!(
        message.contains("alice@example.test"),
        "campaign worker must deliver new work after recovery"
    );
    fault(directory, Fault::RejectInsert("events"));
    smtp.reply.send(Some("250 Accepted\r\n")).unwrap();
    smtp.thread.join().unwrap();

    while !process
        .output()
        .contains("campaign_outcome_storage_unavailable")
    {
        let guard = process.output.0.lock().unwrap();
        drop(
            process
                .output
                .1
                .wait_timeout(guard, Duration::from_millis(25))
                .unwrap(),
        );
    }
    assert!(process.child.try_wait().unwrap().is_none());
    fault(directory, Fault::RestoreInsert("events"));
    process.crash();
    fault(directory, Fault::ExpireDelivery);
    (*process, *base) = campaign_process::start_verified(directory, &[], |_| {});

    let delivery = loop {
        let delivery = get(&client(), base, &format!("/api/campaigns/{id}/delivery"));
        if delivery["counts"]["unknown"] == 1 {
            break delivery;
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let item = &delivery["items"][0];
    assert_eq!(item["attempts"].as_array().unwrap().len(), 1);
    assert_eq!(
        get(&client(), base, &format!("/api/campaigns/{id}/results"))["totals"]["accepted"],
        0,
        "unconfirmed delivery is not counted as accepted"
    );
    post(
        base,
        &format!("/api/campaigns/{id}/delivery/{}", item["id"]),
        &json!({"generation":item["generation"],"action":"confirm_accepted"}),
    );
    let reconciled = get(&client(), base, &format!("/api/campaigns/{id}/delivery"));
    assert_eq!(reconciled["counts"]["accepted"], 1);
    assert_eq!(
        reconciled["items"][0]["attempts"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        get(&client(), base, &format!("/api/campaigns/{id}/results"))["totals"]["accepted"],
        1
    );
    assert_eq!(
        get(&client(), base, &format!("/api/campaigns/{id}/results"))["progression"],
        json!({"accepted":1,"opened":0,"clicked":0,"submitted":0,"reported":0}),
        "the same address in another campaign must not inherit its interactions"
    );
    let stale = request(
        &client(),
        base,
        reqwest::Method::POST,
        &format!("/api/campaigns/{id}/delivery/{}", item["id"]),
        Some(&json!({"generation":item["generation"],"action":"confirm_accepted"})),
    );
    assert_eq!(stale.status(), 412);
    assert_eq!(
        get(&client(), base, &format!("/api/campaigns/{id}/results"))["totals"]["accepted"],
        1
    );
}

fn campaign_workflow(
    directory: &Workspace,
    process: &mut Process,
    base: &str,
    fault: &mut impl FnMut(&Path, Fault<'_>),
) -> (i64, String) {
    let smtp = campaign_process::probe_many(2);
    let group = post(
        base,
        "/api/groups/",
        &json!({"name":"Delivery audience","targets":[{"email":"alice@example.test","first_name":"Alice"},{"email":"bob@example.test","first_name":"Bob"}]}),
    );
    let template = post(
        base,
        "/api/templates/",
        &json!({"name":"Personalized","subject":"Hello {{.FirstName}}","text":"Visit {{.URL}}","html":"<a href='{{.URL}}'>Training</a>{{.Tracker}}"}),
    );
    let page = post(
        base,
        "/api/pages/",
        &json!({"name":"Private page","html":"<p>Hello {{.FirstName}}</p><form><input name='password' type='password'></form>"}),
    );
    let smtp_profile = post(
        base,
        "/api/smtp/",
        &json!({"name":"Loopback SMTP","host":smtp.address,"from_address":"Training <training@example.test>"}),
    );
    let draft = json!({"name":"Provider delivery","url":campaign_process::origin(base, "training.example.test"),"selection":{"groups":[{"id":group["id"]}],"template":{"id":template["id"]},"page":{"id":page["id"]},"smtp":{"id":smtp_profile["id"]}}});
    // Every write boundary must roll back, including children written before the injected failure.
    for table in [
        "campaigns",
        "ruphish_campaign_policy",
        "ruphish_campaign_groups",
        "events",
        "results",
        "ruphish_delivery",
        "ruphish_campaign_launch",
        "ruphish_webhook_jobs",
    ] {
        fault(directory, Fault::RejectInsert(table));
        let response = client()
            .post(format!("{base}/api/campaigns/"))
            .bearer_auth("synthetic-bootstrap-token")
            .header("idempotency-key", "rollback-launch")
            .json(&draft)
            .send()
            .unwrap();
        assert!(
            response.status().is_server_error(),
            "{table}: {}",
            response.status()
        );
        assert_eq!(get(&client(), base, "/api/campaigns/"), json!([]));
        fault(directory, Fault::AssertEmptyLaunch);
        fault(directory, Fault::RestoreInsert(table));
    }
    // Independent processes, pools and workers race the same idempotent launch.
    let mut peer = Process::start(directory, "all");
    let peer_base = format!("http://{}", peer.ready()["admin"].as_str().unwrap());
    let launch_idempotent = |endpoint: &str| {
        client()
            .post(format!("{endpoint}/api/campaigns/"))
            .bearer_auth("synthetic-bootstrap-token")
            .header("idempotency-key", "rollback-launch")
            .json(&draft)
            .send()
            .unwrap()
            .error_for_status()
            .unwrap()
            .json::<Value>()
            .unwrap()
    };
    let (campaign, duplicate) = std::thread::scope(|scope| {
        let first = scope.spawn(|| launch_idempotent(base));
        let second = scope.spawn(|| launch_idempotent(&peer_base));
        (first.join().unwrap(), second.join().unwrap())
    });
    assert_eq!(campaign["id"], duplicate["id"]);
    let id = campaign["id"].as_i64().unwrap();
    let mut recipients = Vec::new();
    for _ in 0..2 {
        let mail = smtp.received.recv().unwrap();
        let person = if mail.contains("Subject: Hello Alice") {
            "Alice"
        } else {
            assert!(mail.contains("Subject: Hello Bob"));
            "Bob"
        };
        recipients.push(person);
        assert!(mail.contains("https://training.example.test"));
        smtp.reply.send(Some("250 Accepted\r\n")).unwrap();
    }
    recipients.sort();
    assert_eq!(recipients, ["Alice", "Bob"]);
    smtp.thread.join().unwrap();

    loop {
        let delivery = get(&client(), base, &format!("/api/campaigns/{id}/delivery"));
        if delivery["counts"]["accepted"] == 2 {
            for item in delivery["items"].as_array().unwrap() {
                assert_eq!(item["attempts"].as_array().unwrap().len(), 1);
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    // The second process must observe account invalidation immediately on its next request.
    let session = auth_process::browser();
    assert_eq!(
        auth_process::sign_in(&session, &peer_base, "admin", "established-admin-password").status(),
        200
    );
    let editor = auth_process::browser();
    assert_eq!(
        auth_process::sign_in(&editor, base, "admin", "established-admin-password").status(),
        200
    );
    let response = auth_process::post(
        &editor,
        base,
        "/internal/password",
        &[
            ("current_password", "established-admin-password"),
            ("new_password", "changed-process-password"),
            ("confirm_new_password", "changed-process-password"),
        ],
    );
    assert_eq!(response.status(), 200, "{}", response.text().unwrap());
    assert!(auth_process::bootstrap(&session, &peer_base)["user"].is_null());
    assert_eq!(
        auth_process::sign_in(&editor, base, "admin", "changed-process-password").status(),
        200
    );
    assert_eq!(
        auth_process::post(
            &editor,
            base,
            "/internal/password",
            &[
                ("current_password", "changed-process-password"),
                ("new_password", "established-admin-password"),
                ("confirm_new_password", "established-admin-password"),
            ],
        )
        .status(),
        200
    );
    peer.stop();
    let public = format!("http://{}", process.ready()["phish"].as_str().unwrap());
    let results = campaign["results"].as_array().unwrap();
    let alice = results
        .iter()
        .find(|r| r["email"] == "alice@example.test")
        .unwrap();
    let rid = alice["id"].as_str().unwrap();
    let request_public = |method, path: &str| {
        client()
            .request(method, format!("{public}{path}"))
            .header("host", "training.example.test")
            .header(
                "x-forwarded-host",
                campaign_process::origin(base, "training.example.test")
                    .trim_start_matches("https://"),
            )
            .header("x-forwarded-proto", "https")
    };
    let url = format!("/?rid={rid}");
    fault(directory, Fault::RejectInsert("ruphish_webhook_jobs"));
    let failed = request_public(reqwest::Method::POST, &url)
        .json(&json!({"event_id":"a".repeat(32),"password_present":true}))
        .send()
        .unwrap();
    assert!(failed.status().is_server_error());
    assert_eq!(
        get(&client(), base, &format!("/api/campaigns/{id}/results"))["totals"],
        json!({"accepted":2,"opened":0,"clicked":0,"submitted":0,"reported":0})
    );
    fault(directory, Fault::RestoreInsert("ruphish_webhook_jobs"));
    // Submission first, then click/open: cumulative metrics must not regress or count the same recipient twice.
    for _ in 0..2 {
        assert_eq!(
            request_public(reqwest::Method::POST, &url)
                .json(&json!({"event_id":"a".repeat(32),"password_present":true}))
                .send()
                .unwrap()
                .status(),
            200
        );
    }
    let submitted = get(&client(), base, &format!("/api/campaigns/{id}/results"));
    assert_eq!(
        submitted["totals"],
        json!({"accepted":2,"opened":0,"clicked":0,"submitted":1,"reported":0})
    );
    assert_eq!(
        submitted["progression"],
        json!({"accepted":2,"opened":1,"clicked":1,"submitted":1,"reported":0})
    );
    let page = request_public(reqwest::Method::GET, &url).send().unwrap();
    assert_eq!(page.status(), 200);
    assert!(page.text().unwrap().contains("Hello Alice"));
    for _ in 0..2 {
        assert_eq!(
            request_public(reqwest::Method::GET, &format!("/track?rid={rid}"))
                .send()
                .unwrap()
                .status(),
            200
        );
    }
    for body in [
        json!({"event_id":"b".repeat(32),"password":"PARTICIPANT-CANARY"}),
        json!([{"event_id":"b".repeat(32)}]),
    ] {
        assert_eq!(
            request_public(reqwest::Method::POST, &url)
                .json(&body)
                .send()
                .unwrap()
                .status(),
            400
        );
    }
    assert_eq!(
        request_public(reqwest::Method::POST, &url)
            .form(&[("password", "PARTICIPANT-CANARY")])
            .send()
            .unwrap()
            .status(),
        415
    );
    let analysis = get(&client(), base, &format!("/api/campaigns/{id}/results"));
    assert_eq!(
        analysis["totals"],
        json!({"accepted":2,"opened":2,"clicked":1,"submitted":1,"reported":0})
    );
    assert_eq!(
        analysis["unique"],
        json!({"accepted":2,"opened":1,"clicked":1,"submitted":1,"reported":0})
    );
    assert_eq!(
        analysis["progression"],
        json!({"accepted":2,"opened":1,"clicked":1,"submitted":1,"reported":0})
    );
    let page1 = get(
        &client(),
        base,
        &format!("/api/campaigns/{id}/results?limit=1"),
    );
    let page2 = get(
        &client(),
        base,
        &format!("/api/campaigns/{id}/results?limit=1&offset=1"),
    );
    assert_eq!(page1["recipient_total"], 2);
    assert_eq!(page1["totals"], page2["totals"]);
    assert_ne!(page1["recipients"][0]["id"], page2["recipients"][0]["id"]);
    let selected = get(
        &client(),
        base,
        &format!("/api/campaigns/{id}/results?search=BOB"),
    );
    assert_eq!(selected["recipient_total"], 1);
    assert_eq!(
        selected["totals"],
        json!({"accepted":1,"opened":0,"clicked":0,"submitted":0,"reported":0})
    );
    let bad = request(
        &client(),
        base,
        reqwest::Method::GET,
        &format!("/api/campaigns/{id}/results?recipients=999999"),
        None,
    );
    assert_eq!(bad.status(), 404);
    let events = get(&client(), base, &format!("/api/campaigns/{id}"));
    assert_eq!(
        events["timeline"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["message"] == "Submitted Data")
            .count(),
        1
    );
    let empty = get(
        &client(),
        base,
        &format!("/api/campaigns/{id}/results?search=nobody"),
    );
    assert_eq!(
        empty["totals"],
        json!({"accepted":0,"opened":0,"clicked":0,"submitted":0,"reported":0})
    );
    for format in ["csv", "xlsx"] {
        let response = request(
            &client(),
            base,
            reqwest::Method::GET,
            &format!("/api/campaigns/{id}/export?format={format}&scope=events"),
            None,
        );
        assert_eq!(response.status(), 200);
        let bytes = response.bytes().unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("PARTICIPANT-CANARY"));
        if format == "csv" {
            assert!(String::from_utf8_lossy(&bytes).contains("alice@example.test"));
        } else {
            let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
            let mut sheets = String::new();
            for index in 0..archive.len() {
                let mut entry = archive.by_index(index).unwrap();
                if entry.name().unwrap().ends_with(".xml") {
                    entry.read_to_string(&mut sheets).unwrap();
                }
            }
            assert!(sheets.contains("alice@example.test"));
            assert!(sheets.contains("Submitted Data"));
            assert!(!sheets.contains("PARTICIPANT-CANARY"));
        }
    }
    (id, rid.to_owned())
}
