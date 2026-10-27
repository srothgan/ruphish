//! Real loopback HTTP tests of transport contracts; application routes remain separate work.

use axum::{
    Router,
    http::{Method, StatusCode, Uri},
    response::Response,
    routing::{any, get, post},
};
use ruphish_contracts::legacy::{Envelope, Group};
use ruphish_http::response;
use serde_json::{Value, json};
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
async fn typed_api_values_preserve_explicit_absence_collections_integer_and_time_precision() {
    let fixture = [
        (
            "default-group",
            json!({"id":0,"name":"","modified_date":null,"targets":[]}),
        ),
        (
            "empty-targets",
            json!({"id":9007199254740993_i64,"name":"Ada & <Team>","modified_date":"2020-01-02T03:04:05.1234Z","targets":[]}),
        ),
        (
            "escaped-envelope",
            json!({"success":false,"message":"<tag> & \u{2028}\u{2029}","data":null}),
        ),
    ];
    let router = Router::new()
        .route(
            "/default-group",
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
                        modified_date: Some(time_value()),
                        targets: vec![],
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
    for (id, expected) in fixture {
        let result = client.get(format!("{base}/{id}")).send().await.unwrap();
        assert_eq!(result.status(), StatusCode::OK);
        assert_eq!(result.headers()["content-type"], "application/json");
        assert_eq!(result.headers()["access-control-allow-origin"], "*");
        let body = result.text().await.unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&body).unwrap(),
            expected,
            "{id}"
        );
        assert!(!body.contains("user_id"));
        assert!(!body.contains("SMTPId"));
        if id == "escaped-envelope" {
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
