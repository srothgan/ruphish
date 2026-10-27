//! Spreadsheet output from an authorized, immutable analysis snapshot.
use ruphish_application::{reporting::*, resources::Error};

fn date(value: Option<&str>) -> String {
    use ruphish_application::datetime;
    value
        .and_then(|value| datetime::parse_stored(value).ok())
        .filter(|value| *value != datetime::ZERO_TIMESTAMP)
        .map(datetime::utc)
        .unwrap_or_default()
}

fn recipients(a: &Analysis) -> Vec<Vec<String>> {
    let mut rows = vec![
        vec![
            "Recipient ID",
            "Email",
            "Name",
            "Delivery",
            "Accepted events",
            "Open events",
            "Click events",
            "Submission events",
            "Report events",
            "Historical reported",
            "First Name",
            "Last Name",
            "Position",
            "Send Date (UTC)",
            "Modified Date (UTC)",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
    ];
    rows.extend(a.recipients.iter().map(|r| {
        let mut v = vec![
            r.id.to_string(),
            r.email.clone(),
            r.name.clone(),
            r.delivery
                .map_or("historical", |state| state.as_str())
                .into(),
        ];
        v.extend(r.counts.iter().map(ToString::to_string));
        v.push(r.historical_reported.to_string());
        v.extend([
            r.first_name.clone(),
            r.last_name.clone(),
            r.position.clone(),
            date(r.send_date.as_deref()),
            date(r.modified_date.as_deref()),
        ]);
        v
    }));
    rows
}
fn events(a: &Analysis) -> Vec<Vec<String>> {
    let mut rows = vec![
        vec![
            "Event ID",
            "Recipient ID",
            "Email",
            "Time (UTC)",
            "Event",
            "Source",
            "Password present",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
    ];
    rows.extend(a.events.iter().map(|e| {
        vec![
            e.id.to_string(),
            e.recipient_id.map(|v| v.to_string()).unwrap_or_default(),
            e.email.clone(),
            e.time.clone(),
            e.kind.label().into(),
            e.source.as_str().into(),
            e.password_present
                .map(|v| v.to_string())
                .unwrap_or_default(),
        ]
    }));
    rows
}
fn csv_text(s: &str) -> String {
    if s.trim_start_matches([' ', '\t', '\r', '\n'])
        .starts_with(['=', '+', '-', '@'])
        || s.starts_with(['\t', '\r', '\n'])
    {
        format!("'{s}")
    } else {
        s.into()
    }
}
pub(crate) fn generate(
    a: Analysis,
    f: &Filter,
    format: &str,
    scope: &str,
) -> Result<Download, Error> {
    if format == "csv" {
        let mut writer = csv::Writer::from_writer(Vec::new());
        let rows = if scope == "events" {
            events(&a)
        } else {
            recipients(&a)
        };
        for row in rows {
            writer
                .write_record(row.iter().map(|s| csv_text(s)))
                .map_err(|_| Error::Internal)?;
        }
        let bytes = writer.into_inner().map_err(|_| Error::Internal)?;
        return Ok(Download {
            content_type: "text/csv; charset=utf-8",
            extension: "csv",
            bytes,
        });
    }
    let mut book = rust_xlsxwriter::Workbook::new();
    let mut summary = vec![
        vec!["Campaign".into(), a.name.clone()],
        vec!["Campaign ID".into(), a.id.to_string()],
        vec!["Snapshot (UTC Unix seconds)".into(), a.as_of.to_string()],
        vec!["Audience".into(), a.audience.to_string()],
        vec![
            "From (inclusive UTC seconds)".into(),
            f.from.map(|v| v.to_string()).unwrap_or_default(),
        ],
        vec![
            "Until (exclusive UTC seconds)".into(),
            f.until.map(|v| v.to_string()).unwrap_or_default(),
        ],
        vec!["Recipient search".into(), f.search.clone()],
        vec![
            "Selected recipient IDs".into(),
            f.recipients
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
        ],
        vec![
            "Event filter".into(),
            f.event
                .map(ruphish_domain::events::Kind::label)
                .unwrap_or_default()
                .into(),
        ],
        vec![
            "Source filter".into(),
            f.source
                .map(ruphish_domain::reporting::Source::as_str)
                .unwrap_or_default()
                .into(),
        ],
    ];
    for (index, name) in ["Accepted", "Opened", "Clicked", "Submitted", "Reported"]
        .iter()
        .enumerate()
    {
        summary.push(vec![
            format!("{name} unique recipients"),
            a.unique[index].to_string(),
        ]);
        summary.push(vec![
            format!("{name} total events"),
            a.totals[index].to_string(),
        ]);
    }
    summary.push(vec![
        "Filtered event rows".into(),
        a.event_total.to_string(),
    ]);
    summary.push(vec![
        "Filtered recipient rows".into(),
        a.recipient_total.to_string(),
    ]);
    let mut sheets = vec![("Summary", summary)];
    if scope != "events" {
        sheets.push(("Recipients", recipients(&a)));
    }
    if scope != "recipients" {
        sheets.push(("Events", events(&a)));
    }
    for (name, rows) in sheets {
        let sheet = book
            .add_worksheet_with_constant_memory()
            .map_err(|_| Error::Internal)?;
        sheet.set_name(name).map_err(|_| Error::Internal)?;
        sheet.set_freeze_panes(1, 0).map_err(|_| Error::Internal)?;
        for (i, row) in rows.iter().enumerate() {
            for (j, text) in row.iter().enumerate() {
                let numeric = match name {
                    "Recipients" => i > 0 && (j == 0 || (4..=8).contains(&j)),
                    "Events" => i > 0 && j <= 1,
                    "Summary" => j == 1 && (matches!(i, 1..=5) || i >= 9),
                    _ => false,
                };
                if numeric && let Ok(number) = text.parse::<f64>() {
                    sheet
                        .write_number(i as u32, j as u16, number)
                        .map_err(|_| Error::Internal)?;
                } else {
                    // Explicit string cells never become spreadsheet formulas.
                    sheet
                        .write_string(i as u32, j as u16, text)
                        .map_err(|_| Error::Invalid("export_cell_too_large".into()))?;
                }
            }
        }
    }
    let bytes = book.save_to_buffer().map_err(|_| Error::Internal)?;
    Ok(Download {
        content_type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        extension: "xlsx",
        bytes,
    })
}
