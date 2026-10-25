//! Session-aware HTTP client shared by every admin feature module.
//!
//! [`ruphish_ui::browser_http`] owns pure transport: building requests, sending
//! them, decoding bodies and enforcing timeouts. This client layers the reactive
//! browser session on top — it feeds every response status into the
//! [`SessionMonitor`], refuses to act once a session is no longer active, and
//! returns localized error messages. Feature modules depend only on `Api`.
use gloo_net::http::Request;
use leptos::prelude::*;
use ruphish_contracts::auth::{BrowserSession, UserEdit};
use ruphish_ui::api_error::ApiError;
use ruphish_ui::session::{SessionAction, SessionMonitor};
use serde_json::Value;

#[derive(Clone, Copy)]
pub(crate) struct Api {
    pub(crate) features: RwSignal<ruphish_contracts::settings::Features>,
    feature_generation: RwSignal<u64>,
    monitor: RwSignal<SessionMonitor>,
    pub(crate) i18n: leptos_i18n::I18nContext<ruphish_ui::localization::Locale>,
}
impl Api {
    /// Construct a client bound to the browser locale with a fresh session monitor.
    pub(crate) fn new(i18n: leptos_i18n::I18nContext<ruphish_ui::localization::Locale>) -> Self {
        Self {
            features: RwSignal::new(ruphish_contracts::settings::Features::default()),
            feature_generation: RwSignal::new(0),
            monitor: RwSignal::new(SessionMonitor::default()),
            i18n,
        }
    }
    /// Current session disposition; tracked so reactive consumers rebuild on change.
    pub(crate) fn session_action(self) -> SessionAction {
        self.monitor.with(|m| m.action())
    }
    /// Reconcile the monitor with a freshly loaded session snapshot.
    pub(crate) fn observe_session(self, session: &BrowserSession) {
        self.monitor.update(|m| m.observe(session));
    }
    /// Record a successful, terminal sign-out.
    pub(crate) fn mark_signed_out(self) {
        self.monitor.update(|m| m.signed_out());
    }
    /// Feature generation snapshot used to discard stale background refreshes.
    pub(crate) fn feature_ticket(self) -> u64 {
        self.feature_generation.get_untracked()
    }
    pub(crate) fn set_features(self, value: ruphish_contracts::settings::Features) {
        self.feature_generation.update(|n| *n = n.wrapping_add(1));
        self.features.set(value);
    }
    pub(crate) fn invalid_read(self) -> String {
        leptos_i18n::t_string!(self.i18n, ui.invalid_read_response).to_owned()
    }
    /// An invalid-read failure as a read error, with no originating HTTP status.
    pub(crate) fn invalid_read_err(self) -> ApiError {
        ApiError::local(self.invalid_read())
    }
    /// Decode required wire fields before a read can enter a query cache.
    pub(crate) async fn read<T: serde::de::DeserializeOwned>(
        self,
        path: &str,
    ) -> Result<T, ApiError> {
        let value = self.get(path).await?;
        serde_json::from_value(value).map_err(|_| self.invalid_read_err())
    }

    pub(crate) async fn launch(
        self,
        path: &str,
        token: &str,
        key: &str,
        value: &Value,
    ) -> Result<Value, (u16, String)> {
        let i18n = self.i18n;

        if !self.active() {
            return Err((
                401,
                leptos_i18n::t_string!(i18n, ui.your_session_ended).into(),
            ));
        }
        let request = ruphish_ui::browser_http::launch_request(path, token, key, value)
            .map_err(|e| (0, e))?;
        let response=self.send_request(path, request).await.map_err(|_|(0,leptos_i18n::t_string!(i18n, ui.launch_outcome_is_not_known_retry_uses_the_same_launch_key_do_not_create_another).into()))?;
        let status = response.status;
        let value = self
            .response(path, response)
            .await
            .map_err(|e| (status, e.message))?;
        if value["id"].as_i64().is_none_or(|id| id <= 0) {
            return Err((0, leptos_i18n::t_string!(i18n, ui.launch_outcome_is_not_known_retry_uses_the_same_launch_key_do_not_create_another).into()));
        }
        Ok(value)
    }
    pub(crate) async fn resource(
        self,
        method: &str,
        path: &str,
        token: &str,
        value: Option<&Value>,
        revision: Option<&str>,
    ) -> Result<(Value, Option<String>), String> {
        self.resource_response(method, path, token, value, revision)
            .await
            .map_err(|error| error.message)
    }

    /// A read retaining the concurrency revision and structured failure.
    pub(crate) async fn read_resource(
        self,
        path: &str,
    ) -> Result<(Value, Option<String>), ApiError> {
        self.resource_response("GET", path, "", None, None).await
    }

    async fn resource_response(
        self,
        method: &str,
        path: &str,
        token: &str,
        value: Option<&Value>,
        revision: Option<&str>,
    ) -> Result<(Value, Option<String>), ApiError> {
        if !self.active() {
            return Err(ApiError::local(
                leptos_i18n::t_string!(self.i18n, ui.your_session_ended_sign_in_again_to_continue)
                    .to_owned(),
            ));
        }
        let request =
            ruphish_ui::browser_http::resource_request(method, path, token, value, revision)
                .map_err(ApiError::local)?;
        let response = self
            .send_request(path, request)
            .await
            .map_err(ApiError::local)?;
        let etag = response.etag.clone();
        self.response(path, response)
            .await
            .map(|value| (value, etag))
    }
    /// Only explicit acknowledgements confirm actions without a resource response.
    pub(crate) async fn command(
        self,
        method: &str,
        path: &str,
        token: &str,
        value: Option<&Value>,
        revision: Option<&str>,
    ) -> Result<(), String> {
        let (value, _) = self.resource(method, path, token, value, revision).await?;
        if value["success"] != true {
            return Err(self.invalid_read());
        }
        Ok(())
    }

    pub(crate) async fn download(self, path: &str, filename: &str) -> Result<(), String> {
        let i18n = self.i18n;

        if !self.active() {
            return Err(leptos_i18n::t_string!(i18n, ui.your_session_ended).into());
        }
        let request = ruphish_ui::browser_http::resource_request("GET", path, "", None, None)?;
        let observed_path = path.to_owned();
        let (response, file) = ruphish_ui::browser_http::download(request, move |status| {
            self.monitor
                .update(|monitor| monitor.response(&observed_path, status));
        })
        .await?;
        if !self.active() {
            return Err(leptos_i18n::t_string!(i18n, ui.your_session_ended).into());
        }
        if !(200..300).contains(&response.status) {
            self.response(path, response).await.map_err(|e| e.message)?;
            return Err(leptos_i18n::t_string!(i18n, ui.the_export_could_not_be_generated).into());
        }
        ruphish_ui::browser_http::save_file(&file, filename)
    }
    pub(crate) fn active(self) -> bool {
        self.monitor
            .with_untracked(|m| m.action() == SessionAction::Continue)
    }
}
impl Api {
    async fn send_request(
        self,
        path: &str,
        request: Request,
    ) -> Result<ruphish_ui::browser_http::Received, String> {
        let path = path.to_owned();
        ruphish_ui::browser_http::send(request, move |status| {
            self.monitor.update(|m| m.response(&path, status))
        })
        .await
    }
    async fn response(
        self,
        _path: &str,
        response: ruphish_ui::browser_http::Received,
    ) -> Result<Value, ApiError> {
        let i18n = self.i18n;

        let status = response.status;
        if !self.active() {
            return Err(ApiError::http(
                status,
                "AUTH_REQUIRED".into(),
                leptos_i18n::t_string!(i18n, ui.your_session_ended_sign_in_again_to_continue)
                    .to_string(),
            ));
        }
        ruphish_ui::api_error::response(
            status,
            ruphish_ui::browser_http::decode(status, &response.body),
            |code| ruphish_ui::localization::failure_label(i18n, code),
        )
    }

    pub(crate) async fn get(self, path: &str) -> Result<Value, ApiError> {
        let i18n = self.i18n;

        if !self.active() {
            return Err(ApiError::local(
                leptos_i18n::t_string!(i18n, ui.your_session_ended_sign_in_again_to_continue)
                    .to_string(),
            ));
        }
        let request = Request::get(path)
            .header("x-ruphish-client", "browser")
            .header("accept", "application/json")
            .build()
            .map_err(|_| {
                ApiError::local(leptos_i18n::t_string!(i18n, ui.cannot_prepare_request).to_string())
            })?;
        self.response(
            path,
            self.send_request(path, request)
                .await
                .map_err(ApiError::local)?,
        )
        .await
    }

    pub(crate) async fn form(
        self,
        path: &str,
        token: &str,
        fields: &[(&str, String)],
    ) -> Result<Value, String> {
        let i18n = self.i18n;

        if !self.active() {
            return Err(leptos_i18n::t_string!(
                i18n,
                ui.your_session_ended_sign_in_again_to_continue
            )
            .into());
        }
        let body = serde_urlencoded::to_string(fields)
            .map_err(|_| leptos_i18n::t_string!(i18n, ui.cannot_prepare_the_request).to_owned())?;
        let request = Request::post(path)
            .header("x-ruphish-client", "browser")
            .header("accept", "application/json")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("x-csrf-token", token)
            .body(body)
            .map_err(|_| leptos_i18n::t_string!(i18n, ui.cannot_prepare_the_request).to_owned())?;
        self.response(path, self.send_request(path, request).await?)
            .await
            .map_err(|e| e.message)
    }
    pub(crate) async fn json(
        self,
        method: &str,
        path: &str,
        token: &str,
        value: Option<&UserEdit>,
    ) -> Result<Value, String> {
        let i18n = self.i18n;

        if !self.active() {
            return Err(leptos_i18n::t_string!(
                i18n,
                ui.your_session_ended_sign_in_again_to_continue
            )
            .into());
        }
        let builder = match method {
            "PUT" => Request::put(path),
            "DELETE" => Request::delete(path),
            _ => Request::post(path),
        }
        .header("x-ruphish-client", "browser")
        .header("accept", "application/json")
        .header("x-csrf-token", token);
        let request = if let Some(value) = value {
            builder.json(value)
        } else {
            builder.build()
        }
        .map_err(|_| leptos_i18n::t_string!(i18n, ui.cannot_prepare_the_request).to_owned())?;
        self.response(path, self.send_request(path, request).await?)
            .await
            .map_err(|e| e.message)
    }
}
