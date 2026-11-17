//! Exercise the real public router in memory. Doubles record dispatch, not persistence.

use axum::{
    body::{Body, to_bytes},
    extract::ConnectInfo,
    http::{HeaderValue, Method, Request, StatusCode, header},
};
use ruphish_application::{
    attribution::Saved,
    auth::Identity,
    delivery::{MessageInput, Previews},
    domains::{Check, Domain, Edit, Observation, Repository},
    interactions::{Action, Output, Public},
    resources::Error,
};
use serde_json::{Value, json};
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex, OnceLock},
};
use tower::ServiceExt;

#[derive(Debug, PartialEq, Eq)]
enum Dispatch {
    View,
    Head,
    Open,
    Report,
    Inspect,
    Submit { id: String, present: bool },
}

#[derive(Debug, PartialEq, Eq)]
struct Call {
    origin: String,
    rid: String,
    action: Dispatch,
}

#[derive(Default)]
struct Recording(Mutex<Vec<Call>>);

#[async_trait::async_trait]
impl Public for Recording {
    async fn learning_page(&self, _: &str, _: &str) -> Result<Output, Error> {
        panic!("submission harness must not dispatch to learning pages");
    }

    async fn interact(
        &self,
        origin: &str,
        rid: &str,
        action: Action,
        _: i64,
    ) -> Result<Output, Error> {
        let (action, output) = match action {
            Action::View { .. } => (
                Dispatch::View,
                Output::Html {
                    html: "<p>Synthetic page</p>".into(),
                    csp: "default-src 'none'".into(),
                },
            ),
            Action::Head => (Dispatch::Head, Output::Empty),
            Action::Open => (Dispatch::Open, Output::Pixel),
            Action::Report => (Dispatch::Report, Output::Empty),
            Action::Inspect => (
                Dispatch::Inspect,
                Output::Transparency {
                    send_date: String::new(),
                },
            ),
            Action::Submit {
                event_id,
                password_present,
                ..
            } => (
                Dispatch::Submit {
                    id: event_id,
                    present: password_present,
                },
                Output::Submitted { redirect: None },
            ),
        };
        self.0.lock().unwrap().push(Call {
            origin: origin.into(),
            rid: rid.into(),
            action,
        });
        Ok(output)
    }
}

// Every unrelated service call fails the harness rather than silently succeeding.
struct Unused;

#[async_trait::async_trait]
impl Previews for Unused {
    fn id(&self) -> Result<String, Error> {
        panic!("unexpected preview ID allocation")
    }
    async fn contains(&self, _: &str) -> Result<bool, Error> {
        panic!("unexpected preview lookup")
    }
    async fn store(&self, _: Identity, _: &MessageInput) -> Result<(), Error> {
        panic!("unexpected preview write")
    }
    async fn render(&self, _: &str) -> Result<String, Error> {
        panic!("unexpected preview render")
    }
}

#[async_trait::async_trait]
impl Repository for Unused {
    async fn list(&self, _: Identity) -> Result<Vec<Domain>, Error> {
        panic!("unexpected domain list")
    }
    async fn get(&self, _: Identity, _: i64) -> Result<Domain, Error> {
        panic!("unexpected domain read")
    }
    async fn save(
        &self,
        _: Identity,
        _: Option<i64>,
        _: Edit,
        _: i64,
    ) -> Result<Saved<Domain>, Error> {
        panic!("unexpected domain write")
    }
    async fn delete(&self, _: Identity, _: i64, _: i64) -> Result<(), Error> {
        panic!("unexpected domain delete")
    }
    async fn request_check(&self, _: Identity, _: i64, _: i64) -> Result<Saved<Domain>, Error> {
        panic!("unexpected verification request")
    }
    async fn claim(&self, _: i64) -> Result<Option<Check>, Error> {
        panic!("unexpected verification claim")
    }
    async fn finish(&self, _: &Check, _: Observation, _: i64) -> Result<(), Error> {
        panic!("unexpected verification completion")
    }
    async fn serves(&self, _: &str) -> Result<bool, Error> {
        panic!("unexpected static asset routing")
    }
    async fn routing_proof(&self, _: &str, _: &str, _: i64) -> Result<Option<String>, Error> {
        panic!("unexpected routing proof")
    }
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
    })
}

/// First byte selects a transport contract; remaining bytes mutate its values/body.
pub fn check(data: &[u8]) {
    if data.len() > 8192 {
        return;
    }
    let mode = data.first().copied().unwrap_or(0) % 26;
    let payload = data.get(1..).unwrap_or_default();
    let id: String = (0..16)
        .map(|index| format!("{:02x}", payload.get(index).copied().unwrap_or(0)))
        .collect();
    let rid = format!("f{id}");
    let host = format!("h{}.example.test", &id[..8]);
    let mut origin = format!("http://{host}");
    let mut request = Request::builder()
        .uri(format!("/welcome?rid={rid}"))
        .header(header::HOST, &host)
        .body(Body::empty())
        .unwrap();
    let peer: SocketAddr = "192.0.2.10:1234".parse().unwrap();
    request.extensions_mut().insert(ConnectInfo(peer));
    let mut trusted = false;
    let mut expected = Some(Dispatch::View);
    let mut status = Some(StatusCode::OK);
    let json_body = serde_json::to_vec(&json!({"event_id": id, "password_present": true})).unwrap();

    match mode {
        0 => {}
        1 => {
            *request.method_mut() = Method::HEAD;
            expected = Some(Dispatch::Head);
        }
        2 => {
            *request.uri_mut() = format!("/track?rid={rid}").parse().unwrap();
            expected = Some(Dispatch::Open);
        }
        3 => {
            *request.uri_mut() = format!("/report?rid={rid}").parse().unwrap();
            expected = Some(Dispatch::Report);
            status = Some(StatusCode::NO_CONTENT);
        }
        4 => {
            *request.uri_mut() = format!("/welcome?rid={rid}%2b").parse().unwrap();
            expected = Some(Dispatch::Inspect);
        }
        5 => {
            post(
                &mut request,
                json_body.clone(),
                "application/json; charset=utf-8",
            );
            expected = Some(Dispatch::Submit {
                id: id.clone(),
                present: true,
            });
        }
        6 => {
            *request.uri_mut() = format!("/welcome?rid={rid}&%72id={rid}").parse().unwrap();
            expected = None;
            status = Some(StatusCode::BAD_REQUEST);
        }
        7 => {
            let body = serde_json::to_vec(&json!({"event_id": id, "password_present": false, "password": String::from_utf8_lossy(payload)})).unwrap();
            let oversized = body.len() > 512;
            post(&mut request, body, "application/json");
            expected = None;
            status = Some(if oversized {
                StatusCode::PAYLOAD_TOO_LARGE
            } else {
                StatusCode::BAD_REQUEST
            });
        }
        8 => {
            post(
                &mut request,
                format!(
                    "{{\"event_id\":\"{id}\",\"password_present\":true,\"password_present\":false}}"
                )
                .into_bytes(),
                "application/json",
            );
            expected = None;
            status = Some(StatusCode::BAD_REQUEST);
        }
        9 => {
            post(
                &mut request,
                serde_json::to_vec(&json!([id, true])).unwrap(),
                "application/json",
            );
            expected = None;
            status = Some(StatusCode::BAD_REQUEST);
        }
        10 => {
            post(
                &mut request,
                json_body.clone(),
                "application/x-www-form-urlencoded",
            );
            expected = None;
            status = Some(StatusCode::UNSUPPORTED_MEDIA_TYPE);
        }
        11 => {
            let mut oversized = json_body.clone();
            oversized.resize(513, b' ');
            post(&mut request, oversized, "application/json");
            expected = None;
            status = Some(StatusCode::PAYLOAD_TOO_LARGE);
        }
        12 => {
            *request.uri_mut() = format!("/welcome?rid={rid}&padding={}", "x".repeat(4096))
                .parse()
                .unwrap();
            expected = None;
            status = Some(StatusCode::BAD_REQUEST);
        }
        13 => {
            request.headers_mut().append(
                header::HOST,
                HeaderValue::from_static("different.example.test"),
            );
            expected = None;
            status = Some(StatusCode::NOT_FOUND);
        }
        14 => {
            *request.uri_mut() = format!("http://different.example.test/welcome?rid={rid}")
                .parse()
                .unwrap();
            expected = None;
            status = Some(StatusCode::NOT_FOUND);
        }
        15 => {
            trusted = true;
            request
                .headers_mut()
                .insert("x-forwarded-proto", HeaderValue::from_static("https"));
            request.headers_mut().insert(
                "x-forwarded-host",
                HeaderValue::from_str(&host.to_uppercase()).unwrap(),
            );
            request.headers_mut().insert(
                header::HOST,
                HeaderValue::from_static("internal.example.test"),
            );
            origin = format!("https://{host}");
        }
        16 => {
            // Untrusted peers cannot replace the transport origin, regardless of header value.
            let value = HeaderValue::from_bytes(payload)
                .unwrap_or_else(|_| HeaderValue::from_static("https, http"));
            request
                .headers_mut()
                .insert("x-forwarded-proto", value.clone());
            request.headers_mut().insert("x-forwarded-host", value);
        }
        17 => {
            trusted = true;
            request
                .headers_mut()
                .insert("x-forwarded-host", HeaderValue::from_str(&host).unwrap());
            expected = None;
            status = Some(StatusCode::NOT_FOUND);
        }
        18 | 19 => {
            trusted = true;
            request
                .headers_mut()
                .insert("x-forwarded-proto", HeaderValue::from_static("https"));
            request
                .headers_mut()
                .insert("x-forwarded-host", HeaderValue::from_str(&host).unwrap());
            let name = if mode == 18 {
                "x-forwarded-proto"
            } else {
                "x-forwarded-host"
            };
            let value = request.headers()[name].clone();
            request.headers_mut().append(name, value);
            expected = None;
            status = Some(StatusCode::NOT_FOUND);
        }
        20 => {
            post(&mut request, json_body.clone(), "application/json");
            *request.uri_mut() = format!("/track?rid={rid}").parse().unwrap();
            expected = None;
            status = Some(StatusCode::METHOD_NOT_ALLOWED);
        }
        21 => {
            *request.method_mut() = [Method::PUT, Method::DELETE, Method::PATCH]
                [payload.first().copied().unwrap_or(0) as usize % 3]
                .clone();
            expected = None;
            status = Some(StatusCode::METHOD_NOT_ALLOWED);
        }
        22 => {
            *request.method_mut() = Method::OPTIONS;
            expected = None;
            status = Some(StatusCode::NO_CONTENT);
        }
        23 => {
            post(&mut request, payload.to_vec(), "application/json");
            expected = None;
            status = None;
        }
        24 => {
            let Ok(query) = std::str::from_utf8(payload) else {
                return;
            };
            let Ok(uri) = format!("/welcome?rid={rid}&{query}").parse() else {
                return;
            };
            *request.uri_mut() = uri;
            status = None;
        }
        25 => {
            let Ok(authority) = HeaderValue::from_bytes(payload) else {
                return;
            };
            trusted = true;
            request
                .headers_mut()
                .insert("x-forwarded-proto", HeaderValue::from_static("https"));
            request.headers_mut().insert("x-forwarded-host", authority);
            status = None;
        }
        _ => unreachable!(),
    }
    let recording = Arc::new(Recording::default());
    let router = ruphish_http::interactions::router(ruphish_http::interactions::Services {
        assets: Default::default(),
        domains: Arc::new(Unused),
        interactions: recording.clone(),
        previews: Arc::new(Unused),
        tls: false,
        trusted_proxies: if trusted {
            vec!["192.0.2.0/24".parse().unwrap()]
        } else {
            vec![]
        },
        contact: "synthetic@example.test".into(),
    });
    runtime().block_on(async {
        let response = router.oneshot(request).await.unwrap();
        assert_eq!(response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
        assert!(!response.headers().contains_key(header::SET_COOKIE));
        assert!(
            !response
                .headers()
                .contains_key(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
        );
        if let Some(status) = status {
            assert_eq!(response.status(), status, "case {mode}");
        }
        let response_status = response.status();
        let body = to_bytes(response.into_body(), 8192).await.unwrap();
        let calls = recording.0.lock().unwrap();
        if mode == 24 || mode == 25 {
            if response_status == StatusCode::OK {
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].rid, rid);
                assert_eq!(calls[0].action, Dispatch::View);
                if mode == 24 {
                    assert_eq!(calls[0].origin, origin);
                } else {
                    let accepted = url::Url::parse(&calls[0].origin).unwrap();
                    assert_eq!(accepted.scheme(), "https");
                    assert!(accepted.host_str().is_some());
                    assert!(accepted.username().is_empty() && accepted.password().is_none());
                    assert!(accepted.query().is_none() && accepted.fragment().is_none());
                    assert_eq!(accepted.path(), "/");
                }
            } else {
                assert_eq!(
                    response_status,
                    if mode == 24 {
                        StatusCode::BAD_REQUEST
                    } else {
                        StatusCode::NOT_FOUND
                    }
                );
                assert!(calls.is_empty());
            }
        } else if mode == 23 {
            if response_status == StatusCode::OK {
                assert!(payload.len() <= 512);
                let value: Value = serde_json::from_slice(payload).unwrap();
                let object = value.as_object().unwrap();
                assert_eq!(object.len(), 2);
                assert_eq!(
                    calls.as_slice(),
                    &[Call {
                        origin,
                        rid,
                        action: Dispatch::Submit {
                            id: object["event_id"].as_str().unwrap().into(),
                            present: object["password_present"].as_bool().unwrap(),
                        },
                    }]
                );
            } else {
                assert!(matches!(
                    response_status,
                    StatusCode::BAD_REQUEST | StatusCode::PAYLOAD_TOO_LARGE
                ));
                assert!(calls.is_empty());
            }
        } else if let Some(action) = expected {
            assert_eq!(
                calls.as_slice(),
                &[Call {
                    origin,
                    rid,
                    action
                }]
            );
        } else {
            assert!(
                calls.is_empty(),
                "rejected request dispatched to the application"
            );
        }
        if mode == 1 {
            assert!(body.is_empty());
        }
    });
}

fn post(request: &mut Request<Body>, bytes: Vec<u8>, content_type: &'static str) {
    *request.method_mut() = Method::POST;
    request
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    *request.body_mut() = Body::from(bytes);
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_dispatch_and_rejection_scenario_runs() {
        for mode in 0..26 {
            let mut data = vec![mode];
            data.extend_from_slice(
                br#"{"event_id":"0123456789abcdef0123456789abcdef","password_present":true}"#,
            );
            super::check(&data);
            data[1..].fill(0xff);
            super::check(&data);
        }
        for (mode, payload) in [
            (24, "x=%ff%2b"),
            (24, "rid=duplicate"),
            (25, "[2001:db8::1]:8443"),
            (25, "user@example.test"),
            (25, "example.test,other.test"),
        ] {
            let mut data = vec![mode];
            data.extend_from_slice(payload.as_bytes());
            super::check(&data);
        }
    }
}
