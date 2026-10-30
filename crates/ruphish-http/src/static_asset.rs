//! Immutable representations shared by disk snapshots and standalone executables.
use axum::{
    body::Bytes,
    http::{HeaderMap, Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use sha2::{Digest, Sha256};

#[derive(Clone)]
pub(crate) struct Representation {
    pub(crate) bytes: Bytes,
    pub(crate) etag: String,
}

#[derive(Clone)]
pub(crate) struct Asset {
    pub(crate) identity: Representation,
    pub(crate) gzip: Option<Representation>,
    pub(crate) brotli: Option<Representation>,
}

impl Asset {
    pub(crate) fn identity(bytes: Bytes) -> Self {
        Self {
            identity: Representation {
                etag: format!("\"{}\"", hex::encode(Sha256::digest(&bytes))),
                bytes,
            },
            gzip: None,
            brotli: None,
        }
    }

    pub(crate) fn document(&self, method: &Method, headers: &HeaderMap) -> Response {
        self.respond(
            "text/html; charset=utf-8",
            "no-store",
            method,
            headers,
            false,
        )
    }

    pub(crate) fn response(
        &self,
        mime: &'static str,
        cache: &'static str,
        method: &Method,
        headers: &HeaderMap,
    ) -> Response {
        self.respond(mime, cache, method, headers, true)
    }

    fn respond(
        &self,
        mime: &'static str,
        cache: &'static str,
        method: &Method,
        headers: &HeaderMap,
        conditional: bool,
    ) -> Response {
        let Some((representation, encoding)) = self.negotiate(headers) else {
            return (
                StatusCode::NOT_ACCEPTABLE,
                [
                    (header::VARY, "Accept-Encoding"),
                    (header::CACHE_CONTROL, "no-store"),
                ],
            )
                .into_response();
        };
        let etag = &representation.etag;
        let size = representation.bytes.len();
        let mut status = StatusCode::OK;
        let mut body = representation.bytes.clone();
        let mut range = None;
        if conditional
            && headers
                .get_all(header::IF_NONE_MATCH)
                .iter()
                .filter_map(|v| v.to_str().ok())
                .any(|v| {
                    v.split(',').any(|tag| {
                        tag.trim() == "*"
                            || tag.trim().strip_prefix("W/").unwrap_or(tag.trim()) == etag
                    })
                })
        {
            status = StatusCode::NOT_MODIFIED;
            body = Bytes::new();
        } else if conditional
            && *method == Method::GET
            && headers
                .get(header::IF_RANGE)
                .is_none_or(|value| value == etag.as_str())
            && let Some(value) = headers
                .get(header::RANGE)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("bytes="))
            && !value.contains(',')
        {
            let bounds = value.split_once('-').and_then(|(first, last)| {
                if first.is_empty() {
                    let suffix = last.parse::<usize>().ok()?;
                    (suffix > 0 && size > 0)
                        .then_some((size.saturating_sub(suffix), size.saturating_sub(1)))
                } else {
                    let first = first.parse::<usize>().ok()?;
                    let last = if last.is_empty() {
                        size.saturating_sub(1)
                    } else {
                        last.parse::<usize>().ok()?.min(size.saturating_sub(1))
                    };
                    (first <= last && first < size).then_some((first, last))
                }
            });
            if let Some((first, last)) = bounds {
                status = StatusCode::PARTIAL_CONTENT;
                body = body.slice(first..=last);
                range = Some(format!("bytes {first}-{last}/{size}"));
            } else {
                status = StatusCode::RANGE_NOT_SATISFIABLE;
                body = Bytes::new();
                range = Some(format!("bytes */{size}"));
            }
        }
        let length = body.len();
        let mut response = (
            status,
            [
                (header::CONTENT_TYPE, mime),
                (header::ETAG, etag.as_str()),
                (header::VARY, "Accept-Encoding"),
                (header::CACHE_CONTROL, cache),
                (header::ACCEPT_RANGES, "bytes"),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            ],
            if *method == Method::HEAD {
                Bytes::new()
            } else {
                body
            },
        )
            .into_response();
        if status == StatusCode::NOT_MODIFIED {
            response.headers_mut().remove(header::CONTENT_LENGTH);
        } else if let Ok(length) = length.to_string().parse() {
            response
                .headers_mut()
                .insert(header::CONTENT_LENGTH, length);
        }
        if status != StatusCode::RANGE_NOT_SATISFIABLE
            && let Some(encoding) = encoding
        {
            response.headers_mut().insert(
                header::CONTENT_ENCODING,
                header::HeaderValue::from_static(encoding),
            );
        }
        if let Some(range) = range.and_then(|s| s.parse().ok()) {
            response.headers_mut().insert(header::CONTENT_RANGE, range);
        }
        response
    }

    fn negotiate(&self, headers: &HeaderMap) -> Option<(&Representation, Option<&'static str>)> {
        if !headers.contains_key(header::ACCEPT_ENCODING) {
            return Some((&self.identity, None));
        }
        let mut gzip = None;
        let mut br = None;
        let mut identity = None;
        let mut wildcard = None;
        for item in headers
            .get_all(header::ACCEPT_ENCODING)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|v| v.split(','))
        {
            let mut parts = item.trim().split(';');
            let coding = parts.next().unwrap_or("").trim();
            let quality = parts.next().map_or(1000, |parameter| {
                parameter
                    .trim()
                    .split_once('=')
                    .filter(|(key, _)| key.trim().eq_ignore_ascii_case("q"))
                    .and_then(|(_, value)| parse_quality(value.trim()))
                    .unwrap_or(0)
            });
            let quality = if parts.next().is_some() { 0 } else { quality };
            let slot = if coding.eq_ignore_ascii_case("gzip") {
                &mut gzip
            } else if coding.eq_ignore_ascii_case("br") {
                &mut br
            } else if coding.eq_ignore_ascii_case("identity") {
                &mut identity
            } else if coding == "*" {
                &mut wildcard
            } else {
                continue;
            };
            // Conflicting repeated declarations must never bypass an explicit refusal.
            *slot = Some(slot.map_or(quality, |previous: u16| previous.min(quality)));
        }
        let mut selected = None;
        let mut best = 0;
        for (representation, encoding, quality) in [
            (self.brotli.as_ref(), "br", br.or(wildcard).unwrap_or(0)),
            (self.gzip.as_ref(), "gzip", gzip.or(wildcard).unwrap_or(0)),
        ] {
            if quality > best
                && let Some(representation) = representation
            {
                best = quality;
                selected = Some((representation, Some(encoding)));
            }
        }
        if identity.is_some_and(|q| q > best)
            || (selected.is_none()
                && identity.unwrap_or(if wildcard == Some(0) { 0 } else { 1000 }) > 0)
        {
            return Some((&self.identity, None));
        }
        selected
    }
}

fn parse_quality(value: &str) -> Option<u16> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if fraction.len() > 3 || !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    match whole {
        "1" if fraction.bytes().all(|b| b == b'0') => Some(1000),
        "0" => Some(fraction.bytes().enumerate().fold(0, |total, (i, b)| {
            total + u16::from(b - b'0') * [100, 10, 1][i]
        })),
        _ => None,
    }
}

#[cfg(test)]
#[path = "static_asset_tests.rs"]
mod tests;
