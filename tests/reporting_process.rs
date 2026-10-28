//! Real executable reporting, integration APIs, verified IMAP and restart behavior.
use super::mailbox;
use super::*;
use sqlx::Row;
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
    let value: Value = response.json().unwrap();
    if path.split('?').next().unwrap_or(path).ends_with("/results") {
        let snapshot: ruphish_contracts::campaign_reads::Results =
            serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(snapshot).unwrap(), value);
    }
    value
}

#[test]
fn event_edits_enforce_csrf_scope_closed_metadata_and_replay_after_restart() {
    let directory = workspace();
    write_config(directory.path(), &configuration());
    let mut process = Process::start(directory.path(), "admin");
    let mut base = format!("http://{}", process.ready()["admin"].as_str().unwrap());
    let browser = super::auth_process::browser();
    let session = super::auth_process::setup(&browser, &base);
    database(
        directory.path(),
        "INSERT INTO users(id,username,api_key,role_id,password_change_required,account_locked) VALUES(2,'other-events','other-event-key',2,0,0); INSERT INTO ruphish_auth_state VALUES(2,0); INSERT INTO campaigns(id,user_id,name,status,launch_date,completed_date) VALUES(91,1,'Event edits','Completed','2026-01-01T00:00:00Z','2026-09-01T00:00:00Z'),(92,2,'Other','Completed','2026-01-01T00:00:00Z','2026-09-01T00:00:00Z'); INSERT INTO results(id,campaign_id,user_id,r_id,email,status) VALUES(91,91,1,'event-http','a@example.test','Email Sent'),(92,92,2,'event-other','b@example.test','Email Sent'); INSERT INTO events(id,campaign_id,recipient_id,email,time,kind,metadata,source) VALUES(901,91,91,'a@example.test','2026-06-01T00:00:00Z','link_clicked','{}','public'),(902,91,91,'a@example.test','2026-06-01T00:00:01Z','email_sent','{}','application');",
    );
    let api = client();
    let path = "/api/campaigns/91/recipients/91/events";
    let change = json!({"request_id":"http-event-correction-1","event_id":901,"revision":0,"action":"edit","value":{"kind":"form_submitted","time":"2026-06-02T03:00:00+03:00","presence":{"username_present":false,"password_present":true}}});
    assert_eq!(
        api.post(format!("{base}{path}"))
            .json(&change)
            .send()
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        browser
            .post(format!("{base}{path}"))
            .header("origin", &base)
            .json(&change)
            .send()
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        api.post(format!("{base}{path}"))
            .bearer_auth("other-event-key")
            .json(&change)
            .send()
            .unwrap()
            .status(),
        404
    );
    for invalid in [
        json!({"request_id":"invalid-participant-value","event_id":901,"revision":0,"action":"edit","value":{"kind":"form_submitted","time":"2026-06-02T00:00:00Z","presence":{"password":"forbidden-participant-value"}}}),
        json!({"request_id":"invalid-lifecycle-event","event_id":901,"revision":0,"action":"edit","value":{"kind":"email_sent","time":"2026-06-02T00:00:00Z","presence":{}}}),
        json!({"request_id":"invalid-presence-value","event_id":901,"revision":0,"action":"edit","value":{"kind":"form_submitted","time":"2026-06-02T00:00:00Z","presence":{"password_present":"true"}}}),
        json!({"request_id":"readonly-smtp-event","event_id":902,"revision":0,"action":"delete","value":null}),
    ] {
        assert_eq!(
            request(&api, &base, reqwest::Method::POST, path, Some(&invalid)).status(),
            400
        );
    }
    let duplicate = r#"{"request_id":"duplicate-json-fields","event_id":901,"revision":0,"revision":1,"action":"delete","value":null}"#;
    assert_eq!(
        api.post(format!("{base}{path}"))
            .bearer_auth("synthetic-bootstrap-token")
            .header("content-type", "application/json")
            .body(duplicate)
            .send()
            .unwrap()
            .status(),
        400
    );
    let saved = browser
        .post(format!("{base}{path}"))
        .header("origin", &base)
        .header("x-csrf-token", session["csrf_token"].as_str().unwrap())
        .json(&change)
        .send()
        .unwrap();
    assert_eq!(saved.status(), 200, "{}", saved.text().unwrap());
    let mut stale = change.clone();
    stale["request_id"] = json!("http-stale-event-revision");
    stale["value"]["time"] = json!("2026-06-04T00:00:00Z");
    assert_eq!(
        request(&api, &base, reqwest::Method::POST, path, Some(&stale)).status(),
        412
    );
    let mut reused = change.clone();
    reused["value"]["time"] = json!("2026-06-03T00:00:00Z");
    assert_eq!(
        request(&api, &base, reqwest::Method::POST, path, Some(&reused)).status(),
        412
    );
    let history = get(&api, &base, "/api/campaigns/91/recipients/91/history");
    let event = history["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == 901)
        .unwrap();
    assert_eq!(event["time"], "2026-06-02T00:00:00Z");
    assert_eq!(
        event["record"]["current"]["kind"]["value"],
        "form_submitted"
    );
    assert_eq!(event["record"]["source"], "public");
    assert_eq!(event["record"]["edited"], true);
    let results = get(&api, &base, "/api/campaigns/91/results");
    assert_eq!(results["totals"]["clicked"], 0);
    assert_eq!(results["totals"]["submitted"], 1);
    assert_eq!(results["progression"]["clicked"], 1);
    let filtered = get(
        &api,
        &base,
        "/api/campaigns/91/results?from=1780358400&until=1780444800",
    );
    assert_eq!(filtered["totals"]["submitted"], 1);
    assert!(
        filtered["buckets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["counts"]["submitted"] == 1)
    );
    process.stop();
    process = Process::start(directory.path(), "admin");
    base = format!("http://{}", process.ready()["admin"].as_str().unwrap());
    let replay = request(&api, &base, reqwest::Method::POST, path, Some(&change));
    assert_eq!(replay.status(), 200);
    let saved: Value = replay.json().unwrap();
    assert_eq!(saved["event"]["revision"], 1);
    assert_eq!(saved["event"]["edited"], true);
    assert!(!saved.to_string().contains("forbidden-participant-value"));
    assert_eq!(
        request(
            &api,
            &base,
            reqwest::Method::GET,
            "/api/campaigns/91/recipients/91/events/901",
            None
        )
        .status(),
        404
    );
    database(
        directory.path(),
        "INSERT INTO events(id,campaign_id,recipient_id,email,time,kind,metadata,source) VALUES(903,92,92,'b@example.test','2026-06-01T00:00:00Z','link_clicked','{}','public');",
    );
    assert_eq!(
        super::auth_process::post(
            &browser,
            &base,
            "/auth/impersonate",
            &[("username", "other-events")]
        )
        .status(),
        200
    );
    let impersonated = super::auth_process::bootstrap(&browser, &base);
    let other_change = json!({"request_id":"impersonated-correction-key","event_id":903,"revision":0,"action":"delete","value":null});
    let corrected = browser
        .post(format!("{base}/api/campaigns/92/recipients/92/events"))
        .header("origin", &base)
        .header("x-csrf-token", impersonated["csrf_token"].as_str().unwrap())
        .json(&other_change)
        .send()
        .unwrap();
    assert_eq!(corrected.status(), 200, "{}", corrected.text().unwrap());
    let history: Value = api
        .get(format!("{base}/api/campaigns/92/recipients/92/history"))
        .bearer_auth("other-event-key")
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert!(history["events"].as_array().unwrap().is_empty());
    database(
        directory.path(),
        "DELETE FROM role_permissions WHERE permission_id=(SELECT id FROM permissions WHERE slug='modify_objects');",
    );
    assert_eq!(
        request(&api, &base, reqwest::Method::POST, path, Some(&change)).status(),
        403
    );
    assert_eq!(
        get(&api, &base, "/api/campaigns/91/results")["events"][0]["edited"],
        true
    );
    database(
        directory.path(),
        "UPDATE users SET account_locked=1 WHERE id=1;",
    );
    assert_eq!(
        request(&api, &base, reqwest::Method::POST, path, Some(&change)).status(),
        401
    );
    process.stop();
}

#[test]
fn sorted_analysis_exports_and_delivery_cursors_survive_rejections_deletion_and_restart() {
    let directory = workspace();
    write_config(directory.path(), &configuration());
    let mut process = Process::start(directory.path(), "admin");
    let mut base = format!("http://{}", process.ready()["admin"].as_str().unwrap());
    let client = client();
    let mut setup = String::from(
        "UPDATE users SET password_change_required=0; INSERT INTO users(id,username,api_key,role_id,password_change_required,account_locked) VALUES(2,'other-sort','other-sort-key',2,0,0); INSERT INTO ruphish_auth_state VALUES(2,0); INSERT INTO campaigns(id,user_id,name,status) VALUES(91,1,'Sorted','Completed'),(92,1,'Empty','Completed'),(93,2,'Other owner','Completed');",
    );
    for index in 0..55 {
        let id = 1000 + index;
        setup.push_str(&format!("INSERT INTO results(id,campaign_id,user_id,r_id,email,first_name,last_name,status,reported) VALUES({id},91,1,'sort-{index}','person{index:03}@example.test','Same','','Email Sent',0); INSERT INTO ruphish_delivery(result_id,campaign_id,state,due) VALUES({id},91,'accepted',{index});"));
    }
    setup.push_str("INSERT INTO results(id,campaign_id,user_id,r_id,email,status,reported) VALUES(2000,93,2,'other-sort','other@example.test','Email Sent',0);");
    database(directory.path(), &setup);
    assert_eq!(
        client
            .get(format!("{base}/api/campaigns/91/results?sort=email"))
            .send()
            .unwrap()
            .status(),
        401
    );
    let page = get(
        &client,
        &base,
        "/api/campaigns/91/results?sort=email&order=desc&offset=50&limit=5",
    );
    assert_eq!(page["recipient_total"], 55);
    assert_eq!(
        page["recipients"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["id"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        [1004, 1003, 1002, 1001, 1000]
    );
    let exported = request(
        &client,
        &base,
        reqwest::Method::GET,
        "/api/campaigns/91/export?format=csv&scope=recipients&sort=email&order=desc&offset=50&limit=5",
        None,
    );
    assert_eq!(exported.status(), 200);
    let csv = exported.text().unwrap();
    assert!(csv.starts_with("Recipient ID,"));
    let ids: Vec<i64> = csv
        .lines()
        .skip(1)
        .map(|line| line.split(',').next().unwrap().parse().unwrap())
        .collect();
    assert_eq!(ids, (1000..1055).rev().collect::<Vec<_>>());
    for path in [
        "/api/campaigns/91/results?sort=unknown",
        "/api/campaigns/91/results?sort=email%3BDROP%20TABLE%20results",
        "/api/campaigns/91/results?order=descending",
        "/api/campaigns/91/results?event_sort=unknown",
        "/api/campaigns/91/export?format=csv&scope=recipients&sort=unknown",
    ] {
        assert_eq!(
            request(&client, &base, reqwest::Method::GET, path, None).status(),
            400,
            "{path}"
        );
    }
    assert_eq!(
        request(
            &client,
            &base,
            reqwest::Method::GET,
            "/api/campaigns/93/results?sort=email",
            None
        )
        .status(),
        404
    );
    assert_eq!(
        request(
            &client,
            &base,
            reqwest::Method::GET,
            "/api/campaigns/91/results?sort=name&recipient=2000",
            None
        )
        .status(),
        404
    );
    assert_eq!(
        client
            .get(format!("{base}/api/campaigns/91/delivery?sort=email"))
            .bearer_auth("other-sort-key")
            .send()
            .unwrap()
            .status(),
        404
    );
    let first = get(
        &client,
        &base,
        "/api/campaigns/91/delivery?sort=email&order=desc&limit=2",
    );
    assert_eq!(first["items"][0]["id"], 1054);
    assert_eq!(first["items"][1]["id"], 1053);
    let cursor = first["next_cursor"].as_str().unwrap();
    let mut cursor_url = reqwest::Url::parse("http://fixture.test").unwrap();
    cursor_url.query_pairs_mut().append_pair("cursor", cursor);
    let encoded = cursor_url.query().unwrap().to_owned();
    database(
        directory.path(),
        "DELETE FROM ruphish_delivery WHERE result_id=1053; DELETE FROM results WHERE id=1053;",
    );
    let second = get(
        &client,
        &base,
        &format!("/api/campaigns/91/delivery?sort=email&order=desc&limit=2&{encoded}"),
    );
    assert_eq!(second["items"][0]["id"], 1052);
    assert_eq!(second["items"][1]["id"], 1051);
    for path in [
        format!("/api/campaigns/91/delivery?sort=email&order=asc&{encoded}"),
        format!("/api/campaigns/91/delivery?sort=due&order=desc&{encoded}"),
        format!("/api/campaigns/92/delivery?sort=email&order=desc&{encoded}"),
        format!("/api/campaigns/91/delivery?{encoded}"),
        format!("/api/campaigns/91/delivery?sort=email&order=desc&after=1&{encoded}"),
        "/api/campaigns/91/delivery?sort=email&cursor=%7B".into(),
    ] {
        assert_eq!(
            request(&client, &base, reqwest::Method::GET, &path, None).status(),
            400,
            "{path}"
        );
    }
    process.stop();
    let mut process = Process::start(directory.path(), "admin");
    base = format!("http://{}", process.ready()["admin"].as_str().unwrap());
    let resumed = get(
        &client,
        &base,
        &format!("/api/campaigns/91/delivery?sort=email&order=desc&limit=2&{encoded}"),
    );
    assert_eq!(resumed["items"], second["items"]);
    assert_eq!(
        get(
            &client,
            &base,
            "/api/campaigns/92/results?sort=opened&order=desc"
        )["recipient_total"],
        0
    );
    process.stop();
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
        "UPDATE users SET password_change_required=0; UPDATE ruphish_settings SET reporting_mailboxes_enabled=1; INSERT INTO campaigns(id,user_id,name,url,status) VALUES(91,1,'Reporting test','https://training.test/','Completed'); INSERT INTO results(id,campaign_id,user_id,r_id,email,status,reported) VALUES(91,91,1,'abc123','person@example.test','Email Sent',0);",
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
    let validated = request(
        &client,
        &base,
        reqwest::Method::POST,
        &format!("/api/reporting-mailboxes/{id}/validate"),
        None,
    );
    assert_eq!(validated.status(), 200);
    let validated: Value = validated.json().unwrap();
    assert_eq!(validated["id"], id);
    assert_eq!(validated["name"], "Reports");
    assert_eq!(validated["has_password"], true);
    assert!(validated.get("password").is_none());
    assert!(serde_json::from_value::<ruphish_contracts::integrations::Mailbox>(validated).is_ok());

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
    let queued = request(
        &client,
        &base,
        reqwest::Method::POST,
        &format!("/api/reporting-mailboxes/{id}/check"),
        None,
    );
    assert_eq!(queued.status(), 202);
    let queued: Value = queued.json().unwrap();
    assert_eq!(queued["id"], id);
    assert_eq!(queued["enabled"], true);
    assert!(queued.get("password").is_none());
    assert!(serde_json::from_value::<ruphish_contracts::integrations::Mailbox>(queued).is_ok());
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
    let analysis = get(&client, &base, "/api/campaigns/91/results");
    assert_eq!(analysis["totals"]["reported"], 1);
    assert_eq!(analysis["events"][0]["source"], "mailbox");
    assert_eq!(analysis["unique"]["opened"], 0);
    assert_eq!(analysis["unique"]["clicked"], 0);
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
        get(&client, &base, "/api/campaigns/91/results")["totals"]["reported"],
        1
    );
    assert_eq!(peer.state.lock().unwrap().seen, vec![2]);
    for (path, code) in [
        ("/api/campaigns/91/results?limit=garbage", 400),
        ("/api/campaigns/91/export?format=exe", 400),
        ("/api/campaigns/999/results", 404),
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
    let resumed_response = request(
        &client,
        &base,
        reqwest::Method::POST,
        &format!("/api/reporting-mailboxes/{id}/resume"),
        Some(&json!({"revision":reset["revision"]})),
    );
    assert_eq!(resumed_response.status(), 200);
    let resumed_response: Value = resumed_response.json().unwrap();
    assert_eq!(resumed_response["id"], id);
    assert_eq!(resumed_response["enabled"], true);
    assert!(resumed_response.get("password").is_none());
    assert!(
        serde_json::from_value::<ruphish_contracts::integrations::Mailbox>(resumed_response)
            .is_ok()
    );
    let resumed = wait_check(
        &client,
        &base,
        id,
        reset["history"][0]["id"].as_i64().unwrap(),
    );
    assert_eq!(resumed["code"], "passed");
    assert_eq!(
        get(&client, &base, "/api/campaigns/91/results")["totals"]["reported"],
        1
    );
    process.stop();
    let log = std::fs::read_to_string(directory.path().join("server.log")).unwrap();
    for secret in ["MAILBOX-SECRET-CANARY", "PARTICIPANT-CANARY"] {
        assert!(!log.contains(secret));
        assert!(!process.output().contains(secret));
    }
    for row in database(directory.path(), "SELECT details FROM event_history") {
        assert!(
            !row.get::<String, _>("details")
                .contains("PARTICIPANT-CANARY")
        );
    }
}

#[test]
fn mailbox_certificate_policy_covers_tls_modes_and_restart() {
    for (name, expired, trusted, starttls, verified) in [
        ("127.0.0.1", false, true, false, true),
        ("127.0.0.1", false, false, false, false),
        ("wrong.example.test", false, true, false, false),
        ("127.0.0.1", true, true, false, false),
        ("127.0.0.1", false, false, true, false),
    ] {
        let directory = workspace();
        let peer = mailbox::Mailbox::configured(name, expired, starttls);
        if trusted {
            std::fs::write(directory.path().join("mailbox-ca.pem"), &peer.certificate).unwrap();
        }
        let mut config = configuration();
        config["logging"]["level"] = json!("trace");
        write_config(directory.path(), &config);
        let mut process = Process::start(directory.path(), "admin");
        let mut base = format!("http://{}", process.ready()["admin"].as_str().unwrap());
        database(
            directory.path(),
            "UPDATE users SET password_change_required=0; UPDATE ruphish_settings SET reporting_mailboxes_enabled=1;",
        );
        let client = client();
        let mut draft = json!({"name":"Certificate policy","enabled":false,"host":"127.0.0.1","port":peer.address.port(),"username":"service","password":"MAILBOX-SECRET-CANARY","tls":!starttls,"folder":"INBOX","frequency":60});
        let response = request(
            &client,
            &base,
            reqwest::Method::POST,
            "/api/reporting-mailboxes/",
            Some(&draft),
        );
        assert_eq!(response.status(), 201);
        let mut saved: Value = response.json().unwrap();
        assert_eq!(saved["ignore_cert_errors"], false);
        let id = saved["id"].as_i64().unwrap();
        let path = format!("/api/reporting-mailboxes/{id}");
        let response = request(
            &client,
            &base,
            reqwest::Method::POST,
            &format!("{path}/validate"),
            None,
        );
        assert_eq!(
            response.status().is_success(),
            verified,
            "{name}, expired={expired}, trusted={trusted}, starttls={starttls}"
        );
        assert_eq!(peer.state.lock().unwrap().logins, usize::from(verified));
        draft["revision"] = saved["revision"].clone();
        draft["ignore_cert_errors"] = json!(true);
        draft["enabled"] = json!(true);
        draft.as_object_mut().unwrap().remove("password");
        let response = request(&client, &base, reqwest::Method::PUT, &path, Some(&draft));
        assert_eq!(response.status(), 200);
        saved = response.json().unwrap();
        assert_eq!(saved["ignore_cert_errors"], true);
        assert!(saved.get("password").is_none());
        let checked = wait_check(&client, &base, id, 0);
        assert_eq!(checked["code"], "passed");
        assert_eq!(get(&client, &base, &path)["ignore_cert_errors"], true);
        process.stop();
        process = Process::start(directory.path(), "admin");
        base = format!("http://{}", process.ready()["admin"].as_str().unwrap());
        saved = get(&client, &base, &path);
        assert_eq!(saved["ignore_cert_errors"], true);
        assert_eq!(
            request(
                &client,
                &base,
                reqwest::Method::POST,
                &format!("{path}/validate"),
                None
            )
            .status(),
            200
        );
        // Disabling bypass restores verification and preserves the scan checkpoint.
        let checkpoint = database(
            directory.path(),
            "SELECT uidvalidity,last_uid,initialized FROM ruphish_mailboxes",
        );
        draft["revision"] = saved["revision"].clone();
        draft["ignore_cert_errors"] = json!(false);
        draft["enabled"] = json!(false);
        assert_eq!(
            request(&client, &base, reqwest::Method::PUT, &path, Some(&draft)).status(),
            200
        );
        let after = database(
            directory.path(),
            "SELECT uidvalidity,last_uid,initialized FROM ruphish_mailboxes",
        );
        for field in ["uidvalidity", "last_uid", "initialized"] {
            assert_eq!(
                checkpoint[0].get::<i64, _>(field),
                after[0].get::<i64, _>(field)
            );
        }
        assert_eq!(
            request(
                &client,
                &base,
                reqwest::Method::POST,
                &format!("{path}/validate"),
                None
            )
            .status()
            .is_success(),
            verified
        );
        process.stop();
        let log = std::fs::read_to_string(directory.path().join("server.log")).unwrap();
        assert!(!log.contains("MAILBOX-SECRET-CANARY"));
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
        "UPDATE users SET password_change_required=0; UPDATE ruphish_settings SET reporting_mailboxes_enabled=1; INSERT INTO users(id,username,api_key,role_id,password_change_required,account_locked) VALUES(2,'other','other-key',2,0,0); INSERT INTO ruphish_auth_state VALUES(2,0); INSERT INTO campaigns(id,user_id,name,url,status) VALUES(91,1,'Owner report','https://training.test','Completed'); INSERT INTO results(id,campaign_id,user_id,r_id,email,status,reported) VALUES(91,91,1,'abc123','person@example.test','Email Sent',1);",
    );
    for path in ["/api/campaigns/91/results", "/api/campaigns/91/export"] {
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
    let hook_id = id;
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
    let mailbox = json!({"name":"Owner mailbox","ignore_cert_errors":true,"enabled":false,"host":"127.0.0.1","port":993,"username":"service","password":"MAILBOX-SECRET-CANARY","tls":true,"folder":"INBOX","frequency":60});
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
    for path in ["/api/imap/", "/api/imap/validate"] {
        for method in [reqwest::Method::GET, reqwest::Method::POST] {
            let response = request(&client, &base, method, path, Some(&mailbox));
            assert_eq!(response.status(), 404);
        }
    }
    assert_eq!(
        get(&client, &base, &format!("/api/reporting-mailboxes/{id}")),
        saved
    );
    let mut invalid = mailbox.clone();
    invalid["host"] = json!("host\r\nLOGIN INJECTED-CANARY");
    invalid["password"] = json!("INPUT-CANARY");
    let rejected = request(
        &client,
        &base,
        reqwest::Method::POST,
        "/api/reporting-mailboxes/",
        Some(&invalid),
    );
    assert_eq!(rejected.status(), 400);
    assert!(!rejected.text().unwrap().contains("CANARY"));
    assert_eq!(
        get(&client, &base, "/api/reporting-mailboxes/")
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let analysis = get(&client, &base, "/api/campaigns/91/results");
    assert_eq!(analysis["recipients"][0]["historical_reported"], true);
    assert_eq!(analysis["unique"]["reported"], 0);
    let mailbox_path = format!("{base}/api/reporting-mailboxes/{id}");
    for (key, revision, status) in [
        ("other-key", "1", 404),
        ("synthetic-bootstrap-token", "2", 412),
        ("synthetic-bootstrap-token", "1", 200),
        ("synthetic-bootstrap-token", "1", 404),
    ] {
        assert_eq!(
            client
                .delete(&mailbox_path)
                .bearer_auth(key)
                .header("if-match", revision)
                .send()
                .unwrap()
                .status(),
            status
        );
    }
    // Legacy clients can delete without fetching or supplying a revision.
    for status in [200, 404] {
        assert_eq!(
            request(
                &client,
                &base,
                reqwest::Method::DELETE,
                &format!("/api/webhooks/{hook_id}"),
                None,
            )
            .status(),
            status
        );
    }
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
        INSERT INTO events(campaign_id,email,time,kind,metadata) VALUES
        (91,'a@example.test','2026-10-01T12:00:01Z','email_opened','{}'),
        (91,'b@example.test','2026-10-02T12:00:42Z','link_clicked','{}'),
        (91,'c@example.test','2026-10-03T12:00:59Z','email_opened','{}'),
        (91,'a@example.test','2026-10-31T12:00:01Z','email_reported','{}'); UPDATE events SET recipient_id=(SELECT min(r.id) FROM results r WHERE r.campaign_id=events.campaign_id AND r.email=events.email HAVING count(*)=1) WHERE recipient_id IS NULL;");
    database(
        directory.path(),
        "UPDATE results SET first_name='Alice',last_name='Example',position='=1+1',send_date='2026-10-01T14:00:00.123456789+02:00',modified_date='2026-10-02T01:00:00-03:00' WHERE id=91",
    );
    let by_name = get(
        &client,
        &base,
        "/api/campaigns/91/results?search=ALICE%20EXAMPLE",
    );
    assert_eq!(by_name["recipient_total"], 1);
    assert_eq!(by_name["recipients"][0]["id"], 91);
    let by_email = get(
        &client,
        &base,
        "/api/campaigns/91/results?search=b%40example.test",
    );
    assert_eq!(by_email["recipient_total"], 1);
    assert_eq!(by_email["recipients"][0]["id"], 92);
    let selected = get(
        &client,
        &base,
        "/api/campaigns/91/results?recipients=91,92&limit=1",
    );
    assert_eq!(selected["recipient_total"], 2);
    assert_eq!(selected["event_total"], 3);
    assert_eq!(
        selected["unique"],
        json!({"accepted":0,"opened":1,"clicked":1,"submitted":0,"reported":1})
    );
    assert_eq!(selected["recipients"][0]["id"], 91);
    let next = get(
        &client,
        &base,
        "/api/campaigns/91/results?recipients=91,92&limit=1&offset=1",
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
        buckets.iter().any(|b| b["counts"]
            == json!({"accepted":0,"opened":0,"clicked":0,"submitted":0,"reported":0})),
        "Quiet intervals must not be bridged as sustained activity"
    );
    let mut sum = [0_i64; 5];
    for bucket in buckets {
        for (index, value) in sum.iter_mut().enumerate() {
            *value += bucket["counts"]
                [["accepted", "opened", "clicked", "submitted", "reported"][index]]
                .as_i64()
                .unwrap();
        }
    }
    assert_eq!(sum, [0, 1, 1, 0, 1]);
    assert_eq!(selected["bucket_seconds"].as_i64().unwrap() % 60, 0);
    let union = get(
        &client,
        &base,
        "/api/campaigns/91/results?recipients=91,92&event=email_opened,link_clicked&limit=1&offset=1",
    );
    assert_eq!(union["event_total"], 2);
    assert_eq!(
        union["totals"],
        json!({"accepted":0,"opened":1,"clicked":1,"submitted":0,"reported":0})
    );
    assert_eq!(union["first_observed"]["opened"], 1790856001_i64);
    assert_eq!(union["last_observed"]["clicked"], 1790942442_i64);
    assert!(union["first_observed"]["reported"].is_null());
    let all_union = get(
        &client,
        &base,
        "/api/campaigns/91/results?recipients=91,92&event=email_opened,link_clicked",
    );
    assert_eq!(union["buckets"], all_union["buckets"]);
    let union_export = request(
        &client,
        &base,
        reqwest::Method::GET,
        "/api/campaigns/91/export?recipients=91,92&event=email_opened,link_clicked&limit=1&offset=1&format=csv&scope=events",
        None,
    );
    assert_eq!(union_export.status(), 200);
    assert_eq!(union_export.text().unwrap().lines().count(), 3);
    let duplicate = get(&client, &base, "/api/campaigns/91/results?recipients=91,91");
    assert_eq!(duplicate["recipient_total"], 1);
    assert_eq!(duplicate["event_total"], 2);
    let intersect = get(
        &client,
        &base,
        "/api/campaigns/91/results?recipients=91,92&recipient=92&event=link_clicked",
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
    let recipients = request(
        &client,
        &base,
        reqwest::Method::GET,
        "/api/campaigns/91/export?recipients=91,92&limit=1&offset=1&format=csv&scope=recipients",
        None,
    );
    assert_eq!(recipients.status(), 200);
    let csv = recipients.text().unwrap();
    let mut lines = csv.lines();
    assert!(
        lines
            .next()
            .unwrap()
            .ends_with(",First Name,Last Name,Position,Send Date (UTC),Modified Date (UTC)")
    );
    let rows: Vec<_> = lines.collect();
    assert_eq!(
        rows.len(),
        2,
        "download must include all selected recipients"
    );
    assert!(
        rows[0]
            .ends_with(",Alice,Example,'=1+1,2026-10-01T12:00:00.123456789Z,2026-10-02T04:00:00Z")
    );
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
        ("event=Submitted%20Data", 400),
        ("event=unknown", 400),
        ("event=%27%20OR%201=1--", 400),
        ("source=unknown", 400),
    ] {
        for route in ["results", "export"] {
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
            &format!("/api/campaigns/91/results?recipients={excessive}"),
            None
        )
        .status(),
        400
    );
    let before = get(&client, &base, "/api/campaigns/91/results");
    database(
        directory.path(),
        "INSERT INTO events(campaign_id,recipient_id,email,time,kind,source,metadata) VALUES(91,91,'a@example.test','2026-10-04T00:00:00Z','Proxied request','legacy','{}')",
    );
    let after = get(&client, &base, "/api/campaigns/91/results");
    assert_eq!(
        after["totals"], before["totals"],
        "unrecognized history must not invent known actions"
    );
    assert_eq!(
        after["event_total"].as_i64().unwrap(),
        before["event_total"].as_i64().unwrap() + 1
    );
    assert!(after["events"].as_array().unwrap().iter().any(|e| e["kind"]
        == json!({"type":"historical","value":"Proxied request"})
        && e["source"] == "legacy"));
    let csv = request(
        &client,
        &base,
        reqwest::Method::GET,
        "/api/campaigns/91/export?scope=events",
        None,
    );
    assert_eq!(csv.status(), 200);
    assert!(csv.text().unwrap().contains("Proxied request"));
    process.stop();
}

#[test]
fn consolidated_campaign_reads_share_identity_scope_and_bounded_history() {
    let directory = workspace();
    write_config(directory.path(), &configuration());
    let mut process = Process::start(directory.path(), "admin");
    let base = format!("http://{}", process.ready()["admin"].as_str().unwrap());
    let client = client();
    database(
        directory.path(),
        "UPDATE users SET password_change_required=0; INSERT INTO users(id,username,api_key,role_id,password_change_required,account_locked) VALUES(2,'outside','outside-key',2,0,0); INSERT INTO ruphish_auth_state VALUES(2,0); INSERT INTO campaigns(id,user_id,name,status) VALUES(91,1,'Completed read','Completed'),(92,2,'Outside','Completed'); INSERT INTO results(id,campaign_id,user_id,r_id,email,status,reported) VALUES(91,91,1,'read-recipient','read@example.test','Email Sent',0),(92,92,2,'outside-recipient','outside@example.test','Email Sent',0);",
    );
    for index in 0..105 {
        database(
            directory.path(),
            &format!(
                "INSERT INTO events(campaign_id,recipient_id,email,time,kind,metadata,source) VALUES(91,91,'read@example.test','2026-01-01T00:00:{:02}Z','link_clicked','{{}}','public')",
                index % 60
            ),
        );
    }
    let context = get(&client, &base, "/api/campaigns/91/context");
    assert_eq!(context["id"], 91);
    assert_eq!(context["status"], "Completed");
    assert_eq!(context["total"], 1);
    assert!(context.get("results").is_none());
    let results = get(&client, &base, "/api/campaigns/91/results?limit=1");
    assert_eq!(results["totals"]["clicked"], 105);
    assert_eq!(results["unique"]["clicked"], 1);
    assert_eq!(results["events"].as_array().unwrap().len(), 1);
    let history = get(&client, &base, "/api/campaigns/91/recipients/91/history");
    let events = history["events"].as_array().unwrap();
    assert_eq!(events.len(), 100);
    let older = get(
        &client,
        &base,
        &format!(
            "/api/campaigns/91/recipients/91/history?before_event={}",
            history["next_event"]
        ),
    );
    assert_eq!(older["events"].as_array().unwrap().len(), 5);
    for path in [
        "/api/campaigns/91/context",
        "/api/campaigns/91/results",
        "/api/campaigns/91/recipients/91/history",
    ] {
        assert_eq!(
            client.get(format!("{base}{path}")).send().unwrap().status(),
            401
        );
        assert_eq!(
            client
                .get(format!("{base}{path}"))
                .bearer_auth("outside-key")
                .send()
                .unwrap()
                .status(),
            404
        );
    }
    for path in [
        "/api/campaigns/91/recipients/92/history",
        "/api/campaigns/999/context",
        "/api/campaigns/91/results?recipient=92",
    ] {
        assert_eq!(
            request(&client, &base, reqwest::Method::GET, path, None).status(),
            404
        );
    }
    assert_eq!(
        request(
            &client,
            &base,
            reqwest::Method::GET,
            "/api/campaigns/91/analysis",
            None
        )
        .status(),
        404
    );
    process.stop();
}
