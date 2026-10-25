//! Final rendered HTML policy and the sole participant submission implementation.
use ruphish_application::resources::Error;
#[path = "landing_diagnostics.rs"]
mod diagnostics;

/// Safe document plus its matching response policy. Nonces are response-specific.
pub struct Document {
    /// Final HTML, after template expansion and policy enforcement.
    pub html: String,
    /// Policy that must accompany this document on every serving path.
    pub csp: String,
    /// Advisory findings in the final processed form; no entered values.
    pub warnings: Vec<ruphish_application::resources::PageWarning>,
}

/// Page-specific capabilities supplied by the caller, never by authored HTML.
pub enum Mode<'a> {
    /// Render compatible template expressions; only live pages receive a collector endpoint.
    Landing {
        /// Sample or campaign context for this rendering.
        context: &'a crate::template::Context,
        /// Trusted submission URL; absent in previews and validation.
        endpoint: Option<&'a str>,
    },
    /// Static content, without recipient expansion, controls or a collector.
    Learning,
}

/// Keep template failures distinguishable for existing public error contracts.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// Compatible template expansion failed before document processing.
    #[error("{0}")]
    Template(Error),
    /// Document preparation or content policy failed.
    #[error("{0}")]
    Document(Error),
}

impl From<RenderError> for Error {
    fn from(error: RenderError) -> Self {
        match error {
            RenderError::Template(error) | RenderError::Document(error) => error,
        }
    }
}

/// Prepare and sanitize a complete page identically for previews, validation and public serving.
pub fn render(source: &str, mode: Mode<'_>) -> Result<Document, RenderError> {
    let (source, endpoint, learning) = match mode {
        Mode::Landing { context, endpoint } => (
            std::borrow::Cow::Owned(
                crate::resources::render(source, context).map_err(RenderError::Template)?,
            ),
            endpoint,
            false,
        ),
        Mode::Learning => (std::borrow::Cow::Borrowed(source), None, true),
    };
    prepare_document(source.as_ref(), endpoint, learning).map_err(RenderError::Document)
}

fn prepare_document(
    source: &str,
    endpoint: Option<&str>,
    learning: bool,
) -> Result<Document, Error> {
    if source.len() > crate::resources::MAX_DOCUMENT {
        return Err(Error::Invalid("landing_document_too_large".into()));
    }
    let parsed = scraper::Html::parse_document(source);
    let title_selector = scraper::Selector::parse("head > title").map_err(|_| Error::Internal)?;
    let title = parsed
        .select(&title_selector)
        .next()
        .map_or_else(String::new, |node| {
            format!(
                "<title>{}</title>",
                ammonia::clean_text(&node.text().collect::<String>())
            )
        });
    let mut cleaner = ammonia::Builder::default();
    cleaner
        .add_url_schemes(&["data"])
        .rm_clean_content_tags(&["style"])
        // Fragment sanitization must not turn document metadata into visible body text.
        // The first head title is escaped and placed back into the document head below.
        .add_clean_content_tags(&["title"])
        // Full authored pages need semantic containers omitted by Ammonia's fragment defaults.
        // Attributes and descendants still pass through the same content policy.
        .add_tags(&[
            "main", "section", "style", "form", "input", "textarea", "select", "option",
            "optgroup", "button", "label", "fieldset", "legend", "datalist",
        ])
        .add_generic_attributes(&[
            "id",
            "class",
            "style",
            "title",
            "lang",
            "dir",
            "role",
            "aria-label",
            "aria-describedby",
            "aria-labelledby",
        ])
        .add_tag_attributes("form", &["id", "name", "autocomplete"])
        .add_tag_attributes(
            "input",
            &[
                "name",
                "id",
                "type",
                "value",
                "placeholder",
                "required",
                "disabled",
                "readonly",
                "checked",
                "autocomplete",
                "min",
                "max",
                "minlength",
                "maxlength",
                "pattern",
                "step",
                "form",
                "list",
            ],
        )
        .add_tag_attributes(
            "textarea",
            &[
                "name",
                "id",
                "placeholder",
                "required",
                "disabled",
                "readonly",
                "autocomplete",
                "rows",
                "cols",
                "maxlength",
                "form",
            ],
        )
        .add_tag_attributes(
            "select",
            &["name", "id", "required", "disabled", "multiple", "form"],
        )
        .add_tag_attributes("option", &["value", "selected", "disabled"])
        .add_tag_attributes("optgroup", &["label", "disabled"])
        .add_tag_attributes("button", &["type", "name", "value", "disabled", "form"])
        .add_tag_attributes("label", &["for"])
        .add_tag_attributes("fieldset", &["disabled", "name", "form"])
        .attribute_filter(|element, attribute, value| {
            if value.trim_start().to_ascii_lowercase().starts_with("data:")
                && !(element == "img" && attribute == "src")
            {
                return None;
            }
            // Network-loaded resources are excluded from this initial policy. Data images
            // are independent documents; they cannot execute in the parent document.
            if element == "img" && matches!(attribute, "src" | "srcset") {
                return (attribute == "src"
                    && [
                        "data:image/png;base64,",
                        "data:image/jpeg;base64,",
                        "data:image/gif;base64,",
                    ]
                    .iter()
                    .any(|prefix| value.starts_with(prefix)))
                .then(|| value.into());
            }
            Some(value.into())
        });
    if learning {
        cleaner.rm_tags(&[
            "form", "input", "textarea", "select", "option", "optgroup", "button", "datalist",
        ]);
        let mut attributes = cleaner.clone_tag_attributes();
        attributes.retain(|tag, _| !matches!(*tag, "textarea" | "select" | "button" | "datalist"));
        cleaner.tag_attributes(attributes);
        cleaner.add_clean_content_tags(&["textarea", "select", "button", "datalist"]);
    }
    let body = cleaner.clean(source).to_string();
    let mut warnings = diagnostics::inspect(source, &body)?;
    if learning {
        warnings.retain(|warning| {
            matches!(warning.code, "authored_execution_removed" | "html_repaired")
        });
        let selector =
            scraper::Selector::parse("form, input, textarea, select, button, [contenteditable]")
                .map_err(|_| Error::Internal)?;
        if parsed.select(&selector).next().is_some() {
            warnings.push(ruphish_application::resources::PageWarning {
                code: "learning_controls_removed",
                control: None,
            });
        }
    }
    let [html_attributes, body_attributes] = root_attributes(source)?;
    let mut bytes = [0; 24];
    getrandom::fill(&mut bytes).map_err(|_| Error::Internal)?;
    let nonce = hex::encode(bytes);
    let mut html = format!(
        "<!doctype html><html{html_attributes}><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">{title}</head><body{body_attributes}>{body}"
    );
    if let Some(endpoint) = endpoint {
        let config = serde_json::to_string(endpoint)
            .map_err(|_| Error::Internal)?
            .replace('<', "\\u003c");
        html.push_str(&format!(
            "<script nonce=\"{nonce}\">(()=>{{const endpoint={config};const messages={};{} }})();</script>",
            include_str!("landing-locales.json"),
            include_str!("landing-collector.js")
        ));
    }
    html.push_str("</body></html>");
    Ok(Document {
        html,
        warnings,
        csp: if endpoint.is_none() {
            "default-src 'none'; script-src 'none'; style-src 'unsafe-inline'; img-src data:; connect-src 'none'; form-action 'none'; base-uri 'none'; object-src 'none'; frame-src 'none'; frame-ancestors 'none'".into()
        } else {
            format!(
                "default-src 'none'; script-src 'nonce-{nonce}'; script-src-attr 'none'; style-src 'unsafe-inline'; img-src data:; connect-src 'self'; form-action 'none'; base-uri 'none'; object-src 'none'; frame-src 'none'; frame-ancestors 'none'"
            )
        },
    })
}

// Ammonia sanitizes a fragment, so document-root presentation attributes must
// be carried separately. Only non-navigation attributes can survive this path.
fn root_attributes(source: &str) -> Result<[String; 2], Error> {
    let roots = std::cell::RefCell::new([None::<String>, None]);
    let handler = lol_html::element!("html, body", |element| {
        let index = usize::from(element.tag_name() == "body");
        if roots.borrow()[index].is_some() {
            return Ok(());
        }
        let mut attributes = String::new();
        for name in ["id", "class", "style", "lang", "dir"] {
            if let Some(value) = element.get_attribute(name) {
                let value = value
                    .replace('&', "&amp;")
                    .replace('"', "&quot;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;");
                attributes.push_str(&format!(" {name}=\"{value}\""));
            }
        }
        roots.borrow_mut()[index] = Some(attributes);
        Ok(())
    });
    let settings = lol_html::Settings::new()
        .with_memory_settings(
            lol_html::MemorySettings::new()
                .with_max_allowed_memory_usage(crate::resources::MAX_DOCUMENT),
        )
        .append_element_content_handler(handler);
    let mut parser = lol_html::HtmlRewriter::new(settings, |_: &[u8]| {});
    parser
        .write(source.as_bytes())
        .map_err(|_| Error::Invalid("landing_document_invalid".into()))?;
    parser
        .end()
        .map_err(|_| Error::Invalid("landing_document_invalid".into()))?;
    Ok(roots.into_inner().map(Option::unwrap_or_default))
}

/// Redirects may be relative or HTTP(S), preserving legacy educational destinations.
pub fn redirect(source: &str, base: &str) -> Result<Option<String>, Error> {
    if source.is_empty() {
        return Ok(None);
    }
    if source.bytes().any(|b| b.is_ascii_control()) || source.contains('\\') {
        return Err(Error::Invalid("landing_redirect_invalid".into()));
    }
    let url = url::Url::parse(base)
        .and_then(|base| base.join(source))
        .map_err(|_| Error::Invalid("landing_redirect_invalid".into()))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(Error::Invalid("landing_redirect_invalid".into()));
    }
    Ok(Some(url.to_string()))
}
