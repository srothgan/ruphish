//! Confirmed resource metadata stored in the shared inventory query cache.
use ruphish_contracts::resource_details::{Content, Resource};

/// One inventory row, with an explicitly unknown count until a summary read supplies it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SummaryRow {
    /// Persistent identity.
    pub id: i64,
    /// Confirmed display name.
    pub name: String,
    /// Confirmed last modification, when known by the server.
    pub modified_date: Option<String>,
    /// Group recipients or active campaigns; never inferred from an unsaved draft.
    pub count: Option<usize>,
}

/// Apply a decoded successful save to an already loaded inventory.
/// Saves omit active-campaign counts, so retain the last known count on edits and
/// leave it unknown on creates. Complete saved group recipients determine their count.
pub fn saved(rows: &mut Vec<SummaryRow>, resource: &Resource) {
    let count = match &resource.content {
        Content::Group(group) => Some(group.targets.len()),
        _ => None,
    };
    let metadata = &resource.metadata;
    if let Some(row) = rows.iter_mut().find(|row| row.id == metadata.id) {
        row.name.clone_from(&metadata.name);
        row.modified_date.clone_from(&metadata.modified_date);
        if count.is_some() {
            row.count = count;
        }
    } else {
        rows.push(SummaryRow {
            id: metadata.id,
            name: metadata.name.clone(),
            modified_date: metadata.modified_date.clone(),
            count,
        });
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "Confirmed response fixtures and assertions"
)]
mod tests {
    use super::*;
    use ruphish_contracts::resource_details::{Kind, decode};
    use serde_json::json;

    #[test]
    fn resource_saves_preserve_usage_counts_and_do_not_invent_creation_counts() {
        for (kind, content) in [
            (
                Kind::Template,
                json!({"subject":"","text":"","html":"","envelope_sender":"","attachments":[]}),
            ),
            (Kind::Page, json!({"html":"","redirect_url":""})),
            (Kind::LearningPage, json!({"html":"","public_id":"public"})),
            (
                Kind::Profile,
                json!({"interface_type":"SMTP","host":"localhost:25","from_address":"sender@example.test","ignore_cert_errors":false,"headers":[],"has_password":true}),
            ),
        ] {
            let mut value = content;
            value["id"] = json!(7);
            value["name"] = json!("Confirmed name");
            value["modified_date"] = json!(null);
            let resource = decode(kind, value).unwrap();
            let untouched = SummaryRow {
                id: 99,
                name: "Other".into(),
                modified_date: None,
                count: Some(2),
            };
            let mut rows = vec![untouched.clone()];
            saved(&mut rows, &resource);
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0], untouched);
            assert_eq!(
                rows[1],
                SummaryRow {
                    id: 7,
                    name: "Confirmed name".into(),
                    modified_date: None,
                    count: None
                }
            );
            rows[1].count = Some(8);
            rows[1].name = "Old name".into();
            rows[1].modified_date = Some("Old date".into());
            saved(&mut rows, &resource);
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[1].name, "Confirmed name");
            assert_eq!(rows[1].modified_date, None);
            assert_eq!(rows[1].count, Some(8));
        }
    }

    #[test]
    fn group_saves_use_confirmed_recipients_including_empty_edits() {
        let mut value = json!({"id":7,"name":"Group","modified_date":"2026-10-29T10:00:00Z","targets":[{"email":"one@example.test","first_name":"","last_name":"","position":""},{"email":"two@example.test","first_name":"","last_name":"","position":""}]});
        let mut rows = vec![];
        saved(&mut rows, &decode(Kind::Group, value.clone()).unwrap());
        assert_eq!(rows[0].count, Some(2));
        assert_eq!(
            rows[0].modified_date.as_deref(),
            Some("2026-10-29T10:00:00Z")
        );
        value["targets"] = json!([]);
        value["name"] = json!("Empty group");
        saved(&mut rows, &decode(Kind::Group, value).unwrap());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Empty group");
        assert_eq!(rows[0].count, Some(0));
    }
}
