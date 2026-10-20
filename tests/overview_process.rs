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
    assert_eq!(empty.campaigns.total, 0);
    assert!(empty.recent.is_empty());
    database(
        dir.path(),
        r#"INSERT INTO campaigns(id,user_id,name,status,launch_date) VALUES(1,1,'Dashboard campaign','Completed',datetime('now','-1 day'));
        INSERT INTO results(id,campaign_id,user_id,r_id,email,status) VALUES(1,1,1,'dashboard-rid','private@example.test','Email Sent');
        INSERT INTO events(campaign_id,email,message,time,details) VALUES(1,'private@example.test','Email Sent',datetime('now','-1 day'),''),(1,'private@example.test','Email Opened',datetime('now'),''),(1,'private@example.test','Email Opened',datetime('now'),''),(1,'private@example.test','Submitted Data',datetime('now'),'{"password_present":true,"password":"DASHBOARD-SECRET-CANARY"}');"#,
    );
    database(
        dir.path(),
        r#"INSERT INTO results(id,campaign_id,user_id,r_id,email,status) VALUES(2,1,1,'previous-rid','previous@example.test','Email Sent');
        INSERT INTO events(campaign_id,email,message,time,details) VALUES(1,'previous@example.test','Email Sent',datetime('now','-10 days'),''),(1,'previous@example.test','Email Opened',datetime('now','-9 days'),''),(1,'previous@example.test','Email Reported',datetime('now'),'');"#,
    );
    let snapshot: Snapshot = read(&base, "/api/overview", key).json().unwrap();
    assert_eq!(snapshot.campaigns.completed, 1);
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
    assert_eq!(other.campaigns.total, 0);
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
    assert_eq!(restored.campaigns, snapshot.campaigns);
    assert_eq!(restored.recent, snapshot.recent);
    process.stop();
    let logs = std::fs::read_to_string(dir.path().join("server.log")).unwrap();
    for secret in ["DASHBOARD-SECRET-CANARY", "private@example.test"] {
        assert!(!logs.contains(secret));
        assert!(!process.output().contains(secret));
    }
}
