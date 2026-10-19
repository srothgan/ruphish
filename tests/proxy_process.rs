//! Raw HTTP preserves duplicate headers at the actual executable boundary.
use super::*;

fn request(address: &str, path: &str, headers: &str, body: Option<&str>) -> u16 {
    let mut stream = std::net::TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let method = if body.is_some() { "POST" } else { "GET" };
    let body = body.unwrap_or("");
    write!(stream, "{method} {path} HTTP/1.1\r\nHost: internal.invalid\r\nConnection: close\r\n{headers}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
    let mut response = String::new();
    BufReader::new(stream).read_line(&mut response).unwrap();
    response.split_whitespace().nth(1).unwrap().parse().unwrap()
}

#[test]
fn proxy_authority_rejects_ambiguous_or_untrusted_headers_without_tracking() {
    let directory = workspace();
    let (mut process, base) = start(directory.path(), &["--disable-mailer"]);
    let public = process.ready()["phish"].as_str().unwrap().to_owned();
    let campaign = post(&base, "/api/campaigns/", &resources(&base, "127.0.0.1:1"));
    let path = format!("/api/campaigns/{}", campaign["id"]);
    let rid = campaign["results"][0]["id"].as_str().unwrap();
    let valid = "X-Forwarded-Host: training.example.test\r\nX-Forwarded-Proto: https\r\n";
    let cases = [
        ("missing both", ""),
        ("missing host", "X-Forwarded-Proto: https\r\n"),
        (
            "missing protocol",
            "X-Forwarded-Host: training.example.test\r\n",
        ),
        (
            "http",
            "X-Forwarded-Host: training.example.test\r\nX-Forwarded-Proto: http\r\n",
        ),
        (
            "duplicate host",
            "X-Forwarded-Host: training.example.test\r\nX-Forwarded-Host: training.example.test\r\nX-Forwarded-Proto: https\r\n",
        ),
        (
            "conflicting hosts",
            "X-Forwarded-Host: training.example.test\r\nX-Forwarded-Host: example.test\r\nX-Forwarded-Proto: https\r\n",
        ),
        (
            "duplicate protocol",
            "X-Forwarded-Host: training.example.test\r\nX-Forwarded-Proto: https\r\nX-Forwarded-Proto: https\r\n",
        ),
        (
            "host list",
            "X-Forwarded-Host: training.example.test, example.test\r\nX-Forwarded-Proto: https\r\n",
        ),
        (
            "conflicting protocols",
            "X-Forwarded-Host: training.example.test\r\nX-Forwarded-Proto: https\r\nX-Forwarded-Proto: http\r\n",
        ),
        (
            "protocol list",
            "X-Forwarded-Host: training.example.test\r\nX-Forwarded-Proto: https, http\r\n",
        ),
        (
            "unknown domain",
            "X-Forwarded-Host: unknown.example.test\r\nX-Forwarded-Proto: https\r\n",
        ),
        (
            "different verified domain",
            "X-Forwarded-Host: example.test\r\nX-Forwarded-Proto: https\r\n",
        ),
        (
            "wrong port",
            "X-Forwarded-Host: training.example.test:8443\r\nX-Forwarded-Proto: https\r\n",
        ),
    ];
    let submission = r#"{"event_id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","password_present":true}"#;
    let before = read(&base, &path);
    for (name, headers) in cases {
        for (endpoint, body) in [("/track", None), ("/", None), ("/", Some(submission))] {
            assert_eq!(
                request(&public, &format!("{endpoint}?rid={rid}"), headers, body),
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
        request(&public, &format!("/track?rid={rid}"), valid, None),
        200
    );
    assert_ne!(read(&base, &path)["timeline"], before["timeline"]);
    assert_eq!(request(&public, &format!("/?rid={rid}"), valid, None), 200);
    assert_eq!(
        request(&public, &format!("/?rid={rid}"), valid, Some(submission)),
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
            request(public, &format!("{endpoint}?rid={rid}"), valid, body),
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
