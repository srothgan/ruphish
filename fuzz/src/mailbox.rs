//! MIME correlation: arbitrary parser input and generated reports with known identities.

use base64::{Engine, engine::general_purpose::STANDARD};
use ruphish_adapters::fuzzing::identify_mailbox;

/// Exercise decoding and identity selection without mail services or persistence.
pub fn check(data: &[u8]) {
    if data.len() > 64 * 1024 {
        return;
    }
    for restriction in ["", "example.test"] {
        if let Ok((rid, origin)) = identify_mailbox(data, restriction) {
            assert!(!rid.is_empty() && rid.len() <= 128);
            assert!(rid.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
            let url = url::Url::parse(&origin).expect("accepted origin is a URL");
            assert!(matches!(url.scheme(), "http" | "https"));
            assert!(url.username().is_empty() && url.password().is_none());
            assert!(url.query().is_none() && url.fragment().is_none());
            assert_eq!(url.path(), "/");
        }
    }

    let rid = data.iter().take(16).fold(String::from("r"), |mut s, byte| {
        use std::fmt::Write;
        write!(s, "{byte:02x}").expect("writing to a String");
        s
    });
    let link = format!("https://training.test/?rid={rid}");
    let expected = Ok((rid.clone(), "https://training.test".into()));
    let mode = data.first().copied().unwrap_or(0) % 8;
    match mode {
        0 => {
            let count = usize::from(data.get(1).copied().unwrap_or(0) % 16) + 1;
            let message = format!(
                "From: employee@example.test\r\n\r\n{}",
                format!("{link}\n").repeat(count)
            );
            assert_eq!(
                identify_mailbox(message.as_bytes(), "EXAMPLE.TEST"),
                expected
            );
        }
        1 => {
            let message = format!(
                "Subject: distinct IDs\r\n\r\n{link}\nhttps://training.test/?rid={rid}-other"
            );
            assert_eq!(
                identify_mailbox(message.as_bytes(), ""),
                Err("mailbox_ambiguous")
            );
        }
        2 => {
            let message =
                format!("Subject: distinct origins\r\n\r\n{link}\nhttp://training.test/?rid={rid}");
            assert_eq!(
                identify_mailbox(message.as_bytes(), ""),
                Err("mailbox_ambiguous")
            );
        }
        3 => {
            let original = format!("From: sender@other.test\r\n\r\n{link}");
            let message = format!(
                "From: employee@example.test\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=report.EML\r\nContent-Transfer-Encoding: base64\r\n\r\n{}",
                STANDARD.encode(original)
            );
            assert_eq!(
                identify_mailbox(message.as_bytes(), "EXAMPLE.TEST"),
                expected
            );
            assert_eq!(
                identify_mailbox(message.as_bytes(), "other.test"),
                Err("mailbox_sender_filtered")
            );
        }
        4 => {
            let count = usize::from(data.get(1).copied().unwrap_or(0) % 11);
            let mut message = format!("Subject: nested report\r\n\r\n{link}");
            for _ in 0..count {
                message = format!("Content-Type: message/rfc822\r\n\r\n{message}");
            }
            assert_eq!(
                identify_mailbox(message.as_bytes(), ""),
                if count > 8 {
                    Err("mailbox_nesting_limit")
                } else {
                    expected
                }
            );
        }
        5 => {
            let message = format!(
                "Subject: wrapped\r\n\r\nhttps://filter.test/?url=https%3A%2F%2Ftraining.test%2F%3Frid%3D{rid}"
            );
            assert_eq!(identify_mailbox(message.as_bytes(), ""), expected);
        }
        6 => {
            // Keep the hot fuzz loop representative and cheap. The exact 257-attempt
            // boundary is exercised once by the deterministic test below.
            let count = 1 + usize::from(data.get(1).copied().unwrap_or(0) % 32);
            let message = format!(
                "Subject: unrelated URLs\r\n\r\n{}",
                "https://outside.test/path\n".repeat(count)
            );
            assert_eq!(
                identify_mailbox(message.as_bytes(), ""),
                Err("mailbox_unmatched")
            );
        }
        _ => {
            let message = format!(
                "From: employee@unrelated.test\r\nContent-Type: message/rfc822\r\n\r\nFrom: employee@example.test\r\n\r\n{link}"
            );
            assert_eq!(
                identify_mailbox(message.as_bytes(), "example.test"),
                Err("mailbox_sender_filtered")
            );
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn correlation_oracles_cover_success_ambiguity_sender_and_budgets() {
        for mode in 0..8 {
            for edge in [0, 8, 9, 10, 255] {
                super::check(&[mode, edge]);
            }
        }
        let attempts = "https://outside.test/path\n".repeat(257);
        let message = format!("Subject: scan budget\r\n\r\n{attempts}");
        assert_eq!(
            ruphish_adapters::fuzzing::identify_mailbox(message.as_bytes(), ""),
            Err("mailbox_scan_limit")
        );
        super::check(include_bytes!("../seeds/mailbox_identify/forwarded.eml"));
    }
}
