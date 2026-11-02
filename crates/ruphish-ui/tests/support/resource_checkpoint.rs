use ruphish_contracts::resource_details::{self, Kind};
use ruphish_ui::resource_state::Confirmed;
use serde_json::{Value, json};

pub fn checkpoint(kind: Kind, fields: Value, revision: &str) -> Confirmed {
    let mut response = json!({"revision":1,"created_by":null,"edited_by":null,"edited_at":null,"modified_date":"2026-10-25T12:00:00Z"});
    if kind == Kind::Template {
        response["subject"] = json!("");
        response["envelope_sender"] = json!("");
        response["attachments"] = json!([]);
    }
    response
        .as_object_mut()
        .unwrap()
        .extend(fields.as_object().unwrap().clone());
    Confirmed::new(
        resource_details::decode(kind, response).unwrap(),
        Some(revision.into()),
    )
    .unwrap()
}
