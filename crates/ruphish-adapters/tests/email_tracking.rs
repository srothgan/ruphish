//! The actual MIME composer owns tracking; authored content remains independent.
use ruphish_adapters::{imports::Tools, outbound::Policy, resources::Validate, smtp::Smtp};
use ruphish_application::{
    delivery::{MessageInput, Sender},
    resources::{Content, ContentTools, Profile, Recipient, Resource, Template, Validator},
};
use scraper::{Html, Selector};

fn message(html: &str) -> MessageInput {
    MessageInput {
        message_id: None,
        recipient: Recipient {
            email: "recipient@example.test".into(),
            first_name: "Ada".into(),
            ..Recipient::default()
        },
        template: Template {
            html: html.into(),
            text: "Plain alternative".into(),
            ..Template::default()
        },
        profile: Profile {
            from_address: "sender@example.test".into(),
            ..Profile::default()
        },
        template_id: 0,
        page_id: 0,
        url: "https://training.example.test/path?a=1&b=2".into(),
        rid: "synthetic-rid".into(),
    }
}

#[allow(clippy::unwrap_used)] // Successful composition and MIME parsing are assertions in this test helper.
fn composed_html(input: &MessageInput) -> String {
    let mail = Smtp::new(Policy::default(), String::new())
        .compose(input)
        .unwrap();
    let parsed = mail_parser::MessageParser::default()
        .parse(&mail.bytes)
        .unwrap();
    assert_eq!(parsed.body_text(0).unwrap().trim(), "Plain alternative");
    // mail-parser's body_html convenience accessor synthesizes HTML for plain text.
    parsed
        .parts
        .iter()
        .find_map(|part| match &part.body {
            mail_parser::PartType::Html(html) => Some(html.to_string()),
            _ => None,
        })
        .unwrap_or_default()
}

#[test]
fn full_documents_fragments_and_legacy_expressions_get_one_final_image() {
    for source in [
        "<p>Hello {{.FirstName}}</p>",
        "<!DOCTYPE html><HTML><HEAD><style>p{color:red}</style></HEAD><BODY><p>Hello</p></BODY></HTML>",
        "<html><body><table><tr><td>Hello",
        "<body><!-- literal </body> -->Hello</body>",
        "<body>{{.Tracker}}{{ .Tracker }}<img src='{{.TrackingURL}}'><p>Hello</p></body>",
        "{{if eq .FirstName \"Nobody\"}}{{.Tracker}}{{end}}<p>Hello</p>",
        "<body><!--[if mso]><table><tr><td>Outlook</td></tr></table><![endif]--><p>Hello</p></body>",
        "<body><!--[if mso]>{{.Tracker}}<img src='{{.TrackingURL}}'><![endif]--><p>Hello</p></body>",
        "<p>Hello</p><textarea>Unclosed raw text",
        "<BODY><p>Uppercase end</p></BODY   >",
        "<body><img src='https://training.example.test/path/track?a=1&#38;b=2&#38;rid=synthetic-rid#ignored'></body>",
        "<body><style>p:after{content:'</body>'}</style><p>Hello</p></body>",
    ] {
        let input = message(source);
        let html = composed_html(&input);
        assert_eq!(input.template.html, source);
        let doc = Html::parse_document(&html);
        let images: Vec<_> = doc.select(&Selector::parse("img").unwrap()).collect();
        assert_eq!(images.len(), 1, "{source}: {html}");
        assert_eq!(
            images[0].value().attr("src"),
            Some("https://training.example.test/path/track?a=1&b=2&rid=synthetic-rid")
        );
        assert_eq!(images[0].value().attr("style"), Some("display:none"));
        let body = doc
            .select(&Selector::parse("body").unwrap())
            .next()
            .unwrap();
        assert_eq!(
            body.last_child().unwrap().id(),
            images[0].id(),
            "tracker must be the last body child"
        );
        assert_eq!(
            html.matches("/track?").count(),
            1,
            "conditional markup must not retain another tracker"
        );
        if source.contains("<td>Outlook</td>") {
            assert!(
                html.contains("<!--[if mso]><table><tr><td>Outlook</td></tr></table><![endif]-->")
            );
        }
    }
}

#[test]
fn plain_text_has_no_html_alternative_and_ordinary_images_survive() {
    assert!(composed_html(&message("")).is_empty());
    let html = composed_html(&message(
        "<img src='cid:logo.png'><img src='https://images.example.test/logo.png'>",
    ));
    let doc = Html::parse_document(&html);
    let images: Vec<_> = doc.select(&Selector::parse("img").unwrap()).collect();
    assert_eq!(images.len(), 3);
    assert_eq!(images[0].value().attr("src"), Some("cid:logo.png"));
    assert_eq!(
        images[1].value().attr("src"),
        Some("https://images.example.test/logo.png")
    );
}

#[test]
fn resource_preview_and_campaign_review_omit_legacy_and_managed_trackers() {
    let source = "<p>Hello {{.FirstName}}</p>{{.Tracker}}<img src='{{.TrackingURL}}'><img src='cid:logo.png'>";
    let input = message(source);
    let tools = Tools {
        policy: Policy::default(),
    };
    let draft = tools.preview(source, &input.recipient).unwrap();
    let review = Smtp::new(Policy::default(), String::new())
        .review(&input)
        .unwrap();
    for html in [draft, review.html] {
        assert!(html.contains("Hello Ada"));
        assert!(!html.contains("/track"), "{html}");
        assert!(!html.contains("{{.Tracker}}"));
        assert!(html.contains("cid:logo.png"));
    }
    assert_eq!(input.template.html, source);
}

#[test]
fn save_validation_and_delivery_share_active_content_rejections() {
    for source in [
        "<script>alert(1)</script>",
        "<iframe src='https://example.test'></iframe>",
        "<form><input></form>",
        "<img src='x' onerror='alert(1)'>",
        "<meta http-equiv='refresh' content='0;url=https://example.test'>",
        "<meta http-equiv=' REFRESH ' content='0;url=https://example.test'>",
        "<a href='java&#x09;script:alert(1)'>Open</a>",
        "<!--[if mso]><img src=x onerror='alert(1)'><![endif]-->",
        "<template><script>alert(1)</script></template>",
        "<body ONLOAD='alert(1)'>Hello</body>",
    ] {
        let input = message(source);
        let resource: Resource = Resource {
            id: 0,
            name: "Example".into(),
            modified_date: String::new(),
            content: Content::Template(input.template.clone()),
        };
        assert!(
            Validate.validate(&resource.clone().into()).is_err(),
            "save accepted {source}"
        );
        assert!(
            Smtp::new(Policy::default(), String::new())
                .compose(&input)
                .is_err(),
            "send accepted {source}"
        );
    }
    let mut input =
        message("{{if eq .FirstName \"Unsafe\"}}<script>x</script>{{else}}<p>Hello</p>{{end}}");
    let resource: Resource = Resource {
        id: 0,
        name: "Example".into(),
        modified_date: String::new(),
        content: Content::Template(input.template.clone()),
    };
    Validate.validate(&resource.clone().into()).unwrap();
    input.recipient.first_name = "Unsafe".into();
    assert!(
        Smtp::new(Policy::default(), String::new())
            .compose(&input)
            .is_err()
    );
}

#[test]
fn tracker_url_is_attribute_escaped_and_oversized_rendering_fails_closed() {
    let mut input = message("<p>Hello</p>");
    input.rid = "synthetic\" onerror=\"bad&other=value".into();
    let html = composed_html(&input);
    let doc = Html::parse_document(&html);
    let selector = Selector::parse("img").unwrap();
    let image = doc.select(&selector).next().unwrap();
    assert!(image.value().attr("onerror").is_none());
    let url = url::Url::parse(image.value().attr("src").unwrap()).unwrap();
    assert_eq!(
        url.query_pairs().find(|(name, _)| name == "rid").unwrap().1,
        input.rid
    );
    input.template.html = "x".repeat(ruphish_adapters::resources::MAX_DOCUMENT + 1);
    let resource: Resource = Resource {
        id: 0,
        name: "Large".into(),
        modified_date: String::new(),
        content: Content::Template(input.template.clone()),
    };
    assert!(Validate.validate(&resource.clone().into()).is_err());
    assert!(
        Smtp::new(Policy::default(), String::new())
            .compose(&input)
            .is_err()
    );
}
