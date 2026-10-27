//! Authentication acceptance through real executable processes and SQLite state.
use super::*;
use reqwest::blocking::{Client, Response};
use sqlx::Row;

pub(super) fn browser() -> Client {
    Client::builder()
        .no_proxy()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap()
}
fn endpoint(process: &mut Process) -> String {
    format!("http://{}", process.ready()["admin"].as_str().unwrap())
}
pub(super) fn bootstrap(client: &Client, base: &str) -> Value {
    let response = client.get(format!("{base}/auth/session")).send().unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    response.json().unwrap()
}
pub(super) fn post(client: &Client, base: &str, path: &str, fields: &[(&str, &str)]) -> Response {
    let token = bootstrap(client, base)["csrf_token"]
        .as_str()
        .unwrap()
        .to_owned();
    client
        .post(format!("{base}{path}"))
        .header("x-csrf-token", token)
        .header("origin", base)
        .header("referer", format!("{base}/login"))
        .header("sec-fetch-site", "same-origin")
        .header("accept", "application/json")
        .form(fields)
        .send()
        .unwrap()
}
pub(super) fn sign_in(client: &Client, base: &str, username: &str, password: &str) -> Response {
    post(
        client,
        base,
        "/login",
        &[("username", username), ("password", password)],
    )
}
pub(super) fn setup(client: &Client, base: &str) -> Value {
    assert_eq!(
        sign_in(client, base, "admin", "synthetic-bootstrap-password").status(),
        200
    );
    let reset = post(
        client,
        base,
        "/reset_password",
        &[
            ("password", "established-admin-password"),
            ("confirm_password", "established-admin-password"),
        ],
    );
    assert_eq!(reset.status(), 200, "{}", reset.text().unwrap());
    bootstrap(client, base)
}
fn admin_key() -> &'static str {
    "synthetic-bootstrap-token"
}

#[test]
fn generic_self_password_updates_require_the_dedicated_reauthenticated_flow() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = endpoint(&mut process);
    let admin = browser();
    let session = setup(&admin, &base);
    let edit = json!({"username":"admin","password":"unauthorized-replacement","role":"admin","password_change_required":false,"account_locked":false});
    let response = admin
        .put(format!("{base}/api/users/1"))
        .header("x-csrf-token", session["csrf_token"].as_str().unwrap())
        .header("origin", &base)
        .json(&edit)
        .send()
        .unwrap();
    assert_eq!(response.status(), 401);
    let response = client()
        .put(format!("{base}/api/users/1"))
        .bearer_auth(admin_key())
        .json(&edit)
        .send()
        .unwrap();
    assert_eq!(response.status(), 401);
    assert_eq!(
        sign_in(&browser(), &base, "admin", "established-admin-password").status(),
        200
    );
    assert_eq!(
        sign_in(&browser(), &base, "admin", "unauthorized-replacement").status(),
        401
    );
    assert_eq!(
        post(
            &admin,
            &base,
            "/internal/password",
            &[
                ("current_password", "established-admin-password"),
                ("new_password", "verified-replacement"),
                ("confirm_new_password", "verified-replacement")
            ]
        )
        .status(),
        200
    );
    assert_eq!(
        sign_in(&browser(), &base, "admin", "verified-replacement").status(),
        200
    );
    process.stop();
}

#[test]
fn dedicated_user_pages_preserve_browser_crud_and_reject_unauthorized_access() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = endpoint(&mut process);
    let admin = browser();
    let anonymous = browser();
    let session = setup(&admin, &base);
    for path in ["/users", "/users/new", "/users/1/edit"] {
        let denied = anonymous.get(format!("{base}{path}")).send().unwrap();
        assert_eq!(denied.status(), 307);
        assert_eq!(denied.headers()["location"], format!("/login?next={path}"));
        assert_eq!(
            admin.get(format!("{base}{path}")).send().unwrap().status(),
            200
        );
    }
    let count =
        || database(directory.path(), "SELECT count(*) AS n FROM users")[0].get::<i64, _>("n");
    assert_eq!(count(), 1, "Opening forms must not create accounts");
    let send = |method, path: &str, body: &Value, csrf: &str| {
        admin
            .request(method, format!("{base}{path}"))
            .header("x-ruphish-client", "browser")
            .header("origin", &base)
            .header("x-csrf-token", csrf)
            .json(body)
            .send()
            .unwrap()
    };
    let csrf = session["csrf_token"].as_str().unwrap();
    let mut draft = json!({"username":"new-user","password":"new-user-password","role":"user","password_change_required":true,"account_locked":false});
    assert_eq!(
        send(reqwest::Method::POST, "/api/users/", &draft, "invalid").status(),
        403
    );
    draft["password"] = json!("");
    assert_eq!(
        send(reqwest::Method::POST, "/api/users/", &draft, csrf).status(),
        400
    );
    assert_eq!(count(), 1);
    draft["password"] = json!("new-user-password");
    let created = send(reqwest::Method::POST, "/api/users/", &draft, csrf);
    assert_eq!(created.status(), 200);
    let created: Value = created.json().unwrap();
    let id = created["id"].as_i64().unwrap();
    assert_eq!(created["password_change_required"], true);
    assert_eq!(count(), 2);
    assert_eq!(
        send(reqwest::Method::POST, "/api/users/", &draft, csrf).status(),
        400
    );
    assert_eq!(
        count(),
        2,
        "Duplicate creation must not write another account"
    );
    let member = browser();
    assert_eq!(
        sign_in(&member, &base, "new-user", "new-user-password").status(),
        200
    );
    assert_eq!(
        member
            .get(format!("{base}/users/new"))
            .send()
            .unwrap()
            .headers()["location"],
        "/reset_password?next=%2Fusers%2Fnew"
    );
    let api_path = format!("/api/users/{id}");
    draft["password"] = json!("");
    draft["password_change_required"] = json!(false);
    draft["username"] = json!("renamed-user");
    assert_eq!(
        send(reqwest::Method::PUT, &api_path, &draft, csrf).status(),
        200
    );
    let stored: Value = admin
        .get(format!("{base}{api_path}"))
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(stored["username"], "renamed-user");
    assert_eq!(stored["password_change_required"], false);
    assert_eq!(
        admin
            .get(format!("{base}/users/{id}/edit"))
            .send()
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        sign_in(&member, &base, "renamed-user", "new-user-password").status(),
        200,
        "Blank update password must retain the existing credential"
    );
    for path in [
        "/settings".to_owned(),
        "/settings/starter-templates".to_owned(),
        "/users".to_owned(),
        "/users/new".to_owned(),
        format!("/users/{id}/edit"),
    ] {
        // Intentional browser-page compatibility change: forbidden pages keep
        // HTTP 403 but return the inert UI shell instead of an API JSON envelope.
        let denied = member.get(format!("{base}{path}")).send().unwrap();
        assert_eq!(denied.status(), 403);
        assert_eq!(denied.headers()["content-type"], "text/html; charset=utf-8");
        assert_eq!(denied.headers()["cache-control"], "no-store");
        assert_eq!(
            denied.text().unwrap(),
            std::fs::read_to_string(directory.path().join("dist/admin/index.html")).unwrap()
        );
    }
    for path in [
        "/user",
        "/user/settings",
        "/user/password",
        "/internal/preferences",
    ] {
        assert_eq!(
            member.get(format!("{base}{path}")).send().unwrap().status(),
            200
        );
    }
    for path in ["/api/users/", "/api/settings"] {
        let denied = member.get(format!("{base}{path}")).send().unwrap();
        assert_eq!(denied.status(), 403);
        assert_eq!(denied.headers()["content-type"], "application/json");
        let body: Value = denied.json().unwrap();
        assert_eq!(body["success"], false);
        assert_eq!(body["code"], "AUTH_FORBIDDEN");
    }
    for path in ["/not-a-page", "/api/not-a-resource"] {
        let missing = member.get(format!("{base}{path}")).send().unwrap();
        assert_eq!(missing.status(), 404);
        assert_eq!(missing.text().unwrap(), "404 page not found\n");
    }
    let member_session = bootstrap(&member, &base);
    let saved = member
        .put(format!("{base}/internal/preferences"))
        .header("origin", &base)
        .header(
            "x-csrf-token",
            member_session["csrf_token"].as_str().unwrap(),
        )
        .json(&json!({"language":"de","time_zone":"Europe/Berlin"}))
        .send()
        .unwrap();
    assert_eq!(saved.status(), 200);
    assert_eq!(bootstrap(&member, &base)["preferences"]["language"], "de");
    assert_eq!(
        bootstrap(&admin, &base)["preferences"],
        session["preferences"]
    );
    assert_eq!(
        admin
            .get(format!("{base}/api/users/999999"))
            .send()
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        send(reqwest::Method::PUT, "/api/users/999999", &draft, csrf).status(),
        404
    );
    assert_eq!(count(), 2, "Missing edit targets must not become creates");
    assert_eq!(
        post(
            &admin,
            &base,
            "/impersonate",
            &[("username", "renamed-user")]
        )
        .status(),
        200
    );
    assert_eq!(
        admin
            .get(format!("{base}/users/new"))
            .send()
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        admin
            .get(format!("{base}/users/{id}/edit"))
            .send()
            .unwrap()
            .status(),
        403
    );
    process.stop();
}

#[test]
fn unknown_browser_pages_load_the_real_ui_without_masking_protocol_or_asset_errors() {
    // Intentional compatibility difference: direct admin document navigation must
    // reach the styled UI fallback with HTTP 404. API, asset and public misses
    // retain their plain-text contract; a successful shell response is not enough.
    let directory = workspace_with_built_assets();
    let mut process = Process::start(directory.path(), "all");
    let ready = process.ready();
    let base = format!("http://{}", ready["admin"].as_str().unwrap());
    let public = format!("http://{}", ready["phish"].as_str().unwrap());
    let admin = browser();
    setup(&admin, &base);
    let shell = std::fs::read_to_string(directory.path().join("dist/admin/index.html")).unwrap();
    let html = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";
    for path in ["/does-not-exist", "/nested/unknown/page?from=bookmark"] {
        // Repeating the request covers entering a URL and refreshing that URL.
        for _ in 0..2 {
            let response = admin
                .get(format!("{base}{path}"))
                .header("accept", html)
                .send()
                .unwrap();
            assert_eq!(response.status(), 404);
            assert_eq!(
                response.headers()["content-type"],
                "text/html; charset=utf-8"
            );
            assert_eq!(response.headers()["cache-control"], "no-store");
            assert_eq!(response.headers()["x-content-type-options"], "nosniff");
            assert_eq!(response.text().unwrap(), shell);
        }
    }
    let head = admin
        .head(format!("{base}/does-not-exist"))
        .header("accept", html)
        .send()
        .unwrap();
    assert_eq!(head.status(), 404);
    assert_eq!(head.headers()["content-type"], "text/html; charset=utf-8");
    assert!(head.bytes().unwrap().is_empty());
    let anonymous = browser()
        .get(format!("{base}/does-not-exist"))
        .header("accept", html)
        .send()
        .unwrap();
    assert_eq!(anonymous.status(), 307);
    assert_eq!(
        anonymous.headers()["location"],
        "/login?next=/does-not-exist"
    );
    for accept in ["*/*", "application/json", "text/plain", "text/html;q=0,*/*"] {
        let response = admin
            .get(format!("{base}/does-not-exist"))
            .header("accept", accept)
            .send()
            .unwrap();
        assert_eq!(response.status(), 404, "{accept}");
        assert_eq!(response.text().unwrap(), "404 page not found\n");
    }
    for path in [
        "/api/not-a-resource",
        "/api",
        "/auth/not-a-route",
        "/internal/not-a-route",
        "/missing.wasm",
        "/missing.js",
        "/missing.css",
        "/snippets/missing/module.js",
        "/.git/config",
        "/config.json",
    ] {
        let response = admin
            .get(format!("{base}{path}"))
            .header("accept", html)
            .send()
            .unwrap();
        assert_eq!(response.status(), 404, "{path}");
        assert_eq!(response.text().unwrap(), "404 page not found\n", "{path}");
    }
    for method in [
        reqwest::Method::POST,
        reqwest::Method::PUT,
        reqwest::Method::DELETE,
    ] {
        let response = admin
            .request(method, format!("{base}/does-not-exist"))
            .header("accept", html)
            .send()
            .unwrap();
        assert_eq!(response.status(), 404);
        assert_eq!(response.text().unwrap(), "404 page not found\n");
    }
    let response = admin
        .get(format!("{public}/does-not-exist"))
        .header("accept", html)
        .send()
        .unwrap();
    assert_eq!(response.status(), 404);
    assert_eq!(response.text().unwrap(), "404 page not found\n");
    let valid = admin
        .get(format!("{base}/templates"))
        .header("accept", html)
        .send()
        .unwrap();
    assert_eq!(valid.status(), 200);
    assert_eq!(valid.text().unwrap(), shell);
    assert_eq!(
        admin
            .get(format!("{base}/api/templates/summary"))
            .send()
            .unwrap()
            .status(),
        200
    );
    process.stop();
}

#[test]
fn browser_detail_navigation_preserves_ownership_and_handles_deleted_records() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = endpoint(&mut process);
    let admin = browser();
    let session = setup(&admin, &base);
    let csrf = session["csrf_token"].as_str().unwrap();
    create(&admin, &base, "other-reader", "user");
    let member = browser();
    assert_eq!(
        sign_in(&member, &base, "other-reader", "another-user-password").status(),
        200
    );
    let shell = std::fs::read_to_string(directory.path().join("dist/admin/index.html")).unwrap();
    let anonymous = browser();
    for (family, draft) in [
        (
            "groups",
            json!({"name":"Owner-only group","targets":[{"email":"synthetic@example.test","first_name":"Private"}]}),
        ),
        (
            "templates",
            json!({"name":"Owner-only template","text":"Private template body"}),
        ),
        (
            "pages",
            json!({"name":"Owner-only page","html":"<p>Private page body</p>"}),
        ),
        (
            "smtp",
            json!({"name":"Owner-only profile","host":"127.0.0.1:25","from_address":"sender@example.test","password":"synthetic-private-service-secret"}),
        ),
    ] {
        let created = admin
            .post(format!("{base}/api/{family}/"))
            .header("origin", &base)
            .header("x-csrf-token", csrf)
            .header("x-ruphish-client", "browser")
            .json(&draft)
            .send()
            .unwrap();
        assert_eq!(created.status(), 201);
        let record: Value = created.json().unwrap();
        let id = record["id"].as_i64().unwrap();
        let api_path = format!("{base}/api/{family}/{id}");
        // Both valid and missing detail/edit URLs serve only the inert shell.
        // Reading the record is independently authenticated and owner-scoped.
        for address in [id.to_string(), "999999".into(), "not-an-id".into()] {
            for suffix in ["", "/edit"] {
                let path = format!("/{family}/{address}{suffix}");
                let redirect = anonymous.get(format!("{base}{path}")).send().unwrap();
                assert_eq!(redirect.status(), 307);
                assert_eq!(
                    redirect.headers()["location"],
                    format!("/login?next={path}")
                );
                for user in [&admin, &member] {
                    let page = user.get(format!("{base}{path}")).send().unwrap();
                    assert_eq!(page.status(), 200);
                    assert_eq!(page.headers()["cache-control"], "no-store");
                    assert_eq!(page.text().unwrap(), shell);
                }
            }
        }
        let legitimate = admin.get(&api_path).send().unwrap();
        assert_eq!(legitimate.status(), 200);
        let revision = legitimate.headers().get("etag").cloned();
        let saved: Value = legitimate.json().unwrap();
        assert_eq!(saved["name"], draft["name"]);
        let foreign = member.get(&api_path).send().unwrap();
        assert_eq!(foreign.status(), 404);
        let foreign: Value = foreign.json().unwrap();
        assert_eq!(foreign["code"], "RESOURCE_NOT_FOUND");
        let missing = member
            .get(format!("{base}/api/{family}/999999"))
            .send()
            .unwrap();
        assert_eq!(missing.status(), 404);
        assert_eq!(
            foreign,
            missing.json::<Value>().unwrap(),
            "missing and foreign records must not be distinguishable"
        );
        let member_csrf = bootstrap(&member, &base)["csrf_token"]
            .as_str()
            .unwrap()
            .to_owned();
        let denied_delete = member
            .delete(&api_path)
            .header("origin", &base)
            .header("x-ruphish-client", "browser")
            .header("x-csrf-token", &member_csrf)
            .send()
            .unwrap();
        assert_eq!(denied_delete.status(), 404);
        assert_eq!(
            admin
                .get(&api_path)
                .send()
                .unwrap()
                .json::<Value>()
                .unwrap(),
            saved
        );
        let mut delete = admin
            .delete(&api_path)
            .header("origin", &base)
            .header("x-ruphish-client", "browser")
            .header("x-csrf-token", csrf);
        if let Some(revision) = revision {
            delete = delete.header("if-match", revision);
        }
        assert_eq!(delete.send().unwrap().status(), 200);
        let deleted = admin.get(&api_path).send().unwrap();
        assert_eq!(deleted.status(), 404);
        assert_eq!(deleted.headers()["cache-control"], "no-store");
        assert_eq!(
            deleted.json::<Value>().unwrap()["code"],
            "RESOURCE_NOT_FOUND"
        );
        let list: Value = admin
            .get(format!("{base}/api/{family}/"))
            .send()
            .unwrap()
            .json()
            .unwrap();
        assert!(list.as_array().unwrap().iter().all(|row| row["id"] != id));
    }
    assert_eq!(post(&member, &base, "/logout", &[]).status(), 200);
    assert_eq!(
        member
            .get(format!("{base}/groups/999999"))
            .send()
            .unwrap()
            .status(),
        307
    );
    assert_eq!(
        member
            .get(format!("{base}/api/groups/999999"))
            .send()
            .unwrap()
            .status(),
        401
    );
    process.stop();
}

#[test]
fn account_routes_preserve_setup_and_reauthenticated_password_changes() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = endpoint(&mut process);
    let client = browser();
    let password_page = || client.get(format!("{base}/user/password")).send().unwrap();
    let anonymous = password_page();
    assert_eq!(anonymous.status(), 307);
    assert_eq!(
        anonymous.headers()["location"],
        "/login?next=/user/password"
    );
    assert_eq!(
        sign_in(&client, &base, "admin", "synthetic-bootstrap-password").status(),
        200
    );
    let forced = password_page();
    assert_eq!(forced.status(), 307);
    assert_eq!(
        forced.headers()["location"],
        "/reset_password?next=%2Fuser%2Fpassword"
    );
    for path in ["/user", "/user/settings"] {
        assert_eq!(
            client.get(format!("{base}{path}")).send().unwrap().status(),
            307
        );
    }
    assert_eq!(
        client
            .get(format!("{base}/internal/preferences"))
            .send()
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        post(&client, &base, "/internal/password", &[]).status(),
        403
    );
    assert_eq!(
        post(
            &client,
            &base,
            "/reset_password",
            &[
                ("password", "established-admin-password"),
                ("confirm_password", "established-admin-password")
            ]
        )
        .status(),
        200
    );
    assert_eq!(password_page().status(), 200);
    for path in ["/user", "/user/settings", "/settings"] {
        let response = client.get(format!("{base}{path}")).send().unwrap();
        assert_eq!(response.status(), 200, "{path}");
        assert!(response.text().unwrap().contains("Ruphish"));
        assert_eq!(post(&client, &base, path, &[]).status(), 405, "{path}");
    }
    for path in ["/settings/password", "/api/user-preferences"] {
        assert_eq!(
            client.get(format!("{base}{path}")).send().unwrap().status(),
            404
        );
    }
    for method in [reqwest::Method::GET, reqwest::Method::POST] {
        let response = client
            .request(method, format!("{base}/reset_password"))
            .header(
                "x-csrf-token",
                bootstrap(&client, &base)["csrf_token"].as_str().unwrap(),
            )
            .header("origin", &base)
            .send()
            .unwrap();
        assert_eq!(response.status(), 307);
        assert_eq!(response.headers()["location"], "/user/password");
    }
    let change = |current: &str| {
        post(
            &client,
            &base,
            "/internal/password",
            &[
                ("current_password", current),
                ("new_password", "replacement-admin-password"),
                ("confirm_new_password", "replacement-admin-password"),
            ],
        )
    };
    assert_eq!(change("incorrect-current-password").status(), 400);
    assert_eq!(bootstrap(&client, &base)["user"]["username"], "admin");
    assert_eq!(change("established-admin-password").status(), 200);
    // Browser route refresh works after changing credentials; it is not a new mutation API.
    assert_eq!(password_page().status(), 200);
    assert_eq!(
        client
            .post(format!("{base}/user/password"))
            .send()
            .unwrap()
            .status(),
        405
    );
    assert_eq!(post(&client, &base, "/logout", &[]).status(), 200);
    assert_eq!(password_page().status(), 307);
    assert_eq!(
        sign_in(&client, &base, "admin", "established-admin-password").status(),
        401
    );
    assert_eq!(
        sign_in(&client, &base, "admin", "replacement-admin-password").status(),
        200
    );
    process.stop();
}

#[test]
fn internal_account_endpoints_require_sessions_and_reject_explicit_api_credentials() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = endpoint(&mut process);
    let admin = browser();
    let anonymous = browser();
    let session = setup(&admin, &base);
    let csrf = session["csrf_token"].as_str().unwrap();
    let chosen =
        json!({"language":"de","format_locale":null,"time_zone":null,"show_utc_offset":true});
    for (method, path) in [
        (reqwest::Method::GET, "/internal/preferences"),
        (reqwest::Method::PUT, "/internal/preferences"),
        (reqwest::Method::POST, "/internal/password"),
    ] {
        assert_eq!(
            anonymous
                .request(method.clone(), format!("{base}{path}"))
                .send()
                .unwrap()
                .status(),
            401
        );
        // Valid, invalid and empty credentials all reject, even alongside a valid session/CSRF.
        for browser in [&anonymous, &admin] {
            for key in [admin_key(), "invalid", ""] {
                for query in [false, true] {
                    let url = if query {
                        format!("{base}{path}?api_key={key}")
                    } else {
                        format!("{base}{path}")
                    };
                    let request = browser
                        .request(method.clone(), url)
                        .header("origin", &base)
                        .header("x-csrf-token", csrf);
                    let request = if query {
                        request
                    } else {
                        request.bearer_auth(key)
                    };
                    let response = request.send().unwrap();
                    assert_eq!(response.status(), 401, "{path}, query={query}");
                    assert_eq!(
                        response.json::<Value>().unwrap()["code"],
                        "AUTH_API_KEY_INVALID"
                    );
                }
            }
        }
        if method != reqwest::Method::GET {
            assert_eq!(
                admin
                    .request(method.clone(), format!("{base}{path}"))
                    .header("origin", &base)
                    .json(&chosen)
                    .send()
                    .unwrap()
                    .status(),
                403
            );
            assert_eq!(
                admin
                    .request(method, format!("{base}{path}"))
                    .header("origin", "https://untrusted.invalid")
                    .header("x-csrf-token", csrf)
                    .json(&chosen)
                    .send()
                    .unwrap()
                    .status(),
                403
            );
        }
    }
    assert_eq!(
        post(
            &admin,
            &base,
            "/internal/password",
            &[("api_key", admin_key())]
        )
        .status(),
        401
    );
    assert_eq!(
        bootstrap(&admin, &base)["preferences"],
        session["preferences"]
    );
    assert_eq!(
        admin
            .get(format!("{base}/internal/preferences"))
            .send()
            .unwrap()
            .json::<Value>()
            .unwrap(),
        session["preferences"]
    );
    // Programmable account APIs continue to support keys without a browser session or CSRF.
    assert_eq!(
        anonymous
            .get(format!("{base}/api/users/"))
            .bearer_auth(admin_key())
            .send()
            .unwrap()
            .status(),
        200
    );
    let reset = anonymous
        .post(format!("{base}/api/reset"))
        .bearer_auth(admin_key())
        .send()
        .unwrap();
    assert_eq!(reset.status(), 200);
    let reset: Value = reset.json().unwrap();
    assert_eq!(
        anonymous
            .get(format!("{base}/api/users/1"))
            .bearer_auth(reset["data"].as_str().unwrap())
            .send()
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        sign_in(&browser(), &base, "admin", "established-admin-password").status(),
        200
    );
    process.stop();
}

#[test]
fn browser_resource_create_read_edit_and_reload_routes_preserve_saved_data() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = endpoint(&mut process);
    let anonymous = browser();
    let admin = browser();
    let session = setup(&admin, &base);
    for family in ["groups", "templates", "pages", "smtp"] {
        for suffix in ["new", "1", "1/edit"] {
            let path = format!("/{family}/{suffix}");
            let denied = anonymous.get(format!("{base}{path}")).send().unwrap();
            assert_eq!(denied.status(), 307, "{path}");
            assert!(
                denied.headers()["location"]
                    .to_str()
                    .unwrap()
                    .starts_with("/login")
            );
            let loaded = admin.get(format!("{base}{path}")).send().unwrap();
            assert_eq!(loaded.status(), 200, "{path}");
            assert!(loaded.text().unwrap().contains("Ruphish"));
        }
    }
    let send = |method, path: &str, value: &Value| {
        admin
            .request(method, format!("{base}{path}"))
            .header("x-ruphish-client", "browser")
            .header("x-csrf-token", session["csrf_token"].as_str().unwrap())
            .header("origin", &base)
            .json(value)
            .send()
            .unwrap()
    };
    let group =
        json!({"name":"Équipe","targets":[{"email":"alice@example.test","first_name":"Alice"}]});
    let created = send(reqwest::Method::POST, "/api/groups/", &group);
    assert_eq!(created.status(), 201);
    let created: Value = created.json().unwrap();
    let path = format!("/api/groups/{}", created["id"]);
    let read = || {
        admin
            .get(format!("{base}{path}"))
            .header("x-ruphish-client", "browser")
            .send()
            .unwrap()
    };
    let fetched = read();
    assert_eq!(fetched.status(), 200);
    assert_eq!(fetched.json::<Value>().unwrap(), created);
    let mut edited = created.clone();
    edited["name"] = json!("");
    assert_eq!(send(reqwest::Method::PUT, &path, &edited).status(), 400);
    assert_eq!(read().json::<Value>().unwrap(), created);
    edited["name"] = json!("Renamed group");
    let saved = send(reqwest::Method::PUT, &path, &edited);
    assert_eq!(saved.status(), 200);
    let saved: Value = saved.json().unwrap();
    assert_eq!(read().json::<Value>().unwrap(), saved);
    process.stop();
}
fn create(client: &Client, base: &str, name: &str, role: &str) -> Value {
    let response = client
        .post(format!("{base}/api/users/"))
        .bearer_auth(admin_key())
        .json(&json!({"username":name,"password":"another-user-password","role":role}))
        .send()
        .unwrap();
    assert_eq!(response.status(), 200, "{}", response.text().unwrap());
    response.json().unwrap()
}

#[test]
fn self_deletion_and_stale_browser_requests_cannot_remove_accounts() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = endpoint(&mut process);
    let first = browser();
    let session = setup(&first, &base);
    let csrf = session["csrf_token"].as_str().unwrap();
    let second = browser();
    assert_eq!(
        sign_in(&second, &base, "admin", "established-admin-password").status(),
        200
    );
    // A second administrator must not make self-deletion permissible.
    let other = create(&first, &base, "other-admin", "admin");
    let other_id = other["id"].as_i64().unwrap();
    let target = create(&first, &base, "target", "user");
    let target_id = target["id"].as_i64().unwrap();
    let target_browser = browser();
    assert_eq!(
        sign_in(&target_browser, &base, "target", "another-user-password").status(),
        200
    );
    let delete = |client: &Client, id: i64, token: &str| {
        client
            .delete(format!("{base}/api/users/{id}"))
            .header("x-ruphish-client", "browser")
            .header("x-csrf-token", token)
            .header("origin", &base)
            .header("sec-fetch-site", "same-origin")
            .send()
            .unwrap()
    };
    let denied = delete(&first, 1, csrf);
    assert_eq!(denied.status(), 403);
    assert_eq!(
        denied.json::<Value>().unwrap()["message"],
        "You cannot delete your own account"
    );
    assert_eq!(
        first
            .delete(format!("{base}/api/users/1"))
            .bearer_auth(admin_key())
            .send()
            .unwrap()
            .status(),
        403
    );
    assert_eq!(bootstrap(&first, &base)["user"]["id"], 1);
    assert_eq!(
        database(directory.path(), "SELECT count(*) AS n FROM users")[0].get::<i64, _>("n"),
        3
    );

    // An explicit bad key never falls back to a valid cookie, even with the UI marker.
    let bad_key = first
        .get(format!("{base}/api/users/"))
        .header("x-ruphish-client", "browser")
        .bearer_auth("invalid")
        .send()
        .unwrap();
    assert_eq!(bad_key.status(), 401);
    assert_eq!(
        bad_key.json::<Value>().unwrap()["message"],
        "Invalid API Key"
    );
    assert_eq!(post(&first, &base, "/logout", &[]).status(), 200);
    for id in [target_id, other_id, 1] {
        let stale = delete(&first, id, csrf);
        assert_eq!(stale.status(), 401);
        assert_eq!(
            stale.json::<Value>().unwrap()["message"],
            "Your session ended. Sign in again to continue."
        );
    }
    let stale_list = first
        .get(format!("{base}/api/users/"))
        .header("x-ruphish-client", "browser")
        .send()
        .unwrap();
    assert_eq!(stale_list.status(), 401);
    assert_eq!(
        stale_list.json::<Value>().unwrap()["message"],
        "Your session ended. Sign in again to continue."
    );
    let legacy = first.get(format!("{base}/api/users/")).send().unwrap();
    assert_eq!(legacy.status(), 401);
    assert_eq!(
        legacy.json::<Value>().unwrap()["message"],
        "API Key not set"
    );
    assert!(bootstrap(&first, &base)["user"].is_null());
    assert_eq!(
        database(directory.path(), "SELECT count(*) AS n FROM users")[0].get::<i64, _>("n"),
        3
    );

    // Independent login survives logout, and authorized deletion still works.
    let second_session = bootstrap(&second, &base);
    assert_eq!(second_session["user"]["id"], 1);
    assert_eq!(
        delete(
            &second,
            target_id,
            second_session["csrf_token"].as_str().unwrap()
        )
        .status(),
        200
    );
    assert!(bootstrap(&target_browser, &base)["user"].is_null());
    assert_eq!(
        delete(&target_browser, other_id, "stale-token").status(),
        401
    );
    assert_eq!(
        database(
            directory.path(),
            "SELECT count(*) AS n FROM users WHERE username='target'"
        )[0]
        .get::<i64, _>("n"),
        0
    );
    process.stop();
    let logs = std::fs::read_to_string(directory.path().join("server.log")).unwrap();
    let deletions: Vec<_> = logs
        .lines()
        .filter(|line| line.contains("account_deleted"))
        .collect();
    assert_eq!(
        deletions.len(),
        1,
        "Rejected deletions must not log success"
    );
    assert!(deletions[0].contains("actor_id=1"));
    assert!(deletions[0].contains(&format!("user_id={target_id}")));
    assert!(!logs.contains(admin_key()));
    assert!(!logs.contains(csrf));
    assert!(!logs.contains("another-user-password"));
    let mut restarted = Process::start(directory.path(), "admin");
    let restarted_base = endpoint(&mut restarted);
    assert_eq!(bootstrap(&second, &restarted_base)["user"]["id"], 1);
    let survivors: Value = second
        .get(format!("{restarted_base}/api/users/"))
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(
        survivors
            .as_array()
            .unwrap()
            .iter()
            .map(|user| user["username"].as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>(),
        ["admin", "other-admin"].into_iter().collect()
    );
    restarted.stop();
}

#[test]
fn browser_reset_restart_csrf_rotation_and_logout_are_persistent() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = endpoint(&mut process);
    let browser = browser();
    let response = browser.get(format!("{base}/")).send().unwrap();
    assert_eq!(response.status(), 307);
    assert_eq!(response.headers()["location"], "/login?next=/");
    assert!(bootstrap(&browser, &base)["user"].is_null());
    assert_eq!(
        browser
            .post(format!("{base}/login"))
            .form(&[
                ("username", "admin"),
                ("password", "synthetic-bootstrap-password")
            ])
            .send()
            .unwrap()
            .status(),
        403
    );
    let login = sign_in(&browser, &base, "admin", "synthetic-bootstrap-password");
    assert_eq!(login.status(), 200);
    let cookies = login
        .headers()
        .get_all("set-cookie")
        .iter()
        .map(|v| v.to_str().unwrap())
        .collect::<Vec<_>>();
    let old_cookie = cookies
        .iter()
        .find(|c| c.starts_with("ruphish.sid="))
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    assert!(
        cookies
            .iter()
            .any(|c| c.contains("HttpOnly") && c.contains("SameSite=Strict"))
    );
    let initial = bootstrap(&browser, &base);
    assert_eq!(initial["user"]["password_change_required"], true);
    assert!(
        initial
            .to_string()
            .find("synthetic-bootstrap-token")
            .is_none()
    );
    assert_eq!(
        browser
            .get(format!("{base}/api/users/"))
            .bearer_auth(admin_key())
            .send()
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        browser
            .get(format!("{base}/settings"))
            .send()
            .unwrap()
            .headers()["location"],
        "/reset_password?next=%2Fsettings"
    );
    let token = initial["csrf_token"].as_str().unwrap();
    assert_eq!(
        browser
            .post(format!("{base}/reset_password"))
            .header("x-csrf-token", token)
            .header("origin", "https://untrusted.invalid")
            .form(&[
                ("password", "established-admin-password"),
                ("confirm_password", "established-admin-password")
            ])
            .send()
            .unwrap()
            .status(),
        403
    );
    for (candidate, confirmation) in [
        ("short", "short"),
        ("long-enough-password", "mismatch"),
        (
            "synthetic-bootstrap-password",
            "synthetic-bootstrap-password",
        ),
    ] {
        assert_eq!(
            post(
                &browser,
                &base,
                "/reset_password",
                &[("password", candidate), ("confirm_password", confirmation)]
            )
            .status(),
            400
        );
    }
    assert_eq!(
        post(
            &browser,
            &base,
            "/reset_password",
            &[
                ("password", "established-admin-password"),
                ("confirm_password", "established-admin-password")
            ]
        )
        .status(),
        200
    );
    let stale = client()
        .get(format!("{base}/auth/session"))
        .header("cookie", &old_cookie)
        .send()
        .unwrap()
        .json::<Value>()
        .unwrap();
    assert!(stale["user"].is_null());
    let rows = database(
        directory.path(),
        "SELECT hash,password_change_required,api_key FROM users",
    );
    assert!(rows[0].get::<String, _>("hash").starts_with("$argon2id$"));
    assert_eq!(rows[0].get::<i64, _>("password_change_required"), 0);
    assert_eq!(rows[0].get::<String, _>("api_key"), admin_key());
    process.stop();
    let mut restarted = Process::start(directory.path(), "admin");
    let base = endpoint(&mut restarted);
    assert_eq!(bootstrap(&browser, &base)["user"]["username"], "admin");
    assert_eq!(
        browser
            .get(format!("{base}/api/users/"))
            .bearer_auth("invalid")
            .send()
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        browser
            .post(format!("{base}/api/reset"))
            .send()
            .unwrap()
            .status(),
        403
    );
    let key = post(&browser, &base, "/api/reset", &[])
        .json::<Value>()
        .unwrap()["data"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(key.len(), 64);
    assert!(key.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(
        browser
            .get(format!("{base}/api/users/"))
            .bearer_auth(admin_key())
            .send()
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        browser
            .get(format!("{base}/api/users/"))
            .bearer_auth(&key)
            .send()
            .unwrap()
            .status(),
        200
    );
    assert_eq!(post(&browser, &base, "/logout", &[]).status(), 200);
    assert!(bootstrap(&browser, &base)["user"].is_null());
    assert_eq!(
        browser
            .get(format!("{base}/api/users/"))
            .send()
            .unwrap()
            .status(),
        401
    );
    restarted.stop();
    let logs = std::fs::read_to_string(directory.path().join("server.log")).unwrap();
    for secret in [
        admin_key(),
        key.as_str(),
        "synthetic-bootstrap-password",
        "established-admin-password",
        old_cookie.as_str(),
    ] {
        assert!(!logs.contains(secret));
    }
}

#[test]
fn accounts_permissions_locking_impersonation_and_atomic_last_admin() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = endpoint(&mut process);
    let admin = browser();
    setup(&admin, &base);
    let alice = create(&admin, &base, "alice", "user");
    let bob = create(&admin, &base, "bob", "user");
    let alice_id = alice["id"].as_i64().unwrap();
    let bob_id = bob["id"].as_i64().unwrap();
    let alice_key = alice["api_key"].as_str().unwrap();
    assert!(alice.get("hash").is_none());
    let alice_browser = browser();
    assert_eq!(
        sign_in(&alice_browser, &base, "alice", "another-user-password").status(),
        200
    );
    assert_eq!(
        alice_browser
            .get(format!("{base}/api/users/"))
            .bearer_auth(alice_key)
            .send()
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        alice_browser
            .get(format!("{base}/api/users/{bob_id}"))
            .bearer_auth(alice_key)
            .send()
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        alice_browser
            .get(format!("{base}/api/users/{alice_id}"))
            .bearer_auth(alice_key)
            .send()
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        alice_browser
            .put(format!("{base}/api/users/{alice_id}"))
            .bearer_auth(alice_key)
            .json(&json!({"username":"alice","role":"admin"}))
            .send()
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        post(&admin, &base, "/impersonate", &[("username", "alice")]).status(),
        200
    );
    let mode = bootstrap(&admin, &base);
    assert_eq!(mode["actor"], "admin");
    assert_eq!(mode["user"]["username"], "alice");
    assert_eq!(
        admin
            .get(format!("{base}/internal/preferences"))
            .send()
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        admin
            .put(format!("{base}/internal/preferences"))
            .header("x-csrf-token", mode["csrf_token"].as_str().unwrap())
            .header("origin", &base)
            .json(&json!({"language":"de"}))
            .send()
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        admin.get(format!("{base}/users")).send().unwrap().status(),
        403
    );
    assert_eq!(
        post(
            &admin,
            &base,
            "/internal/password",
            &[
                ("new_password", "unwanted-password"),
                ("confirm_new_password", "unwanted-password")
            ]
        )
        .status(),
        403
    );
    assert_eq!(post(&admin, &base, "/impersonate/exit", &[]).status(), 200);
    assert_eq!(bootstrap(&admin, &base)["user"]["username"], "admin");
    let response = admin
        .put(format!("{base}/api/users/{alice_id}"))
        .bearer_auth(admin_key())
        .json(&json!({"username":"alice","role":"user","account_locked":true}))
        .send()
        .unwrap();
    assert_eq!(response.status(), 200, "{}", response.text().unwrap());
    assert!(bootstrap(&alice_browser, &base)["user"].is_null());
    assert_eq!(
        sign_in(&alice_browser, &base, "alice", "another-user-password").status(),
        401
    );
    assert_eq!(
        admin
            .get(format!("{base}/api/users/{alice_id}"))
            .bearer_auth(alice_key)
            .send()
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        admin
            .delete(format!("{base}/api/users/1"))
            .bearer_auth(admin_key())
            .send()
            .unwrap()
            .status(),
        403
    );
    for edit in [
        json!({"username":"admin","role":"user"}),
        json!({"username":"admin","role":"admin","account_locked":true}),
    ] {
        assert_eq!(
            admin
                .put(format!("{base}/api/users/1"))
                .bearer_auth(admin_key())
                .json(&edit)
                .send()
                .unwrap()
                .status(),
            500
        );
        let unchanged = bootstrap(&admin, &base);
        assert_eq!(unchanged["user"]["username"], "admin");
        assert!(
            unchanged["user"]["permissions"]
                .as_array()
                .unwrap()
                .contains(&json!("modify_system"))
        );
    }
    let other = create(&admin, &base, "other-admin", "admin");
    let other_id = other["id"].as_i64().unwrap();
    let other_key = other["api_key"].as_str().unwrap().to_owned();
    let barrier = Arc::new(std::sync::Barrier::new(3));
    let mut workers = Vec::new();
    for (id, name, key) in [
        (1, "admin", admin_key().to_owned()),
        (other_id, "other-admin", other_key),
    ] {
        let base = base.clone();
        let barrier = barrier.clone();
        workers.push(std::thread::spawn(move || {
            let client = browser();
            barrier.wait();
            client
                .put(format!("{base}/api/users/{id}"))
                .bearer_auth(key)
                .json(&json!({"username":name,"role":"user"}))
                .send()
                .unwrap()
                .status()
                .as_u16()
        }));
    }
    barrier.wait();
    let mut statuses: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
    statuses.sort();
    assert_eq!(statuses, [200, 500]);
    assert_eq!(database(directory.path(), "SELECT count(*) AS n FROM users u JOIN roles r ON r.id=u.role_id WHERE r.slug='admin' AND u.account_locked=0")[0].get::<i64,_>("n"),1);
    process.stop();
}

#[test]
fn malformed_credentials_rate_limits_and_storage_failures_fail_closed() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = endpoint(&mut process);
    let browser = browser();
    setup(&browser, &base);
    assert_eq!(
        browser
            .post(format!("{base}/api/users/"))
            .bearer_auth(admin_key())
            .body("not-json")
            .send()
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        browser
            .post(format!("{base}/api/users/"))
            .bearer_auth(admin_key())
            .body("x".repeat(17 * 1024))
            .send()
            .unwrap()
            .status(),
        413
    );
    database(
        directory.path(),
        "CREATE TRIGGER reject_user_insert BEFORE INSERT ON users BEGIN SELECT RAISE(ABORT,'synthetic storage failure'); END;",
    );
    assert_eq!(
        browser
            .post(format!("{base}/api/users/"))
            .bearer_auth(admin_key())
            .json(
                &json!({"username":"must-not-exist","role":"user","password":"valid-long-password"})
            )
            .send()
            .unwrap()
            .status(),
        500
    );
    assert_eq!(
        database(
            directory.path(),
            "SELECT count(*) AS n FROM users WHERE username='must-not-exist'"
        )[0]
        .get::<i64, _>("n"),
        0
    );
    database(
        directory.path(),
        "DROP TRIGGER reject_user_insert; INSERT INTO users(username,hash,api_key,role_id,password_change_required,account_locked) VALUES('malformed-hash','not-a-hash','malformed-key',2,0,0);",
    );
    // Fast malformed-hash attempts exercise the admission limit without assuming
    // expensive real password hashes finish before the token bucket refills.
    let mut limited = false;
    for _ in 0..6 {
        let status = sign_in(&browser, &base, "malformed-hash", "invalid-password").status();
        if status == 429 {
            limited = true;
            break;
        }
        assert_eq!(status, 401);
    }
    assert!(
        limited,
        "A fresh burst cannot exceed five admitted login attempts"
    );
    let token = bootstrap(&browser, &base)["csrf_token"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        browser
            .post(format!("{base}/login"))
            .header("x-csrf-token", token)
            .header("x-forwarded-for", "198.51.100.99")
            .form(&[
                ("username", "admin"),
                ("password", "established-admin-password")
            ])
            .send()
            .unwrap()
            .status(),
        429
    );
    database(
        directory.path(),
        "CREATE TRIGGER reject_session_delete BEFORE DELETE ON ruphish_sessions BEGIN SELECT RAISE(ABORT,'synthetic storage failure'); END;",
    );
    assert_eq!(post(&browser, &base, "/logout", &[]).status(), 500);
    assert_eq!(bootstrap(&browser, &base)["user"]["username"], "admin");
    database(directory.path(), "DROP TRIGGER reject_session_delete;");
    assert_eq!(post(&browser, &base, "/logout", &[]).status(), 200);
    assert!(bootstrap(&browser, &base)["user"].is_null());
    process.stop();
}

#[test]
fn go_account_observations_match_except_approved_account_security_policies() {
    let directory = workspace();
    let source = fixture("authentication/legacy-accounts.sqlite3");
    let original = std::fs::read(&source).unwrap();
    std::fs::copy(&source, directory.path().join("installation.sqlite3")).unwrap();
    let mut process = Process::start(directory.path(), "admin");
    let base = endpoint(&mut process);
    let client = browser();
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/authentication/http.json")).unwrap();
    let mut user_key = String::new();
    for mut case in cases {
        fn absent_dates(value: &mut Value) {
            match value {
                Value::Array(rows) => rows.iter_mut().for_each(absent_dates),
                Value::Object(fields) => {
                    if fields.get("last_login") == Some(&json!("0001-01-01T00:00:00Z")) {
                        fields.insert("last_login".into(), Value::Null);
                    }
                    fields.values_mut().for_each(absent_dates);
                }
                _ => {}
            }
        }
        absent_dates(&mut case["json"]);
        let key = if case["key"] == "<created-user-key>" {
            &user_key
        } else {
            case["key"].as_str().unwrap()
        };
        let mut request = client
            .request(
                case["method"].as_str().unwrap().parse().unwrap(),
                format!("{base}{}", case["path"].as_str().unwrap()),
            )
            .header("content-type", "application/json")
            .body(case["body"].as_str().unwrap().to_owned());
        if !key.is_empty() {
            request = request.bearer_auth(key);
        }
        let response = request.send().unwrap();
        let status = response.status().as_u16();
        let mut value: Value = response.json().unwrap();
        if case["id"] == "create" {
            user_key = value["api_key"].as_str().unwrap().to_owned();
            assert_eq!(user_key.len(), 64);
        }
        normalize_key(&mut value);
        // Error codes are an additive contract extension. Assert them separately
        // while retaining every field from the independent Go observation.
        let expected_code = match case["id"].as_str().unwrap() {
            "missing-key" => Some("AUTH_REQUIRED"),
            "invalid-key" => Some("AUTH_API_KEY_INVALID"),
            "missing-user" => Some("ACCOUNT_NOT_FOUND"),
            "empty-name" => Some("USERNAME_REQUIRED"),
            "empty-role" => Some("ROLE_REQUIRED"),
            "duplicate-create" | "duplicate-update" => Some("USERNAME_TAKEN"),
            "unknown-role" => Some("ROLE_NOT_FOUND"),
            "forbidden-list" | "forbidden-get" | "forbidden-role" => Some("AUTH_FORBIDDEN"),
            "delete-last-admin" => Some("SELF_DELETION"),
            "self-update" => Some("AUTH_INVALID_CREDENTIALS"),
            _ => None,
        };
        if let Some(expected_code) = expected_code {
            let code = value.as_object_mut().unwrap().remove("code");
            assert_eq!(code, Some(json!(expected_code)), "{}", case["id"]);
        }
        let validation_field = match case["id"].as_str().unwrap() {
            "empty-name" | "duplicate-create" | "duplicate-update" => Some("username"),
            "empty-role" | "unknown-role" => Some("role"),
            _ => None,
        };
        if let Some(field) = validation_field {
            let report: ruphish_contracts::validation::ValidationReport = serde_json::from_value(
                value.as_object_mut().unwrap().remove("validation").unwrap(),
            )
            .unwrap();
            assert!(
                report.issues.iter().any(|issue| issue.field == field),
                "{}",
                case["id"]
            );
        }
        // DEC-043 rejects the credential-changing part of the original self-edit.
        // Keep the independent Go fixture, and separately retain harmless self-edits.
        if case["id"] == "self-update" {
            assert_eq!(case["status"], 200);
            assert_eq!(status, 401);
            assert_eq!(
                value,
                json!({"success":false,"message":"Invalid Username/Password","data":null})
            );
            let unchanged: Value = client
                .get(format!("{base}/api/users/2"))
                .bearer_auth(&user_key)
                .send()
                .unwrap()
                .json()
                .unwrap();
            assert_eq!(unchanged["username"], "alice");
            let mut edit: Value = serde_json::from_str(case["body"].as_str().unwrap()).unwrap();
            edit.as_object_mut().unwrap().remove("password");
            let renamed = client
                .put(format!("{base}/api/users/2"))
                .bearer_auth(&user_key)
                .json(&edit)
                .send()
                .unwrap();
            assert_eq!(renamed.status(), 200);
            let mut renamed: Value = renamed.json().unwrap();
            normalize_key(&mut renamed);
            assert_eq!(renamed, case["json"]);
            continue;
        }
        // Account policy rejects all self-deletion, including the last admin.
        // Preserve and verify the independent Go observation rather than rewriting it.
        if case["id"] == "delete-last-admin" {
            assert_eq!(case["status"], 500);
            assert_eq!(
                case["json"]["message"],
                "Cannot remove the only administrator"
            );
            assert_eq!(status, 403);
            assert_eq!(
                value,
                json!({"success":false,"message":"You cannot delete your own account","data":null})
            );
            continue;
        }
        assert_eq!(
            status,
            case["status"].as_u64().unwrap() as u16,
            "{}: {value}",
            case["id"]
        );
        assert_eq!(value, case["json"], "{}", case["id"]);
    }
    process.stop();
    assert_eq!(std::fs::read(source).unwrap(), original);
    assert_eq!(backups(directory.path()).len(), 1);
}
fn normalize_key(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if let Some(key) = map.get_mut("api_key")
                && key != "1111111111111111111111111111111111111111111111111111111111111111"
            {
                *key = json!("<generated-api-key>");
            }
            for child in map.values_mut() {
                normalize_key(child);
            }
        }
        Value::Array(values) => {
            for child in values {
                normalize_key(child);
            }
        }
        _ => {}
    }
}

#[test]
fn two_processes_share_revocation_and_mixed_hashes_survive_migration() {
    let directory = workspace();
    std::fs::copy(
        fixture("authentication/legacy-accounts.sqlite3"),
        directory.path().join("installation.sqlite3"),
    )
    .unwrap();
    let original_hash =
        database(directory.path(), "SELECT hash FROM users WHERE id=1")[0].get::<String, _>(0);
    let mut first = Process::start(directory.path(), "admin");
    let first_base = endpoint(&mut first);
    let mut second = Process::start(directory.path(), "admin");
    let second_base = endpoint(&mut second);
    let first_browser = browser();
    let other_browser = browser();
    assert_eq!(
        sign_in(
            &first_browser,
            &first_base,
            "admin",
            "FixtureOnlyPassword42!"
        )
        .status(),
        200
    );
    assert_eq!(
        sign_in(
            &other_browser,
            &second_base,
            "admin",
            "FixtureOnlyPassword42!"
        )
        .status(),
        200
    );
    assert_eq!(
        database(directory.path(), "SELECT hash FROM users WHERE id=1")[0].get::<String, _>(0),
        original_hash,
        "Login must not silently rewrite imported hashes"
    );
    assert_eq!(
        bootstrap(&first_browser, &second_base)["user"]["username"],
        "admin"
    );
    let candidate = format!("{}-tail-A", "é".repeat(40));
    assert_eq!(
        post(
            &first_browser,
            &first_base,
            "/internal/password",
            &[
                ("current_password", "FixtureOnlyPassword42!"),
                ("new_password", &candidate),
                ("confirm_new_password", &candidate)
            ]
        )
        .status(),
        200
    );
    assert!(bootstrap(&other_browser, &second_base)["user"].is_null());
    assert_eq!(
        bootstrap(&first_browser, &second_base)["user"]["username"],
        "admin"
    );
    assert_eq!(
        sign_in(
            &other_browser,
            &second_base,
            "admin",
            "FixtureOnlyPassword42!"
        )
        .status(),
        401
    );
    assert_eq!(
        sign_in(
            &other_browser,
            &second_base,
            "admin",
            &candidate.replace("tail-A", "tail-B")
        )
        .status(),
        401,
        "Argon2id must check bytes beyond the old bcrypt boundary"
    );
    assert_eq!(
        sign_in(&other_browser, &second_base, "admin", &candidate).status(),
        200
    );
    assert_eq!(
        post(&first_browser, &second_base, "/logout", &[]).status(),
        200
    );
    assert!(bootstrap(&first_browser, &first_base)["user"].is_null());
    assert_eq!(
        bootstrap(&other_browser, &first_base)["user"]["username"],
        "admin",
        "Logout revokes this session, not every independent login"
    );
    first.stop();
    second.stop();
    let changed =
        database(directory.path(), "SELECT hash FROM users WHERE id=1")[0].get::<String, _>(0);
    assert!(changed.starts_with("$argon2id$"));
    let restore = tempfile::tempdir().unwrap();
    std::fs::copy(
        &backups(directory.path())[0],
        restore.path().join("installation.sqlite3"),
    )
    .unwrap();
    assert_eq!(
        database(restore.path(), "SELECT hash FROM users WHERE id=1")[0].get::<String, _>(0),
        original_hash
    );
}

#[test]
fn read_only_permissions_redirects_validation_and_security_headers() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = endpoint(&mut process);
    let admin = browser();
    setup(&admin, &base);
    let user = create(&admin, &base, "reader", "user");
    let id = user["id"].as_i64().unwrap();
    let key = user["api_key"].as_str().unwrap();
    let reader = browser();
    assert_eq!(
        sign_in(&reader, &base, "reader", "another-user-password").status(),
        200
    );
    database(
        directory.path(),
        "INSERT INTO roles(id,slug,name,description) VALUES(99,'read-only','Read only','Synthetic view-only role'); INSERT INTO role_permissions(role_id,permission_id) SELECT 99,id FROM permissions WHERE slug='view_objects'; UPDATE users SET role_id=99 WHERE username='reader';",
    );
    let live = bootstrap(&reader, &base);
    assert_eq!(live["user"]["permissions"], json!(["view_objects"]));
    assert_eq!(
        reader
            .get(format!("{base}/api/users/{id}"))
            .bearer_auth(key)
            .send()
            .unwrap()
            .status(),
        200
    );
    for method in [reqwest::Method::PUT, reqwest::Method::DELETE] {
        assert_eq!(
            reader
                .request(method, format!("{base}/api/users/{id}"))
                .bearer_auth(key)
                .json(&json!({"username":"reader","role":"read-only"}))
                .send()
                .unwrap()
                .status(),
            403
        );
    }
    assert_eq!(
        reader
            .post(format!("{base}/api/reset"))
            .bearer_auth(key)
            .send()
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        post(&reader, &base, "/impersonate", &[("username", "admin")]).status(),
        403
    );
    assert_eq!(
        reader
            .get(format!("{base}/api/users/999"))
            .bearer_auth(admin_key())
            .send()
            .unwrap()
            .status(),
        404
    );
    let options = reader
        .request(reqwest::Method::OPTIONS, format!("{base}/api/users/"))
        .header("origin", "https://integration.example.test")
        .header("access-control-request-headers", "authorization")
        .send()
        .unwrap();
    assert_eq!(options.status(), 200);
    assert_eq!(
        options.headers()["access-control-allow-methods"],
        "POST, GET, OPTIONS, PUT, DELETE"
    );
    assert!(
        options
            .headers()
            .get("access-control-allow-credentials")
            .is_none()
    );
    let token = bootstrap(&admin, &base)["csrf_token"]
        .as_str()
        .unwrap()
        .to_owned();
    let redirect = admin
        .post(format!(
            "{base}/login?next=https://outside.invalid/settings?discard=1"
        ))
        .form(&[
            ("username", "admin"),
            ("password", "established-admin-password"),
            ("csrf_token", &token),
        ])
        .send()
        .unwrap();
    assert_eq!(redirect.status(), 302);
    assert_eq!(redirect.headers()["location"], "/settings");
    assert_eq!(redirect.headers()["x-frame-options"], "DENY");
    assert_eq!(
        redirect.headers()["content-security-policy"],
        "frame-ancestors 'none';"
    );
    assert_eq!(
        admin
            .get(format!("{base}/api/users/1?api_key=bad"))
            .bearer_auth(admin_key())
            .send()
            .unwrap()
            .status(),
        401
    );
    process.stop();
}

#[test]
fn verified_https_cookie_tampering_expiry_and_logout_replay() {
    let directory = workspace();
    let mut config = configuration();
    config["admin_server"]["use_tls"] = json!(true);
    write_config(directory.path(), &config);
    let mut process = Process::start(directory.path(), "admin");
    let base = format!("https://{}", process.ready()["admin"].as_str().unwrap());
    let cert =
        reqwest::Certificate::from_pem(&std::fs::read(directory.path().join("admin.crt")).unwrap())
            .unwrap();
    let browser = Client::builder()
        .no_proxy()
        .cookie_store(true)
        .add_root_certificate(cert.clone())
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap();
    let plain = Client::builder()
        .no_proxy()
        .add_root_certificate(cert)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap();
    let login = sign_in(&browser, &base, "admin", "synthetic-bootstrap-password");
    assert_eq!(login.status(), 200);
    let cookie = login
        .headers()
        .get_all("set-cookie")
        .iter()
        .map(|v| v.to_str().unwrap())
        .find(|v| v.starts_with("__Host-ruphish.sid="))
        .unwrap();
    assert!(
        cookie.contains("Secure")
            && cookie.contains("HttpOnly")
            && cookie.contains("SameSite=Strict")
            && cookie.contains("Path=/")
    );
    assert!(!cookie.contains("Domain="));
    let cookie = cookie.split(';').next().unwrap().to_owned();
    assert!(
        plain
            .get(format!("{base}/auth/session"))
            .header("cookie", format!("{cookie}x"))
            .send()
            .unwrap()
            .json::<Value>()
            .unwrap()["user"]
            .is_null()
    );
    // Forced setup does not trap the user in a session: explicit logout remains available.
    assert_eq!(post(&browser, &base, "/logout", &[]).status(), 200);
    assert!(
        plain
            .get(format!("{base}/auth/session"))
            .header("cookie", &cookie)
            .send()
            .unwrap()
            .json::<Value>()
            .unwrap()["user"]
            .is_null()
    );
    assert_eq!(
        sign_in(&browser, &base, "admin", "synthetic-bootstrap-password").status(),
        200
    );
    database(directory.path(), "UPDATE ruphish_sessions SET expires=0;");
    assert!(bootstrap(&browser, &base)["user"].is_null());
    process.stop();
}

#[test]
fn configured_origins_and_csrf_key_rotation_enforce_browser_boundaries() {
    let directory = workspace();
    let mut config = configuration();
    config["admin_server"]["trusted_origins"] = json!(["console.example.test:8080"]);
    config["admin_server"]["csrf_key"] = json!("synthetic-csrf-key-one");
    write_config(directory.path(), &config);
    let mut process = Process::start(directory.path(), "admin");
    let base = endpoint(&mut process);
    let browser = browser();
    setup(&browser, &base);
    let token = bootstrap(&browser, &base)["csrf_token"]
        .as_str()
        .unwrap()
        .to_owned();
    for (origin, expected) in [
        ("http://console.example.test:8080", 200),
        ("http://console.example.test:8080.attacker.invalid", 403),
        ("http://console.example.test:8081", 403),
        ("https://console.example.test:8080", 403),
        ("null", 403),
        ("http://console.example.test:8080/path", 403),
    ] {
        let response = browser
            .post(format!("{base}/api/reset"))
            .header("x-csrf-token", &token)
            .header("origin", origin)
            .send()
            .unwrap();
        assert_eq!(response.status().as_u16(), expected, "{origin}");
    }
    process.stop();
    config["admin_server"]["csrf_key"] = json!("synthetic-csrf-key-two");
    write_config(directory.path(), &config);
    let mut process = Process::start(directory.path(), "admin");
    let base = endpoint(&mut process);
    assert_eq!(
        browser
            .post(format!("{base}/api/reset"))
            .header("x-csrf-token", &token)
            .send()
            .unwrap()
            .status(),
        403
    );
    assert_eq!(bootstrap(&browser, &base)["user"]["username"], "admin");
    assert_eq!(post(&browser, &base, "/api/reset", &[]).status(), 200);
    process.stop();
}

#[test]
fn inherited_login_redirects_and_role_method_cases_use_real_operations() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = endpoint(&mut process);
    let admin = browser();
    setup(&admin, &base);
    let fresh = browser();
    assert_eq!(sign_in(&fresh, &base, "admin", "bogus").status(), 401);
    assert!(bootstrap(&fresh, &base)["user"].is_null());
    let token = bootstrap(&fresh, &base)["csrf_token"]
        .as_str()
        .unwrap()
        .to_owned();
    for (query, expected) in [("", "/"), ("?next=/campaigns", "/campaigns")] {
        let response = fresh
            .post(format!("{base}/login{query}"))
            .form(&[
                ("username", "admin"),
                ("password", "established-admin-password"),
                ("csrf_token", &token),
            ])
            .send()
            .unwrap();
        assert_eq!(response.status(), 302);
        assert_eq!(response.headers()["location"], expected);
    }
    assert_eq!(fresh.get(format!("{base}/")).send().unwrap().status(), 200);
    for role in ["admin", "user"] {
        let user = create(&admin, &base, &format!("methods-{role}"), role);
        let id = user["id"].as_i64().unwrap();
        let key = user["api_key"].as_str().unwrap();
        let url = format!("{base}/api/users/{id}");
        for method in [
            reqwest::Method::GET,
            reqwest::Method::HEAD,
            reqwest::Method::OPTIONS,
        ] {
            assert_eq!(
                admin
                    .request(method, &url)
                    .bearer_auth(key)
                    .send()
                    .unwrap()
                    .status(),
                200
            );
        }
        assert_eq!(
            admin
                .put(&url)
                .bearer_auth(key)
                .json(&json!({"username":format!("changed-{role}"),"role":role}))
                .send()
                .unwrap()
                .status(),
            200
        );
        let new_key = admin
            .post(format!("{base}/api/reset"))
            .bearer_auth(key)
            .send()
            .unwrap()
            .json::<Value>()
            .unwrap()["data"]
            .as_str()
            .unwrap()
            .to_owned();
        assert_eq!(
            admin
                .delete(&url)
                .bearer_auth(&new_key)
                .send()
                .unwrap()
                .status(),
            403
        );
        assert_eq!(
            admin
                .get(&url)
                .bearer_auth(&new_key)
                .send()
                .unwrap()
                .status(),
            200
        );
        assert_eq!(
            admin
                .delete(&url)
                .bearer_auth(admin_key())
                .send()
                .unwrap()
                .status(),
            200
        );
        assert_eq!(
            admin
                .get(&url)
                .bearer_auth(admin_key())
                .send()
                .unwrap()
                .status(),
            404
        );
    }
    process.stop();
}

#[test]
fn browser_origin_checks_support_http1_and_http2_over_verified_tls() {
    let directory = workspace();
    let mut config = configuration();
    config["admin_server"]["use_tls"] = json!(true);
    write_config(directory.path(), &config);
    let mut process = Process::start(directory.path(), "admin");
    let address: std::net::SocketAddr = process.ready()["admin"].as_str().unwrap().parse().unwrap();
    let base = format!("https://localhost:{}", address.port());
    let cert =
        reqwest::Certificate::from_pem(&std::fs::read(directory.path().join("admin.crt")).unwrap())
            .unwrap();
    for version in [reqwest::Version::HTTP_11, reqwest::Version::HTTP_2] {
        let builder = Client::builder()
            .no_proxy()
            .cookie_store(true)
            .resolve("localhost", address)
            .add_root_certificate(cert.clone())
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(20));
        let browser = if version == reqwest::Version::HTTP_11 {
            builder.http1_only()
        } else {
            builder.http2_prior_knowledge()
        }
        .build()
        .unwrap();
        let page = browser.get(format!("{base}/login")).send().unwrap();
        assert_eq!(page.version(), version);
        assert_eq!(page.status(), 200);
        let token = bootstrap(&browser, &base)["csrf_token"]
            .as_str()
            .unwrap()
            .to_owned();
        for origin in [
            "null".to_owned(),
            "https://attacker.invalid".to_owned(),
            format!("http://localhost:{}", address.port()),
            format!("https://localhost:{}", address.port().saturating_sub(1)),
        ] {
            let denied = browser
                .post(format!("{base}/login"))
                .header("origin", &origin)
                .header("sec-fetch-site", "same-origin")
                .header("x-csrf-token", &token)
                .header("accept", "application/json")
                .form(&[
                    ("username", "admin"),
                    ("password", "synthetic-bootstrap-password"),
                ])
                .send()
                .unwrap();
            assert_eq!(denied.version(), version);
            assert_eq!(denied.status(), 403, "{version:?}: {origin}");
            assert!(bootstrap(&browser, &base)["user"].is_null());
        }
        let password = if version == reqwest::Version::HTTP_11 {
            "synthetic-bootstrap-password"
        } else {
            "established-admin-password"
        };
        let login = sign_in(&browser, &base, "admin", password);
        assert_eq!(login.version(), version);
        assert_eq!(
            login.status(),
            200,
            "{version:?}: {}",
            login.text().unwrap()
        );
        if version == reqwest::Version::HTTP_11 {
            assert_eq!(
                post(
                    &browser,
                    &base,
                    "/reset_password",
                    &[
                        ("password", "established-admin-password"),
                        ("confirm_password", "established-admin-password")
                    ]
                )
                .status(),
                200
            );
        } else {
            database(
                directory.path(),
                "UPDATE users SET password_change_required=1 WHERE id=1;",
            );
            assert_eq!(
                post(
                    &browser,
                    &base,
                    "/reset_password",
                    &[
                        ("password", "http2-reset-password"),
                        ("confirm_password", "http2-reset-password")
                    ]
                )
                .status(),
                200
            );
        }
        // Referer fallback is used by requests that do not carry Origin.
        let token = bootstrap(&browser, &base)["csrf_token"]
            .as_str()
            .unwrap()
            .to_owned();
        let rotated = browser
            .post(format!("{base}/api/reset"))
            .header("referer", format!("{base}/settings"))
            .header("sec-fetch-site", "same-origin")
            .header("x-csrf-token", &token)
            .send()
            .unwrap();
        assert_eq!(rotated.version(), version);
        assert_eq!(rotated.status(), 200);
        assert_eq!(post(&browser, &base, "/logout", &[]).status(), 200);
        assert!(bootstrap(&browser, &base)["user"].is_null());
    }
    process.stop();
}

#[test]
fn browser_import_review_has_no_resource_writes_until_explicit_valid_create() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = endpoint(&mut process);
    let admin = browser();
    let session = setup(&admin, &base);
    let send = |path: &str, value: &Value| {
        admin
            .post(format!("{base}{path}"))
            .header("x-ruphish-client", "browser")
            .header("x-csrf-token", session["csrf_token"].as_str().unwrap())
            .header("origin", &base)
            .json(value)
            .send()
            .unwrap()
    };
    let existing = send(
        "/api/groups/",
        &json!({"name":"Existing","targets":[{"email":"alice@example.test","first_name":"Original"}]}),
    );
    assert_eq!(existing.status(), 201);
    let existing: Value = existing.json().unwrap();
    let groups = || {
        admin
            .get(format!("{base}/api/groups/"))
            .header("x-ruphish-client", "browser")
            .send()
            .unwrap()
            .json::<Value>()
            .unwrap()
    };
    let before = groups();
    let csv = json!({"content":"Email,First Name,Last Name,Position\nalice@example.test,Imported,Example,Analyst\nnot-an-email,Invalid,Row,Rejected\n"});
    let reviewed = send("/api/import/group/review", &csv);
    assert_eq!(reviewed.status(), 200);
    let report: Value = reviewed.json().unwrap();
    assert_eq!(report["targets"].as_array().unwrap().len(), 1);
    assert_eq!(report["errors"].as_array().unwrap().len(), 1);
    assert_eq!(report["targets"][0]["first_name"], "Imported");
    assert_eq!(
        groups(),
        before,
        "review must not save or alter an existing recipient"
    );
    let invalid = send(
        "/api/groups/",
        &json!({"name":"","targets":report["targets"]}),
    );
    assert_eq!(invalid.status(), 400);
    let error: Value = invalid.json().unwrap();
    assert_eq!(error["success"], false);
    assert!(error["message"].as_str().is_some_and(|s| !s.is_empty()));
    assert_eq!(groups(), before, "failed Create must not partially save");
    let created = send(
        "/api/groups/",
        &json!({"name":"Reviewed import","targets":report["targets"]}),
    );
    assert_eq!(created.status(), 201);
    let created: Value = created.json().unwrap();
    assert_ne!(created["id"], existing["id"]);
    let all = groups();
    assert_eq!(all.as_array().unwrap().len(), 2);
    assert_eq!(
        all.as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == existing["id"])
            .unwrap(),
        &existing
    );
    assert_eq!(
        send("/api/import/group/review", &json!({"content":42})).status(),
        400
    );
    let denied = admin
        .post(format!("{base}/api/import/group/review"))
        .header("x-ruphish-client", "browser")
        .header("x-csrf-token", "invalid")
        .header("origin", &base)
        .json(&csv)
        .send()
        .unwrap();
    assert_eq!(denied.status(), 403);
    assert_eq!(groups(), all);
    process.stop();
}

#[test]
fn account_display_preferences_survive_restart_and_browser_changes_without_session_revocation() {
    let directory = workspace();
    let mut process = Process::start(directory.path(), "admin");
    let base = endpoint(&mut process);
    let client = browser();
    let session = setup(&client, &base);
    let defaults =
        json!({"language":null,"format_locale":null,"time_zone":null,"show_utc_offset":true});
    assert_eq!(session["preferences"], defaults);
    let chosen = json!({"language":"de","format_locale":"zh-CN","time_zone":"Europe/Berlin","show_utc_offset":false});
    let send = |body: &Value, token: &str| {
        client
            .put(format!("{base}/internal/preferences"))
            .header("origin", &base)
            .header("x-ruphish-client", "browser")
            .header("x-csrf-token", token)
            .json(body)
            .send()
            .unwrap()
    };
    let token = session["csrf_token"].as_str().unwrap();
    assert_eq!(send(&chosen, "invalid").status(), 403);
    assert_eq!(bootstrap(&client, &base)["preferences"], defaults);
    let response = send(&chosen, token);
    assert_eq!(response.status(), 200, "{}", response.text().unwrap());
    assert_eq!(bootstrap(&client, &base)["preferences"], chosen);
    for invalid in [
        json!({"show_utc_offset":"false"}),
        json!({"show_utc_offset":null}),
        json!({"language":"zh","format_locale":null,"time_zone":null}),
        json!({"language":"de","format_locale":"bad_locale","time_zone":null}),
        json!({"language":"de","format_locale":"zh-CN","time_zone":"bad-zone"}),
        json!({"language":"en","format_locale":null,"time_zone":null,"user_id":2}),
    ] {
        assert_eq!(send(&invalid, token).status(), 400);
        assert_eq!(bootstrap(&client, &base)["preferences"], chosen);
    }
    assert_eq!(
        browser()
            .get(format!("{base}/internal/preferences"))
            .header("x-ruphish-client", "browser")
            .send()
            .unwrap()
            .status(),
        401
    );
    let second = browser();
    assert_eq!(
        sign_in(&second, &base, "admin", "established-admin-password").status(),
        200
    );
    assert_eq!(bootstrap(&second, &base)["preferences"], chosen);
    process.stop();
    let mut restarted = Process::start(directory.path(), "admin");
    let base = endpoint(&mut restarted);
    assert_eq!(
        bootstrap(&client, &base)["preferences"],
        chosen,
        "existing session survives preference updates and restart"
    );
    let session = bootstrap(&second, &base);
    let response = second
        .put(format!("{base}/internal/preferences"))
        .header("origin", &base)
        .header("x-ruphish-client", "browser")
        .header("x-csrf-token", session["csrf_token"].as_str().unwrap())
        .json(&defaults)
        .send()
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        bootstrap(&client, &base)["preferences"],
        defaults,
        "clearing stores null, not a snapshot of browser defaults"
    );
    let row = &database(
        directory.path(),
        "SELECT language,format_locale,time_zone FROM ruphish_user_preferences WHERE user_id=1",
    )[0];
    for field in ["language", "format_locale", "time_zone"] {
        assert!(row.get::<Option<String>, _>(field).is_none());
    }
    restarted.stop();
}
