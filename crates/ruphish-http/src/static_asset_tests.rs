//! Protocol expectations independent of negotiation implementation.
#![allow(clippy::unwrap_used, reason = "Test request and response setup")]
use super::*;

fn fixture() -> Asset {
    Asset {
        identity: Representation {
            bytes: Bytes::from_static(b"identity body"),
            etag: "\"identity\"".into(),
        },
        gzip: Some(Representation {
            bytes: Bytes::from_static(b"gzip bytes"),
            etag: "\"gzip\"".into(),
        }),
        brotli: Some(Representation {
            bytes: Bytes::from_static(b"brotli bytes"),
            etag: "\"brotli\"".into(),
        }),
    }
}
fn headers(values: &[(&str, &str)]) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for (key, value) in values {
        headers.append(
            header::HeaderName::from_bytes(key.as_bytes()).unwrap(),
            value.parse().unwrap(),
        );
    }
    headers
}
async fn body(response: Response) -> Bytes {
    axum::body::to_bytes(response.into_body(), 1024)
        .await
        .unwrap()
}

#[tokio::test]
async fn encoding_preferences_refusals_wildcards_and_repeated_headers() {
    for (accept, encoding, expected) in [
        ("", None, "identity body"),
        ("identity", None, "identity body"),
        ("br, gzip", Some("br"), "brotli bytes"),
        ("gzip;q=1, br;q=0.5", Some("gzip"), "gzip bytes"),
        ("gzip;q=0, br;q=0", None, "identity body"),
        ("*", Some("br"), "brotli bytes"),
        ("br;q=0, *;q=0.5", Some("gzip"), "gzip bytes"),
        ("br;q=0.5, identity;q=1", None, "identity body"),
        ("GZIP;Q=0.123", Some("gzip"), "gzip bytes"),
        ("br;q=NaN, gzip;q=1.001", None, "identity body"),
        ("br;q=0;q=1", None, "identity body"),
        ("br;q=0, br;q=1", None, "identity body"),
    ] {
        let response = fixture().response(
            "application/wasm",
            "no-cache",
            &Method::GET,
            &headers(&[("accept-encoding", accept)]),
        );
        assert_eq!(response.status(), 200, "{accept}");
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_ENCODING)
                .map(|v| v.to_str().unwrap()),
            encoding,
            "{accept}"
        );
        assert_eq!(response.headers()[header::VARY], "Accept-Encoding");
        assert_eq!(
            body(response).await.as_ref(),
            expected.as_bytes(),
            "{accept}"
        );
    }
    for accept in ["identity;q=0", "*;q=0", "br;q=0, gzip;q=0, identity;q=0"] {
        assert_eq!(
            fixture()
                .response(
                    "text/plain",
                    "no-cache",
                    &Method::GET,
                    &headers(&[("accept-encoding", accept)])
                )
                .status(),
            406
        );
    }
    let response = fixture().response(
        "text/plain",
        "no-cache",
        &Method::GET,
        &headers(&[("accept-encoding", "gzip;q=0.5"), ("accept-encoding", "br")]),
    );
    assert_eq!(body(response).await.as_ref(), b"brotli bytes");
    assert_eq!(
        body(fixture().response("text/plain", "no-cache", &Method::GET, &HeaderMap::new()))
            .await
            .as_ref(),
        b"identity body"
    );
}

#[tokio::test]
async fn validators_and_ranges_describe_the_selected_representation() {
    let response = fixture().response(
        "text/plain",
        "public, max-age=31536000, immutable",
        &Method::GET,
        &headers(&[("accept-encoding", "gzip"), ("if-none-match", "W/\"gzip\"")]),
    );
    assert_eq!(response.status(), 304);
    assert_eq!(response.headers()[header::ETAG], "\"gzip\"");
    assert_eq!(
        response.headers()[header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
    assert!(!response.headers().contains_key(header::CONTENT_LENGTH));
    assert!(body(response).await.is_empty());
    let response = fixture().response(
        "text/plain",
        "no-cache",
        &Method::GET,
        &headers(&[
            ("accept-encoding", "br"),
            ("if-none-match", "\"gzip\""),
            ("range", "bytes=0-2"),
        ]),
    );
    assert_eq!(response.status(), 206);
    assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes 0-2/12");
    assert_eq!(response.headers()[header::CONTENT_ENCODING], "br");
    assert_eq!(body(response).await.as_ref(), b"bro");
    let response = fixture().response(
        "text/plain",
        "no-cache",
        &Method::GET,
        &headers(&[("range", "bytes=0-2"), ("if-range", "\"old\"")]),
    );
    assert_eq!(response.status(), 200);
    assert_eq!(body(response).await.as_ref(), b"identity body");
    let response = fixture().response(
        "text/plain",
        "no-cache",
        &Method::GET,
        &headers(&[("range", "bytes=-4")]),
    );
    assert_eq!(response.status(), 206);
    assert_eq!(body(response).await.as_ref(), b"body");
    let response = fixture().response(
        "text/plain",
        "no-cache",
        &Method::GET,
        &headers(&[("range", "bytes=999-")]),
    );
    assert_eq!(response.status(), 416);
    assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes */13");
    assert!(body(response).await.is_empty());
    let response = fixture().response(
        "text/plain",
        "no-cache",
        &Method::GET,
        &headers(&[("accept-encoding", "br"), ("range", "bytes=999-")]),
    );
    assert_eq!(response.status(), 416);
    assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes */12");
    assert!(!response.headers().contains_key(header::CONTENT_ENCODING));
}

#[tokio::test]
async fn head_and_session_documents_keep_their_distinct_contracts() {
    let response = fixture().response(
        "text/plain",
        "no-cache",
        &Method::HEAD,
        &headers(&[("accept-encoding", "gzip"), ("range", "bytes=0-2")]),
    );
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()[header::CONTENT_LENGTH], "10");
    assert!(body(response).await.is_empty());
    let response = fixture().document(
        &Method::GET,
        &headers(&[
            ("accept-encoding", "br"),
            ("if-none-match", "*"),
            ("range", "bytes=0-2"),
        ]),
    );
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(body(response).await.as_ref(), b"brotli bytes");
}
