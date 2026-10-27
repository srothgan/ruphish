//! Catalog reads and independent resource copies through the real server.
use super::*;
use reqwest::Method;

// Exercise the same catalog payload construction with an unpublished test fixture.
#[path = "../crates/ruphish-http/src/starter_templates/source.rs"]
mod catalog_source;

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
    for path in ["/api/pages/preview", "/api/learning-pages/preview"] {
        assert_eq!(
            anonymous
                .post(format!("{base}{path}"))
                .json(&json!({"html":"<p>Preview</p>"}))
                .send()
                .unwrap()
                .status(),
            401
        );
        assert_eq!(
            admin
                .post(format!("{base}{path}"))
                .json(&json!({"html":"<p>Preview</p>"}))
                .send()
                .unwrap()
                .status(),
            403
        );
        assert_eq!(send(Method::GET, path, &json!({})).status(), 405);
        assert_eq!(send(Method::POST, path, &json!({"html":42})).status(), 400);
    }
    // Only landing-page previews execute compatible expressions. Learning content stays literal.
    assert_eq!(
        send(
            Method::POST,
            "/api/pages/preview",
            &json!({"html":"<p>{{unfinished</p>"})
        )
        .status(),
        400
    );
    let literal = send(
        Method::POST,
        "/api/learning-pages/preview",
        &json!({"html":"<p>{{unfinished</p>"}),
    );
    assert_eq!(literal.status(), 200);
    assert!(
        literal.json::<Value>().unwrap()["html"]
            .as_str()
            .unwrap()
            .contains("{{unfinished")
    );
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
    assert!(rows.iter().any(|r| r["kind"] == "email"));
    assert!(rows.iter().any(|r| r["kind"] == "landing_page"));
    let learning: std::collections::BTreeSet<_> = rows
        .iter()
        .filter(|row| row["kind"] == "learning_page")
        .map(|row| row["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        learning,
        std::collections::BTreeSet::from([
            "learning-slam",
            "learning-stop-challenge-protect",
            "learning-recognize-resist-report-delete",
        ])
    );
    let branding: std::collections::BTreeSet<_> = rows
        .iter()
        .filter(|r| r["kind"] == "landing_page" && !r["company_logo"].is_null())
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
    assert_eq!(get("/api/learning-pages/"), json!([]));
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
    // Every bundled entry and the unpublished learning fixture use the real create/preview APIs.
    let learning = catalog_source::Source {
        id: "fixture-learning",
        name: "Fixture explanation",
        brand: "Fixture",
        kind: ruphish_contracts::starter_templates::Kind::LearningPage,
        html: "<div class='brand'>Default mark</div><h1>Fixture explanation</h1><p>{{.FirstName}}</p><p>Literal {{ unfinished example</p><form><input name='password'><button>Submit</button></form><script>unsafe()</script>",
        subject: "",
        text: "",
        logo: Some((".brand", 48, "Learning page header")),
    };
    let mut entries = Vec::new();
    for row in rows {
        let entry = get(&format!(
            "/api/starter-templates/{}",
            row["id"].as_str().unwrap()
        ));
        assert_eq!(entry["summary"], *row);
        entries.push(entry);
    }
    assert!(!rows.iter().any(|row| row["id"] == "fixture-learning"));
    entries.push(serde_json::to_value(learning.entry()).unwrap());
    let mut saved = Vec::new();
    for entry in entries {
        let row = &entry["summary"];
        let id = row["id"].as_str().unwrap();
        let resource = &entry["resource"];
        let html = resource["html"].as_str().unwrap();
        assert!(!html.contains("{{.Tracker}}"));
        if row["kind"] == "landing_page" {
            assert!(!html.contains("{{.Email"), "prefill must default off: {id}");
        }
        assert!(resource.get("id").is_none());
        assert!(resource.get("catalog_id").is_none());
        let preview_path = if row["kind"] == "learning_page" {
            assert_eq!(row["prefill"], false);
            assert!(resource.get("redirect_url").is_none());
            assert!(resource.get("public_id").is_none());
            "/api/learning-pages/preview"
        } else if row["kind"] == "landing_page" {
            "/api/pages/preview"
        } else {
            "/api/util/preview"
        };
        let preview = send(Method::POST, preview_path, &json!({"html":html}));
        assert_eq!(preview.status(), 200, "{id}: {}", preview.text().unwrap());
        if row["kind"] == "learning_page" {
            let rendered: Value = preview.json().unwrap();
            let html = rendered["html"].as_str().unwrap();
            if id == "fixture-learning" {
                assert!(html.contains("{{.FirstName}}"));
            }
            for removed in ["<form", "<input", "<button", "<script"] {
                assert!(!html.contains(removed));
            }
        }
        let family = match row["kind"].as_str().unwrap() {
            "email" => "templates",
            "landing_page" => "pages",
            "learning_page" => "learning-pages",
            unexpected => panic!("Unsupported catalog kind: {unexpected}"),
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
        ruphish_contracts::resource_details::decode(
            match family {
                "templates" => ruphish_contracts::resource_details::Kind::Template,
                "learning-pages" => ruphish_contracts::resource_details::Kind::LearningPage,
                _ => ruphish_contracts::resource_details::Kind::Page,
            },
            created.clone(),
        )
        .unwrap();
        assert_eq!(created["html"], resource["html"]);
        if family == "learning-pages" {
            let before = get("/api/learning-pages/");
            let duplicate = send(Method::POST, "/api/learning-pages/", resource);
            assert_eq!(duplicate.status(), 409);
            assert_eq!(get("/api/learning-pages/"), before);
            assert_eq!(
                get(&format!("/api/learning-pages/{}", created["id"]))["html"],
                resource["html"]
            );
        }
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
    let expected_options = saved
        .iter()
        .filter(|(_, _, entry)| entry["summary"]["kind"] != "email")
        .map(|(_, _, entry)| {
            if entry["summary"]["company_logo"].is_null() {
                1
            } else {
                2
            }
        })
        .sum::<usize>();
    assert_eq!(copies.len(), expected_options);
    let mut prepared_copies = Vec::new();
    for copy in copies {
        let learning = copy["kind"] == "learning_page";
        let family = if learning { "learning-pages" } else { "pages" };
        let payload = &copy["resource"];
        let response = send(Method::POST, &format!("/api/{family}/"), payload);
        assert_eq!(response.status(), 201);
        let created: Value = response.json().unwrap();
        assert_eq!(
            get(&format!("/api/{family}/{}", created["id"]))["html"],
            payload["html"]
        );
        prepared_copies.push((
            family,
            created["id"].as_i64().unwrap(),
            json!({"resource":payload}),
        ));
        let preview = send(
            Method::POST,
            if learning {
                "/api/learning-pages/preview"
            } else {
                "/api/pages/preview"
            },
            &json!({
                "html":payload["html"],"recipient":{"email":"prefilled@example.test","first_name":"Example","last_name":"Recipient","position":"Test"},
            }),
        );
        assert_eq!(preview.status(), 200);
        let rendered: Value = preview.json().unwrap();
        if learning {
            let html = rendered["html"].as_str().unwrap();
            if copy["source_id"] == "fixture-learning" {
                assert!(html.contains("{{.FirstName}}"));
                assert!(html.contains("Literal {{ unfinished example"));
                assert_eq!(
                    html.contains("data:image/png;base64,"),
                    copy["branded"] == true
                );
            }
            assert!(!html.contains("prefilled@example.test"));
            assert!(!html.contains("<input"));
            continue;
        }
        assert!(
            rendered["html"]
                .as_str()
                .unwrap()
                .contains("value=\"prefilled@example.test\"")
        );
        assert!(!rendered["html"].as_str().unwrap().contains("{{.Email"));
    }
    // Re-add, edit and delete independently in every resource family.
    for family in ["templates", "pages", "learning-pages"] {
        let (_, first_id, entry) = saved.iter().find(|(f, _, _)| *f == family).unwrap();
        let mut copy = entry["resource"].clone();
        copy["name"] = json!(format!("{} (2)", copy["name"].as_str().unwrap()));
        let response = send(Method::POST, &format!("/api/{family}/"), &copy);
        assert_eq!(response.status(), 201);
        let mut second: Value = response.json().unwrap();
        let second_id = second["id"].as_i64().unwrap();
        assert_ne!(first_id, &second_id);
        if family == "learning-pages" {
            assert_ne!(
                second["public_id"],
                get(&format!("/api/{family}/{first_id}"))["public_id"]
            );
        }
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
        if source_id == "fixture-learning" {
            assert_eq!(
                admin
                    .get(format!("{base}/api/starter-templates/{source_id}"))
                    .send()
                    .unwrap()
                    .status(),
                404
            );
        } else {
            assert_eq!(get(&format!("/api/starter-templates/{source_id}")), *entry);
        }
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
    for family in ["templates", "pages", "learning-pages"] {
        let (_, id, _) = saved.iter().find(|(f, _, _)| *f == family).unwrap();
        for method in [Method::GET, Method::PUT, Method::DELETE] {
            assert_eq!(
                anonymous.request(method, format!("{base}/api/{family}/{id}"))
                    .bearer_auth(user_key)
                    .json(&json!({"id":id,"name":"Other owner's resource","html":"<p>Unauthorized replacement</p>","subject":"Unauthorized"}))
                    .send().unwrap().status(),
                404
            );
        }
    }
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
    for (family, id, entry) in saved.into_iter().chain(prepared_copies) {
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
