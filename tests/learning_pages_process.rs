//! Learning-page HTTP and persistence guarantees against the real executable.
use super::*;

#[test]
fn learning_pages_redirect_without_tracking_survive_restart_and_reject_adversarial_requests() {
    let dns = Dns::start();
    let directory = workspace();
    let cert = configure(directory.path(), &dns);
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
        "UPDATE users SET password_change_required=0",
    );
    let origin = format!("https://training.example.test:{}", public.port());
    post(
        &admin,
        "/api/domains/",
        json!({"name":"Learning domain","origin":origin,"enabled":true}),
    );
    let browser = reqwest::blocking::Client::builder()
        .no_proxy()
        .add_root_certificate(reqwest::Certificate::from_pem(&cert).unwrap())
        .resolve("training.example.test", public)
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let authored = "<!doctype html><html><body><h1>Learn to inspect the sender</h1><p>{{.FirstName}} is literal</p><style>h1{color:navy}</style><a href='https://example.test/help'>Help</a><form action='https://outside.invalid/collect'><input name='password' type='password'><button formaction='https://outside.invalid'>Continue</button></form><iframe src='https://outside.invalid'></iframe><img src='https://outside.invalid/pixel' onerror='unsafe()'><script>unsafe()</script><meta http-equiv='refresh' content='0;url=https://outside.invalid'></body></html>";
    assert_eq!(
        client()
            .post(format!("{admin}/api/learning-pages/"))
            .json(&json!({"name":"Unauthorized","html":authored}))
            .send()
            .unwrap()
            .status(),
        401
    );
    let mut page = post(
        &admin,
        "/api/learning-pages/",
        json!({"name":"Explanation","html":authored,"public_id":"chosen-by-caller","capture_credentials":true,"redirect_url":"https://outside.invalid"}),
    );
    let page_id = page["id"].as_i64().unwrap();
    let public_id = page["public_id"].as_str().unwrap().to_owned();
    assert_eq!(public_id.len(), 32);
    assert!(page.get("redirect_url").is_none());
    assert!(page.get("capture_credentials").is_none());
    let learning_url = format!("{origin}/learn/{public_id}");
    let preview = post(
        &admin,
        "/api/learning-pages/preview",
        json!({"html":authored}),
    );
    assert!(
        preview["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning["code"] == "learning_controls_removed")
    );
    for path in ["preview", "validate"] {
        let clean = post(
            &admin,
            &format!("/api/learning-pages/{path}"),
            json!({"html":"<!doctype html><html><head><title>Explanation</title></head><body><h1>A static explanation</h1></body></html>"}),
        );
        assert_eq!(clean["warnings"], json!([]));
    }
    let response = browser.get(&learning_url).send().unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["referrer-policy"], "no-referrer");
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    let csp = response.headers()["content-security-policy"]
        .to_str()
        .unwrap()
        .to_owned();
    for policy in [
        "script-src 'none'",
        "connect-src 'none'",
        "form-action 'none'",
    ] {
        assert!(csp.contains(policy));
    }
    let html = response.text().unwrap();
    assert_eq!(html, preview["html"].as_str().unwrap());
    assert!(html.contains("Learn to inspect the sender"));
    assert!(html.contains("{{.FirstName}} is literal"));
    for forbidden in [
        "<form",
        "<input",
        "<button",
        "<script",
        "<iframe",
        "outside.invalid",
        "http-equiv",
        "event_id",
        "password_present",
    ] {
        assert!(!html.contains(forbidden), "{forbidden}: {html}");
    }
    assert!(
        browser
            .head(&learning_url)
            .send()
            .unwrap()
            .text()
            .unwrap()
            .is_empty()
    );
    for method in [
        reqwest::Method::POST,
        reqwest::Method::PUT,
        reqwest::Method::DELETE,
    ] {
        assert_eq!(
            browser
                .request(method, &learning_url)
                .body("password=PRIVATE-LEARNING-CANARY&otp=secret")
                .send()
                .unwrap()
                .status(),
            405
        );
    }
    for suffix in [
        "unknown",
        "../config.json",
        "00000000000000000000000000000000",
        "",
    ] {
        assert_eq!(
            browser
                .get(format!("{origin}/learn/{suffix}"))
                .send()
                .unwrap()
                .status(),
            404
        );
    }
    assert_eq!(
        browser
            .get(&learning_url)
            .header("host", "wrong.example.test")
            .send()
            .unwrap()
            .status(),
        404
    );
    let second: Value = client().post(format!("{admin}/api/users/")).bearer_auth("synthetic-bootstrap-token").timeout(Duration::from_secs(20)).json(&json!({"username":"learning-other","password":"synthetic-other-owner-password","role":"user"})).send().unwrap().error_for_status().unwrap().json().unwrap();
    let second_key = second["api_key"].as_str().unwrap();
    for method in [
        reqwest::Method::GET,
        reqwest::Method::PUT,
        reqwest::Method::DELETE,
    ] {
        let response = client()
            .request(method, format!("{admin}/api/learning-pages/{page_id}"))
            .bearer_auth(second_key)
            .json(&page)
            .send()
            .unwrap();
        assert!(!response.status().is_success());
    }
    post(
        &admin,
        "/api/groups/",
        json!({"name":"Audience","targets":[{"email":"alice@example.test","first_name":"Alice"}]}),
    );
    post(
        &admin,
        "/api/templates/",
        json!({"name":"Mail","subject":"Training","text":"Visit {{.URL}}"}),
    );
    post(
        &admin,
        "/api/pages/",
        json!({"name":"Landing","html":"<form><input type='password' name='password'><button>Continue</button></form>","redirect_url":"https://example.test/default/{{.FirstName}}"}),
    );
    post(
        &admin,
        "/api/smtp/",
        json!({"name":"Local","host":"127.0.0.1:1","from_address":"training@example.test"}),
    );
    let mut draft = json!({"name":"Learning campaign","url":origin,"groups":[{"name":"Audience"}],"template":{"name":"Mail"},"page":{"name":"Landing"},"smtp":{"name":"Local"},"policy":{"learning_page_id":page_id}});
    for policy in [
        json!({"learning_page_id":0}),
        json!({"learning_page_id":-1}),
        json!({"learning_page_id":999999}),
        json!({"learning_page_id":page_id,"redirect_url":"https://outside.invalid"}),
    ] {
        let mut invalid = draft.clone();
        invalid["policy"] = policy;
        assert!(
            !client()
                .post(format!("{admin}/api/campaigns/"))
                .bearer_auth("synthetic-bootstrap-token")
                .json(&invalid)
                .send()
                .unwrap()
                .status()
                .is_success()
        );
    }
    assert_eq!(get(&admin, "/api/campaigns/"), json!([]));
    let campaign = post(&admin, "/api/campaigns/", draft.clone());
    let campaign_path = format!("/api/campaigns/{}", campaign["id"]);
    assert_eq!(
        get(&admin, &format!("{campaign_path}/delivery"))["policy"]["learning_page_id"],
        page_id
    );
    let rid = campaign["results"][0]["id"].as_str().unwrap();
    let submit = format!("{origin}/?rid={rid}");
    for _ in 0..2 {
        let response = browser
            .post(&submit)
            .json(&json!({"event_id":"a".repeat(32),"password_present":true}))
            .send()
            .unwrap();
        assert_eq!(response.status(), 200);
        let body: Value = response.json().unwrap();
        assert_eq!(body["redirect"], learning_url);
    }
    let before = get(&admin, &campaign_path);
    for _ in 0..2 {
        assert_eq!(browser.get(&learning_url).send().unwrap().status(), 200);
    }
    assert_eq!(get(&admin, &campaign_path)["timeline"], before["timeline"]);
    assert_eq!(
        before["timeline"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["message"] == "Submitted Data")
            .count(),
        1
    );
    assert_eq!(
        client()
            .delete(format!("{admin}/api/learning-pages/{page_id}"))
            .bearer_auth("synthetic-bootstrap-token")
            .send()
            .unwrap()
            .status(),
        409
    );
    let revision = page["modified_date"].as_str().unwrap().to_owned();
    page["html"] = json!("<h1>Updated explanation</h1>");
    let updated: Value = client()
        .put(format!("{admin}/api/learning-pages/{page_id}"))
        .bearer_auth("synthetic-bootstrap-token")
        .header("if-match", format!("\"{revision}\""))
        .json(&page)
        .send()
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(updated["public_id"], public_id);
    assert!(
        browser
            .get(&learning_url)
            .send()
            .unwrap()
            .text()
            .unwrap()
            .contains("Updated explanation")
    );
    assert_eq!(
        client()
            .put(format!("{admin}/api/learning-pages/{page_id}"))
            .bearer_auth("synthetic-bootstrap-token")
            .header("if-match", format!("\"{revision}\""))
            .json(&page)
            .send()
            .unwrap()
            .status(),
        412
    );
    // Legacy default resolution and explicit no-redirect remain usable.
    for (name, policy, expected) in [
        (
            "Legacy redirect",
            Value::Null,
            Some("https://example.test/default/Alice"),
        ),
        ("No redirect", json!({"redirect_url":""}), None),
    ] {
        draft["name"] = json!(name);
        draft["policy"] = policy;
        let created = post(&admin, "/api/campaigns/", draft.clone());
        let response: Value = browser
            .post(format!(
                "{origin}/?rid={}",
                created["results"][0]["id"].as_str().unwrap()
            ))
            .json(&json!({"event_id":"b".repeat(32),"password_present":false}))
            .send()
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .unwrap();
        assert_eq!(response["redirect"].as_str(), expected);
    }
    process.stop();
    let mut config: Value =
        serde_json::from_slice(&std::fs::read(directory.path().join("config.json")).unwrap())
            .unwrap();
    config["phish_server"]["listen_url"] = json!(public.to_string());
    write_config(directory.path(), &config);
    let mut restarted =
        Process::with_options(directory.path(), "all", None, None, &["--disable-mailer"]);
    let admin = format!("http://{}", restarted.ready()["admin"].as_str().unwrap());
    assert_eq!(
        get(&admin, &format!("/api/learning-pages/{page_id}"))["public_id"],
        public_id
    );
    assert_eq!(
        get(&admin, &format!("{campaign_path}/delivery"))["policy"]["learning_page_id"],
        page_id
    );
    assert!(
        browser
            .get(&learning_url)
            .send()
            .unwrap()
            .text()
            .unwrap()
            .contains("Updated explanation")
    );
    get(&admin, &format!("{campaign_path}/complete"));
    assert_eq!(
        client()
            .delete(format!("{admin}/api/learning-pages/{page_id}"))
            .bearer_auth("synthetic-bootstrap-token")
            .send()
            .unwrap()
            .status(),
        200
    );
    assert_eq!(browser.get(&learning_url).send().unwrap().status(), 404);
    restarted.stop();
    for bytes in [
        std::fs::read(directory.path().join("installation.sqlite3")).unwrap(),
        std::fs::read(directory.path().join("server.log")).unwrap(),
    ] {
        assert!(
            !bytes
                .windows("PRIVATE-LEARNING-CANARY".len())
                .any(|value| value == b"PRIVATE-LEARNING-CANARY")
        );
    }
}
