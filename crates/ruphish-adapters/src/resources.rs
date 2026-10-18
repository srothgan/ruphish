//! Bounded resource parsing and validation using established format libraries.
use crate::template::{self, Context};
use base64::{Engine, engine::general_purpose::STANDARD};
use lettre::message::Mailbox;
use ruphish_application::resources::*;
use std::io::{Cursor, Read, Write};

/// Maximum authored/imported document size in bytes.
pub const MAX_DOCUMENT: usize = 8 * 1024 * 1024;
/// Maximum decoded attachment size before rendering.
pub const MAX_ATTACHMENT: usize = 16 * 1024 * 1024;
/// Total expanded bytes allowed across an Office archive.
pub const MAX_ARCHIVE_EXPANDED: usize = 64 * 1024 * 1024;
/// Concrete resource parser and template validator.
pub struct Validate;
/// Construct a safe validation failure without infrastructure details.
pub(crate) fn invalid(message: impl Into<String>) -> Error {
    Error::Invalid(message.into())
}
/// Synthetic recipient context used for validation and draft previews.
pub fn sample() -> Context {
    Context {
        from: "Training <training@example.test>".into(),
        email: "alice@example.test".into(),
        first_name: "Alice".into(),
        last_name: "Example".into(),
        position: "Analyst".into(),
        url: "https://example.test/?rid=preview-sample".into(),
        base_url: "https://example.test".into(),
        rid: "preview-sample".into(),
        tracking_url: "https://example.test/track?rid=preview-sample".into(),
        tracker: "<img src=\"https://example.test/track?rid=preview-sample\"/>".into(),
    }
}
/// Render within document limits, discarding partial output on failure.
pub fn render(source: &str, context: &Context) -> Result<String, Error> {
    if source.len() > MAX_DOCUMENT {
        return Err(invalid("Document exceeds 8 MiB"));
    }
    let output =
        template::render(source, context).map_err(|e| invalid(format!("Template error: {e}")))?;
    if output.len() > MAX_DOCUMENT {
        return Err(invalid("Rendered document exceeds 8 MiB"));
    }
    Ok(output)
}
/// Parse one mailbox and reject header delimiters.
pub fn mailbox(value: &str) -> Result<Mailbox, Error> {
    if value.contains(['\r', '\n', '\0']) {
        return Err(invalid("Invalid email address"));
    }
    value.parse().map_err(|_| {
        invalid(if value.is_empty() {
            "No email address specified"
        } else {
            "Invalid email address"
        })
    })
}
/// Accept DNS names, IPv4 and bracketed IPv6, retaining the default SMTP port.
pub fn smtp_address(value: &str) -> Result<(String, u16), Error> {
    let url = url::Url::parse(&format!("smtp://{value}"))
        .map_err(|_| invalid("Invalid SMTP host or port"))?;
    if !url.username().is_empty()
        || url.password().is_some()
        || !matches!(url.path(), "" | "/")
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid("Invalid SMTP host or port"));
    }
    let host = url
        .host_str()
        .ok_or_else(|| invalid("No SMTP Host specified"))?
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_owned();
    let port = url.port().unwrap_or(25);
    if port == 0 {
        return Err(invalid("Invalid SMTP port"));
    }
    Ok((host, port))
}
impl Validator for Validate {
    fn validate(&self, resource: &Resource) -> Result<(), Error> {
        let context = sample();
        match &resource.content {
            Content::Group(targets) => {
                for target in targets {
                    mailbox(&target.email)?;
                }
            }
            Content::Template(t) => {
                for source in [&t.subject, &t.text, &t.html] {
                    render(source, &context)?;
                }
                if !t.envelope_sender.is_empty() {
                    mailbox(&t.envelope_sender)?;
                }
                let mut total = 0usize;
                for a in &t.attachments {
                    let bytes = attachment(a, &context)?;
                    total = total.saturating_add(bytes.len());
                    if total > MAX_ARCHIVE_EXPANDED {
                        return Err(invalid("Attachments exceed 64 MiB"));
                    }
                }
            }
            Content::Page(p) => {
                render(&p.html, &context)?;
                render(&p.redirect_url, &context)?;
            }
            Content::Profile(p) => {
                mailbox(&p.from_address)?;
                smtp_address(&p.host)?;
                for header in &p.headers {
                    let key = render(&header.key, &context)?;
                    if key.is_empty() || !key.bytes().all(|c| (33..=126).contains(&c) && c != b':')
                    {
                        return Err(invalid("Invalid rendered header name"));
                    }
                    let value = render(&header.value, &context)?;
                    if value.contains(['\r', '\n', '\0']) {
                        return Err(invalid("Rendered header contains a line break"));
                    }
                }
            }
        }
        Ok(())
    }
}
/// Apply the legacy Office/text template rules without filesystem extraction.
pub fn attachment(a: &Attachment, context: &Context) -> Result<Vec<u8>, Error> {
    if a.content.len() > MAX_ATTACHMENT * 4 / 3 + 4 {
        return Err(invalid("Attachment exceeds 16 MiB"));
    }
    let bytes = STANDARD
        .decode(&a.content)
        .map_err(|_| invalid("Invalid base64 attachment"))?;
    let extension = a.name.rsplit('.').next().unwrap_or("");
    match extension {
        "txt" | "html" | "ics" => Ok(render(
            std::str::from_utf8(&bytes).map_err(|_| invalid("Text attachment is not UTF-8"))?,
            context,
        )?
        .into_bytes()),
        "docx" | "docm" | "pptx" | "xlsx" | "xlsm" => {
            let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
                .map_err(|_| invalid("Invalid Office archive"))?;
            if archive.len() > 4096 {
                return Err(invalid("Office archive has too many entries"));
            }
            let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
            let mut total = 0usize;
            let mut expanded_output = 0usize;
            for index in 0..archive.len() {
                let mut file = archive
                    .by_index(index)
                    .map_err(|_| invalid("Invalid Office archive entry"))?;
                if file.enclosed_name().is_none() || file.size() > MAX_ARCHIVE_EXPANDED as u64 {
                    return Err(invalid("Unsafe Office archive entry"));
                }
                let name = file
                    .name()
                    .map_err(|_| invalid("Invalid archive filename"))?
                    .into_owned();
                let mut body = Vec::new();
                (&mut file)
                    .take((MAX_ARCHIVE_EXPANDED - total + 1) as u64)
                    .read_to_end(&mut body)
                    .map_err(|_| invalid("Invalid compressed Office entry"))?;
                total = total.saturating_add(body.len());
                if total > MAX_ARCHIVE_EXPANDED {
                    return Err(invalid("Office archive exceeds expanded size limit"));
                }
                if name.ends_with(".xml") || name.ends_with(".rels") {
                    let source = std::str::from_utf8(&body)
                        .map_err(|_| invalid("Office XML is not UTF-8"))?;
                    body = render(&office_expressions(source), context)?.into_bytes();
                }
                expanded_output = expanded_output.saturating_add(body.len());
                if expanded_output > MAX_ARCHIVE_EXPANDED {
                    return Err(invalid("Rendered Office archive exceeds 64 MiB"));
                }
                let options = zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated);
                output
                    .start_file(name, options)
                    .map_err(|_| invalid("Cannot compose Office archive"))?;
                output
                    .write_all(&body)
                    .map_err(|_| invalid("Cannot compose Office archive"))?;
            }
            Ok(output
                .finish()
                .map_err(|_| invalid("Cannot finish Office archive"))?
                .into_inner())
        }
        _ => Ok(bytes),
    }
}

// Word escapes tokens in relationship URLs. Decode only the legacy token pattern;
// changing unrelated URL escapes would corrupt ordinary links.
fn office_expressions(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(start) = rest.find("%7b%7b.") {
        output.push_str(&rest[..start]);
        let after = &rest[start + 7..];
        if let Some(end) = after.find("%7d%7d") {
            let name = &after[..end];
            if !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphabetic()) {
                output.push_str("{{.");
                output.push_str(name);
                output.push_str("}}");
                rest = &after[end + 6..];
                continue;
            }
        }
        output.push_str("%7b%7b.");
        rest = after;
    }
    output.push_str(rest);
    output
}
