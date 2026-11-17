//! CSV parser mutations plus known rows with fuzzed quoted values and mappings.

use ruphish_adapters::{imports::Tools, outbound::Policy};
use ruphish_application::resources::{ContentTools, RowErrorKind};

/// Check row isolation and independently constructed expected column values.
pub fn check(data: &[u8]) {
    if data.len() > 64 * 1024 {
        return;
    }
    let tools = Tools {
        policy: Policy::default(),
    };
    if let Ok(report) = tools.csv(data, None) {
        assert!(report.columns.len() <= 1024);
        assert!(report.recipients.len() + report.errors.len() <= 100_000);
        assert!(report.errors.iter().all(|error| error.row >= 2));
        assert!(
            report
                .errors
                .windows(2)
                .all(|pair| pair[0].row < pair[1].row)
        );
    }
    let text = String::from_utf8_lossy(data)
        .chars()
        .take(512)
        .collect::<String>();
    let quoted = format!("\"{}\"", text.replace('"', "\"\""));
    let mode = data.first().copied().unwrap_or(0) % 3;
    let (header, first, last, mapping) = match mode {
        0 => (
            "Email,First Name,Last Name,Position",
            format!("alice@example.test,{quoted},Example,Analyst"),
            "bob@example.test,,,",
            None,
        ),
        1 => (
            "POSITION,LAST_NAME,FIRST NAME, E_M_A_I_L ",
            format!("Analyst,Example,{quoted},alice@example.test"),
            ",,,bob@example.test",
            None,
        ),
        _ => (
            "Address,Given,Family,Role",
            format!("alice@example.test,{quoted},Example,Analyst"),
            "bob@example.test,,,",
            Some([
                "Address".into(),
                "Given".into(),
                "Family".into(),
                "Role".into(),
            ]),
        ),
    };
    let source = format!("\u{feff}{header}\r\n{first}\r\nbroken@example.test\r\n{last}\r\n");
    let report = tools
        .csv(source.as_bytes(), mapping)
        .expect("generated CSV is valid");
    assert_eq!(report.recipients.len(), 2);
    assert_eq!(report.recipients[0].email, "alice@example.test");
    assert_eq!(report.recipients[0].first_name, text.trim_start());
    assert_eq!(report.recipients[0].last_name, "Example");
    assert_eq!(report.recipients[0].position, "Analyst");
    assert_eq!(report.recipients[1].email, "bob@example.test");
    assert!(report.recipients[1].first_name.is_empty());
    assert!(report.recipients[1].last_name.is_empty());
    assert!(report.recipients[1].position.is_empty());
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.errors[0].row, 3);
    assert!(matches!(report.errors[0].kind, RowErrorKind::Columns));
}

#[cfg(test)]
mod tests {
    #[test]
    fn generated_rows_preserve_quotes_newlines_and_mapping() {
        for value in [
            "\0\r\n\"quoted\"",
            "A quoted, value\nnext line",
            "Bé\rvalue",
            "",
            "   leading spaces",
        ] {
            super::check(value.as_bytes());
        }
        super::check(include_bytes!("../seeds/csv_import/rows.csv"));
    }
}
