//! Browser session transitions shared by the HTTP client and functional tests.
use ruphish_contracts::auth::{BrowserSession, BrowserUser};

/// Navigation required before authenticated content can be used again.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub enum SessionAction {
    /// The currently mounted account view remains valid.
    #[default]
    Continue,
    /// A login owns the next transition; background reads cannot change identity.
    SigningIn,
    /// Authentication succeeded; replace the login document with this destination.
    Redirect(String),
    /// Authentication ended; clear the view and sign in again.
    SignIn,
    /// Explicit sign-out succeeded; clear authenticated state and show login.
    SignedOut,
    /// Identity or permissions changed; rebuild the view from current server state.
    Reload,
}

impl SessionAction {
    /// Requests and the current form may remain active until navigation is required.
    pub fn active(&self) -> bool {
        matches!(self, Self::Continue | Self::SigningIn)
    }
}

/// A terminal invalidation prevents late successful requests from restoring stale UI.
#[derive(Clone, Default)]
pub struct SessionMonitor {
    baseline: Option<(Option<BrowserUser>, Option<String>)>,
    action: SessionAction,
    generation: u64,
}
impl SessionMonitor {
    /// Current UI disposition, independent of incidental server error wording.
    pub fn action(&self) -> SessionAction {
        self.action.clone()
    }
    /// Capture the authentication generation before an asynchronous request.
    pub fn generation(&self) -> u64 {
        self.generation
    }
    /// Only idle reads from the current authentication generation may be applied.
    pub fn accepts_read(&self, generation: u64) -> bool {
        generation == self.generation && self.action == SessionAction::Continue
    }
    /// Claim the sign-in transition and invalidate reads begun before this attempt.
    pub fn begin_login(&mut self) -> bool {
        if self.action != SessionAction::Continue {
            return false;
        }
        self.generation = self.generation.wrapping_add(1);
        self.action = SessionAction::SigningIn;
        true
    }
    /// Release a failed attempt or commit its sole navigation after confirmation.
    pub fn finish_login(&mut self, destination: Option<String>) {
        if self.action == SessionAction::SigningIn {
            self.generation = self.generation.wrapping_add(1);
            self.action = destination.map_or(SessionAction::Continue, SessionAction::Redirect);
        }
    }
    /// Successful logout is terminal, including for outstanding successful requests.
    pub fn signed_out(&mut self) {
        self.action = SessionAction::SignedOut;
    }
    /// A rejected login is an inline form error; protected-request 401s end the view.
    pub fn response(&mut self, path: &str, status: u16) {
        if self.action == SessionAction::Continue && status == 401 && path != "/auth/login" {
            self.action = SessionAction::SignIn;
        }
    }
    /// Recheck identity and permissions on focus without remounting unchanged drafts.
    pub fn observe(&mut self, session: &BrowserSession, requested: &str) {
        if self.action != SessionAction::Continue {
            return;
        }
        let next = (session.user.clone(), session.actor.clone());
        if let Some(prior) = &self.baseline {
            if prior.0.is_none()
                && let Some(user) = &next.0
            {
                self.action = SessionAction::Redirect(ruphish_contracts::auth::login_destination(
                    requested,
                    user.password_change_required,
                ));
            } else if prior.0.is_some() && next.0.is_none() {
                self.action = SessionAction::SignIn;
            } else if prior != &next {
                self.action = SessionAction::Reload;
            }
        } else {
            self.baseline = Some(next);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruphish_contracts::auth::Role;

    fn session() -> BrowserSession {
        BrowserSession {
            features: Some(Default::default()),
            csrf_token: "first-token".into(),
            preferences: Default::default(),
            actor: None,
            user: Some(BrowserUser {
                id: 1,
                username: "admin".into(),
                password_change_required: false,
                role: Role {
                    slug: "admin".into(),
                    name: "Administrator".into(),
                    description: String::new(),
                },
                permissions: vec!["modify_system".into()],
            }),
        }
    }

    #[test]
    fn login_owns_navigation_and_discards_reads_from_before_the_attempt() {
        let signed_in = session();
        let mut guest = signed_in.clone();
        guest.user = None;
        let mut monitor = SessionMonitor::default();
        monitor.observe(&guest, "/campaigns");
        let before = monitor.generation();
        assert!(monitor.begin_login());
        assert!(
            !monitor.begin_login(),
            "duplicate submissions cannot claim login"
        );
        monitor.observe(&signed_in, "/campaigns");
        assert_eq!(monitor.action(), SessionAction::SigningIn);
        assert!(!monitor.accepts_read(before));
        monitor.finish_login(Some("/campaigns".into()));
        monitor.observe(&guest, "/");
        monitor.observe(&signed_in, "/");
        monitor.response("/api/groups", 401);
        assert_eq!(
            monitor.action(),
            SessionAction::Redirect("/campaigns".into())
        );
    }

    #[test]
    fn failed_login_restores_refresh_and_retry_without_accepting_old_reads() {
        let mut monitor = SessionMonitor::default();
        for status in [401, 503] {
            let before = monitor.generation();
            assert!(monitor.begin_login());
            let during = monitor.generation();
            monitor.response("/auth/login", status);
            monitor.finish_login(None);
            assert_eq!(monitor.action(), SessionAction::Continue);
            assert!(!monitor.accepts_read(before));
            assert!(!monitor.accepts_read(during));
            assert!(monitor.accepts_read(monitor.generation()));
        }
        assert!(monitor.begin_login());
        monitor.finish_login(Some("/reset_password".into()));
        assert_eq!(
            monitor.action(),
            SessionAction::Redirect("/reset_password".into())
        );
    }

    #[test]
    fn authentication_in_another_tab_uses_the_return_destination_or_password_setup() {
        for required in [false, true] {
            let mut signed_in = session();
            if let Some(user) = &mut signed_in.user {
                user.password_change_required = required;
            }
            let mut guest = signed_in.clone();
            guest.user = None;
            let mut monitor = SessionMonitor::default();
            monitor.observe(&guest, "/campaigns");
            monitor.observe(&signed_in, "/campaigns");
            assert_eq!(
                monitor.action(),
                SessionAction::Redirect(
                    if required {
                        "/reset_password"
                    } else {
                        "/campaigns"
                    }
                    .into(),
                )
            );
        }
    }

    #[test]
    fn logout_remains_terminal_even_with_an_outstanding_login_completion() {
        let mut monitor = SessionMonitor::default();
        assert!(monitor.begin_login());
        monitor.signed_out();
        monitor.finish_login(Some("/campaigns".into()));
        assert_eq!(monitor.action(), SessionAction::SignedOut);
        assert!(!monitor.begin_login());
        assert!(!monitor.accepts_read(monitor.generation()));
    }

    #[test]
    fn stale_account_view_cannot_be_restored_by_late_success_after_a_rejected_delete() {
        let signed_in = session();
        let mut monitor = SessionMonitor::default();
        monitor.observe(&signed_in, "/");
        monitor.response("/api/users/2", 403);
        assert_eq!(monitor.action(), SessionAction::Continue);
        monitor.response("/api/users/2", 401);
        assert_eq!(monitor.action(), SessionAction::SignIn);
        monitor.response("/api/users/", 200);
        monitor.observe(&signed_in, "/");
        assert_eq!(monitor.action(), SessionAction::SignIn);
    }

    #[test]
    fn returning_to_a_tab_preserves_unchanged_drafts_but_replaces_revoked_or_changed_identity() {
        let signed_in = session();
        let mut monitor = SessionMonitor::default();
        monitor.observe(&signed_in, "/");
        let mut refreshed = signed_in.clone();
        refreshed.csrf_token = "new-masked-token".into();
        refreshed.preferences.language = Some("de".into());
        refreshed.preferences.time_zone = Some("Asia/Shanghai".into());
        monitor.observe(&refreshed, "/");
        assert_eq!(monitor.action(), SessionAction::Continue);
        let mut logged_out = monitor.clone();
        refreshed.user = None;
        logged_out.observe(&refreshed, "/");
        assert_eq!(logged_out.action(), SessionAction::SignIn);
        let mut changed = signed_in.clone();
        if let Some(user) = &mut changed.user {
            user.id = 2;
        }
        let mut other_identity = monitor.clone();
        other_identity.observe(&changed, "/");
        assert_eq!(other_identity.action(), SessionAction::Reload);
        let mut changed = signed_in;
        if let Some(user) = &mut changed.user {
            user.permissions.clear();
        }
        monitor.observe(&changed, "/");
        assert_eq!(monitor.action(), SessionAction::Reload);
    }

    #[test]
    fn invalid_login_and_transient_failures_remain_inline_without_redirect_loops() {
        let mut monitor = SessionMonitor::default();
        for status in [401, 403, 429, 503] {
            monitor.response("/auth/login", status);
        }
        monitor.response("/api/users/", 500);
        assert_eq!(monitor.action(), SessionAction::Continue);
    }

    #[test]
    fn successful_signout_is_terminal_but_failed_signout_preserves_the_session() {
        let signed_in = session();
        let mut monitor = SessionMonitor::default();
        monitor.observe(&signed_in, "/");
        for status in [403, 500, 503] {
            monitor.response("/auth/logout", status);
            assert_eq!(monitor.action(), SessionAction::Continue);
        }
        monitor.signed_out();
        monitor.response("/api/users/1", 200);
        monitor.observe(&signed_in, "/");
        monitor.response("/api/groups/", 401);
        assert_eq!(monitor.action(), SessionAction::SignedOut);
    }
}
