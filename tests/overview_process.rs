//! Dashboard endpoints exercised through the real executable and durable database.
use super::*;
use ruphish_contracts::overview::{PostureResponse, Snapshot};
fn read(base: &str, path: &str, key: Option<&str>) -> reqwest::blocking::Response {
    let request = client().get(format!("{base}{path}"));
    let request = if let Some(key) = key {
        request.bearer_auth(key)
    } else {
        request
    };
    request.send().unwrap()
}
#[test]
fn dashboard_supports_authenticated_browser_and_api_reads_restart_and_revocation() {
    let dir = workspace();
    let mut process = Process::with_options(
        dir.path(),
        "admin",
        Some("synthetic-bootstrap-password"),
        Some("synthetic-bootstrap-token"),
        &["--disable-mailer"],
    );
    let mut base = format!("http://{}", process.ready()["admin"].as_str().unwrap());
    database(dir.path(),"UPDATE users SET password_change_required=0;
        INSERT INTO users(id,username,api_key,role_id,password_change_required,account_locked) VALUES(2,'other','other-key',2,0,0); INSERT INTO ruphish_auth_state VALUES(2,0);");
    for path in ["/api/overview", "/api/overview/posture"] {
        assert_eq!(read(&base, path, None).status(), 401);
        assert_eq!(read(&base, path, Some("wrong-key")).status(), 401);
    }
    let key = Some("synthetic-bootstrap-token");
    let response = read(&base, "/api/overview", key);
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let empty: Snapshot = response.json().unwrap();
    assert!(empty.recent.is_empty());
    database(
        dir.path(),
        r#"INSERT INTO campaigns(id,user_id,name,status,launch_date) VALUES(1,1,'Dashboard campaign','Completed',datetime('now','-1 day'));
        INSERT INTO results(id,campaign_id,user_id,r_id,email,status) VALUES(1,1,1,'dashboard-rid','private@example.test','Email Sent');
        INSERT INTO events(campaign_id,email,kind,time,metadata) VALUES(1,'private@example.test','email_sent',datetime('now','-1 day'),''),(1,'private@example.test','email_opened',datetime('now'),''),(1,'private@example.test','email_opened',datetime('now'),''),(1,'private@example.test','form_submitted',datetime('now'),'{"password_present":true,"password":"DASHBOARD-SECRET-CANARY"}'); UPDATE events SET recipient_id=(SELECT min(r.id) FROM results r WHERE r.campaign_id=events.campaign_id AND r.email=events.email HAVING count(*)=1) WHERE recipient_id IS NULL;"#,
    );
    database(
        dir.path(),
        r#"INSERT INTO results(id,campaign_id,user_id,r_id,email,status) VALUES(2,1,1,'previous-rid','previous@example.test','Email Sent');
        INSERT INTO events(campaign_id,email,kind,time,metadata) VALUES(1,'previous@example.test','email_sent',datetime('now','-10 days'),''),(1,'previous@example.test','email_opened',datetime('now','-9 days'),''),(1,'previous@example.test','email_reported',datetime('now'),''); UPDATE events SET recipient_id=(SELECT min(r.id) FROM results r WHERE r.campaign_id=events.campaign_id AND r.email=events.email HAVING count(*)=1) WHERE recipient_id IS NULL;"#,
    );
    let snapshot: Snapshot = read(&base, "/api/overview", key).json().unwrap();
    assert_eq!(snapshot.recent[0].outcomes.opened, 2);
    for path in [
        "/api/overview/posture",
        "/api/overview/posture?window=7d",
        "/api/overview/posture?window=30d",
        "/api/overview/posture?window=365d",
    ] {
        let response = read(&base, path, key);
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["cache-control"], "no-store");
        let text = response.text().unwrap();
        for secret in [
            "private@example.test",
            "DASHBOARD-SECRET-CANARY",
            "password_present",
            "details",
        ] {
            assert!(!text.contains(secret));
        }
        let payload: Value = serde_json::from_str(&text).unwrap();
        if let Some(counts) = payload["previous"]["counts"].as_object() {
            let mut fields: Vec<_> = counts.keys().map(String::as_str).collect();
            fields.sort_unstable();
            assert_eq!(
                fields,
                ["clicked", "credentials_entered", "opened", "reported"]
            );
        }
        let p: PostureResponse = serde_json::from_str(&text).unwrap();
        let expected = if p.window == "7d" { 1 } else { 2 };
        assert_eq!(p.counts.delivered, expected);
        assert_eq!(p.counts.opened, expected);
        assert_eq!(p.counts.credentials_entered, 1);
        if p.window == "all" {
            assert!(p.previous.is_none());
        } else {
            let previous = p.previous.unwrap();
            let days = match p.window.as_str() {
                "7d" => 7,
                "30d" => 30,
                "365d" => 365,
                _ => unreachable!(),
            };
            assert_eq!(p.as_of - previous.until, days * 86400);
            assert_eq!(Some(previous.until), p.since);
            assert_eq!(previous.until - previous.since, p.as_of - previous.until);
            let expected = i64::from(p.window == "7d");
            assert_eq!(previous.counts.opened, expected);
            assert_eq!(previous.counts.reported, 0);
        }
    }
    for query in [
        "window=day",
        "window=7d&window=all",
        "owner=1",
        "window=7d&until=9999999999999",
    ] {
        let response = read(&base, &format!("/api/overview/posture?{query}"), key);
        assert_eq!(response.status(), 400);
        assert_eq!(
            response.json::<Value>().unwrap()["message"],
            "overview_invalid_window"
        );
    }
    assert_eq!(
        read(
            &base,
            "/api/overview/posture?window=7d&api_key=synthetic-bootstrap-token",
            None
        )
        .status(),
        200
    );
    let other: Snapshot = read(&base, "/api/overview", Some("other-key"))
        .json()
        .unwrap();
    assert!(other.recent.is_empty());
    let other: PostureResponse = read(&base, "/api/overview/posture", Some("other-key"))
        .json()
        .unwrap();
    assert_eq!(other.counts.delivered, 0);
    let browser = reqwest::blocking::Client::builder()
        .no_proxy()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap();
    let session: Value = browser
        .get(format!("{base}/auth/session"))
        .send()
        .unwrap()
        .json()
        .unwrap();
    let login = browser
        .post(format!("{base}/login"))
        .header("origin", &base)
        .header("x-csrf-token", session["csrf_token"].as_str().unwrap())
        .header("accept", "application/json")
        .form(&[
            ("username", "admin"),
            ("password", "synthetic-bootstrap-password"),
        ])
        .send()
        .unwrap();
    assert_eq!(login.status(), 200, "{}", login.text().unwrap());
    for path in [
        "/",
        "/campaigns",
        "/analysis",
        "/api/overview",
        "/api/overview/posture?window=7d",
    ] {
        let response = browser
            .get(format!("{base}{path}"))
            .header("x-ruphish-client", "browser")
            .send()
            .unwrap();
        assert_eq!(
            response.status(),
            200,
            "{path}: {}",
            response.text().unwrap()
        );
    }
    database(
        dir.path(),
        "INSERT INTO ruphish_auth_state(user_id,epoch) VALUES(1,1) ON CONFLICT(user_id) DO UPDATE SET epoch=epoch+1;",
    );
    assert_eq!(
        browser
            .get(format!("{base}/api/overview"))
            .header("x-ruphish-client", "browser")
            .send()
            .unwrap()
            .status(),
        401
    );
    process.stop();
    process = Process::with_options(
        dir.path(),
        "admin",
        Some("synthetic-bootstrap-password"),
        Some("synthetic-bootstrap-token"),
        &["--disable-mailer"],
    );
    base = format!("http://{}", process.ready()["admin"].as_str().unwrap());
    let restored: Snapshot = read(&base, "/api/overview", key).json().unwrap();
    assert_eq!(restored.recent, snapshot.recent);
    process.stop();
    let logs = std::fs::read_to_string(dir.path().join("server.log")).unwrap();
    for secret in ["DASHBOARD-SECRET-CANARY", "private@example.test"] {
        assert!(!logs.contains(secret));
        assert!(!process.output().contains(secret));
    }
}

#[test]
fn progression_is_shared_across_summaries_without_inventing_recorded_events() {
    use sqlx::Row;
    let dir = workspace();
    let mut process = Process::with_options(
        dir.path(),
        "admin",
        Some("synthetic-bootstrap-password"),
        Some("synthetic-bootstrap-token"),
        &["--disable-mailer"],
    );
    process.ready();
    let key = Some("synthetic-bootstrap-token");
    database(dir.path(), "UPDATE users SET password_change_required=0;
        INSERT INTO campaigns(id,user_id,name,status) VALUES(1,1,'Progression','Completed'),(2,1,'No acceptance','Completed'),(3,2,'Other owner','Completed');
        INSERT INTO users(id,username,api_key,role_id,password_change_required,account_locked) VALUES(2,'other','other-key',2,0,0); INSERT INTO ruphish_auth_state VALUES(2,0);");
    database(
        dir.path(),
        "UPDATE campaigns SET launch_date='0001-01-01T00:00:00Z' WHERE id=1",
    );
    // Repeated observations precede the later acceptance. Neither insertion
    // order nor duplicates may change unique progression or create timestamps.
    database(
        dir.path(),
        r#"
        INSERT INTO results(id,campaign_id,user_id,r_id,email,status) VALUES
        (1,1,1,'rid-1','sent@test','Email Sent'),
        (2,1,1,'rid-2','open@test','Email Sent'),
        (3,1,1,'rid-3','click@test','Email Sent'),
        (4,1,1,'rid-4','submit@test','Email Sent'),
        (5,1,1,'rid-5','presence@test','Email Sent'),
        (6,1,1,'rid-6','report@test','Email Sent');
        WITH fixture(email,kind,details) AS (VALUES
            ('open@test','email_opened',''),
            ('click@test','link_clicked',''),
            ('submit@test','form_submitted','{"password_present":false}'),
            ('presence@test','form_submitted','{"password_present":true}'),
            ('report@test','email_reported','')
        ) INSERT INTO events(campaign_id,email,kind,time,metadata)
          SELECT 1,email,kind,datetime('now','-2 hours'),details FROM fixture CROSS JOIN (SELECT 1 UNION ALL SELECT 2);
        INSERT INTO events(campaign_id,email,kind,time,metadata)
          SELECT 1,email,'email_sent',datetime('now','-1 hour'),'' FROM results CROSS JOIN (SELECT 1 UNION ALL SELECT 2);
     UPDATE events SET recipient_id=(SELECT min(r.id) FROM results r WHERE r.campaign_id=events.campaign_id AND r.email=events.email HAVING count(*)=1) WHERE recipient_id IS NULL;"#,
    );
    database(
        dir.path(),
        r#"
        INSERT INTO results(id,campaign_id,user_id,r_id,email,status,reported) VALUES
        (7,1,1,'ambiguous-a','ambiguous@test','Submitted Data',1),
        (8,1,1,'ambiguous-b','ambiguous@test','Submitted Data',1),
        (9,1,1,'state-only','state@test','Submitted Data',1),
        (10,2,1,'unsent','presence@test','Submitted Data',0),
        (11,3,2,'other','presence@test','Submitted Data',0);
        INSERT INTO events(campaign_id,email,kind,time,metadata) VALUES
        (1,'ambiguous@test','email_sent',datetime('now','-1 hour'),''),
        (1,'ambiguous@test','form_submitted',datetime('now','-1 hour'),'{"password_present":true}'),
        (1,'orphan@test','email_sent',datetime('now','-1 hour'),''),
        (1,'orphan@test','link_clicked',datetime('now','-1 hour'),''),
        (2,'presence@test','form_submitted',datetime('now','-1 hour'),'{"password_present":true}'),
        (3,'presence@test','email_sent',datetime('now','-1 hour'),''),
        (3,'presence@test','form_submitted',datetime('now','-1 hour'),'{"password_present":true}');
     UPDATE events SET recipient_id=(SELECT min(r.id) FROM results r WHERE r.campaign_id=events.campaign_id AND r.email=events.email HAVING count(*)=1) WHERE recipient_id IS NULL;"#,
    );
    for pass in 0..2 {
        let base = format!("http://{}", process.ready()["admin"].as_str().unwrap());
        let fetch = |path: &str| -> Value {
            let response = read(&base, path, key);
            assert_eq!(
                response.status(),
                200,
                "{path}: {}",
                response.text().unwrap()
            );
            response.json().unwrap()
        };
        let p = fetch("/api/overview/posture?window=all");
        for (field, count) in [
            ("delivered", 6),
            ("opened", 4),
            ("clicked", 3),
            ("reported", 1),
            ("credentials_entered", 1),
        ] {
            assert_eq!(p[field], count, "{field}");
        }
        let a = fetch("/api/campaigns/1/analysis");
        assert_eq!(a["progression"], json!([6, 4, 3, 2, 1]));
        assert_eq!(a["unique"], json!([6, 1, 1, 2, 1]));
        // Unassigned observations still appear in raw history and exports.
        assert_eq!(a["totals"], json!([14, 2, 3, 5, 2]));
        assert_eq!(a["event_total"], 26);
        let chart_opens: i64 = a["buckets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b["counts"][1].as_i64().unwrap())
            .sum();
        assert_eq!(chart_opens, 2);
        let s = fetch("/api/campaigns/1/summary");
        assert_eq!(
            s["stats"],
            json!({"total":9,"sent":6,"opened":4,"clicked":3,"submitted_data":2,"email_reported":1,"error":0})
        );
        let summaries = fetch("/api/campaigns/summary");
        assert_eq!(
            summaries["campaigns"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["id"] == 1)
                .unwrap()["stats"],
            s["stats"]
        );
        let snapshot = fetch("/api/overview");
        let recent = snapshot["recent"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == 1)
            .unwrap();
        assert!(
            recent["launch"].is_null(),
            "historical zero dates must become explicit absence"
        );
        assert_eq!(recent["outcomes"]["opened"], 4);
        assert_eq!(recent["outcomes"]["clicked"], 3);
        let filtered = fetch("/api/campaigns/1/analysis?event=Submitted%20Data&limit=1");
        assert_eq!(filtered["progression"], json!([0, 2, 2, 2, 0]));
        assert_eq!(filtered["unique"], json!([0, 0, 0, 2, 0]));
        let recipient = fetch("/api/campaigns/1/analysis?recipient=5");
        assert_eq!(recipient["progression"], json!([1, 1, 1, 1, 0]));
        assert_eq!(recipient["recipients"][0]["counts"], json!([2, 0, 0, 2, 0]));
        let unsent = fetch("/api/campaigns/2/summary");
        assert_eq!(unsent["stats"]["sent"], 0);
        assert_eq!(unsent["stats"]["opened"], 1);
        assert_eq!(unsent["stats"]["clicked"], 1);
        let other = read(&base, "/api/campaigns/1/analysis", Some("other-key"));
        assert_eq!(other.status(), 404);
        let csv = read(
            &base,
            "/api/campaigns/1/export?format=csv&scope=events",
            key,
        )
        .text()
        .unwrap();
        // This controlled fixture has no multiline values; inspect exported
        // rows independently of the production CSV writer.
        let records = csv.lines().skip(1).collect::<Vec<_>>();
        assert_eq!(records.len(), 26);
        assert_eq!(
            records
                .iter()
                .filter(|r| r.contains(",Email Opened,"))
                .count(),
            2
        );
        assert_eq!(
            database(dir.path(), "SELECT count(*) AS n FROM event_history")[0].get::<i64, _>("n"),
            29
        );
        process.stop();
        if pass == 0 {
            process = Process::with_options(
                dir.path(),
                "admin",
                Some("synthetic-bootstrap-password"),
                Some("synthetic-bootstrap-token"),
                &["--disable-mailer"],
            );
        }
    }
}
