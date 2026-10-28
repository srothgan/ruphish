//! Offline, immutable starter content. Creation uses the normal resource APIs.
use crate::{
    auth::{AuthState, Context, Rejection, failure},
    resources::response,
};
use axum::{Router, extract::Path, http::StatusCode, response::Response, routing::get};
use ruphish_contracts::{errors::ErrorCode, starter_templates::*};

mod source;
use source::Source;

macro_rules! email {
    ($id:literal, $brand:literal, $name:literal) => {
        Source {
            id: concat!("email-", $id),
            name: $name,
            brand: $brand,
            html: include_str!(concat!("../starter-templates/emails/", $id, "/email.html")),
            script: None,
            kind: Kind::Email,
            subject: include_str!(concat!("../starter-templates/emails/", $id, "/subject.txt")),
            text: include_str!(concat!("../starter-templates/emails/", $id, "/email.txt")),
            logo: None,
        }
    };
}
macro_rules! page {
    ($id:literal, $brand:literal, $name:literal, $logo:expr) => {
        page!($id, $brand, $name, $logo, None)
    };
    ($id:literal, $brand:literal, $name:literal, $logo:expr, $script:expr) => {
        Source {
            id: concat!("page-", $id),
            name: $name,
            brand: $brand,
            html: include_str!(concat!(
                "../starter-templates/landing-pages/",
                $id,
                "/landing.html"
            )),
            kind: Kind::LandingPage,
            script: $script,
            subject: "",
            text: "",
            logo: $logo,
        }
    };
}

macro_rules! learning {
    ($id:literal, $name:literal) => {
        Source {
            id: concat!("learning-", $id),
            name: $name,
            brand: "Ruphish",
            html: include_str!(concat!(
                "../starter-templates/learning-pages/",
                $id,
                "/learning.html"
            )),
            kind: Kind::LearningPage,
            script: None,
            subject: "",
            text: "",
            logo: Some((".learning-logo", 48, "Learning page header")),
        }
    };
}

// These files are part of the executable, including future binary/container builds.
// No runtime filesystem lookup, remote service, mutable registry or database linkage.
const CATALOG: &[Source] = &[
    email!(
        "onedrive-document-invitation",
        "Microsoft",
        "OneDrive - Document sharing invitation (EN)"
    ),
    email!(
        "google-drive-document-invitation",
        "Google",
        "Google Drive - Document sharing invitation (EN)"
    ),
    email!(
        "dropbox-file-invitation",
        "Dropbox",
        "Dropbox - File sharing invitation (EN)"
    ),
    page!(
        "onedrive-shared-document",
        "Microsoft",
        "OneDrive / SharePoint - Shared document",
        Some((".microsoft", 36, "Entra company branding")),
        Some(include_str!("starter_templates/interaction.js"))
    ),
    page!(
        "google-drive-shared-document",
        "Google",
        "Google Drive - Shared document",
        None,
        Some(include_str!("starter_templates/interaction.js"))
    ),
    page!(
        "dropbox-shared-file",
        "Dropbox",
        "Dropbox - Shared file",
        None,
        Some(include_str!("starter_templates/interaction.js"))
    ),
    page!(
        "microsoft-two-step",
        "Microsoft",
        "Microsoft Entra - Two-step sign-in",
        Some((".microsoft", 36, "Entra company branding")),
        Some(include_str!("starter_templates/interaction.js"))
    ),
    page!(
        "microsoft-two-step-side-panel",
        "Microsoft",
        "Microsoft Entra - Two-step sign-in (side panel)",
        Some((".microsoft", 36, "Entra company branding")),
        Some(include_str!("starter_templates/interaction.js"))
    ),
    page!(
        "google-workspace-two-step",
        "Google",
        "Google Workspace - Two-step sign-in",
        None,
        Some(include_str!("starter_templates/interaction.js"))
    ),
    email!(
        "microsoft-shared-document",
        "Microsoft",
        "Microsoft 365 - Shared document (EN)"
    ),
    email!(
        "microsoft-account-security",
        "Microsoft",
        "Microsoft account - Account security (EN)"
    ),
    email!(
        "microsoft-teams-invitation",
        "Microsoft",
        "Microsoft Teams - Team invitation (EN)"
    ),
    email!(
        "google-shared-document",
        "Google",
        "Google Workspace - Shared document (EN)"
    ),
    email!(
        "google-account-security",
        "Google",
        "Google Account - Account security (EN)"
    ),
    email!(
        "dropbox-shared-folder",
        "Dropbox",
        "Dropbox - Shared folder (EN)"
    ),
    email!(
        "docusign-signature-request",
        "Docusign",
        "Docusign - Signature request (EN)"
    ),
    email!(
        "adobe-document-review",
        "Adobe",
        "Adobe Acrobat - PDF review (EN)"
    ),
    email!(
        "slack-workspace-invitation",
        "Slack",
        "Slack - Workspace invitation (EN)"
    ),
    email!(
        "zoom-meeting-invitation",
        "Zoom",
        "Zoom - Meeting invitation (EN)"
    ),
    email!(
        "atlassian-jira-assignment",
        "Atlassian",
        "Jira - Task assignment (EN)"
    ),
    email!(
        "atlassian-confluence-access",
        "Atlassian",
        "Confluence - Knowledge-space access (EN)"
    ),
    email!(
        "salesforce-report-access",
        "Salesforce",
        "Salesforce - Shared report (EN)"
    ),
    email!(
        "github-repository-invitation",
        "GitHub",
        "GitHub - Repository invitation (EN)"
    ),
    email!(
        "github-account-security",
        "GitHub",
        "GitHub - Account security (EN)"
    ),
    email!(
        "paypal-payment-request",
        "PayPal",
        "PayPal - Payment request (EN)"
    ),
    email!(
        "amazon-business-invoice",
        "Amazon",
        "Amazon Business - Invoice available (EN)"
    ),
    email!(
        "apple-icloud-storage",
        "Apple",
        "iCloud - Storage review (EN)"
    ),
    email!(
        "linkedin-message-notification",
        "LinkedIn",
        "LinkedIn - Message notification (EN)"
    ),
    email!(
        "workday-employee-document",
        "Workday",
        "Workday - Employee document (EN)"
    ),
    page!(
        "microsoft-full-screen",
        "Microsoft",
        "Microsoft - Sign in (full-screen background)",
        Some((".microsoft", 36, "Entra company branding"))
    ),
    page!(
        "microsoft-side-panel",
        "Microsoft",
        "Microsoft - Sign in (side panel)",
        Some((".microsoft", 36, "Entra company branding"))
    ),
    page!(
        "google-workspace",
        "Google",
        "Google - Sign in (wide)",
        None
    ),
    page!(
        "google-account",
        "Google",
        "Google - Sign in (compact)",
        None
    ),
    page!("dropbox", "Dropbox", "Dropbox - Log in", None),
    page!("docusign", "Docusign", "Docusign - Log in", None),
    page!("adobe-acrobat", "Adobe", "Adobe - Sign in", None),
    page!(
        "slack",
        "Slack",
        "Slack - Password sign-in adaptation",
        None
    ),
    page!(
        "zoom",
        "Zoom",
        "Zoom - Sign in",
        Some((".masthead", 36, "Organization vanity-URL login"))
    ),
    page!("jira", "Atlassian", "Atlassian Cloud - Log in", None),
    page!(
        "confluence",
        "Atlassian",
        "Confluence Cloud - Atlassian sign-in",
        None
    ),
    page!(
        "salesforce",
        "Salesforce",
        "Salesforce - Log in",
        Some(("img.logo", 80, "My Domain login"))
    ),
    page!(
        "github-repository",
        "GitHub",
        "GitHub - Sign in (light)",
        None
    ),
    page!("github-account", "GitHub", "GitHub - Sign in (dark)", None),
    page!("paypal", "PayPal", "PayPal - Log in", None),
    page!(
        "amazon-business",
        "Amazon",
        "Amazon Business - Sign in",
        None
    ),
    page!(
        "apple-icloud",
        "Apple",
        "Apple Account - iCloud sign-in",
        None
    ),
    page!("linkedin", "LinkedIn", "LinkedIn - Sign in", None),
    page!(
        "workday",
        "Workday",
        "Workday - Native tenant sign-in",
        Some((".brand", 48, "Native tenant login"))
    ),
    learning!("slam", "SLAM (EN)"),
    learning!("stop-challenge-protect", "Stop, Challenge, Protect (EN)"),
    learning!(
        "recognize-resist-report-delete",
        "Recognize, Resist and Report, Delete (EN)"
    ),
];

pub(crate) fn routes() -> Router<AuthState> {
    Router::new()
        .route("/api/starter-templates/", get(list))
        .route("/api/starter-templates/{id}", get(detail))
}

async fn authorize(context: &Context) -> Result<(), Rejection> {
    let user = context.api(None).await?;
    if !user.permits("modify_system") {
        return Err(failure(
            StatusCode::FORBIDDEN,
            ErrorCode::AuthForbidden,
            "Forbidden",
        ));
    }
    Ok(())
}

async fn list(context: Context) -> Result<Response, Rejection> {
    authorize(&context).await?;
    response(
        StatusCode::OK,
        &CATALOG.iter().map(Source::summary).collect::<Vec<_>>(),
        None,
    )
}

async fn detail(context: Context, Path(id): Path<String>) -> Result<Response, Rejection> {
    authorize(&context).await?;
    let source = CATALOG.iter().find(|entry| entry.id == id).ok_or_else(|| {
        failure(
            StatusCode::NOT_FOUND,
            ErrorCode::ResourceNotFound,
            "Starter template not found",
        )
    })?;
    response(StatusCode::OK, &source.entry(), None)
}
