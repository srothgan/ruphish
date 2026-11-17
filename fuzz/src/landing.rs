//! Separate output guarantees for authored landing and static learning pages.

use ruphish_adapters::{
    landing::{self, Mode},
    resources::sample,
};
use scraper::{Html, Selector};

fn count(document: &Html, selector: &str) -> usize {
    document
        .select(&Selector::parse(selector).expect("constant selector"))
        .count()
}

fn assert_learning(document: &landing::Document) {
    let html = Html::parse_document(&document.html);
    assert_eq!(
        count(
            &html,
            "script,iframe,object,embed,form,input,textarea,select,button,[contenteditable]"
        ),
        0
    );
    for node in html.select(&Selector::parse("*").expect("constant selector")) {
        assert!(
            node.value()
                .attrs()
                .all(|(name, _)| !name.starts_with("on"))
        );
    }
    assert!(document.csp.contains("script-src 'none'"));
    assert!(document.csp.contains("connect-src 'none'"));
}

fn assert_landing(document: &landing::Document) {
    let html = Html::parse_document(&document.html);
    assert_eq!(
        count(
            &html,
            "iframe,frame,frameset,object,embed,form[action],form[target],input[formaction],button[formaction],input[formtarget],button[formtarget]"
        ),
        0
    );
    let scripts = Selector::parse("head script").expect("constant selector");
    let first_script = html
        .select(&scripts)
        .next()
        .expect("trusted collector precedes authored scripts");
    assert!(
        first_script
            .value()
            .attr("data-ruphish-collector")
            .is_some()
    );
    assert!(
        document
            .csp
            .starts_with("sandbox allow-scripts allow-forms;")
    );
    assert!(!document.csp.contains("allow-same-origin"));
    assert!(document.csp.contains("form-action 'none'"));
}

/// Apply both page modes to arbitrary markup and verify a controlled authored example.
pub fn check(data: &[u8]) {
    if data.len() > 64 * 1024 {
        return;
    }
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    let context = sample();
    if let Ok(document) = landing::render(source, Mode::Learning) {
        assert_learning(&document);
    }
    let endpoint = if data.first().copied().unwrap_or(0) % 2 == 0 {
        None
    } else {
        Some("https://training.test/?rid=fuzz-recipient")
    };
    if let Ok(document) = landing::render(
        source,
        Mode::Landing {
            context: &context,
            endpoint,
        },
    ) {
        assert_landing(&document);
    }
    let source = "<h1>{{.FirstName}}</h1><script id='authored'>window.authoredCanary=1</script><form action='https://outside.test' target='_top'><input name='password'><button formaction='https://outside.test' formtarget='_top'>Send</button></form><iframe src='https://outside.test'></iframe>";
    let document = landing::render(
        source,
        Mode::Landing {
            context: &context,
            endpoint,
        },
    )
    .expect("controlled landing page");
    assert_landing(&document);
    let html = Html::parse_document(&document.html);
    assert_eq!(
        count(&html, "script#authored"),
        1,
        "authored landing execution is supported"
    );
    assert_eq!(count(&html, "form input[name='password']"), 1);
    let document = landing::render(source, Mode::Learning).expect("controlled learning page");
    assert_learning(&document);
    assert!(
        document.html.contains("{{.FirstName}}"),
        "learning pages do not execute templates"
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn policies_handle_fragments_raw_text_and_authored_behavior() {
        for source in [
            "",
            "<title>title",
            "<script>unterminated",
            "<form action='/next'><input name='password'>",
            "<main onclick='bad()'><h1>Lesson</h1></main>",
        ] {
            super::check(source.as_bytes());
        }
        super::check(include_bytes!("../seeds/landing_html/authored.html"));
    }

    #[test]
    #[should_panic]
    fn learning_oracle_detects_an_executable_document() {
        super::assert_learning(&ruphish_adapters::landing::Document {
            html: "<script>bad()</script>".into(),
            csp: "script-src 'none'; connect-src 'none'".into(),
            warnings: vec![],
        });
    }
}
