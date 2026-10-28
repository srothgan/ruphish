//! Browser capability boundary for authored landing pages and their functional previews.

/// Content directives shared by the response header and isolated preview frame.
/// Authored code is permitted; native forms, embedded browsing contexts and workers are not.
pub const CONTENT_POLICY: &str = "default-src 'none'; script-src 'unsafe-inline' 'unsafe-eval' https: http: data: blob:; style-src 'unsafe-inline' https: http: data: blob:; img-src https: http: data: blob:; font-src https: http: data: blob:; connect-src https: http:; form-action 'none'; base-uri https: http:; object-src 'none'; frame-src 'none'; worker-src 'none'";

/// Form permission is necessary for validation/submit events; form-action blocks navigation.
/// Never add allow-same-origin: authored code must not inherit application credentials.
pub const SANDBOX: &str = "allow-scripts allow-forms";

/// Response policy for public authored pages, including non-tracking test previews.
pub fn response_policy() -> String {
    format!("sandbox {SANDBOX}; {CONTENT_POLICY}; frame-ancestors 'none'")
}

/// Supported page asset media types, shared by bundle import and public static serving.
pub fn asset_mime(path: &str) -> Option<&'static str> {
    match path.rsplit('.').next()?.to_ascii_lowercase().as_str() {
        "css" => Some("text/css"),
        "js" => Some("text/javascript"),
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "svg" => Some("image/svg+xml"),
        "webp" => Some("image/webp"),
        "avif" => Some("image/avif"),
        "ico" => Some("image/x-icon"),
        "woff" => Some("font/woff"),
        "woff2" => Some("font/woff2"),
        "ttf" => Some("font/ttf"),
        "otf" => Some("font/otf"),
        _ => None,
    }
}
