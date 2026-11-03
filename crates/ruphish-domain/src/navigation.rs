//! Shared destinations when entering an authenticated browser session.

/// Preserve the supported path-only return destination without leaving this origin
/// or returning to an authentication entry/exit page. Password setup takes priority.
pub fn login_destination(requested: &str, password_change_required: bool) -> String {
    if password_change_required {
        return "/reset_password".into();
    }
    if requested.contains('\\') || requested.chars().any(char::is_control) {
        return "/".into();
    }
    let path = url::Url::parse(requested)
        .ok()
        .map(|url| url.path().to_owned())
        .unwrap_or_else(|| requested.split(['?', '#']).next().unwrap_or("").to_owned());
    let lower = path.to_ascii_lowercase();
    if lower.contains("%5c") || lower.contains("%2f") {
        return "/".into();
    }
    // The URL parser canonicalizes dot segments (including encoded dots) exactly
    // as the browser does, before checking for authentication-page loops.
    let Ok(url) = url::Url::parse(&format!(
        "https://return.invalid/{}",
        path.trim_start_matches('/')
    )) else {
        return "/".into();
    };
    // Dot-segment removal can expose another leading slash. Never return an
    // authority-relative URL such as //outside.invalid after canonicalization.
    let path = format!("/{}", url.path().trim_start_matches('/'));
    if matches!(
        path.trim_end_matches('/'),
        "/login" | "/logout" | "/reset_password"
    ) {
        "/".into()
    } else {
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destinations_are_local_canonical_paths_without_authentication_loops() {
        for (input, expected) in [
            ("", "/"),
            ("/campaigns?status=active#top", "/campaigns"),
            ("https://outside.invalid/settings?discard=1", "/settings"),
            ("//outside.invalid/path", "/outside.invalid/path"),
            ("/login?next=/login", "/"),
            ("/login/", "/"),
            ("/logout", "/"),
            ("/reset_password", "/"),
            ("/groups/../login", "/"),
            ("/groups/%2e%2e/login", "/"),
            ("/groups/..//outside.invalid", "/outside.invalid"),
            ("/groups/..//login", "/"),
            ("/%2Foutside.invalid", "/"),
            ("/\\outside.invalid", "/"),
            ("/%5coutside.invalid", "/"),
            ("/\n/outside.invalid", "/"),
        ] {
            assert_eq!(login_destination(input, false), expected, "{input}");
            assert_eq!(login_destination(input, true), "/reset_password");
        }
    }
}
