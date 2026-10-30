//! Happy and adversarial validation workflows through the executable and real storage.
use super::*;
use reqwest::blocking::{Client, Response};

fn client() -> Client {
    Client::builder().no_proxy().build().unwrap()
}
fn request(
    base: &str,
    method: &str,
    path: &str,
    body: Option<&Value>,
    revision: Option<&str>,
) -> Response {
    let mut request = client()
        .request(method.parse().unwrap(), format!("{base}{path}"))
        .bearer_auth("synthetic-bootstrap-token");
    if let Some(body) = body {
        request = request.json(body);
    }
    if let Some(revision) = revision {
        request = request.header(
            "if-match",
            if path == "/api/settings" {
                revision.to_owned()
            } else {
                format!("\"{revision}\"")
            },
        );
    }
    request.send().unwrap()
}
fn read(base: &str, path: &str) -> Value {
    request(base, "GET", path, None, None)
        .error_for_status()
        .unwrap()
        .json()
        .unwrap()
}
fn accepted(response: Response) -> Value {
    response.error_for_status().unwrap().json().unwrap()
}
fn rejected(response: Response, status: u16, fields: &[&str]) -> Value {
    assert_eq!(response.status(), status);
    let body: Value = response.json().unwrap();
    assert_eq!(body["success"], false);
    assert!(
        body["validation"].is_object(),
        "missing field report for {fields:?}: {body}"
    );
    let report: ruphish_contracts::validation::ValidationReport =
        serde_json::from_value(body["validation"].clone()).unwrap();
    for field in fields {
        assert!(
            report.issues.iter().any(|i| i.field == *field),
            "missing {field}: {body}"
        );
    }
    assert!(report.issues.len() <= 100);
    for issue in body["validation"]["issues"].as_array().unwrap() {
        assert_eq!(
            issue.as_object().unwrap().len(),
            2,
            "diagnostics contain only paths and codes"
        );
    }
    body
}
fn start(directory: &Workspace) -> (Process, String) {
    let mut process = Process::start(directory.path(), "admin");
    let base = format!("http://{}", process.ready()["admin"].as_str().unwrap());
    auth_process::setup(&auth_process::browser(), &base);
    (process, base)
}

#[test]
fn resource_validation_rejects_bypassed_clients_preserves_edits_and_survives_restart() {
    let directory = workspace();
    let (mut process, base) = start(&directory);
    for (family, body, fields) in [
        (
            "groups",
            json!({"name":"","targets":[]}),
            vec!["name", "targets"],
        ),
        (
            "templates",
            json!({"name":"","text":"","html":""}),
            vec!["name", "html", "text"],
        ),
        (
            "smtp",
            json!({"name":"","host":"","from_address":""}),
            vec!["host", "from_address"],
        ),
    ] {
        rejected(
            request(&base, "POST", &format!("/api/{family}/"), Some(&body), None),
            400,
            &fields,
        );
        assert!(
            read(&base, &format!("/api/{family}/"))
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
    let wrong_type = request(
        &base,
        "POST",
        "/api/templates/",
        Some(&json!({"name":42,"text":"Training"})),
        None,
    );
    assert_eq!(wrong_type.status(), 400);
    let wrong_type: Value = wrong_type.json().unwrap();
    assert_eq!(wrong_type["code"], "REQUEST_INVALID");
    assert!(
        read(&base, "/api/templates/")
            .as_array()
            .unwrap()
            .is_empty()
    );
    rejected(
        request(
            &base,
            "POST",
            "/api/smtp/",
            Some(
                &json!({"from_address":"sender@example.test","host":"smtp.example.test:25","headers":[{"key":"X-Test\r\nInjected", "value":"value"}]}),
            ),
            None,
        ),
        400,
        &["headers.0.key"],
    );
    let group = accepted(request(
        &base,
        "POST",
        "/api/groups/",
        Some(&json!({"name":"Validation audience","targets":[{"email":"first@example.test"}]})),
        None,
    ));
    let group_path = format!("/api/groups/{}", group["id"]);
    let mut invalid_group = group.clone();
    invalid_group["targets"] =
        json!([{"email":"first@example.test"},{"email":"adversarial-invalid-address"}]);
    let error = rejected(
        request(
            &base,
            "PUT",
            &group_path,
            Some(&invalid_group),
            group["modified_date"].as_str(),
        ),
        400,
        &["targets.1.email"],
    );
    assert!(
        !error["validation"]
            .to_string()
            .contains("adversarial-invalid-address")
    );
    assert_eq!(read(&base, &group_path), group);

    let template = accepted(request(
        &base,
        "POST",
        "/api/templates/",
        Some(
            &json!({"name":"Validation template","subject":"","text":"Training {{.FirstName}}","attachments":[]}),
        ),
        None,
    ));
    let path = format!("/api/templates/{}", template["id"]);
    let mut edit = template.clone();
    edit["name"] = json!("");
    edit["text"] = json!("");
    edit["html"] = json!("");
    rejected(
        request(
            &base,
            "PUT",
            &path,
            Some(&edit),
            template["modified_date"].as_str(),
        ),
        400,
        &["name", "html", "text"],
    );
    assert_eq!(
        read(&base, &path),
        template,
        "invalid replacement must not destroy saved content"
    );
    edit = template.clone();
    edit["html"] = json!("{{unterminated");
    rejected(
        request(
            &base,
            "PUT",
            &path,
            Some(&edit),
            template["modified_date"].as_str(),
        ),
        400,
        &["html"],
    );
    assert_eq!(read(&base, &path), template);
    edit = template.clone();
    edit["attachments"] =
        json!([{"name":"invalid.txt","type":"text/plain","content":"not-base64!"}]);
    rejected(
        request(
            &base,
            "PUT",
            &path,
            Some(&edit),
            template["modified_date"].as_str(),
        ),
        400,
        &["attachments.0"],
    );
    assert_eq!(read(&base, &path), template);
    edit = template.clone();
    edit["text"] = json!("Corrected training");
    let saved = accepted(request(
        &base,
        "PUT",
        &path,
        Some(&edit),
        template["modified_date"].as_str(),
    ));
    assert_eq!(saved["text"], "Corrected training");
    let stale = request(
        &base,
        "PUT",
        &path,
        Some(&template),
        template["modified_date"].as_str(),
    );
    assert_eq!(stale.status(), 412);
    assert_eq!(read(&base, &path), saved);
    rejected(
        request(
            &base,
            "POST",
            "/api/templates/",
            Some(&json!({"name":"Validation template","text":"Other"})),
            None,
        ),
        409,
        &["name"],
    );
    process.stop();
    let mut process = Process::start(directory.path(), "admin");
    let base = format!("http://{}", process.ready()["admin"].as_str().unwrap());
    assert_eq!(read(&base, &path), saved);
    assert_eq!(read(&base, &group_path), group);
    process.stop();
}

#[test]
fn password_and_account_validation_preserve_credentials_until_a_legitimate_replacement() {
    let directory = workspace();
    let (mut process, base) = start(&directory);
    let browser = auth_process::browser();
    assert_eq!(
        auth_process::sign_in(&browser, &base, "admin", "established-admin-password").status(),
        200
    );
    rejected(
        request(
            &base,
            "POST",
            "/api/users/",
            Some(&json!({"username":"","role":"","password":"short"})),
            None,
        ),
        400,
        &["username", "role", "password"],
    );
    assert_eq!(read(&base, "/api/users/").as_array().unwrap().len(), 1);
    for (candidate, confirmation, fields) in [
        (
            "tiny".to_owned(),
            "different".to_owned(),
            vec!["password", "confirmation"],
        ),
        ("x".repeat(1025), "x".repeat(1025), vec!["password"]),
    ] {
        let response = auth_process::post(
            &browser,
            &base,
            "/internal/password",
            &[
                ("current_password", "established-admin-password"),
                ("new_password", &candidate),
                ("confirm_new_password", &confirmation),
            ],
        );
        let body = rejected(response, 400, &fields);
        assert!(!body["validation"].to_string().contains(&candidate));
        assert_eq!(
            auth_process::sign_in(
                &auth_process::browser(),
                &base,
                "admin",
                "established-admin-password"
            )
            .status(),
            200
        );
    }
    let invalid_current = auth_process::post(
        &browser,
        &base,
        "/internal/password",
        &[
            ("current_password", "incorrect-current"),
            ("new_password", "new-valid-password"),
            ("confirm_new_password", "new-valid-password"),
        ],
    );
    assert!(!invalid_current.status().is_success());
    // Four two-byte characters satisfy the existing eight-byte policy.
    let replacement = "éééé";
    assert_eq!(
        auth_process::post(
            &browser,
            &base,
            "/internal/password",
            &[
                ("current_password", "established-admin-password"),
                ("new_password", replacement),
                ("confirm_new_password", replacement),
            ]
        )
        .status(),
        200
    );
    process.stop();
    assert!(!process.output().contains("incorrect-current"));
    // Verify persisted credentials after reopening, within a fresh login budget.
    let mut process = Process::start(directory.path(), "admin");
    let base = format!("http://{}", process.ready()["admin"].as_str().unwrap());
    assert_eq!(
        auth_process::sign_in(&auth_process::browser(), &base, "admin", replacement).status(),
        200
    );
    assert_eq!(
        auth_process::sign_in(
            &auth_process::browser(),
            &base,
            "admin",
            "established-admin-password"
        )
        .status(),
        401
    );
    let user = accepted(request(
        &base,
        "POST",
        "/api/users/",
        Some(
            &json!({"username":"validation-user","role":"user","password":"initial-user-password"}),
        ),
        None,
    ));
    let mut user_edit = user.clone();
    user_edit["password"] = json!("");
    user_edit["username"] = json!("renamed-validation-user");
    user_edit["role"] = json!("user");
    accepted(request(
        &base,
        "PUT",
        &format!("/api/users/{}", user["id"]),
        Some(&user_edit),
        None,
    ));
    assert_eq!(
        auth_process::sign_in(
            &auth_process::browser(),
            &base,
            "renamed-validation-user",
            "initial-user-password"
        )
        .status(),
        200
    );
    process.stop();
    assert!(!process.output().contains("incorrect-current"));
    assert!(!process.output().contains("initial-user-password"));
}

#[test]
fn settings_and_integration_validation_rejects_invalid_configuration_without_mutation() {
    let directory = workspace();
    let (mut process, base) = start(&directory);
    let saved = read(&base, "/api/settings");
    let mut policy = saved["policy"].clone();
    policy["verification_validity_seconds"] = json!(0);
    policy["verification_recheck_seconds"] = json!(0);
    rejected(
        request(
            &base,
            "PUT",
            "/api/settings",
            Some(&json!({"policy":policy})),
            Some(&saved["revision"].to_string()),
        ),
        400,
        &[
            "verification_validity_seconds",
            "verification_recheck_seconds",
        ],
    );
    assert_eq!(read(&base, "/api/settings"), saved);
    policy = saved["policy"].clone();
    policy["verification_validity_seconds"] = json!(3600);
    policy["verification_recheck_seconds"] = json!(1800);
    rejected(
        request(
            &base,
            "PUT",
            "/api/settings",
            Some(&json!({"policy":policy})),
            Some(&saved["revision"].to_string()),
        ),
        400,
        &["verification_recheck_seconds"],
    );
    policy["verification_recheck_seconds"] = json!(300);
    let settings = accepted(request(
        &base,
        "PUT",
        "/api/settings",
        Some(&json!({"policy":policy})),
        Some(&saved["revision"].to_string()),
    ));
    assert_eq!(settings["policy"], policy);
    rejected(
        request(
            &base,
            "POST",
            "/api/domains/",
            Some(
                &json!({"name":"","origin":"https://user:password@example.test/path","enabled":true}),
            ),
            None,
        ),
        400,
        &["name", "origin"],
    );
    assert!(read(&base, "/api/domains/").as_array().unwrap().is_empty());
    rejected(
        request(
            &base,
            "POST",
            "/api/webhooks/",
            Some(
                &json!({"name":"","url":"https://user:secret@example.test/#fragment","secret":""}),
            ),
            None,
        ),
        400,
        &["name", "url", "secret"],
    );
    assert!(read(&base, "/api/webhooks/").as_array().unwrap().is_empty());
    let hook = accepted(request(
        &base,
        "POST",
        "/api/webhooks/",
        Some(
            &json!({"name":"Validation hook","url":"https://example.test/events","secret":"synthetic-signing-secret","is_active":false}),
        ),
        None,
    ));
    let updated = accepted(request(
        &base,
        "PUT",
        &format!("/api/webhooks/{}", hook["id"]),
        Some(
            &json!({"name":"Renamed hook","url":"https://example.test/events","revision":hook["revision"],"is_active":false}),
        ),
        None,
    ));
    assert_eq!(updated["has_secret"], true);
    assert!(!updated.to_string().contains("synthetic-signing-secret"));
    rejected(
        request(
            &base,
            "POST",
            "/api/reporting-mailboxes/",
            Some(
                &json!({"name":"","enabled":false,"host":"bad/host","port":0,"username":"","password":"","tls":true,"folder":"","frequency":0}),
            ),
            None,
        ),
        400,
        &[
            "name",
            "host",
            "port",
            "username",
            "password",
            "folder",
            "frequency",
        ],
    );
    assert!(
        read(&base, "/api/reporting-mailboxes/")
            .as_array()
            .unwrap()
            .is_empty()
    );
    process.stop();
    assert!(!process.output().contains("synthetic-signing-secret"));
}
