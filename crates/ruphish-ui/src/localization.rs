//! One reactive locale for the entire browser application, including sign-in.
use leptos::prelude::*;
use leptos_i18n::Locale as _;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/web/language.mjs")]
extern "C" {
    #[wasm_bindgen(js_name = browserLanguages)]
    fn browser_languages() -> js_sys::Array;
}

// The upstream generator does not document every generated item.
#[allow(missing_docs)]
#[allow(
    clippy::expect_used,
    reason = "Upstream code parses only build-validated locale literals"
)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/i18n/mod.rs"));
}
pub use generated::i18n;
pub use i18n::{Locale, use_i18n};

/// Resolve error text outside render computations, with English for standalone consumers.
pub fn current_locale() -> Locale {
    use_context::<leptos_i18n::I18nContext<Locale>>()
        .map(|context| context.get_locale_untracked())
        .unwrap_or_default()
}

/// Match supported languages in preference order, including regional BCP-47 tags.
pub fn resolve(saved: Option<&str>, browser: &[String]) -> Locale {
    fn supported(value: &str) -> Option<Locale> {
        let language = value.split('-').next()?;
        Locale::get_all()
            .iter()
            .copied()
            .find(|locale| locale.as_str().eq_ignore_ascii_case(language))
    }
    saved
        .and_then(supported)
        .or_else(|| browser.iter().find_map(|value| supported(value)))
        .unwrap_or_default()
}

/// Install the context before constructing any application views.
pub fn provide(locale: Locale) -> leptos_i18n::I18nContext<Locale> {
    let context = leptos_i18n::context::init_i18n_subcontext(Some(locale.into()));
    provide_context(context);
    context
}

/// Resolve browser defaults without using previous browser-only overrides.
pub fn browser_locale() -> Locale {
    let languages = browser_languages()
        .iter()
        .filter_map(|value| value.as_string())
        .collect::<Vec<_>>();
    resolve(None, &languages)
}

/// A translation closure captures its owner context before event/render callbacks run.
#[macro_export]
macro_rules! tr {
    ($($key:tt)*) => {{
        let context = $crate::localization::use_i18n();
        move || leptos_i18n::t_string!(context, $($key)*).to_string()
    }};
}

/// Reactive text for shared component properties and native attributes.
#[macro_export]
macro_rules! trs {
    ($($key:tt)*) => { leptos::prelude::Signal::derive($crate::tr!($($key)*)) };
}

/// Translate frontend import failures while preserving server explanations and support codes.
pub fn import_error(
    context: leptos_i18n::I18nContext<Locale>,
    error: &crate::import_state::ImportError,
) -> String {
    use crate::import_state::ImportError;
    let message = match error {
        ImportError::InvalidJson => leptos_i18n::t_string!(
            context,
            ui.invalid_json_check_the_file_syntax_and_import_again
        )
        .to_string(),
        ImportError::WrongResourceType => leptos_i18n::t_string!(
            context,
            ui.the_imported_fields_do_not_match_this_resource_type_choose_the_correct_resource
        )
        .to_string(),
        ImportError::UnsupportedFormat => leptos_i18n::t_string!(
            context,
            ui.this_format_is_not_supported_for_this_resource_choose_a_supported_file_or_import
        )
        .to_string(),
        ImportError::TooLarge => {
            leptos_i18n::t_string!(context, ui.import_content_exceeds_8_mib_use_a_smaller_file)
                .to_string()
        }
        ImportError::NoValidRecipients => leptos_i18n::t_string!(
            context,
            ui.no_valid_recipients_were_found_correct_the_csv_and_import_it_again
        )
        .to_string(),
        ImportError::ReadFailed => {
            leptos_i18n::t_string!(context, ui.could_not_read_this_file_select_it_again).to_string()
        }
        ImportError::DraftUnavailable => leptos_i18n::t_string!(
            context,
            ui.this_draft_is_no_longer_available_return_to_the_overview_and_import_again
        )
        .to_string(),
        ImportError::IdUnavailable => leptos_i18n::t_string!(
            context,
            ui.cannot_generate_a_secure_draft_identifier_try_again_in_a_supported_browser_over
        )
        .to_string(),
        ImportError::Capacity => leptos_i18n::t_string!(
            context,
            ui.finish_or_discard_an_existing_import_before_opening_another_draft
        )
        .to_string(),
        ImportError::RequestFailed(message) => message.clone(),
        ImportError::BatchNotSupported(_) => leptos_i18n::t_string!(
            context,
            ui.this_import_accepts_one_resource_at_a_time_split_the_file_and_import_each_separa
        )
        .to_string(),
    };
    format!("{message} ({})", error.code())
}

/// Render import facts without storing language-dependent state.
pub fn import_note(
    context: leptos_i18n::I18nContext<Locale>,
    note: &crate::import_state::ImportNote,
) -> String {
    use crate::import_state::ImportNote;
    match note {
        ImportNote::Unsaved => leptos_i18n::t_string!(
            context,
            ui.loaded_into_an_unsaved_draft_review_the_fields_then_create_the_resource
        )
        .to_string(),
        ImportNote::Email => leptos_i18n::t_string!(
            context,
            ui.only_the_subject_html_and_plain_text_were_imported_attachments_and_the_visible_f
        )
        .to_string(),
        ImportNote::Html => leptos_i18n::t_string!(
            context,
            ui.only_html_content_was_imported_review_the_name_and_other_settings_before_creatin
        )
        .to_string(),
        ImportNote::Csv {
            loaded,
            rejected,
            duplicates,
        } => leptos_i18n::t_string!(
            context,
            messages
                .loaded_recipients_loaded_rejected_rows_rejected_duplicates_duplicate_addresses_o,
            loaded = *loaded,
            rejected = *rejected,
            duplicates = *duplicates
        )
        .to_string(),
    }
}

/// Translate known event/status codes only at the presentation boundary.
pub fn event_label(context: leptos_i18n::I18nContext<Locale>, value: &str) -> String {
    match value {
        "Campaign Created" => leptos_i18n::t_string!(context, ui.campaign_created).to_string(),
        "Email Sent" => leptos_i18n::t_string!(context, ui.email_sent).to_string(),
        "Error Sending Email" => {
            leptos_i18n::t_string!(context, ui.error_sending_email).to_string()
        }
        "Email Opened" => leptos_i18n::t_string!(context, ui.email_opened).to_string(),
        "Clicked Link" => leptos_i18n::t_string!(context, ui.clicked_link).to_string(),
        "Submitted Data" => leptos_i18n::t_string!(context, ui.submitted_data).to_string(),
        "Email Reported" => leptos_i18n::t_string!(context, ui.email_reported).to_string(),
        "Delivery reconciled" => {
            leptos_i18n::t_string!(context, ui.delivery_reconciled).to_string()
        }
        "Completed" => leptos_i18n::t_string!(context, ui.completed).to_string(),
        "In progress" => leptos_i18n::t_string!(context, ui.in_progress).to_string(),
        "Queued" => leptos_i18n::t_string!(context, ui.queued).to_string(),
        "Scheduled" => leptos_i18n::t_string!(context, ui.scheduled).to_string(),
        "Recorded event" => leptos_i18n::t_string!(context, ui.recorded_event).to_owned(),
        _ => value.to_owned(),
    }
}

/// Localized transport state; protocol values remain unchanged.
pub fn delivery_label(context: leptos_i18n::I18nContext<Locale>, state: &str) -> String {
    match state {
        "accepted" => leptos_i18n::t_string!(context, ui.accepted_by_smtp_server).to_string(),
        "retry" => leptos_i18n::t_string!(context, ui.temporary_failure).to_string(),
        "failed" => leptos_i18n::t_string!(context, ui.delivery_failed).to_string(),
        "blocked" => leptos_i18n::t_string!(context, ui.delivery_blocked).to_string(),
        "unknown" => leptos_i18n::t_string!(context, ui.outcome_unknown).to_string(),
        "cancelled" => leptos_i18n::t_string!(context, ui.cancelled).to_string(),
        "preparing" => leptos_i18n::t_string!(context, ui.preparing_message).to_string(),
        "submitting" => leptos_i18n::t_string!(context, ui.submitting_message).to_string(),
        "ready" => leptos_i18n::t_string!(context, ui.scheduled).to_string(),
        _ => leptos_i18n::t_string!(context, ui.recorded_delivery_state).to_string(),
    }
}

/// Render chronological facts without changing their timestamps or grouping.
pub fn timeline_title(
    context: leptos_i18n::I18nContext<Locale>,
    title: &crate::recipient_history::TimelineTitle,
) -> String {
    use crate::recipient_history::TimelineTitle;
    match title {
        TimelineTitle::Started(generation) => leptos_i18n::t_string!(
            context,
            messages.attempt_generation_started,
            generation = *generation
        )
        .to_string(),
        TimelineTitle::Finished(generation, state) => leptos_i18n::t_string!(
            context,
            messages.attempt_generation_outcome,
            generation = *generation,
            outcome = delivery_label(context, state)
        )
        .to_string(),
        TimelineTitle::Event(event) => event_label(context, event),
    }
}

/// Preserve server details while localizing verification recovery guidance.
pub fn verification_error(
    context: leptos_i18n::I18nContext<Locale>,
    error: &str,
    mutation: bool,
) -> String {
    if mutation {
        leptos_i18n::t_string!(
            context,
            messages
                .could_not_confirm_the_verification_request_refresh_status_before_retrying_error,
            error = error
        )
        .to_string()
    } else {
        leptos_i18n::t_string!(
            context,
            messages.unable_to_refresh_verification_status_displayed_results_may_be_stale_error,
            error = error
        )
        .to_string()
    }
}

/// Translate the semantic trend label without changing its styling or arithmetic.
pub fn trend_label(context: leptos_i18n::I18nContext<Locale>, tone: &str) -> String {
    match tone {
        "improving" => leptos_i18n::t_string!(context, ui.improving).to_owned(),
        "worsening" => leptos_i18n::t_string!(context, ui.worsening).to_owned(),
        _ => leptos_i18n::t_string!(context, ui.neutral).to_owned(),
    }
}

/// Translate a received error once, retaining its stable support code.
pub fn failure_label(context: leptos_i18n::I18nContext<Locale>, code: &str) -> String {
    use ruphish_contracts::errors::ErrorCode;
    let parsed = serde_json::from_value::<ErrorCode>(serde_json::Value::String(code.to_owned()));
    let text = match parsed {
        Ok(ErrorCode::DraftInvalid) => leptos_i18n::t_string!(context, errors.draft_invalid),
        Ok(ErrorCode::DraftSchedulePast) => {
            leptos_i18n::t_string!(context, errors.draft_schedule_past)
        }
        Ok(ErrorCode::CsvRowEncoding) => leptos_i18n::t_string!(context, errors.csv_row_encoding),
        Ok(ErrorCode::CsvRowColumns) => leptos_i18n::t_string!(context, errors.csv_row_columns),
        Ok(ErrorCode::CsvRowEmail) => leptos_i18n::t_string!(context, errors.csv_row_email),

        Ok(ErrorCode::InternalError) => leptos_i18n::t_string!(context, errors.internal_error),
        Ok(ErrorCode::AuthInvalidCredentials) => {
            leptos_i18n::t_string!(context, errors.auth_invalid_credentials)
        }
        Ok(ErrorCode::AuthRequired) => leptos_i18n::t_string!(context, errors.auth_required),
        Ok(ErrorCode::AuthForbidden) => leptos_i18n::t_string!(context, errors.auth_forbidden),
        Ok(ErrorCode::AuthCsrfInvalid) => leptos_i18n::t_string!(context, errors.auth_csrf_invalid),
        Ok(ErrorCode::AuthOriginInvalid) => {
            leptos_i18n::t_string!(context, errors.auth_origin_invalid)
        }
        Ok(ErrorCode::AuthApiKeyInvalid) => {
            leptos_i18n::t_string!(context, errors.auth_api_key_invalid)
        }
        Ok(ErrorCode::AuthImpersonationActive) => {
            leptos_i18n::t_string!(context, errors.auth_impersonation_active)
        }
        Ok(ErrorCode::AuthImpersonationInactive) => {
            leptos_i18n::t_string!(context, errors.auth_impersonation_inactive)
        }
        Ok(ErrorCode::AuthChanged) => leptos_i18n::t_string!(context, errors.auth_changed),
        Ok(ErrorCode::FeatureDisabled) => {
            leptos_i18n::t_string!(context, ui.feature_disabled_description)
        }
        Ok(ErrorCode::RateLimited) => leptos_i18n::t_string!(context, errors.rate_limited),
        Ok(ErrorCode::RequestTimeout) => leptos_i18n::t_string!(context, errors.request_timeout),
        Ok(ErrorCode::RequestTooLarge) => leptos_i18n::t_string!(context, errors.request_too_large),
        Ok(ErrorCode::RequestInvalid) => leptos_i18n::t_string!(context, errors.request_invalid),
        Ok(ErrorCode::MethodNotAllowed) => {
            leptos_i18n::t_string!(context, errors.method_not_allowed)
        }
        Ok(ErrorCode::PreferencesInvalid) => {
            leptos_i18n::t_string!(context, errors.preferences_invalid)
        }
        Ok(ErrorCode::PasswordInvalid) => leptos_i18n::t_string!(context, errors.password_invalid),
        Ok(ErrorCode::PasswordRequired) => {
            leptos_i18n::t_string!(context, errors.password_required)
        }
        Ok(ErrorCode::PasswordTooShort) => {
            leptos_i18n::t_string!(context, errors.password_too_short)
        }
        Ok(ErrorCode::PasswordTooLong) => leptos_i18n::t_string!(context, errors.password_too_long),
        Ok(ErrorCode::PasswordMismatch) => {
            leptos_i18n::t_string!(context, errors.password_mismatch)
        }
        Ok(ErrorCode::PasswordReused) => leptos_i18n::t_string!(context, errors.password_reused),
        Ok(ErrorCode::PasswordChangeRequired) => {
            leptos_i18n::t_string!(context, errors.password_change_required)
        }
        Ok(ErrorCode::UsernameRequired) => {
            leptos_i18n::t_string!(context, errors.username_required)
        }
        Ok(ErrorCode::RoleRequired) => leptos_i18n::t_string!(context, errors.role_required),
        Ok(ErrorCode::RoleNotFound) => leptos_i18n::t_string!(context, errors.role_not_found),
        Ok(ErrorCode::AccountNotFound) => leptos_i18n::t_string!(context, errors.account_not_found),
        Ok(ErrorCode::UsernameTaken) => leptos_i18n::t_string!(context, errors.username_taken),
        Ok(ErrorCode::LastAdmin) => leptos_i18n::t_string!(context, errors.last_admin),
        Ok(ErrorCode::SelfDeletion) => leptos_i18n::t_string!(context, errors.self_deletion),
        Ok(ErrorCode::ServiceBusy) => leptos_i18n::t_string!(context, errors.service_busy),
        Ok(ErrorCode::ResourceNotFound) => {
            leptos_i18n::t_string!(context, errors.resource_not_found)
        }
        Ok(ErrorCode::ResourceNameTaken) => {
            leptos_i18n::t_string!(context, errors.resource_name_taken)
        }
        Ok(ErrorCode::ResourceChanged) => leptos_i18n::t_string!(context, errors.resource_changed),
        Ok(ErrorCode::ResourceInUse) => leptos_i18n::t_string!(context, errors.resource_in_use),
        Ok(ErrorCode::ResourceInvalid) => leptos_i18n::t_string!(context, errors.resource_invalid),
        Ok(ErrorCode::DeliveryFailed) => leptos_i18n::t_string!(context, errors.delivery_failed),
        Ok(ErrorCode::RevisionRequired) => {
            leptos_i18n::t_string!(context, errors.revision_required)
        }
        Ok(ErrorCode::CsvInvalid) => leptos_i18n::t_string!(context, errors.csv_invalid),
        Ok(ErrorCode::CsvRowsInvalid) => leptos_i18n::t_string!(context, errors.csv_rows_invalid),
        Ok(ErrorCode::CampaignInvalid) => leptos_i18n::t_string!(context, errors.campaign_invalid),
        Ok(ErrorCode::LaunchKeyInvalid) => {
            leptos_i18n::t_string!(context, errors.launch_key_invalid)
        }
        Ok(ErrorCode::DeliveryActionInvalid) => {
            leptos_i18n::t_string!(context, errors.delivery_action_invalid)
        }
        Ok(ErrorCode::MailboxTlsRequired) => {
            leptos_i18n::t_string!(context, errors.mailbox_tls_required)
        }
        Ok(ErrorCode::MailboxPortInvalid) => {
            leptos_i18n::t_string!(context, errors.mailbox_port_invalid)
        }
        Ok(ErrorCode::AnalysisFilterInvalid) => {
            leptos_i18n::t_string!(context, errors.analysis_filter_invalid)
        }
        Ok(ErrorCode::OverviewWindowInvalid) => {
            leptos_i18n::t_string!(context, errors.overview_window_invalid)
        }
        Ok(ErrorCode::ResourceIdMismatch) => {
            leptos_i18n::t_string!(context, errors.resource_id_mismatch)
        }
        Err(_) => leptos_i18n::t_string!(context, errors.unknown),
    };
    format!("{code} - {text}")
}

/// Translate built-in role slugs while preserving custom role display names.
pub fn role_label(
    context: leptos_i18n::I18nContext<Locale>,
    role: &ruphish_contracts::auth::Role,
) -> String {
    match role.slug.as_str() {
        "admin" => leptos_i18n::t_string!(context, ui.administrator_2).to_owned(),
        "user" => leptos_i18n::t_string!(context, ui.user).to_owned(),
        _ => role.name.clone(),
    }
}
