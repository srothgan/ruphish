//! Stable, browser-safe failure identities independent of diagnostic prose.
use serde::{Deserialize, Serialize};

/// Public error categories. New values are additive; clients must tolerate unknown codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    /// The saved draft contains invalid or unsupported input.
    DraftInvalid,
    /// Choose a new launch time or immediate delivery before launching.
    DraftSchedulePast,
    /// Invalid CSV encoding or record.
    CsvRowEncoding,
    /// Column count does not match the header.
    CsvRowColumns,
    /// Invalid email address.
    CsvRowEmail,

    /// The operation could not be completed. Try again.
    InternalError,
    /// Invalid username or password.
    AuthInvalidCredentials,
    /// Sign in again to continue.
    AuthRequired,
    /// You do not have permission for this action.
    AuthForbidden,
    /// Reload the page and try again.
    AuthCsrfInvalid,
    /// The request origin is not permitted.
    AuthOriginInvalid,
    /// The API key is missing or invalid.
    AuthApiKeyInvalid,
    /// Exit the current user view first.
    AuthImpersonationActive,
    /// No user view is active.
    AuthImpersonationInactive,
    /// Your account changed. Sign in again.
    AuthChanged,
    /// Too many requests. Try again shortly.
    RateLimited,
    /// The request timed out. Refresh before retrying.
    RequestTimeout,
    /// The request is too large. Reduce its size.
    RequestTooLarge,
    /// The request is invalid. Check the supplied values.
    RequestInvalid,
    /// This request method is not supported.
    MethodNotAllowed,
    /// Check the selected display preferences.
    PreferencesInvalid,
    /// The current password is incorrect.
    PasswordInvalid,
    /// Enter a password.
    PasswordRequired,
    /// Use a password with at least 8 UTF-8 bytes.
    PasswordTooShort,
    /// The password exceeds the supported length.
    PasswordTooLong,
    /// The passwords do not match.
    PasswordMismatch,
    /// Choose a different password.
    PasswordReused,
    /// Set your password before continuing.
    PasswordChangeRequired,
    /// Enter a username.
    UsernameRequired,
    /// Choose a role.
    RoleRequired,
    /// The selected role is unavailable.
    RoleNotFound,
    /// The account could not be found.
    AccountNotFound,
    /// The username is already in use.
    UsernameTaken,
    /// The last administrator must be retained.
    LastAdmin,
    /// You cannot delete your own account.
    SelfDeletion,
    /// The service is busy. Try again shortly.
    ServiceBusy,
    /// The requested resource could not be found.
    ResourceNotFound,
    /// This resource name is already in use.
    ResourceNameTaken,
    /// This resource changed. Reload it before saving; your draft is retained.
    ResourceChanged,
    /// A campaign that is not completed still uses this resource.
    ResourceInUse,
    /// The resource is invalid. Review its content and settings.
    ResourceInvalid,
    /// Delivery could not be confirmed. Check the mail server before retrying.
    DeliveryFailed,
    /// Reload the saved resource before trying again.
    RevisionRequired,
    /// Check the CSV file and its column mapping.
    CsvInvalid,
    /// The CSV contains invalid rows. Review the import report.
    CsvRowsInvalid,
    /// Review the campaign settings before continuing.
    CampaignInvalid,
    /// The launch identifier is invalid. Reopen the campaign draft.
    LaunchKeyInvalid,
    /// This delivery action is invalid. Refresh the delivery state.
    DeliveryActionInvalid,
    /// Mail server certificate verification is required.
    MailboxTlsRequired,
    /// Choose a valid mail server port.
    MailboxPortInvalid,
    /// Check the selected analysis filters.
    AnalysisFilterInvalid,
    /// Choose an available reporting period.
    OverviewWindowInvalid,
    /// The resource identifier does not match the request.
    ResourceIdMismatch,
}

impl ErrorCode {
    /// Canonical protocol spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DraftInvalid => "DRAFT_INVALID",
            Self::DraftSchedulePast => "DRAFT_SCHEDULE_PAST",
            Self::CsvRowEncoding => "CSV_ROW_ENCODING",
            Self::CsvRowColumns => "CSV_ROW_COLUMNS",
            Self::CsvRowEmail => "CSV_ROW_EMAIL",

            Self::InternalError => "INTERNAL_ERROR",
            Self::AuthInvalidCredentials => "AUTH_INVALID_CREDENTIALS",
            Self::AuthRequired => "AUTH_REQUIRED",
            Self::AuthForbidden => "AUTH_FORBIDDEN",
            Self::AuthCsrfInvalid => "AUTH_CSRF_INVALID",
            Self::AuthOriginInvalid => "AUTH_ORIGIN_INVALID",
            Self::AuthApiKeyInvalid => "AUTH_API_KEY_INVALID",
            Self::AuthImpersonationActive => "AUTH_IMPERSONATION_ACTIVE",
            Self::AuthImpersonationInactive => "AUTH_IMPERSONATION_INACTIVE",
            Self::AuthChanged => "AUTH_CHANGED",
            Self::RateLimited => "RATE_LIMITED",
            Self::RequestTimeout => "REQUEST_TIMEOUT",
            Self::RequestTooLarge => "REQUEST_TOO_LARGE",
            Self::RequestInvalid => "REQUEST_INVALID",
            Self::MethodNotAllowed => "METHOD_NOT_ALLOWED",
            Self::PreferencesInvalid => "PREFERENCES_INVALID",
            Self::PasswordInvalid => "PASSWORD_INVALID",
            Self::PasswordRequired => "PASSWORD_REQUIRED",
            Self::PasswordTooShort => "PASSWORD_TOO_SHORT",
            Self::PasswordTooLong => "PASSWORD_TOO_LONG",
            Self::PasswordMismatch => "PASSWORD_MISMATCH",
            Self::PasswordReused => "PASSWORD_REUSED",
            Self::PasswordChangeRequired => "PASSWORD_CHANGE_REQUIRED",
            Self::UsernameRequired => "USERNAME_REQUIRED",
            Self::RoleRequired => "ROLE_REQUIRED",
            Self::RoleNotFound => "ROLE_NOT_FOUND",
            Self::AccountNotFound => "ACCOUNT_NOT_FOUND",
            Self::UsernameTaken => "USERNAME_TAKEN",
            Self::LastAdmin => "LAST_ADMIN",
            Self::SelfDeletion => "SELF_DELETION",
            Self::ServiceBusy => "SERVICE_BUSY",
            Self::ResourceNotFound => "RESOURCE_NOT_FOUND",
            Self::ResourceNameTaken => "RESOURCE_NAME_TAKEN",
            Self::ResourceChanged => "RESOURCE_CHANGED",
            Self::ResourceInUse => "RESOURCE_IN_USE",
            Self::ResourceInvalid => "RESOURCE_INVALID",
            Self::DeliveryFailed => "DELIVERY_FAILED",
            Self::RevisionRequired => "REVISION_REQUIRED",
            Self::CsvInvalid => "CSV_INVALID",
            Self::CsvRowsInvalid => "CSV_ROWS_INVALID",
            Self::CampaignInvalid => "CAMPAIGN_INVALID",
            Self::LaunchKeyInvalid => "LAUNCH_KEY_INVALID",
            Self::DeliveryActionInvalid => "DELIVERY_ACTION_INVALID",
            Self::MailboxTlsRequired => "MAILBOX_TLS_REQUIRED",
            Self::MailboxPortInvalid => "MAILBOX_PORT_INVALID",
            Self::AnalysisFilterInvalid => "ANALYSIS_FILTER_INVALID",
            Self::OverviewWindowInvalid => "OVERVIEW_WINDOW_INVALID",
            Self::ResourceIdMismatch => "RESOURCE_ID_MISMATCH",
        }
    }
}
