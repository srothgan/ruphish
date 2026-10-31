//! Raw HTTP preserves duplicate headers at the actual executable boundary.
use super::*;

#[test]
fn fixed_http_and_ip_origins_respect_verification_and_transport_across_restart() {
    let directory = workspace();
    let (mut process, base) = start(&directory, &["--disable-mailer"]);
    let public = process.ready()["phish"].as_str().unwrap().to_owned();
    let mut input = resources(&base, "127.0.0.1:1");
    let change_policy = |base: &str, enabled: bool| {
        let settings = read(base, "/api/settings");
        let mut policy = settings["policy"].clone();
        policy["verification_enabled"] = json!(enabled);
        client()
            .put(format!("{base}/api/settings"))
            .bearer_auth("synthetic-bootstrap-token")
            .header("if-match", settings["revision"].to_string())
            .json(&json!({"policy":policy}))
            .send()
            .unwrap()
            .error_for_status()
            .unwrap();
    };
    let mut campaigns = Vec::new();
    for origin in [
        "http://training.example.test:8080",
        "http://192.0.2.20:8080",
        "https://192.0.2.20:8443",
        "http://[2001:db8::20]:8080",
        "https://[2001:db8::20]:8443",
    ] {
        let domain = post(
            &base,
            "/api/domains/",
            &json!({"name":origin,"origin":origin,"enabled":true,"revision":null}),
        );
        assert_eq!(domain["verification"]["status"], "unsupported");
        assert_eq!(domain["eligible"], false);
        assert!(domain["verification"]["next_check_at"].is_null());
        assert!(domain["active_check"].is_null());
        if !origin.contains("example.test") {
            assert_eq!(domain["txt_name"], "");
            assert_eq!(domain["txt_value"], "");
        }
        let check = client()
            .post(format!("{base}/api/domains/{}/verify", domain["id"]))
            .bearer_auth("synthetic-bootstrap-token")
            .send()
            .unwrap();
        assert_eq!(check.status(), 400);
        input["url"] = json!(format!("{origin}/training/{{{{.FirstName}}}}?source=email"));
        assert_eq!(
            client()
                .post(format!("{base}/api/campaigns/"))
                .bearer_auth("synthetic-bootstrap-token")
                .json(&input)
                .send()
                .unwrap()
                .status(),
            400
        );
        change_policy(&base, false);
        let campaign = post(&base, "/api/campaigns/", &input);
        let rid = campaign["results"][0]["id"].as_str().unwrap().to_owned();
        let path = format!("/api/campaigns/{}", campaign["id"]);
        let (scheme, host) = origin.split_once("://").unwrap();
        let headers = format!("X-Forwarded-Host: {host}\r\nX-Forwarded-Proto: {scheme}\r\n");
        let before = read(&base, &path);
        let opposite = if scheme == "https" { "http" } else { "https" };
        let wrong = format!("X-Forwarded-Host: {host}\r\nX-Forwarded-Proto: {opposite}\r\n");
        assert_eq!(
            request(&public, &format!("/track?rid={rid}"), &wrong, None),
            404
        );
        assert_eq!(read(&base, &path), before);
        assert_eq!(
            request(&public, &format!("/track?rid={rid}"), &headers, None),
            200
        );
        assert_eq!(
            request(
                &public,
                &format!("/training/Alice?rid={rid}"),
                &headers,
                None
            ),
            200
        );
        assert_eq!(
            request(
                &public,
                &format!("/?rid={rid}"),
                &headers,
                Some(r#"{"event_id":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","password_present":false}"#)
            ),
            200
        );
        change_policy(&base, true);
        let recorded = read(&base, &path);
        assert_eq!(
            request(&public, &format!("/track?rid={rid}"), &headers, None),
            404
        );
        assert_eq!(read(&base, &path), recorded);
        campaigns.push((origin.to_owned(), path, rid));
    }
    change_policy(&base, false);
    process.stop();
    let mut config: Value =
        serde_json::from_slice(&std::fs::read(directory.join("config.json")).unwrap()).unwrap();
    config["domain_verification"]["trusted_proxies"] = json!([]);
    write_config(&directory, &config);
    let mut process = Process::with_options(&directory, "all", None, None, &["--disable-mailer"]);
    let ready = process.ready();
    let base = format!("http://{}", ready["admin"].as_str().unwrap());
    let public = ready["phish"].as_str().unwrap();
    for (origin, path, rid) in campaigns {
        let (scheme, host) = origin.split_once("://").unwrap();
        let before = read(&base, &path);
        let response = client()
            .get(format!("http://{public}/track?rid={rid}"))
            .header("host", host)
            .header("x-forwarded-proto", "https")
            .send()
            .unwrap();
        assert_eq!(response.status(), if scheme == "http" { 200 } else { 404 });
        let after = read(&base, &path);
        if scheme == "https" {
            assert_eq!(before, after);
        } else {
            assert_ne!(before["timeline"], after["timeline"]);
        }
    }
    process.stop();
}

fn request(address: &str, path: &str, headers: &str, body: Option<&str>) -> u16 {
    let mut stream = std::net::TcpStream::connect(address).unwrap();
    let method = if body.is_some() { "POST" } else { "GET" };
    let body = body.unwrap_or("");
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: internal.invalid\r\nConnection: close\r\n{headers}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    BufReader::new(stream)
        .read_line(&mut response)
        .map_err(|error| format!("Read response to {method} {path}, headers {headers:?}: {error}"))
        .unwrap();
    response.split_whitespace().nth(1).unwrap().parse().unwrap()
}

#[test]
fn proxy_authority_rejects_ambiguous_or_untrusted_headers_without_tracking() {
    let directory = workspace();
    let (mut process, base) = start(&directory, &["--disable-mailer"]);
    let public = process.ready()["phish"].as_str().unwrap().to_owned();
    let campaign = post(&base, "/api/campaigns/", &resources(&base, "127.0.0.1:1"));
    let path = format!("/api/campaigns/{}", campaign["id"]);
    let rid = campaign["results"][0]["id"].as_str().unwrap();
    let origin = origin(&base, "training.example.test");
    let authority = origin.trim_start_matches("https://");
    let other = super::origin(&base, "example.test");
    let other = other.trim_start_matches("https://");
    let valid = format!("X-Forwarded-Host: {authority}\r\nX-Forwarded-Proto: https\r\n");
    let cases = [
        ("missing both", ""),
        ("missing host", "X-Forwarded-Proto: https\r\n"),
        ("missing protocol", "X-Forwarded-Host: {training}\r\n"),
        (
            "http",
            "X-Forwarded-Host: {training}\r\nX-Forwarded-Proto: http\r\n",
        ),
        (
            "duplicate host",
            "X-Forwarded-Host: {training}\r\nX-Forwarded-Host: {training}\r\nX-Forwarded-Proto: https\r\n",
        ),
        (
            "conflicting hosts",
            "X-Forwarded-Host: {training}\r\nX-Forwarded-Host: {other}\r\nX-Forwarded-Proto: https\r\n",
        ),
        (
            "duplicate protocol",
            "X-Forwarded-Host: {training}\r\nX-Forwarded-Proto: https\r\nX-Forwarded-Proto: https\r\n",
        ),
        (
            "host list",
            "X-Forwarded-Host: {training}, {other}\r\nX-Forwarded-Proto: https\r\n",
        ),
        (
            "conflicting protocols",
            "X-Forwarded-Host: {training}\r\nX-Forwarded-Proto: https\r\nX-Forwarded-Proto: http\r\n",
        ),
        (
            "protocol list",
            "X-Forwarded-Host: {training}\r\nX-Forwarded-Proto: https, http\r\n",
        ),
        (
            "unknown domain",
            "X-Forwarded-Host: unknown.{other}\r\nX-Forwarded-Proto: https\r\n",
        ),
        (
            "different verified domain",
            "X-Forwarded-Host: {other}\r\nX-Forwarded-Proto: https\r\n",
        ),
        (
            "wrong port",
            "X-Forwarded-Host: training.example.test:443\r\nX-Forwarded-Proto: https\r\n",
        ),
    ];
    let submission = r#"{"event_id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","password_present":true}"#;
    let before = read(&base, &path);
    for (name, headers) in cases {
        let headers = headers
            .replace("{training}", authority)
            .replace("{other}", other);
        for (endpoint, body) in [("/track", None), ("/", None), ("/", Some(submission))] {
            assert_eq!(
                request(&public, &format!("{endpoint}?rid={rid}"), &headers, body),
                404,
                "{name} {endpoint}"
            );
            assert_eq!(
                read(&base, &path),
                before,
                "Rejected {name} changed persisted campaign state"
            );
        }
    }
    assert_eq!(
        request(&public, &format!("/track?rid={rid}"), &valid, None),
        200
    );
    assert_ne!(read(&base, &path)["timeline"], before["timeline"]);
    assert_eq!(request(&public, &format!("/?rid={rid}"), &valid, None), 200);
    assert_eq!(
        request(&public, &format!("/?rid={rid}"), &valid, Some(submission)),
        200
    );
    let accepted = read(&base, &path);
    assert_eq!(accepted["results"][0]["status"], "Submitted Data");
    for name in ["Email Opened", "Clicked Link", "Submitted Data"] {
        assert_eq!(
            accepted["timeline"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|event| event["message"] == name)
                .count(),
            1,
            "{name}"
        );
    }
    process.stop();

    let mut config: Value =
        serde_json::from_slice(&std::fs::read(directory.path().join("config.json")).unwrap())
            .unwrap();
    config["domain_verification"]["trusted_proxies"] = json!([]);
    write_config(directory.path(), &config);
    let mut untrusted =
        Process::with_options(directory.path(), "all", None, None, &["--disable-mailer"]);
    let ready = untrusted.ready();
    let base = format!("http://{}", ready["admin"].as_str().unwrap());
    let public = ready["phish"].as_str().unwrap();
    for (endpoint, body) in [("/track", None), ("/", None), ("/", Some(submission))] {
        assert_eq!(
            request(public, &format!("{endpoint}?rid={rid}"), &valid, body),
            404
        );
        assert_eq!(
            read(&base, &path),
            accepted,
            "Untrusted forwarded headers changed persisted state"
        );
    }
    untrusted.stop();
}
