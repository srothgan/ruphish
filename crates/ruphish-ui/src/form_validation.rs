//! Editable-value projections into authoritative domain validators.
use ruphish_domain::validation::{Code, Report};
use serde_json::Value;

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or("")
}

/// Validate resource content without copying credentials or attachment bodies.
pub fn resource(value: &Value, family: &str) -> Report {
    use ruphish_domain::resources::Fields;
    let name = text(value, "name");
    match family {
        "groups" => Fields::Group {
            name,
            count: value["targets"].as_array().map_or(0, Vec::len),
        }
        .validate(),
        "templates" => Fields::Template {
            name,
            text: text(value, "text"),
            html: text(value, "html"),
        }
        .validate(),
        "pages" | "learning-pages" => Fields::Page { name }.validate(),
        "smtp" => Fields::Profile {
            from: text(value, "from_address"),
            host: text(value, "host"),
            headers: value["headers"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|h| (text(h, "key"), text(h, "value")))
                .collect(),
        }
        .validate(),
        _ => {
            let mut report = Report::default();
            report.reject("", Code::Invalid, "Unknown resource");
            report
        }
    }
}

/// Validate current draft readiness; this does not validate or perform a save.
pub fn campaign(value: &Value, through: usize, now: i64) -> Report {
    match serde_json::from_value::<ruphish_domain::campaign_draft::Configuration>(value.clone()) {
        Ok(configuration) => configuration.readiness(through, now),
        Err(_) => {
            let mut report = Report::default();
            report.reject("name", Code::Invalid, "Invalid draft structure");
            report
        }
    }
}

/// Add current resource availability to the same campaign readiness rules used for navigation.
pub fn campaign_with_resources(
    value: &Value,
    resources: Option<&Value>,
    through: usize,
    now: i64,
) -> Report {
    let mut report = campaign(value, through, now);
    let Some(resources) = resources else {
        return report;
    };
    let contains = |kind: &str, id: &Value| {
        resources[kind]
            .as_array()
            .into_iter()
            .flatten()
            .any(|row| row["id"] == *id)
    };
    if through >= 1 {
        if let Ok(url) = url::Url::parse(text(value, "url")) {
            let origin = url.origin().ascii_serialization();
            if !resources["domains"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|domain| domain["origin"].as_str() == Some(origin.as_str()))
            {
                report.reject("url", Code::Unavailable, "Domain unavailable");
            }
        }
        for kind in ["template", "page", "smtp"] {
            let id = &value["selection"][kind]["id"];
            if id.as_i64().is_some_and(|id| id > 0) && !contains(kind, id) {
                report.reject(
                    format!("selection.{kind}.id"),
                    Code::Unavailable,
                    "Resource unavailable",
                );
            }
        }
        let learning = &value["policy"]["learning_page_id"];
        if learning.as_i64().is_some_and(|id| id > 0) && !contains("learning_page", learning) {
            report.reject(
                "policy.learning_page_id",
                Code::Unavailable,
                "Learning page unavailable",
            );
        }
    }
    if value["selection"]["groups"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|selected| !contains("groups", &selected["id"]))
    {
        report.reject("selection.groups", Code::Unavailable, "Group unavailable");
    }
    report
}

/// Create requires credentials; blank edits retain an already saved secret.
pub fn integration(value: &Value, webhook: bool, has_secret: bool) -> Report {
    let secret_field = if webhook { "secret" } else { "password" };
    let secret = text(value, secret_field);
    let supplied = (!secret.is_empty()).then(|| secret.to_owned());
    let mut report = if webhook {
        ruphish_domain::integrations::WebhookEdit {
            name: text(value, "name").into(),
            url: text(value, "url").into(),
            secret: supplied,
            active: value["is_active"].as_bool().unwrap_or(false),
            revision: value["revision"].as_i64(),
        }
        .validation()
    } else {
        ruphish_domain::integrations::MailboxEdit {
            name: text(value, "name").into(),
            enabled: value["enabled"].as_bool().unwrap_or(false),
            host: text(value, "host").into(),
            port: value["port"]
                .as_u64()
                .and_then(|v| u16::try_from(v).ok())
                .unwrap_or(0),
            username: text(value, "username").into(),
            password: supplied,
            tls: value["tls"].as_bool().unwrap_or(false),
            folder: text(value, "folder").into(),
            restrict_domain: text(value, "restrict_domain").into(),
            delete_reported: value["delete_reported"].as_bool().unwrap_or(false),
            frequency: value["frequency"]
                .as_u64()
                .and_then(|v| u32::try_from(v).ok())
                .unwrap_or(0),
            revision: value["revision"].as_i64(),
        }
        .validation()
    };
    if !has_secret && secret.is_empty() {
        report.reject(secret_field, Code::Required, "A credential is required");
    }
    report
}
