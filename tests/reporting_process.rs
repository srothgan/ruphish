//! Real executable reporting, integration APIs, verified IMAP and restart behavior.
use super::*;
use sqlx::Row;
#[path = "support/mailbox.rs"]
mod mailbox;
fn request(
    client: &reqwest::blocking::Client,
    base: &str,
    method: reqwest::Method,
    path: &str,
    body: Option<&Value>,
) -> reqwest::blocking::Response {
    let mut request = client
        .request(method, format!("{base}{path}"))
        .bearer_auth("synthetic-bootstrap-token");
    if let Some(body) = body {
        request = request.json(body);
    }
    request.send().unwrap()
}
fn get(client: &reqwest::blocking::Client, base: &str, path: &str) -> Value {
    let response = request(client, base, reqwest::Method::GET, path, None);
    assert_eq!(response.status(), 200, "{}", response.text().unwrap());
    // Obtain a fresh response only in the success path (assert formatting is lazy).
    response.json().unwrap()
}
fn wait_check(client: &reqwest::blocking::Client, base: &str, id: i64, after: i64) -> Value {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let value = get(client, base, &format!("/api/reporting-mailboxes/{id}"));
        if value["history"]
            .as_array()
            .unwrap()
            .iter()
            .any(|run| run["id"].as_i64().unwrap() > after && !run["finished"].is_null())
        {
            return value;
        }
        assert!(Instant::now() < deadline, "mailbox did not finish: {value}");
        std::thread::sleep(Duration::from_millis(40));
    }
}
#[test]
fn verified_imap_reports_survive_ack_failure_restart_and_uid_reset_without_message_leakage() {
    let directory = workspace();
    let peer = mailbox::Mailbox::new();
    std::fs::write(directory.path().join("mailbox-ca.pem"), &peer.certificate).unwrap();
    let mut config = configuration();
    config["logging"]["level"] = json!("trace");
    write_config(directory.path(), &config);
    let mut process = Process::start(directory.path(), "admin");
    let mut base = format!("http://{}", process.ready()["admin"].as_str().unwrap());
    let client = client();
    database(
        directory.path(),
        "UPDATE users SET password_change_required=0; INSERT INTO campaigns(id,user_id,name,url,status) VALUES(91,1,'Reporting test','https://training.test/','Completed'); INSERT INTO results(id,campaign_id,user_id,r_id,email,status,reported) VALUES(91,91,1,'abc123','person@example.test','Email Sent',0);",
    );
    let draft = json!({"name":"Reports","enabled":true,"host":"127.0.0.1","port":peer.address.port(),"username":"service","password":"MAILBOX-SECRET-CANARY","tls":true,"folder":"INBOX","frequency":60});
    let created = request(
        &client,
        &base,
        reqwest::Method::POST,
        "/api/reporting-mailboxes/",
        Some(&draft),
    );
    assert_eq!(created.status(), 201);
    let saved: Value = created.json().unwrap();
    let id = saved["id"].as_i64().unwrap();
    assert!(saved.get("password").is_none());
    let initialized = wait_check(&client, &base, id, 0);
    assert_eq!(initialized["code"], "passed");
    let run = initialized["history"][0]["id"].as_i64().unwrap();
    {
        let mut state = peer.state.lock().unwrap();
        state
            .messages
            .insert(1, b"Subject: too large\r\n\r\noversized".to_vec());
        state.oversized.push(1);
        state.messages.insert(2,b"Subject: report\r\n\r\nhttps://training.test/?rid=abc123\r\npassword=PARTICIPANT-CANARY".to_vec());
        state.messages.insert(
            3,
            b"Subject: wrong origin\r\n\r\nhttps://other.test/?rid=abc123".to_vec(),
        );
        state.messages.insert(4, b"Subject: ambiguous\r\n\r\nhttps://training.test/?rid=abc123 https://training.test/?rid=another".to_vec());
        state.reject_ack = true;
    }
    assert_eq!(
        request(
            &client,
            &base,
            reqwest::Method::POST,
            &format!("/api/reporting-mailboxes/{id}/check"),
            None
        )
        .status(),
        202
    );
    let failed = wait_check(&client, &base, id, run);
    assert_eq!(
        failed["code"],
        "mailbox_acknowledgement_failed",
        "{failed}; ledger: {:?}",
        database(
            directory.path(),
            "SELECT uid,code FROM ruphish_mailbox_messages"
        )
        .iter()
        .map(|r| (r.get::<i64, _>("uid"), r.get::<String, _>("code")))
        .collect::<Vec<_>>()
    );
    assert_eq!(failed["history"][0]["matched"], 1);
    assert_eq!(failed["history"][0]["rejected"], 1);
    let analysis = get(&client, &base, "/api/campaigns/91/analysis");
    assert_eq!(analysis["totals"][4], 1);
    assert_eq!(analysis["events"][0]["source"], "mailbox");
    assert_eq!(analysis["unique"][1], 0);
    assert_eq!(analysis["unique"][2], 0);
    process.stop();
    peer.state.lock().unwrap().reject_ack = false;
    process = Process::start(directory.path(), "admin");
    base = format!("http://{}", process.ready()["admin"].as_str().unwrap());
    assert_eq!(
        request(
            &client,
            &base,
            reqwest::Method::POST,
            &format!("/api/reporting-mailboxes/{id}/check"),
            None
        )
        .status(),
        202
    );
    let recovered = wait_check(
        &client,
        &base,
        id,
        failed["history"][0]["id"].as_i64().unwrap(),
    );
    assert_eq!(recovered["code"], "passed");
    assert_eq!(recovered["history"][0]["rejected"], 2);
    assert_eq!(
        get(&client, &base, "/api/campaigns/91/analysis")["totals"][4],
        1
    );
    assert_eq!(peer.state.lock().unwrap().seen, vec![2]);
    for (path, code) in [
        ("/api/campaigns/91/analysis?limit=garbage", 400),
        ("/api/campaigns/91/export?format=exe", 400),
        ("/api/campaigns/999/analysis", 404),
    ] {
        assert_eq!(
            request(&client, &base, reqwest::Method::GET, path, None).status(),
            code
        );
    }
    for format in ["csv", "xlsx"] {
        let response = request(
            &client,
            &base,
            reqwest::Method::GET,
            &format!("/api/campaigns/91/export?format={format}"),
            None,
        );
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert!(
            !response
                .bytes()
                .unwrap()
                .windows(18)
                .any(|b| b == b"PARTICIPANT-CANARY")
        );
    }
    peer.state.lock().unwrap().validity = 42;
    request(
        &client,
        &base,
        reqwest::Method::POST,
        &format!("/api/reporting-mailboxes/{id}/check"),
        None,
    );
    let reset = wait_check(
        &client,
        &base,
        id,
        recovered["history"][0]["id"].as_i64().unwrap(),
    );
    assert_eq!(reset["code"], "mailbox_uidvalidity_changed");
    assert_eq!(
        request(
            &client,
            &base,
            reqwest::Method::POST,
            &format!("/api/reporting-mailboxes/{id}/resume"),
            Some(&json!({"revision":reset["revision"]}))
        )
        .status(),
        200
    );
    let resumed = wait_check(
        &client,
        &base,
        id,
        reset["history"][0]["id"].as_i64().unwrap(),
    );
    assert_eq!(resumed["code"], "passed");
    assert_eq!(
        get(&client, &base, "/api/campaigns/91/analysis")["totals"][4],
        1
    );
    process.stop();
    let log = std::fs::read_to_string(directory.path().join("server.log")).unwrap();
    for secret in ["MAILBOX-SECRET-CANARY", "PARTICIPANT-CANARY"] {
        assert!(!log.contains(secret));
        assert!(!process.output().contains(secret));
    }
    for row in database(directory.path(), "SELECT details FROM events") {
        assert!(
            !row.get::<String, _>("details")
                .contains("PARTICIPANT-CANARY")
        );
    }
}

#[test]
fn reporting_api_rejects_cross_owner_admin_only_stale_and_malformed_requests() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = format!("http://{}", process.ready()["admin"].as_str().unwrap());
    let client = client();
    database(
        directory.path(),
        "UPDATE users SET password_change_required=0; INSERT INTO users(id,username,api_key,role_id,password_change_required,account_locked) VALUES(2,'other','other-key',2,0,0); INSERT INTO ruphish_auth_state VALUES(2,0); INSERT INTO campaigns(id,user_id,name,url,status) VALUES(91,1,'Owner report','https://training.test','Completed'); INSERT INTO results(id,campaign_id,user_id,r_id,email,status,reported) VALUES(91,91,1,'abc123','person@example.test','Email Sent',1);",
    );
    for path in ["/api/campaigns/91/analysis", "/api/campaigns/91/export"] {
        assert_eq!(
            client
                .get(format!("{base}{path}"))
                .bearer_auth("other-key")
                .send()
                .unwrap()
                .status(),
            404
        );
        assert_eq!(
            client.get(format!("{base}{path}")).send().unwrap().status(),
            401
        );
    }
    assert_eq!(
        client
            .get(format!("{base}/api/webhooks/"))
            .bearer_auth("other-key")
            .send()
            .unwrap()
            .status(),
        403
    );
    let hook = json!({"name":"Receiver","url":"http://127.0.0.1:1/events","secret":"HOOK-SECRET-CANARY","is_active":false});
    let saved = request(
        &client,
        &base,
        reqwest::Method::POST,
        "/api/webhooks/",
        Some(&hook),
    );
    assert_eq!(saved.status(), 201);
    let saved: Value = saved.json().unwrap();
    assert!(saved.get("secret").is_none());
    let id = saved["id"].as_i64().unwrap();
    let mut changed = hook.clone();
    changed["revision"] = saved["revision"].clone();
    changed["name"] = json!("Updated receiver");
    changed.as_object_mut().unwrap().remove("secret");
    assert_eq!(
        request(
            &client,
            &base,
            reqwest::Method::PUT,
            &format!("/api/webhooks/{id}"),
            Some(&changed)
        )
        .status(),
        200
    );
    assert_eq!(
        request(
            &client,
            &base,
            reqwest::Method::PUT,
            &format!("/api/webhooks/{id}"),
            Some(&changed)
        )
        .status(),
        412
    );
    assert_eq!(
        client
            .delete(format!("{base}/api/webhooks/{id}"))
            .bearer_auth("synthetic-bootstrap-token")
            .header("if-match", "1")
            .send()
            .unwrap()
            .status(),
        412
    );
    for body in [
        json!([hook.clone()]),
        json!({"name":"broken","url":4}),
        json!({"name":"broken","url":"https://receiver.test","secret":"x","password":"INPUT-CANARY"}),
    ] {
        let response = request(
            &client,
            &base,
            reqwest::Method::POST,
            "/api/webhooks/",
            Some(&body),
        );
        assert_eq!(response.status(), 400);
        assert!(!response.text().unwrap().contains("INPUT-CANARY"));
    }
    let mailbox = json!({"name":"Owner mailbox","enabled":false,"host":"127.0.0.1","port":993,"username":"service","password":"MAILBOX-SECRET-CANARY","tls":true,"folder":"INBOX","frequency":60});
    let saved = request(
        &client,
        &base,
        reqwest::Method::POST,
        "/api/reporting-mailboxes/",
        Some(&mailbox),
    );
    assert_eq!(saved.status(), 201);
    let saved: Value = saved.json().unwrap();
    let id = saved["id"].as_i64().unwrap();
    for method in [
        reqwest::Method::GET,
        reqwest::Method::PUT,
        reqwest::Method::DELETE,
    ] {
        let response = client
            .request(method, format!("{base}/api/reporting-mailboxes/{id}"))
            .bearer_auth("other-key")
            .json(&mailbox)
            .send()
            .unwrap();
        assert_eq!(response.status(), 404);
    }
    for suffix in ["validate", "check"] {
        assert_eq!(
            client
                .post(format!("{base}/api/reporting-mailboxes/{id}/{suffix}"))
                .bearer_auth("other-key")
                .send()
                .unwrap()
                .status(),
            404
        );
    }
    let legacy_view = get(&client, &base, "/api/imap/");
    assert_eq!(legacy_view.as_array().unwrap().len(), 1);
    assert!(legacy_view[0]["port"].is_string());
    assert!(legacy_view[0]["imap_freq"].is_string());
    assert!(legacy_view[0]["modified_date"].is_string());
    assert!(legacy_view[0]["last_login"].is_string());
    assert_eq!(legacy_view[0]["password"], "");
    assert_eq!(
        request(
            &client,
            &base,
            reqwest::Method::GET,
            "/api/imap/validate",
            None
        )
        .status(),
        400
    );
    let legacy = json!({"host":"host\r\nLOGIN INJECTED-CANARY","port":"993","username":"service","password":"INPUT-CANARY","tls":true});
    let rejected = request(
        &client,
        &base,
        reqwest::Method::POST,
        "/api/imap/validate",
        Some(&legacy),
    );
    assert_eq!(rejected.status(), 200);
    let rejected: Value = rejected.json().unwrap();
    assert_eq!(rejected["success"], false);
    assert!(!rejected.to_string().contains("CANARY"));
    assert_eq!(
        get(&client, &base, "/api/reporting-mailboxes/")
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let analysis = get(&client, &base, "/api/campaigns/91/analysis");
    assert_eq!(analysis["recipients"][0]["historical_reported"], true);
    assert_eq!(analysis["unique"][4], 0);
    process.stop();
    let log = std::fs::read_to_string(directory.path().join("server.log")).unwrap();
    assert!(!log.contains("CANARY"));
}

#[test]
fn analysis_multiselect_filters_chart_pages_and_exports_and_rejects_invalid_identity() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = format!("http://{}", process.ready()["admin"].as_str().unwrap());
    let client = client();
    database(directory.path(), "UPDATE users SET password_change_required=0;
        INSERT INTO campaigns(id,user_id,name,url,status) VALUES(91,1,'Analysis selection','https://training.test','Completed'),(92,1,'Other campaign','https://training.test','Completed');
        INSERT INTO results(id,campaign_id,user_id,r_id,email,status,reported) VALUES
        (91,91,1,'r91','a@example.test','Email Sent',0),(92,91,1,'r92','b@example.test','Email Sent',0),(93,91,1,'r93','c@example.test','Email Sent',0),(94,92,1,'r94','outside@example.test','Email Sent',0);
        INSERT INTO events(campaign_id,email,time,message,details) VALUES
        (91,'a@example.test','2026-10-01T12:00:01Z','Email Opened','{}'),
        (91,'b@example.test','2026-10-02T12:00:42Z','Clicked Link','{}'),
        (91,'c@example.test','2026-10-03T12:00:59Z','Email Opened','{}'),
        (91,'a@example.test','2026-10-31T12:00:01Z','Email Reported','{}');");
    database(
        directory.path(),
        "UPDATE results SET first_name='Alice',last_name='Example' WHERE id=91",
    );
    let by_name = get(
        &client,
        &base,
        "/api/campaigns/91/analysis?search=ALICE%20EXAMPLE",
    );
    assert_eq!(by_name["recipient_total"], 1);
    assert_eq!(by_name["recipients"][0]["id"], 91);
    let by_email = get(
        &client,
        &base,
        "/api/campaigns/91/analysis?search=b%40example.test",
    );
    assert_eq!(by_email["recipient_total"], 1);
    assert_eq!(by_email["recipients"][0]["id"], 92);
    let selected = get(
        &client,
        &base,
        "/api/campaigns/91/analysis?recipients=91,92&limit=1",
    );
    assert_eq!(selected["recipient_total"], 2);
    assert_eq!(selected["event_total"], 3);
    assert_eq!(selected["unique"], json!([0, 1, 1, 0, 1]));
    assert_eq!(selected["recipients"][0]["id"], 91);
    let next = get(
        &client,
        &base,
        "/api/campaigns/91/analysis?recipients=91,92&limit=1&offset=1",
    );
    assert_eq!(next["recipients"][0]["id"], 92);
    assert_eq!(next["totals"], selected["totals"]);
    let buckets = selected["buckets"].as_array().unwrap();
    assert!(buckets.len() <= 102);
    assert!(
        buckets
            .iter()
            .all(|b| b["time"].as_i64().unwrap() % 60 == 0)
    );
    assert!(
        buckets
            .iter()
            .any(|b| b["counts"] == json!([0, 0, 0, 0, 0])),
        "Quiet intervals must not be bridged as sustained activity"
    );
    let mut sum = [0_i64; 5];
    for bucket in buckets {
        for (index, value) in sum.iter_mut().enumerate() {
            *value += bucket["counts"][index].as_i64().unwrap();
        }
    }
    assert_eq!(sum, [0, 1, 1, 0, 1]);
    assert_eq!(selected["bucket_seconds"].as_i64().unwrap() % 60, 0);
    let duplicate = get(
        &client,
        &base,
        "/api/campaigns/91/analysis?recipients=91,91",
    );
    assert_eq!(duplicate["recipient_total"], 1);
    assert_eq!(duplicate["event_total"], 2);
    let intersect = get(
        &client,
        &base,
        "/api/campaigns/91/analysis?recipients=91,92&recipient=92&event=Clicked%20Link",
    );
    assert_eq!(intersect["event_total"], 1);
    let csv = request(
        &client,
        &base,
        reqwest::Method::GET,
        "/api/campaigns/91/export?recipients=91,92&format=csv&scope=events",
        None,
    );
    assert_eq!(csv.status(), 200);
    let csv = csv.text().unwrap();
    assert!(csv.contains("a@example.test") && csv.contains("b@example.test"));
    assert!(!csv.contains("c@example.test") && !csv.contains("outside@example.test"));
    let excel = request(
        &client,
        &base,
        reqwest::Method::GET,
        "/api/campaigns/91/export?recipients=91,92&format=xlsx&scope=all",
        None,
    );
    assert_eq!(excel.status(), 200);
    assert!(excel.bytes().unwrap().starts_with(b"PK"));
    for (query, status) in [
        ("recipients=91,94", 404),
        ("recipients=91,999999", 404),
        ("recipients=91,x", 400),
        ("recipients=91,", 400),
        ("recipients=0", 400),
        ("recipients=-1", 400),
        ("recipients=91%20OR%201=1", 400),
    ] {
        for route in ["analysis", "export"] {
            let response = request(
                &client,
                &base,
                reqwest::Method::GET,
                &format!("/api/campaigns/91/{route}?{query}"),
                None,
            );
            assert_eq!(response.status().as_u16(), status, "{route}?{query}");
        }
    }
    let excessive = std::iter::repeat_n("91", 201).collect::<Vec<_>>().join(",");
    assert_eq!(
        request(
            &client,
            &base,
            reqwest::Method::GET,
            &format!("/api/campaigns/91/analysis?recipients={excessive}"),
            None
        )
        .status(),
        400
    );
    process.stop();
}
