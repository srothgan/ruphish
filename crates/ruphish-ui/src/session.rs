//! Browser session transitions shared by the HTTP client and functional tests.
use ruphish_contracts::auth::{BrowserSession, BrowserUser};

/// Navigation required before authenticated content can be used again.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum SessionAction {
    /// The currently mounted account view remains valid.
    #[default]
    Continue,
    /// Authentication ended; clear the view and sign in again.
    SignIn,
    /// Explicit sign-out succeeded; clear authenticated state and show login.
    SignedOut,
    /// Identity or permissions changed; rebuild the view from current server state.
    Reload,
}

/// A terminal invalidation prevents late successful requests from restoring stale UI.
#[derive(Clone, Default)]
pub struct SessionMonitor {
    baseline: Option<(Option<BrowserUser>, Option<String>)>,
    action: SessionAction,
}
impl SessionMonitor {
    /// Current UI disposition, independent of incidental server error wording.
    pub fn action(&self) -> SessionAction {
        self.action
    }
    /// Successful logout is terminal, including for outstanding successful requests.
    pub fn signed_out(&mut self) {
        self.action = SessionAction::SignedOut;
    }
    /// A rejected login is an inline form error; protected-request 401s end the view.
    pub fn response(&mut self, path: &str, status: u16) {
        if self.action == SessionAction::Continue && status == 401 && path != "/login" {
            self.action = SessionAction::SignIn;
        }
    }
    /// Recheck identity and permissions on focus without remounting unchanged drafts.
    pub fn observe(&mut self, session: &BrowserSession) {
        if self.action != SessionAction::Continue {
            return;
        }
        let next = (session.user.clone(), session.actor.clone());
        if let Some(prior) = &self.baseline {
            if prior.0.is_some() && next.0.is_none() {
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
    fn stale_account_view_cannot_be_restored_by_late_success_after_a_rejected_delete() {
        let signed_in = session();
        let mut monitor = SessionMonitor::default();
        monitor.observe(&signed_in);
        monitor.response("/api/users/2", 403);
        assert_eq!(monitor.action(), SessionAction::Continue);
        monitor.response("/api/users/2", 401);
        assert_eq!(monitor.action(), SessionAction::SignIn);
        monitor.response("/api/users/", 200);
        monitor.observe(&signed_in);
        assert_eq!(monitor.action(), SessionAction::SignIn);
    }

    #[test]
    fn returning_to_a_tab_preserves_unchanged_drafts_but_replaces_revoked_or_changed_identity() {
        let signed_in = session();
        let mut monitor = SessionMonitor::default();
        monitor.observe(&signed_in);
        let mut refreshed = signed_in.clone();
        refreshed.csrf_token = "new-masked-token".into();
        refreshed.preferences.language = Some("de".into());
        refreshed.preferences.time_zone = Some("Asia/Shanghai".into());
        monitor.observe(&refreshed);
        assert_eq!(monitor.action(), SessionAction::Continue);
        let mut logged_out = monitor.clone();
        refreshed.user = None;
        logged_out.observe(&refreshed);
        assert_eq!(logged_out.action(), SessionAction::SignIn);
        let mut changed = signed_in.clone();
        if let Some(user) = &mut changed.user {
            user.id = 2;
        }
        let mut other_identity = monitor.clone();
        other_identity.observe(&changed);
        assert_eq!(other_identity.action(), SessionAction::Reload);
        let mut changed = signed_in;
        if let Some(user) = &mut changed.user {
            user.permissions.clear();
        }
        monitor.observe(&changed);
        assert_eq!(monitor.action(), SessionAction::Reload);
    }

    #[test]
    fn invalid_login_and_transient_failures_remain_inline_without_redirect_loops() {
        let mut monitor = SessionMonitor::default();
        for status in [401, 403, 429, 503] {
            monitor.response("/login", status);
        }
        monitor.response("/api/users/", 500);
        assert_eq!(monitor.action(), SessionAction::Continue);
    }

    #[test]
    fn successful_signout_is_terminal_but_failed_signout_preserves_the_session() {
        let signed_in = session();
        let mut monitor = SessionMonitor::default();
        monitor.observe(&signed_in);
        for status in [403, 500, 503] {
            monitor.response("/logout", status);
            assert_eq!(monitor.action(), SessionAction::Continue);
        }
        monitor.signed_out();
        monitor.response("/api/users/1", 200);
        monitor.observe(&signed_in);
        monitor.response("/api/groups/", 401);
        assert_eq!(monitor.action(), SessionAction::SignedOut);
    }
}
