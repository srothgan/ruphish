//! Email normalization and managed tracking, independently inspected as output DOMs.

use ruphish_adapters::{fuzzing, resources::MAX_DOCUMENT};
use scraper::{Html, Selector};

const TRACKER: &str = "https://training.test/track?rid=fuzz-recipient&v=1";

fn matching_images(source: &str) -> usize {
    let document = Html::parse_document(source);
    document
        .select(&Selector::parse("img[src]").expect("constant selector"))
        .filter(|image| {
            image
                .value()
                .attr("src")
                .and_then(|src| url::Url::parse(src).ok())
                .is_some_and(|mut url| {
                    url.set_fragment(None);
                    url.as_str() == TRACKER
                })
        })
        .count()
}

fn assert_policy(source: &str) {
    assert_policy_at_depth(source, 0);
    assert!(source.len() <= MAX_DOCUMENT);
}

fn assert_policy_at_depth(source: &str, depth: usize) {
    assert!(depth <= 4, "conditional nesting must stay bounded");
    let document = Html::parse_document(source);
    assert_eq!(
        document
            .select(
                &Selector::parse("script,iframe,frame,frameset,object,embed,form,base,template,meta[http-equiv='refresh' i]")
                    .expect("constant selector")
            )
            .count(),
        0
    );
    for element in document.select(&Selector::parse("*").unwrap()) {
        for (name, value) in element.value().attrs() {
            assert!(!name.starts_with("on") && name != "srcdoc");
            if matches!(name, "href" | "src" | "action" | "xlink:href") {
                let compact: String = value
                    .chars()
                    .filter(|c| !c.is_ascii_whitespace() && !c.is_ascii_control())
                    .collect();
                if let Some((scheme, _)) = compact.split_once(':') {
                    assert!(
                        !scheme.eq_ignore_ascii_case("javascript")
                            && !scheme.eq_ignore_ascii_case("vbscript")
                    );
                }
            }
            if element.value().name() == "meta" && name == "http-equiv" {
                assert!(!value.trim().eq_ignore_ascii_case("refresh"));
            }
        }
    }
    for node in document.tree.nodes() {
        if let Some(comment) = node.value().as_comment() {
            let lower = comment.to_ascii_lowercase();
            if lower.trim_start().starts_with("[if ")
                && let (Some(start), Some(end)) = (lower.find("]>"), lower.rfind("<![endif]"))
                && start + 2 <= end
            {
                assert_policy_at_depth(&comment[start + 2..end], depth + 1);
            }
        }
    }
}

/// Fuzz raw markup, then verify tracker replacement on a generated valid document.
pub fn check(data: &[u8]) {
    if data.len() > 64 * 1024 {
        return;
    }
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(normalized) = fuzzing::email_document(source) {
        assert_policy(&normalized);
    }
    let _ = fuzzing::email_without_tracker(source, TRACKER);
    for enabled in [false, true] {
        if let Ok(output) = fuzzing::email_tracked(source, TRACKER, enabled) {
            assert_policy(&output);
            assert_eq!(
                matching_images(&output),
                usize::from(enabled && !source.is_empty())
            );
        }
    }

    let copies = usize::from(data.first().copied().unwrap_or(0) % 8);
    let legacy = "<img class='fuzz-old' src='https://training.test:443/track?rid=fuzz-recipient&amp;v=1#old'>";
    let fixture = format!(
        "<p>Message</p><img id='ordinary-image' src='https://images.test/logo.png'><!--[if mso]>{legacy}<![endif]-->{}",
        legacy.repeat(copies)
    );
    for enabled in [false, true] {
        let output =
            fuzzing::email_tracked(&fixture, TRACKER, enabled).expect("generated email is valid");
        assert_eq!(matching_images(&output), usize::from(enabled));
        assert!(
            !output.contains("fuzz-old"),
            "legacy conditional and normal trackers removed"
        );
        let document = Html::parse_document(&output);
        assert_eq!(
            document
                .select(
                    &Selector::parse("img#ordinary-image[src='https://images.test/logo.png']")
                        .expect("constant selector")
                )
                .count(),
            1
        );
    }
    let rejected = [
        "<script>active()</script>",
        "<img src='x' onerror='active()'>",
        "<a href='java&#x73;cript:active()'>link</a>",
        "<!--[if mso]><iframe src='https://outside.test'></iframe><![endif]-->",
    ];
    assert!(
        fuzzing::email_document(
            rejected[usize::from(data.first().copied().unwrap_or(0)) % rejected.len()]
        )
        .is_err()
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn generated_tracking_and_rejection_oracles_cover_markup_variants() {
        for data in [
            b"".as_slice(),
            b"<p>Message",
            b"<title>unterminated",
            b"<textarea>unfinished",
            b"<script>alert(1)</script>",
            b"<form><input></form>",
        ] {
            super::check(data);
        }
        super::check(include_bytes!("../seeds/email_html/conditional.html"));
    }

    #[test]
    #[should_panic]
    fn policy_oracle_detects_active_output() {
        super::assert_policy("<script>bad()</script>");
    }
}
