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

#[test]
fn client_categories_are_coarse_per_event_and_never_capture_raw_headers() {
    let directory = workspace();
    let (mut process, base) = start(&directory, &["--disable-mailer"]);
    let public = process.ready()["phish"].as_str().unwrap().to_owned();
    let input = resources(&base, "127.0.0.1:1");
    let campaign = post(&base, "/api/campaigns/", &input);
    let rid = campaign["results"][0]["id"].as_str().unwrap();
    let path = format!("/api/campaigns/{}", campaign["id"]);
    let origin = origin(&base, "training.example.test");
    let authority = origin.trim_start_matches("https://");
    let transport_headers =
        format!("X-Forwarded-Host: {authority}\r\nX-Forwarded-Proto: https\r\n");
    let base_headers = transport_headers.as_str();
    let cases = [
        (
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/140.0.0.0 Safari/537.36",
            "chrome",
            "windows",
            "desktop",
        ),
        (
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/140.0.0.0 Safari/537.36 Edg/140.0.0.0",
            "edge",
            "windows",
            "desktop",
        ),
        (
            "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0",
            "firefox",
            "linux",
            "desktop",
        ),
        (
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 Version/18.0 Mobile/15E148 Safari/604.1",
            "safari",
            "ios",
            "mobile",
        ),
        (
            "Mozilla/5.0 (iPad; CPU OS 18_0 like Mac OS X) Version/18.0 Mobile/15E148 Safari/604.1",
            "safari",
            "ios",
            "tablet",
        ),
        (
            "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 Chrome/140.0.0.0 Mobile Safari/537.36",
            "chrome",
            "android",
            "mobile",
        ),
        (
            "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 Chrome/140.0.0.0 Safari/537.36",
            "chrome",
            "android",
            "tablet",
        ),
        ("curl/8.0", "unknown", "unknown", "unknown"),
        (
            "CANARY-PRIVATE-CLIENT-UNIQUE-739",
            "unknown",
            "unknown",
            "unknown",
        ),
        ("", "unknown", "unknown", "unknown"),
    ];
    for (ua, browser, os, device) in cases {
        let headers = if ua.is_empty() {
            base_headers.to_owned()
        } else {
            format!("{base_headers}User-Agent: {ua} CANARY-PRIVATE-CLIENT-UNIQUE-739\r\n")
        };
        assert_eq!(
            request(&public, &format!("/?rid={rid}"), &headers, None),
            200
        );
        let saved = read(&base, &path);
        let event = saved["timeline"]
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .find(|e| e["message"] == "Clicked Link")
            .unwrap();
        assert_eq!(
            event["client"],
            json!({"browser":browser,"os":os,"device":device})
        );
        assert!(!saved.to_string().contains("CANARY-PRIVATE"));
    }
    for extra in [
        "User-Agent: Chrome/140.0\r\nUser-Agent: Firefox/140.0\r\n".to_owned(),
        format!("User-Agent: {}\r\n", "x".repeat(2049)),
        "User-Agent: fakeChrome/140.0\r\n".into(),
    ] {
        assert_eq!(
            request(
                &public,
                &format!("/?rid={rid}"),
                &format!("{base_headers}{extra}"),
                None
            ),
            200
        );
        let saved = read(&base, &path);
        let event = saved["timeline"]
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .find(|e| e["message"] == "Clicked Link")
            .unwrap();
        assert_eq!(
            event["client"],
            json!({"browser":"unknown","os":"unknown","device":"unknown"})
        );
    }
    let headers = format!(
        "{base_headers}User-Agent: Mozilla/5.0 (Windows NT 10.0) Chrome/140.0.0.0 CANARY-PRIVATE-CLIENT-UNIQUE-739\r\n"
    );
    for endpoint in ["/track", "/report"] {
        assert!(matches!(
            request(&public, &format!("{endpoint}?rid={rid}"), &headers, None),
            200 | 204
        ));
    }
    let body = r#"{"event_id":"cccccccccccccccccccccccccccccccc","password_present":true}"#;
    assert_eq!(
        request(&public, &format!("/?rid={rid}"), &headers, Some(body)),
        200
    );
    assert_eq!(
        request(&public, &format!("/?rid={rid}"), base_headers, Some(body)),
        200
    );
    let injected = r#"{"event_id":"dddddddddddddddddddddddddddddddd","password_present":true,"client":{"browser":"firefox","os":"linux","device":"desktop"}}"#;
    assert_eq!(
        request(&public, &format!("/?rid={rid}"), &headers, Some(injected)),
        400
    );
    let saved = read(&base, &path);
    let events = saved["timeline"].as_array().unwrap();
    assert!(
        events
            .iter()
            .filter(|e| e["message"] == "Email Opened" || e["message"] == "Email Reported")
            .all(|e| e["client"].is_null())
    );
    let submissions: Vec<_> = events
        .iter()
        .filter(|e| e["message"] == "Submitted Data")
        .collect();
    assert_eq!(submissions.len(), 1);
    assert_eq!(
        submissions[0]["client"],
        json!({"browser":"chrome","os":"windows","device":"desktop"})
    );
    let results = read(&base, &format!("{path}/results"));
    let observed = results["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["kind"]["value"] == "form_submitted")
        .unwrap();
    let recipient = &results["recipients"][0]["id"];
    let change = json!({"request_id":"client-observation-injection", "event_id":observed["id"], "revision":0, "action":"edit", "value":{"time":observed["time"], "kind":"form_submitted", "presence":{"username_present":null,"password_present":true}, "client":{"browser":"firefox","os":"linux","device":"desktop"}}});
    let rejection = client()
        .post(format!("{base}{path}/recipients/{recipient}/events"))
        .bearer_auth("synthetic-bootstrap-token")
        .json(&change)
        .send()
        .unwrap();
    assert_eq!(
        rejection.status(),
        400,
        "client observations cannot be edited"
    );
    assert_eq!(read(&base, &path)["timeline"], saved["timeline"]);
    process.stop();
    let logs = std::fs::read_to_string(directory.join("server.log")).unwrap();
    assert!(!logs.contains("CANARY-PRIVATE-CLIENT-UNIQUE-739"));
    let disk = std::fs::read(directory.join("installation.sqlite3")).unwrap();
    assert!(
        !disk
            .windows(b"CANARY-PRIVATE-CLIENT-UNIQUE-739".len())
            .any(|w| w == b"CANARY-PRIVATE-CLIENT-UNIQUE-739")
    );
    let mut process = Process::with_options(&directory, "all", None, None, &["--disable-mailer"]);
    let ready = process.ready();
    let base = format!("http://{}", ready["admin"].as_str().unwrap());
    assert_eq!(read(&base, &path)["timeline"], saved["timeline"]);
    process.stop();
}
