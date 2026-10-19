//! Reviewable CSV/MIME imports and bounded HTML transformation.
use crate::{
    outbound::Policy,
    resources::{MAX_DOCUMENT, invalid, mailbox, render, sample},
};
use lol_html::{MemorySettings, Settings, element, html_content::ContentType};
use ruphish_application::resources::*;

/// Concrete import and preview tools; fetching shares the SMTP destination policy.
pub struct Tools {
    /// Destination policy shared by checked outbound connections.
    pub policy: Policy,
}
#[async_trait::async_trait]
impl ContentTools for Tools {
    fn csv(&self, bytes: &[u8], columns: Option<[String; 4]>) -> Result<CsvImport, Error> {
        if bytes.len() > MAX_DOCUMENT {
            return Err(invalid("CSV exceeds 8 MiB"));
        }
        let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
        let mut reader = csv::ReaderBuilder::new().flexible(true).from_reader(bytes);
        let headers = reader
            .headers()
            .map_err(|_| invalid("Invalid CSV header"))?
            .clone();
        let normalize = |s: &str| s.trim().to_lowercase().replace([' ', '_'], "");
        let keys = columns.unwrap_or_else(|| {
            [
                "email".into(),
                "first_name".into(),
                "last_name".into(),
                "position".into(),
            ]
        });
        let indexes = keys.map(|key| headers.iter().position(|s| normalize(s) == normalize(&key)));
        if indexes[0].is_none() {
            return Err(invalid(
                "CSV requires an Email column; choose the email column in import review",
            ));
        }
        let mut result = CsvImport {
            recipients: Vec::new(),
            errors: Vec::new(),
            columns: headers.iter().map(str::to_owned).collect(),
        };
        for (index, record) in reader.records().enumerate() {
            if index >= 100_000 {
                return Err(invalid("CSV exceeds 100000 rows"));
            }
            let row = (index + 2) as u64;
            match record {
                Err(_) => result.errors.push(RowError {
                    row,
                    message: "Invalid CSV encoding or record".into(),
                    fields: vec![],
                }),
                Ok(record) => {
                    if record.len() != headers.len() {
                        result.errors.push(RowError {
                            row,
                            message: "Column count does not match the header".into(),
                            fields: record.iter().map(str::to_owned).collect(),
                        });
                        continue;
                    }
                    let value = |n: usize| {
                        indexes[n]
                            .and_then(|i| record.get(i))
                            .unwrap_or("")
                            .trim_start()
                            .to_owned()
                    };
                    let mut recipient = Recipient {
                        email: value(0),
                        first_name: value(1),
                        last_name: value(2),
                        position: value(3),
                    };
                    match mailbox(&recipient.email) {
                        Err(_) => result.errors.push(RowError {
                            row,
                            message: "Invalid email address".into(),
                            fields: record.iter().map(str::to_owned).collect(),
                        }),
                        Ok(address) => {
                            recipient.email = address.email.to_string();
                            result.recipients.push(recipient);
                        }
                    }
                }
            }
        }
        Ok(result)
    }
    fn email(&self, content: &[u8], convert_links: bool) -> Result<EmailImport, Error> {
        if content.len() > MAX_DOCUMENT {
            return Err(invalid("Email import exceeds 8 MiB"));
        }
        let message = mail_parser::MessageParser::default()
            .parse(content)
            .ok_or_else(|| invalid("Invalid email message"))?;
        let text = message.body_text(0).unwrap_or_default().into_owned();
        let mut html = message.body_html(0).unwrap_or_default().into_owned();
        if text.is_empty() && html.is_empty() {
            return Err(invalid("Email has no readable text or HTML body"));
        }
        if convert_links {
            html = rewrite(
                &html,
                vec![element!("a", |el| {
                    el.set_attribute("href", "{{.URL}}")?;
                    Ok(())
                })],
            )?;
        }
        Ok(EmailImport {
            subject: message.subject().unwrap_or("").into(),
            text,
            html,
        })
    }
    async fn site(&self, address: &str) -> Result<String, Error> {
        let (source, final_url) = self.policy.fetch(address).await?;
        site_document(&source, &final_url)
    }
    fn preview(&self, source: &str, recipient: &Recipient) -> Result<String, Error> {
        let mut ctx = sample();
        ctx.email = recipient.email.clone();
        ctx.first_name = recipient.first_name.clone();
        ctx.last_name = recipient.last_name.clone();
        ctx.position = recipient.position.clone();
        render(source, &ctx)
    }
}
/// Preserve document mode and existing base semantics while resolving imported form origins.
pub fn site_document(source: &str, address: &str) -> Result<String, Error> {
    let base = url::Url::parse(address).map_err(|_| invalid("Invalid site URL"))?;
    let mut authored_base = None;
    let mut has_head = false;
    let mut has_html = false;
    rewrite(
        source,
        vec![
            element!("base[href]", |el| {
                if authored_base.is_none() {
                    authored_base = el.get_attribute("href");
                }
                Ok(())
            }),
            element!("head", |_| {
                has_head = true;
                Ok(())
            }),
            element!("html", |_| {
                has_html = true;
                Ok(())
            }),
        ],
    )?;
    let effective = match authored_base.as_deref() {
        Some(href) => base
            .join(href)
            .map_err(|_| invalid("Invalid imported base URL"))?,
        None => base.clone(),
    };
    let base_tag = format!("<base href=\"{}\">", escape(base.as_str()));
    let insert_base = authored_base.is_none();
    let mut inserted = false;
    let rewritten = rewrite(
        source,
        vec![
            element!("base[href]", |el| {
                let href = base
                    .join(&el.get_attribute("href").unwrap_or_default())
                    .map_err(|_| "Invalid base URL")?;
                el.set_attribute("href", href.as_str())?;
                Ok(())
            }),
            element!("head", |el| {
                if insert_base {
                    el.prepend(&base_tag, ContentType::Html);
                }
                Ok(())
            }),
            element!("html", |el| {
                if insert_base && !has_head {
                    el.prepend(&format!("<head>{base_tag}</head>"), ContentType::Html);
                }
                Ok(())
            }),
            element!("*", |el| {
                if insert_base && !has_html && !has_head && !inserted {
                    el.before(&format!("<head>{base_tag}</head>"), ContentType::Html);
                    inserted = true;
                }
                Ok(())
            }),
            element!("form", |el| {
                let action = el.get_attribute("action").unwrap_or_default();
                let action = if action.is_empty() {
                    base.clone()
                } else {
                    effective
                        .join(&action)
                        .map_err(|_| "Invalid form action URL")?
                };
                el.prepend(
                    &format!(
                        "<input type=\"hidden\" name=\"__original_url\" value=\"{}\">",
                        escape(action.as_str())
                    ),
                    ContentType::Html,
                );
                Ok(())
            }),
        ],
    )?;
    if insert_base && !has_html && !has_head && !inserted {
        Ok(format!("{base_tag}{rewritten}"))
    } else {
        Ok(rewritten)
    }
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
fn rewrite<'a>(
    source: &str,
    handlers: Vec<(
        std::borrow::Cow<'a, lol_html::Selector>,
        lol_html::ElementContentHandlers<'a>,
    )>,
) -> Result<String, Error> {
    if source.len() > MAX_DOCUMENT {
        return Err(invalid("HTML exceeds 8 MiB"));
    }
    let (shielded, tokens) = shield(source)?;
    let mut settings = Settings::new()
        .with_memory_settings(MemorySettings::new().with_max_allowed_memory_usage(MAX_DOCUMENT));
    for handler in handlers {
        settings = settings.append_element_content_handler(handler);
    }
    let mut output = Vec::new();
    let mut exceeded = false;
    {
        let mut rewriter = lol_html::HtmlRewriter::new(settings, |chunk: &[u8]| {
            if output.len() + chunk.len() > MAX_DOCUMENT {
                exceeded = true;
            } else if !exceeded {
                output.extend_from_slice(chunk);
            }
        });
        rewriter
            .write(shielded.as_bytes())
            .map_err(|_| invalid("HTML could not be transformed within its limits"))?;
        rewriter
            .end()
            .map_err(|_| invalid("HTML transformation failed"))?;
    }
    if exceeded {
        return Err(invalid("Transformed HTML exceeds 8 MiB"));
    }
    let mut result =
        String::from_utf8(output).map_err(|_| invalid("Transformed HTML is not UTF-8"))?;
    for (token, source) in tokens {
        result = result.replace(&token, &source);
    }
    Ok(result)
}

// Shield complete template actions from the HTML parser, including quoted delimiters.
fn shield(source: &str) -> Result<(String, Vec<(String, String)>), Error> {
    let mut nonce = [0; 16];
    getrandom::fill(&mut nonce).map_err(|_| Error::Internal)?;
    let prefix = format!("ruphish{}token", hex::encode(nonce));
    let mut result = String::with_capacity(source.len());
    let mut tokens = Vec::new();
    let mut rest = source;
    while let Some(start) = rest.find("{{") {
        result.push_str(&rest[..start]);
        let action = &rest[start..];
        let bytes = action.as_bytes();
        let mut i = 2;
        let mut quote = 0;
        let mut escaped = false;
        while i + 1 < bytes.len() {
            let b = bytes[i];
            if quote != 0 {
                if escaped {
                    escaped = false;
                } else if b == b'\\' && quote != b'`' {
                    escaped = true;
                } else if b == quote {
                    quote = 0;
                }
            } else if matches!(b, b'"' | b'\'' | b'`') {
                quote = b;
            } else if b == b'}' && bytes[i + 1] == b'}' {
                break;
            }
            i += 1;
        }
        if i + 1 >= bytes.len() {
            return Err(invalid("Unclosed template expression"));
        }
        let end = i + 2;
        let token = format!("{prefix}{}end", tokens.len());
        result.push_str(&token);
        tokens.push((token, action[..end].to_owned()));
        rest = &action[end..];
    }
    result.push_str(rest);
    Ok((result, tokens))
}

/// Remove native field serialization without modifying the author's stored source.
/// Served previews additionally use a sandbox CSP. This is not a script sanitizer;
/// public interaction rendering must supply its own controlled collector boundary.
pub fn landing_html(page: &Page) -> Result<String, Error> {
    rewrite(
        &page.html,
        vec![
            element!("form", |el| {
                el.set_attribute("action", "")?;
                Ok(())
            }),
            element!("input, textarea, select, button, object", |el| {
                el.remove_attribute("name");
                el.remove_attribute("dirname");
                el.remove_attribute("formaction");
                Ok(())
            }),
        ],
    )
}
