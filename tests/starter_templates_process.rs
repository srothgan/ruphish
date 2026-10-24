//! Catalog reads and independent resource copies through the real server.
use super::*;
use reqwest::Method;

#[test]
fn catalog_is_admin_only_read_only_and_copies_are_normal_independent_resources() {
    use auth_process::{bootstrap, browser, post, setup, sign_in};
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = format!("http://{}", process.ready()["admin"].as_str().unwrap());
    let admin = browser();
    let anonymous = browser();
    for path in [
        "/api/starter-templates/",
        "/api/starter-templates/page-microsoft-full-screen",
    ] {
        assert_eq!(
            anonymous
                .get(format!("{base}{path}"))
                .send()
                .unwrap()
                .status(),
            401
        );
    }
    let session = setup(&admin, &base);
    let csrf = session["csrf_token"].as_str().unwrap();
    let send = |method: Method, path: &str, value: &Value| {
        admin
            .request(method, format!("{base}{path}"))
            .header("x-csrf-token", csrf)
            .header("origin", &base)
            .json(value)
            .send()
            .unwrap()
    };
    let get = |path: &str| {
        let response = admin.get(format!("{base}{path}")).send().unwrap();
        assert_eq!(response.status(), 200, "{path}");
        assert_eq!(response.headers()["cache-control"], "no-store");
        response.json::<Value>().unwrap()
    };
    let route = "/settings/starter-templates";
    assert_eq!(
        anonymous
            .get(format!("{base}{route}"))
            .send()
            .unwrap()
            .status(),
        307
    );
    assert_eq!(
        admin.get(format!("{base}{route}")).send().unwrap().status(),
        200
    );
    let catalog = get("/api/starter-templates/");
    let rows = catalog.as_array().unwrap();
    assert_eq!(rows.iter().filter(|r| r["kind"] == "email").count(), 20);
    assert_eq!(
        rows.iter().filter(|r| r["kind"] == "landing_page").count(),
        19
    );
    let branding: std::collections::BTreeSet<_> = rows
        .iter()
        .filter(|r| !r["company_logo"].is_null())
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        branding,
        std::collections::BTreeSet::from([
            "page-microsoft-full-screen",
            "page-microsoft-side-panel",
            "page-zoom",
            "page-salesforce",
            "page-workday",
        ])
    );
    assert_eq!(get("/api/templates/"), json!([]));
    assert_eq!(get("/api/pages/"), json!([]));
    let unknown = admin
        .get(format!("{base}/api/starter-templates/not-found"))
        .send()
        .unwrap();
    assert_eq!(unknown.status(), 404);
    for method in [Method::POST, Method::PUT, Method::DELETE] {
        assert_eq!(
            send(
                method,
                "/api/starter-templates/page-microsoft-full-screen",
                &json!({})
            )
            .status(),
            405
        );
    }
    // Every approved entry must pass the actual create and preview pipeline.
    let mut saved = Vec::new();
    for row in rows {
        let id = row["id"].as_str().unwrap();
        let entry = get(&format!("/api/starter-templates/{id}"));
        assert_eq!(entry["summary"], *row);
        let resource = &entry["resource"];
        let html = resource["html"].as_str().unwrap();
        assert!(!html.contains("{{.Tracker}}"));
        if row["kind"] == "landing_page" {
            assert!(!html.contains("{{.Email"), "prefill must default off: {id}");
        }
        assert!(resource.get("id").is_none());
        assert!(resource.get("catalog_id").is_none());
        let preview = send(Method::POST, "/api/util/preview", &json!({"html":html}));
        assert_eq!(preview.status(), 200, "{id}: {}", preview.text().unwrap());
        let family = if row["kind"] == "email" {
            "templates"
        } else {
            "pages"
        };
        // Session creation still requires CSRF; the catalog cannot bypass it.
        assert_eq!(
            admin
                .post(format!("{base}/api/{family}/"))
                .json(resource)
                .send()
                .unwrap()
                .status(),
            403
        );
        let response = send(Method::POST, &format!("/api/{family}/"), resource);
        assert_eq!(response.status(), 201, "{id}: {}", response.text().unwrap());
        let created: Value = response.json().unwrap();
        ruphish_contracts::resource_details::validate(
            if family == "templates" {
                ruphish_contracts::resource_details::Kind::Template
            } else {
                ruphish_contracts::resource_details::Kind::Page
            },
            &created,
        )
        .unwrap();
        assert_eq!(created["html"], resource["html"]);
        saved.push((family, created["id"].as_i64().unwrap(), entry));
    }
    // Use the production browser transform with the actual API metadata, then save its output.
    let input = directory.path().join("starter-entries.json");
    std::fs::write(
        &input,
        serde_json::to_vec(&saved.iter().map(|(_, _, entry)| entry).collect::<Vec<_>>()).unwrap(),
    )
    .unwrap();
    let transformed = std::process::Command::new("node")
        .arg(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../ruphish-ui/web/tests/support/starter-options.mjs"),
        )
        .arg(&input)
        .output()
        .unwrap();
    assert!(
        transformed.status.success(),
        "{}",
        String::from_utf8_lossy(&transformed.stderr)
    );
    let copies: Vec<Value> = serde_json::from_slice(&transformed.stdout).unwrap();
    assert_eq!(copies.len(), 24);
    for copy in copies {
        let response = send(Method::POST, "/api/pages/", &copy);
        assert_eq!(response.status(), 201);
        let created: Value = response.json().unwrap();
        assert_eq!(
            get(&format!("/api/pages/{}", created["id"]))["html"],
            copy["html"]
        );
        let preview = send(
            Method::POST,
            "/api/util/preview",
            &json!({
                "html":copy["html"],"recipient":{"email":"prefilled@example.test","first_name":"Example","last_name":"Recipient","position":"Test"},
            }),
        );
        assert_eq!(preview.status(), 200);
        let rendered: Value = preview.json().unwrap();
        assert!(
            rendered["html"]
                .as_str()
                .unwrap()
                .contains("value=\"prefilled@example.test\"")
        );
        assert!(!rendered["html"].as_str().unwrap().contains("{{.Email"));
    }
    // Re-add, edit and delete independently in both resource families.
    for family in ["templates", "pages"] {
        let (_, first_id, entry) = saved.iter().find(|(f, _, _)| *f == family).unwrap();
        let mut copy = entry["resource"].clone();
        copy["name"] = json!(format!("{} (2)", copy["name"].as_str().unwrap()));
        let response = send(Method::POST, &format!("/api/{family}/"), &copy);
        assert_eq!(response.status(), 201);
        let mut second: Value = response.json().unwrap();
        let second_id = second["id"].as_i64().unwrap();
        assert_ne!(first_id, &second_id);
        second["html"] = json!("<html><body><p>Independent edit</p></body></html>");
        assert_eq!(
            send(Method::PUT, &format!("/api/{family}/{second_id}"), &second).status(),
            200
        );
        assert_eq!(
            get(&format!("/api/{family}/{first_id}"))["html"],
            entry["resource"]["html"]
        );
        let source_id = entry["summary"]["id"].as_str().unwrap();
        assert_eq!(get(&format!("/api/starter-templates/{source_id}")), *entry);
        assert_eq!(
            send(
                Method::DELETE,
                &format!("/api/{family}/{second_id}"),
                &json!({})
            )
            .status(),
            200
        );
        assert_eq!(
            get(&format!("/api/{family}/{first_id}"))["html"],
            entry["resource"]["html"]
        );
    }
    // Ordinary users can own resources, but cannot browse the company catalog.
    let response = send(
        Method::POST,
        "/api/users/",
        &json!({
            "username":"catalog-user","password":"catalog-user-password","role":"user", "password_change_required":false,
        }),
    );
    assert_eq!(response.status(), 200);
    let user: Value = response.json().unwrap();
    let user_key = user["api_key"].as_str().unwrap();
    let user_browser = browser();
    assert_eq!(
        sign_in(
            &user_browser,
            &base,
            "catalog-user",
            "catalog-user-password"
        )
        .status(),
        200
    );
    for path in [
        "/api/starter-templates/",
        "/api/starter-templates/page-microsoft-full-screen",
    ] {
        assert_eq!(
            user_browser
                .get(format!("{base}{path}"))
                .send()
                .unwrap()
                .status(),
            403
        );
        assert_eq!(
            anonymous
                .get(format!("{base}{path}"))
                .bearer_auth(user_key)
                .send()
                .unwrap()
                .status(),
            403
        );
    }
    assert_eq!(
        user_browser
            .get(format!("{base}{route}"))
            .send()
            .unwrap()
            .status(),
        403
    );
    let (family, id, _) = &saved[0];
    assert_eq!(
        user_browser
            .get(format!("{base}/api/{family}/{id}"))
            .send()
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        post(
            &admin,
            &base,
            "/impersonate",
            &[("username", "catalog-user")]
        )
        .status(),
        200
    );
    assert_eq!(
        admin
            .get(format!("{base}/api/starter-templates/"))
            .send()
            .unwrap()
            .status(),
        403
    );
    assert_eq!(post(&admin, &base, "/impersonate/exit", &[]).status(), 200);
    assert!(bootstrap(&admin, &base)["user"].is_object());
    process.stop();
    let mut restarted = Process::start(directory.path(), "admin");
    let base = format!("http://{}", restarted.ready()["admin"].as_str().unwrap());
    for (family, id, entry) in saved {
        let resource: Value = anonymous
            .get(format!("{base}/api/{family}/{id}"))
            .bearer_auth("synthetic-bootstrap-token")
            .send()
            .unwrap()
            .json()
            .unwrap();
        assert_eq!(resource["html"], entry["resource"]["html"]);
    }
    restarted.stop();
}
