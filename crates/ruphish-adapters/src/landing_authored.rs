//! Authored code executes only in an opaque-origin sandbox, never as the admin origin.
use super::{Document, diagnostics};
use lol_html::{HtmlRewriter, MemorySettings, Settings, element, html_content::ContentType};
use ruphish_application::resources::Error;
use ruphish_domain::landing::CONTENT_POLICY;

// The HTTP sandbox cannot be relaxed by authored markup. Previews additionally require
// the same iframe sandbox: the sandbox directive is not valid in meta.

pub(super) fn prepare(source: &str, endpoint: Option<&str>) -> Result<Document, Error> {
    // Parse a full document first so the trusted policy/collector always precede authored
    // scripts, even for fragments, omitted heads and malformed document containers.
    let parsed = scraper::Html::parse_document(source);
    let titles = scraper::Selector::parse("head > title").map_err(|_| Error::Internal)?;
    let title = parsed
        .select(&titles)
        .next()
        .map(|node| {
            format!(
                "<title>{}</title>",
                ammonia::clean_text(&node.text().collect::<String>())
            )
        })
        .unwrap_or_default();
    let normalized = parsed.html();
    let mut warnings = diagnostics::inspect(source, &normalized)?;
    warnings.retain(|warning| warning.code != "authored_execution_removed");
    let base_selector = scraper::Selector::parse("base[href]").map_err(|_| Error::Internal)?;
    let default_base = endpoint.unwrap_or("https://preview.invalid/");
    let base = url::Url::parse(default_base).map_err(|_| Error::Internal)?;
    let authored_base = parsed
        .select(&base_selector)
        .next()
        .and_then(|node| node.value().attr("href"));
    let base = match authored_base {
        Some(value) => base
            .join(value)
            .map_err(|_| Error::Invalid("landing_base_invalid".into()))?,
        None => base,
    };
    if !matches!(base.scheme(), "http" | "https")
        || !base.username().is_empty()
        || base.password().is_some()
    {
        return Err(Error::Invalid("landing_base_invalid".into()));
    }
    let config = serde_json::to_string(&endpoint)
        .map_err(|_| Error::Internal)?
        .replace('<', "\\u003c");
    let bootstrap = format!(
        "<meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"{CONTENT_POLICY}\"><meta name=\"referrer\" content=\"no-referrer\">{title}<base href=\"{}\"><script data-ruphish-collector>(()=>{{const endpoint={config};const messages={};{} }})();</script>",
        ammonia::clean_text(base.as_str()),
        include_str!("landing-locales.json"),
        include_str!("landing-collector.js")
    );
    let mut html = Vec::new();
    let mut oversized = false;
    let settings = Settings::new()
        .with_memory_settings(
            MemorySettings::new().with_max_allowed_memory_usage(crate::resources::MAX_DOCUMENT),
        )
        .append_element_content_handler(element!("head", |element| {
            element.prepend(&bootstrap, ContentType::Html);
            Ok(())
        }))
        .append_element_content_handler(element!(
            "base, title, iframe, frame, frameset, object, embed",
            |element| {
                element.remove();
                Ok(())
            }
        ))
        .append_element_content_handler(element!("meta[http-equiv]", |element| {
            // Authored CSP cannot silently disable managed tracking. Refresh can navigate
            // before an observation is acknowledged; redirects belong to the collector.
            element.remove();
            Ok(())
        }))
        .append_element_content_handler(element!("form", |element| {
            element.remove_attribute("action");
            element.remove_attribute("target");
            Ok(())
        }))
        .append_element_content_handler(element!("input, button", |element| {
            element.remove_attribute("formaction");
            element.remove_attribute("formtarget");
            Ok(())
        }));
    let mut rewriter = HtmlRewriter::new(settings, |chunk: &[u8]| {
        if html.len().saturating_add(chunk.len()) <= crate::resources::MAX_DOCUMENT + 64 * 1024 {
            html.extend_from_slice(chunk);
        } else {
            oversized = true;
        }
    });
    rewriter
        .write(normalized.as_bytes())
        .map_err(|_| Error::Invalid("landing_document_invalid".into()))?;
    rewriter
        .end()
        .map_err(|_| Error::Invalid("landing_document_invalid".into()))?;
    if oversized {
        return Err(Error::Invalid("landing_document_too_large".into()));
    }
    Ok(Document {
        html: String::from_utf8(html).map_err(|_| Error::Internal)?,
        csp: ruphish_domain::landing::response_policy(),
        warnings,
    })
}
