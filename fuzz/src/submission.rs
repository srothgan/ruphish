//! Closed public JSON decoding, without conflating decoding with event-ID validation.

use ruphish_contracts::domains::{Submission, decode_request};
use serde_json::{Value, json};

/// Exercise raw bytes and independently constructed valid/invalid schema cases.
pub fn check(data: &[u8]) {
    if data.len() > 4096 {
        return;
    }
    if let Ok(decoded) = decode_request::<Submission>(data) {
        assert_value(&decoded, &serde_json::from_slice::<Value>(data).unwrap());
    }

    // Arbitrary identifiers belong in decoder coverage even when application validation
    // would reject them. Serialization gives quoting, Unicode and escaping coverage.
    let id = String::from_utf8_lossy(data);
    let present = data.first().is_some_and(|byte| byte & 1 != 0);
    let value = json!({"event_id": id, "password_present": present});
    let encoded = serde_json::to_vec(&value).unwrap();
    let decoded = decode_request::<Submission>(&encoded).unwrap();
    assert_eq!(decoded.event_id, id);
    assert_eq!(decoded.password_present, present);

    let quoted = serde_json::to_string(id.as_ref()).unwrap();
    for malformed in [
        format!("{{\"event_id\":{quoted},\"event_id\":{quoted},\"password_present\":false}}"),
        format!("{{\"event_id\":{quoted},\"password_present\":false,\"password_present\":true}}"),
        format!("{{\"event_id\":{quoted},\"password_present\":false,\"password\":{quoted}}}"),
        format!("[{quoted},false]"),
        format!("{{\"event_id\":{quoted}}}"),
        format!("{{\"event_id\":{quoted},\"password_present\":null}}"),
        format!("{{\"event_id\":false,\"password_present\":{present}}}"),
        format!("{} null", String::from_utf8(encoded).unwrap()),
    ] {
        assert!(decode_request::<Submission>(malformed.as_bytes()).is_err());
    }
}

fn assert_value(decoded: &Submission, value: &Value) {
    let object = value
        .as_object()
        .expect("accepted submission must be an object");
    assert_eq!(object.len(), 2);
    assert_eq!(object["event_id"].as_str(), Some(decoded.event_id.as_str()));
    assert_eq!(
        object["password_present"].as_bool(),
        Some(decoded.password_present)
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn schema_seeds_cover_success_rejection_and_escaping() {
        for data in [
            b"".as_slice(),
            b"\xff\x00\"\\",
            "café\n\"".as_bytes(),
            br#"{"event_id":"0123456789abcdef0123456789abcdef","password_present":true}"#,
            br#"{"event_id":"a","password_present":true,"password":"synthetic"}"#,
            br#"["a",true]"#,
        ] {
            super::check(data);
        }
    }
}
