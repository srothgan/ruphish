//! Real loopback HTTP tests of transport contracts; application routes remain separate work.

use axum::{
    Router,
    body::Bytes,
    http::{Method, StatusCode, Uri},
    response::Response,
    routing::{any, get, post},
};
use ruphish_contracts::legacy::{
    self, Envelope, Group, Header, Imap, QuotedNumber, SendingProfile,
};
use ruphish_http::response;
use serde_json::Value;
use std::time::Duration;

struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn serve(router: Router) -> Result<(String, Server), Box<dyn std::error::Error>> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let task = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    Ok((format!("http://{address}"), Server(task)))
}

#[tokio::test]
async fn typed_api_values_survive_real_http_with_go_null_omission_integer_and_time_semantics() {
    let fixture: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/compatibility/wire.json"
    ))
    .unwrap();
    let router = Router::new()
        .route(
            "/nil-targets",
            get(|| async { response::json(StatusCode::OK, &Group::default()) }),
        )
        .route(
            "/empty-targets",
            get(|| async {
                response::json(
                    StatusCode::OK,
                    &Group {
                        id: 9007199254740993,
                        name: "Ada & <Team>".into(),
                        modified_date: time_value(),
                        targets: Some(vec![]),
                    },
                )
            }),
        )
        .route(
            "/smtp-omitted-credentials",
            get(|| async { response::json(StatusCode::OK, &SendingProfile::default()) }),
        )
        .route(
            "/smtp-headers-and-credentials",
            get(|| async {
                response::json(
                    StatusCode::OK,
                    &SendingProfile {
                        id: 7,
                        username: "synthetic-user".into(),
                        password: "synthetic-password".into(),
                        headers: Some(vec![Header {
                            key: "X-Test".into(),
                            value: "{{.FirstName}}".into(),
                        }]),
                        ..SendingProfile::default()
                    },
                )
            }),
        )
        .route(
            "/imap-zero",
            get(|| async { response::json(StatusCode::OK, &Imap::default()) }),
        )
        .route(
            "/imap-quoted-numbers",
            get(|| async {
                response::json(
                    StatusCode::OK,
                    &Imap {
                        port: QuotedNumber(993),
                        imap_freq: QuotedNumber(60),
                        tls: true,
                        folder: "INBOX".into(),
                        ..Imap::default()
                    },
                )
            }),
        )
        .route(
            "/escaped-envelope",
            get(|| async {
                response::json(
                    StatusCode::OK,
                    &Envelope::<()> {
                        success: false,
                        message: "<tag> & \u{2028}\u{2029}".into(),
                        data: None,
                    },
                )
            }),
        );
    let (base, _server) = serve(router).await.unwrap();
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    for case in fixture {
        let result = client
            .get(format!("{base}/{}", case["id"].as_str().unwrap()))
            .send()
            .await
            .unwrap();
        assert_eq!(result.status(), StatusCode::OK);
        assert_eq!(result.headers()["content-type"], "application/json");
        assert_eq!(result.headers()["access-control-allow-origin"], "*");
        let body = result.text().await.unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&body).unwrap(),
            case["expected_json"],
            "{}",
            case["id"]
        );
        assert!(!body.contains("user_id"));
        assert!(!body.contains("SMTPId"));
        if case["id"] == "escaped-envelope" {
            assert!(
                body.contains("\\u003c")
                    && body.contains("\\u0026")
                    && body.contains("\\u2028")
                    && body.contains("\\u2029")
            );
        }
    }
}

#[allow(clippy::expect_used)]
fn time_value() -> ruphish_contracts::legacy::Timestamp {
    serde_json::from_str("\"2020-01-02T03:04:05.1234Z\"")
        .expect("fixed fixture timestamp must parse")
}

async fn decode_imap(bytes: Bytes) -> Response {
    match legacy::decode_object::<Imap>(&bytes) {
        Ok(value) => response::json(StatusCode::OK, &value),
        Err(_) => response::error(StatusCode::BAD_REQUEST, "Invalid JSON structure"),
    }
}

#[tokio::test]
async fn happy_and_adversarial_imap_requests_preserve_types_and_return_legacy_error_envelopes() {
    let (base, _server) = serve(Router::new().route("/imap", post(decode_imap)))
        .await
        .unwrap();
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let mut captured: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/compatibility/requests.json"
    ))
    .unwrap();
    captured.extend(
        serde_json::from_str::<Vec<Value>>(include_str!(
            "../../../tests/fixtures/compatibility/requests-edge.json"
        ))
        .unwrap(),
    );
    for case in captured {
        let result = client
            .post(format!("{base}/imap"))
            .body(case["body"].as_str().unwrap().to_owned())
            .send()
            .await
            .unwrap();
        if case["accepted"].as_bool().unwrap() {
            assert_eq!(result.status(), StatusCode::OK, "{}", case["body"]);
            assert_eq!(
                result.json::<Value>().await.unwrap(),
                case["expected_json"],
                "{}",
                case["body"]
            );
        } else {
            assert_eq!(result.status(), StatusCode::BAD_REQUEST, "{}", case["body"]);
            assert_eq!(
                result.json::<Value>().await.unwrap(),
                serde_json::json!({"success":false,"message":"Invalid JSON structure","data":null})
            );
        }
    }
    for body in [
        r#"{"port":"993","imap_freq":"60","username":"unicode-é","tls":true}"#,
        r#"{"port":"65535","imap_freq":"4294967295"}"#,
    ] {
        let result = client
            .post(format!("{base}/imap"))
            .body(body)
            .send()
            .await
            .unwrap();
        assert_eq!(result.status(), StatusCode::OK);
        let returned: Value = result.json().await.unwrap();
        let input: Value = serde_json::from_str(body).unwrap();
        assert_eq!(returned["port"], input["port"]);
        assert_eq!(returned["imap_freq"], input["imap_freq"]);
    }
    for body in [
        r#"{"port":993}"#,
        r#"{"port":"65536"}"#,
        r#"{"port":"-1"}"#,
        r#"{"imap_freq":"4294967296"}"#,
        r#"{"tls":"true"}"#,
        r#"{"port":"993""#,
        "[]",
    ] {
        let result = client
            .post(format!("{base}/imap"))
            .body(body)
            .send()
            .await
            .unwrap();
        assert_eq!(result.status(), StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(
            result.json::<Value>().await.unwrap(),
            serde_json::json!({"success":false,"message":"Invalid JSON structure","data":null})
        );
    }
}

async fn redirect(method: Method, uri: Uri) -> Response {
    let destination = match uri.query() {
        Some(query) => format!("/api/groups/?{query}"),
        None => "/api/groups/".into(),
    };
    match response::slash_redirect(&method, &destination) {
        Ok(value) => value,
        Err(_) => response::error(StatusCode::BAD_REQUEST, "Invalid redirect destination"),
    }
}

#[tokio::test]
async fn redirects_and_errors_match_go_http_fixtures_for_reads_writes_queries_and_unknown_routes() {
    let router = Router::new()
        .route("/api/groups", any(redirect))
        .route(
            "/bad-json",
            post(|| async { response::error(StatusCode::BAD_REQUEST, "Invalid JSON structure") }),
        )
        .fallback(|| async { response::not_found() });
    let (base, _server) = serve(router).await.unwrap();
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let fixtures: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/gophish-v0.12.1/http.json"
    ))
    .unwrap();
    for id in ["slash-redirect", "write-slash-redirect", "unknown-route"] {
        let case = fixtures.iter().find(|case| case["id"] == id).unwrap();
        let result = client
            .request(
                case["method"].as_str().unwrap().parse().unwrap(),
                format!("{base}{}", case["path"].as_str().unwrap()),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(
            result.status().as_u16(),
            case["expected_status"].as_u64().unwrap() as u16
        );
        for (key, value) in case["expected_headers"].as_object().unwrap() {
            assert_eq!(result.headers()[key], value.as_str().unwrap());
        }
        assert_eq!(
            result.text().await.unwrap(),
            case["expected_text"].as_str().unwrap()
        );
    }
    for method in [
        reqwest::Method::POST,
        reqwest::Method::PUT,
        reqwest::Method::DELETE,
        reqwest::Method::HEAD,
    ] {
        let result = client
            .request(method, format!("{base}/api/groups?a=1&b=%3Ctag%3E"))
            .body("payload")
            .send()
            .await
            .unwrap();
        assert_eq!(result.status(), StatusCode::MOVED_PERMANENTLY);
        assert_eq!(result.headers()["location"], "/api/groups/?a=1&b=%3Ctag%3E");
        assert!(result.text().await.unwrap().is_empty());
    }
    assert!(response::slash_redirect(&Method::GET, "/safe\r\nX-Injected: true").is_err());
}
