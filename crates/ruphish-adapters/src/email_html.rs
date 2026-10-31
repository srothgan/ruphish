//! Rendered email structure and system-owned open-tracking images.
//!
//! Stored Go-template source is never parsed or rewritten here. HTML5 parsing repairs
//! fragments and optional end tags after rendering, preserving email styles/comments.
use crate::resources::{MAX_DOCUMENT, invalid};
use lol_html::{Settings, doc_comments, element, html_content::ContentType, rewrite_str};
use ruphish_application::resources::Error;
use scraper::{Html, Selector};

const MAX_CONDITIONAL_DEPTH: usize = 4;

fn selector(value: &str) -> Result<Selector, Error> {
    Selector::parse(value).map_err(|_| Error::Internal)
}

/// Validate rendered HTML and produce an explicit document/body for composition.
pub(crate) fn document(source: &str) -> Result<String, Error> {
    document_at_depth(source, 0)
}

// Outlook's downlevel-hidden conditional comments contain real email markup.
// Preserve the delimiters exactly, but apply the same rules to their contents.
fn conditional_content(comment: &str) -> Option<(&str, &str, &str)> {
    if !comment
        .trim_start()
        .to_ascii_lowercase()
        .starts_with("[if ")
    {
        return None;
    }
    let start = comment.find("]>")? + 2;
    let end = comment.to_ascii_lowercase().rfind("<![endif]")?;
    (start <= end).then(|| (&comment[..start], &comment[start..end], &comment[end..]))
}

fn document_at_depth(source: &str, depth: usize) -> Result<String, Error> {
    if depth > MAX_CONDITIONAL_DEPTH {
        return Err(invalid("Email conditional markup is nested too deeply"));
    }
    if source.is_empty() {
        return Ok(String::new());
    }
    if source.len() > MAX_DOCUMENT {
        return Err(invalid("Email HTML exceeds 8 MiB"));
    }
    let document = Html::parse_document(source);
    for node in document.tree.nodes() {
        if let Some(comment) = node.value().as_comment()
            && let Some((_, content, _)) = conditional_content(comment)
        {
            document_at_depth(content, depth + 1)?;
        }
    }
    for element in document.select(&selector("*")?) {
        let node = element.value();
        if matches!(
            node.name(),
            "script"
                | "iframe"
                | "frame"
                | "frameset"
                | "object"
                | "embed"
                | "form"
                | "base"
                | "template"
        ) || (node.name() == "meta"
            && node
                .attr("http-equiv")
                .is_some_and(|v| v.trim().eq_ignore_ascii_case("refresh")))
        {
            return Err(invalid("Email HTML contains active content"));
        }
        for (name, value) in node.attrs() {
            if name.starts_with("on") || name == "srcdoc" {
                return Err(invalid("Email HTML contains active attributes"));
            }
            if matches!(name, "href" | "src" | "action" | "xlink:href") {
                let scheme: String = value
                    .chars()
                    .filter(|c| !c.is_ascii_control() && !c.is_ascii_whitespace())
                    .collect();
                if scheme.to_ascii_lowercase().starts_with("javascript:")
                    || scheme.to_ascii_lowercase().starts_with("vbscript:")
                {
                    return Err(invalid("Email HTML contains an executable URL"));
                }
            }
        }
    }
    // The HTML5 tree builder supplies missing html/head/body and closes raw-text
    // elements. Serialization prevents a tracker from becoming text inside them.
    let html = document.html();
    if html.len() > MAX_DOCUMENT {
        return Err(invalid("Normalized email HTML exceeds 8 MiB"));
    }
    Ok(html)
}

fn same_url(value: &str, tracking_url: &str) -> Result<bool, Error> {
    // lol_html deliberately returns encoded attribute values. Decode using an HTML
    // parser so numeric entities and ampersands compare by URL, not source spelling.
    let fragment = Html::parse_fragment(&format!("<img src=\"{}\">", value.replace('"', "&quot;")));
    let images = selector("img")?;
    let decoded = fragment
        .select(&images)
        .next()
        .and_then(|e| e.value().attr("src"))
        .unwrap_or("");
    Ok(decoded == tracking_url
        || url::Url::parse(decoded)
            .ok()
            .zip(url::Url::parse(tracking_url).ok())
            .is_some_and(|(mut a, mut b)| {
                a.set_fragment(None);
                b.set_fragment(None);
                a == b
            }))
}

/// Remove legacy tracking images from rendered previews, or before final insertion.
/// Only this recipient's tracking URL is matched; ordinary email images are retained.
pub(crate) fn without_tracker(source: &str, tracking_url: &str) -> Result<String, Error> {
    without_tracker_at_depth(source, tracking_url, 0)
}

fn without_tracker_at_depth(
    source: &str,
    tracking_url: &str,
    depth: usize,
) -> Result<String, Error> {
    if depth > MAX_CONDITIONAL_DEPTH {
        return Err(invalid("Email conditional markup is nested too deeply"));
    }
    if source.len() > MAX_DOCUMENT {
        return Err(invalid("Email HTML exceeds 8 MiB"));
    }
    rewrite_str(
        source,
        Settings::new()
            .with_memory_settings(
                lol_html::MemorySettings::new().with_max_allowed_memory_usage(MAX_DOCUMENT),
            )
            .append_element_content_handler(element!("img[src]", |el| {
                if let Some(src) = el.get_attribute("src")
                    && same_url(&src, tracking_url)?
                {
                    el.remove();
                }
                Ok(())
            }))
            .append_document_content_handler(doc_comments!(|comment| {
                let text = comment.text();
                if let Some((before, content, after)) = conditional_content(&text) {
                    let cleaned = without_tracker_at_depth(content, tracking_url, depth + 1)?;
                    if cleaned != content {
                        comment.set_text(&format!("{before}{cleaned}{after}"))?;
                    }
                }
                Ok(())
            })),
    )
    .map_err(|_| invalid("Email tracking image could not be processed"))
}

/// Remove authored trackers and, when enabled, attach one managed image at the
/// end of the rendered body. An empty HTML alternative remains plain text.
pub(crate) fn tracked(
    source: &str,
    tracking_url: &str,
    track_opens: bool,
) -> Result<String, Error> {
    let html = document(source)?;
    if html.is_empty() {
        return Ok(html);
    }
    let html = without_tracker(&html, tracking_url)?;
    if !track_opens {
        return Ok(html);
    }
    let escaped = tracking_url
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;");
    let image =
        format!("<img alt=\"\" width=\"1\" height=\"1\" style=\"display:none\" src=\"{escaped}\">");
    let output = rewrite_str(
        &html,
        Settings::new()
            .with_memory_settings(
                lol_html::MemorySettings::new().with_max_allowed_memory_usage(MAX_DOCUMENT),
            )
            .append_element_content_handler(element!("body", |el| {
                el.append(&image, ContentType::Html);
                Ok(())
            })),
    )
    .map_err(|_| invalid("Email tracking image could not be inserted"))?;
    if output.len() > MAX_DOCUMENT {
        return Err(invalid("Tracked email HTML exceeds 8 MiB"));
    }
    Ok(output)
}
