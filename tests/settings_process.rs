//! Real HTTP/session/API-key workflows for installation policy and bulk closure.
use super::*;
use reqwest::{
    Method,
    blocking::{Client, Response},
};

fn request(
    c: &Client,
    base: &str,
    key: &str,
    method: Method,
    path: &str,
    body: Option<&Value>,
    revision: Option<i64>,
) -> Response {
    let mut r = c.request(method, format!("{base}{path}")).bearer_auth(key);
    if let Some(body) = body {
        r = r.json(body);
    }
    if let Some(revision) = revision {
        r = r.header("if-match", format!("\"{revision}\""));
    }
    r.send().unwrap()
}
fn value(response: Response, status: u16) -> Value {
    let actual = response.status();
    let body: Value = response.json().unwrap();
    assert_eq!(actual, status, "{body}");
    body
}
fn get(c: &Client, base: &str, key: &str, path: &str) -> Value {
    value(request(c, base, key, Method::GET, path, None, None), 200)
}
fn post(c: &Client, base: &str, key: &str, path: &str, body: &Value) -> Value {
    value(
        request(c, base, key, Method::POST, path, Some(body), None),
        if path == "/api/users/" { 200 } else { 201 },
    )
}
fn save(c: &Client, base: &str, key: &str, settings: &Value) -> Value {
    value(
        request(
            c,
            base,
            key,
            Method::PUT,
            "/api/settings",
            Some(&json!({"policy":settings["policy"]})),
            settings["revision"].as_i64(),
        ),
        200,
    )
}
fn campaign(c: &Client, base: &str, key: &str, origin: &str) -> Value {
    let group = post(
        c,
        base,
        key,
        "/api/groups/",
        &json!({"name":"Settings audience","targets":[{"email":"person@example.test","first_name":"Test"}]}),
    );
    let template = post(
        c,
        base,
        key,
        "/api/templates/",
        &json!({"name":"Settings email","subject":"Training","text":"{{.URL}}"}),
    );
    let page = post(
        c,
        base,
        key,
        "/api/pages/",
        &json!({"name":"Settings page","html":"<h1>Training</h1><form><input type='password' name='password'></form>"}),
    );
    let smtp = post(
        c,
        base,
        key,
        "/api/smtp/",
        &json!({"name":"Settings SMTP","interface_type":"SMTP","host":"127.0.0.1:1","from_address":"training@example.test"}),
    );
    let input = json!({"name":"Settings campaign","selection":{"groups":[{"id":group["id"]}],"template":{"id":template["id"]},"page":{"id":page["id"]},"smtp":{"id":smtp["id"]}},"url":origin});
    post(c, base, key, "/api/campaigns/", &input)
}

#[test]
fn company_policy_authorization_domains_integrations_and_closure_survive_restart() {
    let directory = workspace();
    let mut config = configuration();
    // All possible verification traffic is confined to loopback, even on adversarial paths.
    config["domain_verification"] =
        json!({"resolvers":["127.0.0.1:9"],"trusted_proxies":["127.0.0.1/32"]});
    write_config(directory.path(), &config);
    let mut process = Process::with_options(
        directory.path(),
        "all",
        Some("synthetic-bootstrap-password"),
        Some("synthetic-bootstrap-token"),
        &["--disable-mailer"],
    );
    let endpoints = process.ready();
    let mut base = format!("http://{}", endpoints["admin"].as_str().unwrap());
    let public = format!("http://{}", endpoints["phish"].as_str().unwrap());
    let browser = auth_process::browser();
    let session = auth_process::setup(&browser, &base);
    let c = client();
    let key = "synthetic-bootstrap-token";
    let initial = get(&c, &base, key, "/api/settings");
    assert_eq!(initial["policy"]["verification_enabled"], false);
    assert_eq!(initial["policy"]["verification_expires"], true);
    assert_eq!(initial["policy"]["reporting_mailboxes_enabled"], false);
    assert_eq!(initial["policy"]["webhooks_enabled"], false);
    assert_eq!(session["features"]["reporting_mailboxes_enabled"], false);
    assert_eq!(
        browser
            .get(format!("{base}/settings/starter-templates"))
            .send()
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        c.get(format!("{base}/api/settings"))
            .send()
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        browser
            .put(format!("{base}/api/settings"))
            .header("if-match", "1")
            .json(&json!({"policy":initial["policy"]}))
            .send()
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        browser
            .put(format!("{base}/api/settings"))
            .header("if-match", "1")
            .header("x-csrf-token", session["csrf_token"].as_str().unwrap())
            .header("origin", "https://untrusted.example.test")
            .json(&json!({"policy":initial["policy"]}))
            .send()
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        request(
            &c,
            &base,
            key,
            Method::PUT,
            "/api/settings",
            Some(&json!({"policy":initial["policy"]})),
            None
        )
        .status(),
        428
    );
    for invalid in [
        json!([]),
        json!({"policy":{}}),
        json!({"policy":initial["policy"],"unexpected":true}),
    ] {
        assert_eq!(
            request(
                &c,
                &base,
                key,
                Method::PUT,
                "/api/settings",
                Some(&invalid),
                Some(1)
            )
            .status(),
            400
        );
    }
    let mut invalid = initial.clone();
    invalid["policy"]["verification_validity_seconds"] = json!(-1);
    assert_eq!(
        request(
            &c,
            &base,
            key,
            Method::PUT,
            "/api/settings",
            Some(&json!({"policy":invalid["policy"]})),
            Some(1)
        )
        .status(),
        400
    );
    assert_eq!(get(&c, &base, key, "/api/settings")["revision"], 1);
    let user = post(
        &c,
        &base,
        key,
        "/api/users/",
        &json!({"username":"policy-operator","password":"Operator-password-42!","role":"user","password_change_required":false}),
    );
    let user_key = user["api_key"].as_str().unwrap();
    assert_eq!(
        auth_process::post(
            &browser,
            &base,
            "/auth/impersonate",
            &[("username", "policy-operator")]
        )
        .status(),
        200
    );
    let impersonated = auth_process::bootstrap(&browser, &base);
    for (method, path) in [
        (Method::GET, "/api/settings"),
        (Method::PUT, "/api/settings"),
        (Method::POST, "/api/campaigns/complete-all/preview"),
        (Method::POST, "/api/campaigns/complete-all"),
    ] {
        assert_eq!(
            browser
                .request(method, format!("{base}{path}"))
                .header("origin", &base)
                .header("x-csrf-token", impersonated["csrf_token"].as_str().unwrap())
                .json(&json!({}))
                .send()
                .unwrap()
                .status(),
            403
        );
    }
    assert_eq!(
        auth_process::post(&browser, &base, "/auth/impersonate/exit", &[]).status(),
        200
    );

    for path in ["/api/settings", "/api/campaigns/complete-all/preview"] {
        let method = if path.ends_with("preview") {
            Method::POST
        } else {
            Method::GET
        };
        assert_eq!(
            request(&c, &base, user_key, method, path, None, None).status(),
            403
        );
    }
    let domain = post(
        &c,
        &base,
        key,
        "/api/domains/",
        &json!({"name":"Policy origin","origin":"https://policy.example.test","enabled":true}),
    );
    assert_eq!(domain["status"], "not_verified");
    assert_eq!(domain["verification"]["status"], "not_required");
    assert_eq!(domain["eligible"], true);
    assert_eq!(domain["active_check"], Value::Null);
    let domain_id = domain["id"].as_i64().unwrap();
    assert_eq!(
        request(
            &c,
            &base,
            key,
            Method::POST,
            &format!("/api/domains/{domain_id}/verify"),
            Some(&json!({})),
            None
        )
        .status(),
        409
    );
    let first = campaign(&c, &base, key, "https://policy.example.test");
    let rid = first["results"][0]["id"].as_str().unwrap();
    let visit = || {
        c.get(format!("{public}/?rid={rid}"))
            .header("x-forwarded-host", "policy.example.test")
            .header("x-forwarded-proto", "https")
            .send()
            .unwrap()
    };
    assert_eq!(visit().status(), 200);
    assert_eq!(
        c.get(format!("{public}/?rid={rid}"))
            .header("x-forwarded-host", "other.example.test")
            .header("x-forwarded-proto", "https")
            .send()
            .unwrap()
            .status(),
        404
    );
    let rejected = c
        .post(format!("{public}/?rid={rid}"))
        .header("x-forwarded-host", "policy.example.test")
        .header("x-forwarded-proto", "https")
        .json(&json!({"password":"SETTINGS-PARTICIPANT-CANARY"}))
        .send()
        .unwrap();
    assert!(!rejected.status().is_success());
    let mut enabled = initial.clone();
    enabled["policy"]["reporting_mailboxes_enabled"] = json!(true);
    enabled["policy"]["webhooks_enabled"] = json!(true);
    let enabled = save(&c, &base, key, &enabled);
    let mailbox = post(
        &c,
        &base,
        key,
        "/api/reporting-mailboxes/",
        &json!({"name":"Retained mailbox","enabled":false,"host":"127.0.0.1","port":1,"username":"service","password":"SETTINGS-SERVICE-SECRET","tls":true,"folder":"INBOX","frequency":60}),
    );
    let hook = post(
        &c,
        &base,
        key,
        "/api/webhooks/",
        &json!({"name":"Retained webhook","url":"http://127.0.0.1:1/unused","secret":"HOOK-SECRET-CANARY","is_active":true}),
    );
    let mut changed = enabled.clone();
    changed["policy"]["verification_enabled"] = json!(true);
    changed["policy"]["reporting_mailboxes_enabled"] = json!(false);
    changed["policy"]["webhooks_enabled"] = json!(false);
    let mut current = save(&c, &base, key, &changed);
    assert_eq!(
        request(
            &c,
            &base,
            key,
            Method::PUT,
            "/api/settings",
            Some(&json!({"policy":initial["policy"]})),
            Some(1)
        )
        .status(),
        412
    );
    assert_eq!(
        get(&c, &base, key, &format!("/api/domains/{domain_id}"))["eligible"],
        false
    );
    assert_eq!(visit().status(), 404);
    assert_eq!(
        auth_process::bootstrap(&browser, &base)["features"]["reporting_mailboxes_enabled"],
        false
    );
    // A disabled feature has an explanatory SPA route, not a missing route.
    assert_eq!(
        browser
            .get(format!("{base}/reporting-mailboxes"))
            .send()
            .unwrap()
            .status(),
        200
    );
    let mailbox_id = mailbox["id"].as_i64().unwrap();
    for action in ["validate", "check", "resume"] {
        let response = request(
            &c,
            &base,
            key,
            Method::POST,
            &format!("/api/reporting-mailboxes/{mailbox_id}/{action}"),
            Some(&json!({"revision":mailbox["revision"]})),
            None,
        );
        assert_eq!(
            response.status(),
            409,
            "{action}: {}",
            response.text().unwrap()
        );
    }
    assert_eq!(
        request(
            &c,
            &base,
            key,
            Method::POST,
            &format!("/api/webhooks/{}/validate", hook["id"]),
            None,
            None
        )
        .status(),
        409
    );
    current["policy"]["verification_enabled"] = json!(false);
    current = save(&c, &base, key, &current);
    assert_eq!(visit().status(), 200);
    post(
        &c,
        &base,
        user_key,
        "/api/domains/",
        &json!({"name":"Other origin","origin":"https://other.example.test","enabled":true}),
    );
    let second = campaign(&c, &base, user_key, "https://other.example.test");
    let draft = value(
        request(
            &c,
            &base,
            user_key,
            Method::PUT,
            "/api/campaign-drafts/settings-http-draft-01",
            Some(&json!({"revision":None::<i64>,"configuration":{},"step":0})),
            None,
        ),
        200,
    );
    assert_eq!(draft["revision"], 1);
    let review = value(
        request(
            &c,
            &base,
            key,
            Method::POST,
            "/api/campaigns/complete-all/preview",
            None,
            None,
        ),
        200,
    );
    assert_eq!(review["campaigns"], 2);
    let body = json!({"token":review["token"],"confirmation":"CLOSE ALL"});
    assert_eq!(
        request(
            &c,
            &base,
            user_key,
            Method::POST,
            "/api/campaigns/complete-all",
            Some(&body),
            None
        )
        .status(),
        403
    );
    assert_eq!(
        request(
            &c,
            &base,
            key,
            Method::GET,
            "/api/campaigns/complete-all",
            None,
            None
        )
        .status(),
        405
    );
    assert_eq!(
        request(
            &c,
            &base,
            key,
            Method::POST,
            "/api/campaigns/complete-all",
            Some(&json!({"token":review["token"],"confirmation":"yes"})),
            None
        )
        .status(),
        400
    );
    let result = value(
        request(
            &c,
            &base,
            key,
            Method::POST,
            "/api/campaigns/complete-all",
            Some(&body),
            None,
        ),
        200,
    );
    assert_eq!(result["campaigns"], 2);
    assert_eq!(result["cancelled_deliveries"], 2);
    assert_eq!(
        get(&c, &base, key, &format!("/api/campaigns/{}", first["id"]))["status"],
        "Completed"
    );
    assert_eq!(
        get(
            &c,
            &base,
            user_key,
            &format!("/api/campaigns/{}", second["id"])
        )["status"],
        "Completed"
    );
    assert_eq!(visit().status(), 404);
    assert_eq!(
        get(
            &c,
            &base,
            user_key,
            "/api/campaign-drafts/settings-http-draft-01"
        )["revision"],
        1
    );
    process.stop();
    assert!(!process.output().contains("SETTINGS-PARTICIPANT-CANARY"));
    assert!(!process.output().contains("SETTINGS-SERVICE-SECRET"));
    let mut process =
        Process::with_options(directory.path(), "admin", None, None, &["--disable-mailer"]);
    base = format!("http://{}", process.ready()["admin"].as_str().unwrap());
    assert_eq!(get(&c, &base, key, "/api/settings"), current);
    assert_eq!(
        value(
            request(
                &c,
                &base,
                key,
                Method::POST,
                "/api/campaigns/complete-all",
                Some(&body),
                None
            ),
            200
        ),
        result
    );
    let saved = get(
        &c,
        &base,
        key,
        &format!("/api/reporting-mailboxes/{mailbox_id}"),
    );
    assert_eq!(saved["has_password"], true);
    assert_eq!(saved["history"], json!([]));
    assert!(saved.get("password").is_none());
    process.stop();
}

#[test]
fn regular_users_manage_integrations_under_admin_feature_switches() {
    let directory = workspace();
    let peer = mailbox::Mailbox::new();
    std::fs::write(directory.path().join("mailbox-ca.pem"), &peer.certificate).unwrap();
    let receiver = webhook_receiver::Receiver::new();
    receiver
        .status
        .store(200, std::sync::atomic::Ordering::SeqCst);
    let mut process = Process::start(directory.path(), "admin");
    let base = format!("http://{}", process.ready()["admin"].as_str().unwrap());
    auth_process::setup(&auth_process::browser(), &base);
    let c = client();
    let admin = "synthetic-bootstrap-token";
    let user = post(
        &c,
        &base,
        admin,
        "/api/users/",
        &json!({"username":"integration-user","password":"Operator-password-42!","role":"user","password_change_required":false}),
    );
    let key = user["api_key"].as_str().unwrap();
    let hook = json!({"name":"Owned receiver","url":receiver.url,"secret":"HOOK-SECRET-CANARY","is_active":false});
    let mail = json!({"name":"Owned mailbox","enabled":false,"host":"127.0.0.1","port":peer.address.port(),"username":"service","password":"MAILBOX-SECRET-CANARY","tls":true,"folder":"INBOX","frequency":60});
    let mut policy = get(&c, &base, admin, "/api/settings");
    for caller in [admin, key] {
        for (path, body) in [
            ("/api/webhooks/", &hook),
            ("/api/reporting-mailboxes/", &mail),
        ] {
            let rejected = value(
                request(&c, &base, caller, Method::POST, path, Some(body), None),
                409,
            );
            assert_eq!(rejected["code"], "FEATURE_DISABLED");
        }
    }
    assert_eq!(get(&c, &base, key, "/api/webhooks/"), json!([]));
    assert_eq!(get(&c, &base, key, "/api/reporting-mailboxes/"), json!([]));
    policy["policy"]["webhooks_enabled"] = json!(true);
    policy["policy"]["reporting_mailboxes_enabled"] = json!(true);
    assert_eq!(
        request(
            &c,
            &base,
            key,
            Method::PUT,
            "/api/settings",
            Some(&json!({"policy":policy["policy"]})),
            policy["revision"].as_i64()
        )
        .status(),
        403
    );
    policy = save(&c, &base, admin, &policy);
    let saved_hook = post(&c, &base, key, "/api/webhooks/", &hook);
    let saved_mail = post(&c, &base, key, "/api/reporting-mailboxes/", &mail);
    assert_eq!(saved_hook["has_secret"], true);
    assert_eq!(saved_mail["has_password"], true);
    assert!(saved_hook.get("secret").is_none());
    assert!(saved_mail.get("password").is_none());
    let hook_path = format!("/api/webhooks/{}", saved_hook["id"]);
    let mail_path = format!("/api/reporting-mailboxes/{}", saved_mail["id"]);
    assert_eq!(get(&c, &base, admin, "/api/webhooks/"), json!([]));
    for method in [Method::GET, Method::PUT, Method::DELETE] {
        for revision in [None, saved_hook["revision"].as_i64(), Some(999)] {
            assert_eq!(
                request(
                    &c,
                    &base,
                    admin,
                    method.clone(),
                    &hook_path,
                    Some(&hook),
                    revision
                )
                .status(),
                404
            );
        }
    }
    assert_eq!(
        request(
            &c,
            &base,
            admin,
            Method::POST,
            &format!("{hook_path}/validate"),
            None,
            None
        )
        .status(),
        404
    );
    assert!(receiver.requests.try_recv().is_err());
    for field in ["owner_id", "user_id", "creator_id"] {
        let mut forged = hook.clone();
        forged[field] = json!(1);
        assert_eq!(
            request(
                &c,
                &base,
                key,
                Method::POST,
                "/api/webhooks/",
                Some(&forged),
                None
            )
            .status(),
            400
        );
    }
    assert_eq!(
        get(&c, &base, key, "/api/webhooks/")
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        request(&c, &base, admin, Method::GET, &mail_path, None, None).status(),
        404
    );
    let mut hook_edit = hook.clone();
    hook_edit.as_object_mut().unwrap().remove("secret");
    hook_edit["revision"] = saved_hook["revision"].clone();
    hook_edit["name"] = json!("Updated owned receiver");
    let updated_hook = value(
        request(
            &c,
            &base,
            key,
            Method::PUT,
            &hook_path,
            Some(&hook_edit),
            None,
        ),
        200,
    );
    assert_eq!(updated_hook["has_secret"], true);
    assert_eq!(
        request(
            &c,
            &base,
            key,
            Method::PUT,
            &hook_path,
            Some(&hook_edit),
            None
        )
        .status(),
        412
    );
    let mut mail_edit = mail.clone();
    mail_edit.as_object_mut().unwrap().remove("password");
    mail_edit["revision"] = saved_mail["revision"].clone();
    let updated_mail = value(
        request(
            &c,
            &base,
            key,
            Method::PUT,
            &mail_path,
            Some(&mail_edit),
            None,
        ),
        200,
    );
    assert_eq!(updated_mail["has_password"], true);
    value(
        request(
            &c,
            &base,
            key,
            Method::POST,
            &format!("{hook_path}/validate"),
            None,
            None,
        ),
        200,
    );
    let (event, payload) = receiver
        .requests
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
    assert_eq!(event, "validation");
    assert_eq!(
        serde_json::from_slice::<Value>(&payload).unwrap(),
        json!({"success":true})
    );
    value(
        request(
            &c,
            &base,
            key,
            Method::POST,
            &format!("{mail_path}/validate"),
            None,
            None,
        ),
        200,
    );
    let logins = peer.state.lock().unwrap().logins;
    assert!(logins > 0);
    policy["policy"]["webhooks_enabled"] = json!(false);
    policy["policy"]["reporting_mailboxes_enabled"] = json!(false);
    policy = save(&c, &base, admin, &policy);
    hook_edit["revision"] = updated_hook["revision"].clone();
    mail_edit["revision"] = updated_mail["revision"].clone();
    for (path, body) in [(&hook_path, &hook_edit), (&mail_path, &mail_edit)] {
        let rejected = value(
            request(&c, &base, key, Method::PUT, path, Some(body), None),
            409,
        );
        assert_eq!(rejected["code"], "FEATURE_DISABLED");
    }
    for path in [
        format!("{hook_path}/validate"),
        format!("{mail_path}/validate"),
        format!("{mail_path}/check"),
        format!("{mail_path}/resume"),
    ] {
        let rejected = value(
            request(
                &c,
                &base,
                key,
                Method::POST,
                &path,
                Some(&json!({"revision":updated_mail["revision"]})),
                None,
            ),
            409,
        );
        assert_eq!(rejected["code"], "FEATURE_DISABLED");
    }
    assert_eq!(get(&c, &base, key, &hook_path), updated_hook);
    assert_eq!(peer.state.lock().unwrap().logins, logins);
    assert!(receiver.requests.try_recv().is_err());
    policy["policy"]["webhooks_enabled"] = json!(true);
    policy["policy"]["reporting_mailboxes_enabled"] = json!(true);
    save(&c, &base, admin, &policy);
    // A live privilege change must reject a previously authorized caller.
    database(
        directory.path(),
        "DELETE FROM role_permissions WHERE role_id=2 AND permission_id=(SELECT id FROM permissions WHERE slug='modify_objects')",
    );
    assert_eq!(
        request(
            &c,
            &base,
            key,
            Method::PUT,
            &hook_path,
            Some(&hook_edit),
            None
        )
        .status(),
        403
    );
    assert_eq!(
        request(
            &c,
            &base,
            key,
            Method::POST,
            &format!("{hook_path}/validate"),
            None,
            None
        )
        .status(),
        403
    );
    assert_eq!(
        request(
            &c,
            &base,
            key,
            Method::DELETE,
            &mail_path,
            None,
            updated_mail["revision"].as_i64()
        )
        .status(),
        403
    );
    assert_eq!(get(&c, &base, key, &hook_path)["id"], saved_hook["id"]);
    assert!(receiver.requests.try_recv().is_err());
    assert_eq!(peer.state.lock().unwrap().logins, logins);
    process.stop();
    assert!(!process.output().contains("SECRET-CANARY"));
}

#[test]
fn mailbox_policy_blocks_network_actions_and_reenable_uses_preserved_credentials() {
    let directory = workspace();
    let peer = mailbox::Mailbox::new();
    std::fs::write(directory.path().join("mailbox-ca.pem"), &peer.certificate).unwrap();
    let mut process = Process::start(directory.path(), "admin");
    let base = format!("http://{}", process.ready()["admin"].as_str().unwrap());
    auth_process::setup(&auth_process::browser(), &base);
    let c = client();
    let key = "synthetic-bootstrap-token";
    let mut policy = get(&c, &base, key, "/api/settings");
    policy["policy"]["reporting_mailboxes_enabled"] = json!(true);
    policy = save(&c, &base, key, &policy);
    let mailbox = post(
        &c,
        &base,
        key,
        "/api/reporting-mailboxes/",
        &json!({"name":"Preserved TLS mailbox","enabled":false,"host":"127.0.0.1","port":peer.address.port(),"username":"service","password":"MAILBOX-SECRET-CANARY","tls":true,"folder":"INBOX","frequency":60}),
    );
    policy["policy"]["reporting_mailboxes_enabled"] = json!(false);
    policy = save(&c, &base, key, &policy);
    let path = format!("/api/reporting-mailboxes/{}/validate", mailbox["id"]);
    assert_eq!(
        request(&c, &base, key, Method::POST, &path, None, None).status(),
        409
    );
    assert_eq!(peer.state.lock().unwrap().logins, 0);
    policy["policy"]["reporting_mailboxes_enabled"] = json!(true);
    policy = save(&c, &base, key, &policy);
    let checked = value(
        request(&c, &base, key, Method::POST, &path, None, None),
        200,
    );
    assert_eq!(checked["has_password"], true);
    assert!(peer.state.lock().unwrap().logins > 0);
    assert!(checked.get("password").is_none());
    policy["policy"]["reporting_mailboxes_enabled"] = json!(false);
    save(&c, &base, key, &policy);
    assert_eq!(
        request(&c, &base, key, Method::POST, &path, None, None).status(),
        409
    );
    process.stop();
    assert!(!process.output().contains("MAILBOX-SECRET-CANARY"));
}
