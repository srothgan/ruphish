//! Real CSV/MIME/Office/HTML transformations with independent content expectations.
use base64::{Engine, engine::general_purpose::STANDARD};
use ruphish_adapters::{
    imports::Tools,
    outbound::Policy,
    resources::{attachment, sample},
};
use ruphish_application::resources::{Attachment, ContentTools, Page};
use std::io::{Cursor, Read, Write};

#[test]
fn page_titles_remain_escaped_head_metadata_in_preview_and_live_modes() {
    use ruphish_adapters::landing::{Mode, render};
    let context = sample();
    for (source, expected_title) in [
        (
            "<!doctype html><html><head><title>TITLE-ONLY &amp; &lt;/title&gt;&lt;img src=x onerror=bad()&gt;</title><title>DISCARDED-TITLE</title></head><body><main><section><h1>Visible lesson</h1></section></main><title>BODY-TITLE</title><script>bad()</script></body></html>",
            "TITLE-ONLY & </title><img src=x onerror=bad()>",
        ),
        ("<!doctype html><title>TITLE-ONLY", "TITLE-ONLY"),
        ("<main><h1>Visible lesson</h1></main>", ""),
    ] {
        for mode in [
            Mode::Learning,
            Mode::Landing {
                context: &context,
                endpoint: None,
            },
            Mode::Landing {
                context: &context,
                endpoint: Some("https://example.test/?rid=test"),
            },
        ] {
            let output = render(source, mode).unwrap();
            let document = scraper::Html::parse_document(&output.html);
            let titles = scraper::Selector::parse("head > title").unwrap();
            let title = document
                .select(&titles)
                .next()
                .map(|n| n.text().collect::<String>())
                .unwrap_or_default();
            assert_eq!(title, expected_title);
            let body = scraper::Selector::parse("body").unwrap();
            let text = document
                .select(&body)
                .next()
                .unwrap()
                .text()
                .collect::<String>();
            for hidden in ["TITLE-ONLY", "DISCARDED-TITLE", "BODY-TITLE"] {
                assert!(!text.contains(hidden), "metadata leaked: {hidden}");
            }
            let unsafe_nodes = scraper::Selector::parse("body title, img, [onerror]").unwrap();
            assert_eq!(document.select(&unsafe_nodes).count(), 0);
        }
    }
}

#[test]
fn semantic_page_layout_preserves_authored_landing_behavior_and_restricts_learning_content() {
    let source = r##"<!doctype html><html><body><main class="lesson" onclick="unsafe()"><nav aria-label="Lesson"><a href="#check">Check</a></nav><section id="check" aria-labelledby="heading" onmouseover="unsafe()"><h2 id="heading">Verify independently</h2><p>Use a known contact.</p><script>unsafe()</script><iframe src="https://outside.invalid"></iframe><img src="https://outside.invalid/pixel" onerror="unsafe()"><a href="javascript:unsafe()">Unsafe destination</a><form action="https://outside.invalid"><input name="password"><button>Send</button></form></section></main></body></html>"##;
    for learning in [false, true] {
        let rendered = if learning {
            ruphish_adapters::landing::render(source, ruphish_adapters::landing::Mode::Learning)
                .unwrap()
        } else {
            ruphish_adapters::landing::render(
                source,
                ruphish_adapters::landing::Mode::Landing {
                    context: &sample(),
                    endpoint: None,
                },
            )
            .unwrap()
        };
        let document = scraper::Html::parse_document(&rendered.html);
        let heading = scraper::Selector::parse("main.lesson > section#check > h2#heading").unwrap();
        assert_eq!(
            document
                .select(&heading)
                .next()
                .unwrap()
                .text()
                .collect::<String>(),
            "Verify independently"
        );
        let navigation =
            scraper::Selector::parse("main > nav[aria-label] > a[href='#check']").unwrap();
        assert_eq!(document.select(&navigation).count(), 1);
        let section = scraper::Selector::parse("section[aria-labelledby='heading']").unwrap();
        assert_eq!(document.select(&section).count(), 1);
        for forbidden in [
            "onclick",
            "onmouseover",
            "onerror",
            "javascript:",
            "unsafe()",
            "<script",
            "<iframe",
            "outside.invalid",
        ] {
            if learning || forbidden == "<iframe" {
                assert!(!rendered.html.contains(forbidden), "{forbidden}");
            } else {
                assert!(
                    rendered.html.contains(forbidden),
                    "authored landing behavior: {forbidden}"
                );
            }
        }
        let controls = scraper::Selector::parse("form,input,button").unwrap();
        assert_eq!(
            document.select(&controls).count(),
            if learning { 0 } else { 3 }
        );
        if learning {
            assert!(rendered.csp.contains("connect-src 'none'"));
        } else {
            assert!(
                rendered
                    .csp
                    .starts_with("sandbox allow-scripts allow-forms;")
            );
            assert!(!rendered.csp.contains("allow-same-origin"));
        }
    }
}

#[test]
fn learning_pages_are_static_and_preserve_explanations_without_collectors_or_form_checks() {
    let source = r#"<!doctype html><html lang="de"><body class="lesson"><style>.lesson{color:navy}</style><h1>Recognize suspicious messages</h1><p>{{.FirstName}} stays literal</p><a href="https://example.test/help">Help</a><form action="https://outside.invalid"><fieldset><legend>Recognize the warning</legend><label for="password">Keep this explanation</label><input id="password" name="password" type="password"></fieldset><button>Discard button text</button></form><textarea>Discard private text</textarea><select><option>Discard choice</option></select><datalist><option>Discard suggestion</option></datalist><div contenteditable>Editable</div><script>unsafe()</script><img src="https://outside.invalid/pixel"></body></html>"#;
    let document =
        ruphish_adapters::landing::render(source, ruphish_adapters::landing::Mode::Learning)
            .unwrap();
    assert!(document.html.contains("Recognize suspicious messages"));
    assert!(document.html.contains("{{.FirstName}} stays literal"));
    assert!(document.html.contains("https://example.test/help"));
    assert!(document.html.contains("class=\"lesson\""));
    assert!(document.html.contains("lang=\"de\""));
    let parsed = scraper::Html::parse_document(&document.html);
    for (selector, expected) in [
        ("fieldset > legend", "Recognize the warning"),
        ("fieldset > label[for='password']", "Keep this explanation"),
    ] {
        let selector = scraper::Selector::parse(selector).unwrap();
        assert_eq!(
            parsed
                .select(&selector)
                .next()
                .unwrap()
                .text()
                .collect::<String>(),
            expected
        );
    }
    for forbidden in [
        "<script",
        "<form",
        "<input",
        "<textarea",
        "<button",
        "<select",
        "<datalist",
        "Discard button text",
        "Discard private text",
        "Discard choice",
        "Discard suggestion",
        "contenteditable",
        "outside.invalid",
        "event_id",
        "password_present",
    ] {
        assert!(
            !document.html.contains(forbidden),
            "{forbidden}: {}",
            document.html
        );
    }
    assert!(document.csp.contains("script-src 'none'"));
    assert!(document.csp.contains("connect-src 'none'"));
    assert!(document.csp.contains("form-action 'none'"));
    let codes: Vec<_> = document
        .warnings
        .iter()
        .map(|warning| warning.code)
        .collect();
    assert!(codes.contains(&"learning_controls_removed"));
    assert!(codes.contains(&"authored_execution_removed"));
    assert!(
        !codes
            .iter()
            .any(|code| code.starts_with("password_") || code.starts_with("form_"))
    );
    assert!(
        ruphish_adapters::landing::render("<!doctype html><html><head><title>Explanation</title></head><body><h1>Explanation</h1></body></html>", ruphish_adapters::landing::Mode::Learning)
            .unwrap()
            .warnings
            .is_empty()
    );
}

#[test]
fn original_go_attachment_files_match_independent_expected_content() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/attachments");
    let cases: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("cases.json")).unwrap()).unwrap();
    let context = ruphish_adapters::template::Context {
        from: "From Address".into(),
        email: "foo@bar.com".into(),
        first_name: "Foo".into(),
        last_name: "Bar".into(),
        position: "Space Janitor".into(),
        base_url: "http://testurl.com".into(),
        url: "http://testurl.com/?rid=1234567".into(),
        tracking_url: "http://testurl.local/track?rid=1234567".into(),
        tracker: "<img alt='' style='display: none' src='http://testurl.local/track?rid=1234567'/>"
            .into(),
        rid: "1234567".into(),
    };
    for case in cases.as_array().unwrap() {
        let name = case["attachment_name"].as_str().unwrap();
        let source = std::fs::read(root.join(case["source"].as_str().unwrap())).unwrap();
        let expected = std::fs::read(root.join(case["expected"].as_str().unwrap())).unwrap();
        let encoded = case["base64"].as_bool().unwrap();
        let decode = |bytes: Vec<u8>| {
            if encoded {
                STANDARD
                    .decode(
                        bytes
                            .into_iter()
                            .filter(|b| !b.is_ascii_whitespace())
                            .collect::<Vec<_>>(),
                    )
                    .unwrap()
            } else {
                bytes
            }
        };
        let actual = attachment(
            &Attachment {
                name: name.into(),
                mime_type: String::new(),
                content: STANDARD.encode(decode(source)),
            },
            &context,
        )
        .unwrap();
        let expected = decode(expected);
        if matches!(
            name.rsplit('.').next(),
            Some("docx" | "docm" | "pptx" | "xlsx")
        ) {
            // Compression, ZIP headers and member ordering are not document content.
            assert_eq!(
                archive_contents(&actual),
                archive_contents(&expected),
                "{name}"
            );
        } else {
            assert_eq!(actual, expected, "{name}");
        }
    }
}

#[test]
fn inherited_email_generation_cases_decode_to_the_expected_mime_and_envelope() {
    use ruphish_application::{
        delivery::{MessageInput, Sender},
        resources::{Profile, Recipient, Template},
    };
    let sender = ruphish_adapters::smtp::Smtp::new(Policy::default(), "test@test.com".into());
    for variant in ["normal", "empty-subject", "url"] {
        let message = MessageInput {
            message_id: None,
            recipient: Recipient {
                email: "firstlast@example.com".into(),
                first_name: "First".into(),
                last_name: "Last".into(),
                position: String::new(),
            },
            template: Template {
                subject: if variant == "empty-subject" {
                    ""
                } else if variant == "url" {
                    "{{.URL}}"
                } else {
                    "{{.FirstName}} - Subject"
                }
                .into(),
                text: if variant == "url" {
                    "{{.URL}}"
                } else {
                    "{{.Email}} - Text"
                }
                .into(),
                html: if variant == "url" {
                    "{{.URL}}"
                } else {
                    "{{.Email}} - HTML"
                }
                .into(),
                ..Template::default()
            },
            profile: Profile {
                from_address: "from@example.com".into(),
                ..Profile::default()
            },
            template_id: 1,
            page_id: 0,
            url: "http://127.0.0.1/{{.Email}}".into(),
            rid: "preview--foobar".into(),
        };
        let prepared = sender.compose(&message).unwrap();
        assert_eq!(prepared.from, "from@example.com");
        assert_eq!(prepared.to, "firstlast@example.com");
        let parsed = mail_parser::MessageParser::default()
            .parse(&prepared.bytes)
            .unwrap();
        let url = "http://127.0.0.1/firstlast@example.com?rid=preview--foobar";
        assert_eq!(
            parsed.subject().unwrap_or(""),
            if variant == "empty-subject" {
                ""
            } else if variant == "url" {
                url
            } else {
                "First - Subject"
            }
        );
        assert_eq!(
            parsed.body_text(0).unwrap().trim_end_matches(['\r', '\n']),
            if variant == "url" {
                url
            } else {
                "firstlast@example.com - Text"
            }
        );
        let html = parsed.body_html(0).unwrap();
        let document = scraper::Html::parse_document(&html);
        let body_selector = scraper::Selector::parse("body").unwrap();
        let body = document.select(&body_selector).next().unwrap();
        assert_eq!(
            body.text().collect::<String>(),
            if variant == "url" {
                url
            } else {
                "firstlast@example.com - HTML"
            }
        );
        assert_eq!(
            document
                .select(&scraper::Selector::parse("img").unwrap())
                .count(),
            1
        );
        assert_eq!(parsed.header_raw("X-Mailer").unwrap().trim(), "gophish");
        assert_eq!(
            parsed.header_raw("X-Gophish-Contact").unwrap().trim(),
            "test@test.com"
        );
    }
}

#[test]
fn original_template_context_preserves_pathless_urls_and_query_order() {
    use ruphish_application::{
        delivery::MessageInput,
        resources::{Profile, Recipient, Template},
    };
    let mut message = MessageInput {
        message_id: None,
        recipient: Recipient {
            first_name: "Foo".into(),
            last_name: "Bar".into(),
            email: "foo@bar.com".into(),
            position: String::new(),
        },
        profile: Profile {
            from_address: "From Address <from@example.com>".into(),
            ..Profile::default()
        },
        template: Template::default(),
        template_id: 1,
        page_id: 0,
        url: "http://example.com".into(),
        rid: "1234567".into(),
    };
    let ctx = ruphish_adapters::smtp::context(&message).unwrap();
    assert_eq!(ctx.from, "From Address");
    assert_eq!(ctx.url, "http://example.com?rid=1234567");
    assert_eq!(ctx.base_url, "http://example.com");
    assert_eq!(ctx.tracking_url, "http://example.com/track?rid=1234567");
    assert_eq!(
        ctx.tracker,
        "<img alt='' style='display: none' src='http://example.com/track?rid=1234567'/>"
    );
    assert_eq!(
        (
            ctx.first_name.as_str(),
            ctx.last_name.as_str(),
            ctx.email.as_str(),
            ctx.rid.as_str()
        ),
        ("Foo", "Bar", "foo@bar.com", "1234567")
    );
    message.url = "https://example.test/path?z=last&rid=old&a=first#section".into();
    let ctx = ruphish_adapters::smtp::context(&message).unwrap();
    assert_eq!(
        ctx.url,
        "https://example.test/path?a=first&rid=1234567&z=last#section"
    );
    assert_eq!(ctx.base_url, "https://example.test#section");
}

#[allow(clippy::unwrap_used)] // A corrupt independent fixture must fail this test helper.
fn archive_contents(bytes: &[u8]) -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut result = std::collections::BTreeMap::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).unwrap();
        let name = file.name().unwrap().into_owned();
        let mut body = Vec::new();
        file.read_to_end(&mut body).unwrap();
        assert!(result.insert(name, body).is_none());
    }
    result
}

#[test]
fn csv_reports_short_and_invalid_rows_without_inheriting_previous_values() {
    let tools = Tools {
        policy: Policy::default(),
    };
    let report=tools.csv(b"\xef\xbb\xbfEmail,First Name,Last Name,Position\r\nalice@example.test,Alice,Example,Analyst\r\nbad@example.test\r\ninvalid,No,Address,Role\r\nbob@example.test,,,\r\n",None).unwrap();
    assert_eq!(report.recipients.len(), 2);
    assert_eq!(report.recipients[1].email, "bob@example.test");
    assert_eq!(report.recipients[1].first_name, "");
    assert_eq!(report.recipients[1].position, "");
    assert_eq!(
        report.errors.iter().map(|e| e.row).collect::<Vec<_>>(),
        vec![3, 4]
    );
    let quoted = tools
        .csv(
            b"Email,First Name\n\"quoted@example.test\",\"First\nSecond\"\n",
            None,
        )
        .unwrap();
    assert_eq!(quoted.recipients[0].first_name, "First\nSecond");
    // Original util.TestParseCSVEmail, including removal of mailbox brackets.
    let legacy = tools
        .csv(
            b"First Name,Last Name,Email\nJohn,Doe,<johndoe@example.com>",
            None,
        )
        .unwrap();
    assert_eq!(
        legacy.recipients,
        vec![ruphish_application::resources::Recipient {
            first_name: "John".into(),
            last_name: "Doe".into(),
            email: "johndoe@example.com".into(),
            position: String::new(),
        }]
    );
}
#[test]
fn mime_decodes_alternatives_and_link_conversion_preserves_expressions() {
    let raw=b"From: training@example.test\r\nSubject: =?utf-8?Q?Caf=C3=A9?=\r\nMIME-Version: 1.0\r\nContent-Type: multipart/alternative; boundary=parts\r\n\r\n--parts\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nHello {{.FirstName}}\r\n--parts\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<a href=\"https://old.example.test\">Hello {{.FirstName}}</a>\r\n--parts--\r\n";
    let result = Tools {
        policy: Policy::default(),
    }
    .email(raw, true)
    .unwrap();
    assert_eq!(result.subject, "Caf\u{e9}");
    assert!(result.text.contains("Hello {{.FirstName}}"));
    assert!(result.html.contains("href=\"{{.URL}}\""));
    assert!(result.html.contains("{{.FirstName}}"));
    assert!(
        Tools {
            policy: Policy::default()
        }
        .email(b"not an email", false)
        .is_err()
    );
}
#[test]
fn final_page_policy_preserves_names_and_rendered_expressions_without_changing_authored_source() {
    let authored = "<form action='/outside'><input name='user'><input type='password' name='secret'><p>{{printf \"<b>%s</b>\" .FirstName}}</p></form>";
    let page = Page {
        html: authored.into(),
        ..Default::default()
    };
    let html = ruphish_adapters::landing::render(
        &page.html,
        ruphish_adapters::landing::Mode::Landing {
            context: &sample(),
            endpoint: None,
        },
    )
    .unwrap()
    .html;
    assert!(html.contains("name=\"user\""));
    assert!(html.contains("name=\"secret\""));
    assert!(html.contains("<b>Alice</b>"));
    assert!(!html.contains("action="));
    assert_eq!(page.html, authored);
}

#[test]
fn processed_pages_preserve_controls_but_remove_native_actions_and_embeds() {
    let source = "<form id='training' action='/outside'><input name='username'><input type='password' name='password'><textarea name='secret' dirname='direction'></textarea><button formaction='https://outside.test'>Submit</button></form><input form='training' name='associated'><object name='embedded'></object>";
    let page = Page {
        html: source.into(),
        ..Default::default()
    };
    let html = ruphish_adapters::landing::render(
        &page.html,
        ruphish_adapters::landing::Mode::Landing {
            context: &sample(),
            endpoint: None,
        },
    )
    .unwrap()
    .html;
    assert!(html.contains("name=\"associated\""));
    assert!(html.contains("form=\"training\""));
    assert!(html.contains("dirname="));
    assert!(!html.contains("formaction="));
    assert!(!html.contains("<object"));
    assert_eq!(page.html, source);
}

#[test]
fn imported_documents_preserve_doctype_and_existing_base_and_resolve_form_origins() {
    use ruphish_adapters::imports::site_document;
    let source = "<!doctype html><html><head><title>Example</title></head><body><form action='../submit'><input name='user'></form></body></html>";
    let result = site_document(source, "https://example.test/forms/start").unwrap();
    assert!(result.starts_with(
        "<!doctype html><html><head><base href=\"https://example.test/forms/start\">"
    ));
    assert!(result.contains("name=\"__original_url\" value=\"https://example.test/submit\""));
    let existing = "<!doctype html><html><head><base href='/different/'></head><body><form action='submit'><p>{{.FirstName}}</p></form></body></html>";
    let result = site_document(existing, "https://example.test/forms/start").unwrap();
    assert_eq!(result.matches("<base ").count(), 1);
    assert!(result.contains("href=\"https://example.test/different/\""));
    assert!(result.contains("value=\"https://example.test/different/submit\""));
    assert!(result.contains("{{.FirstName}}"));
    let fragment = site_document(
        "<form><input name='x'></form>",
        "https://example.test/final",
    )
    .unwrap();
    assert!(fragment.starts_with("<head><base href=\"https://example.test/final\"></head>"));
    assert!(fragment.contains("value=\"https://example.test/final\""));
}
#[test]
fn office_rendering_preserves_binary_entries_and_unrelated_url_escapes() {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, bytes) in [
        (
            "word/document.xml",
            b"<t>Hello {{.FirstName}}</t>".as_slice(),
        ),
        (
            "word/_rels/document.xml.rels",
            b"<r target=\"https://example.test/a%20b/%7b%7b.Email%7d%7d\"/>".as_slice(),
        ),
        ("word/vbaProject.bin", b"\x00\xffmacro-bytes\x00".as_slice()),
    ] {
        writer.start_file(name, options).unwrap();
        writer.write_all(bytes).unwrap();
    }
    let source = writer.finish().unwrap().into_inner();
    let result = attachment(
        &Attachment {
            name: "training.docm".into(),
            mime_type: String::new(),
            content: STANDARD.encode(source),
        },
        &sample(),
    )
    .unwrap();
    let mut zip = zip::ZipArchive::new(Cursor::new(result)).unwrap();
    let mut text = String::new();
    zip.by_name("word/document.xml")
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    assert_eq!(text, "<t>Hello Alice</t>");
    text.clear();
    zip.by_name("word/_rels/document.xml.rels")
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    assert_eq!(
        text,
        "<r target=\"https://example.test/a%20b/alice@example.test\"/>"
    );
    let mut binary = Vec::new();
    zip.by_name("word/vbaProject.bin")
        .unwrap()
        .read_to_end(&mut binary)
        .unwrap();
    assert_eq!(binary, b"\x00\xffmacro-bytes\x00");
}
#[test]
fn attachment_failures_do_not_produce_partial_output() {
    for (name, bytes) in [
        ("bad.txt", b"Prefix {{.Unknown}}".as_slice()),
        ("bad.docx", b"not a zip".as_slice()),
        ("bad.txt", b"\xff".as_slice()),
    ] {
        assert!(
            attachment(
                &Attachment {
                    name: name.into(),
                    content: STANDARD.encode(bytes),
                    mime_type: String::new()
                },
                &sample()
            )
            .is_err()
        );
    }
    assert!(
        attachment(
            &Attachment {
                name: "bad.pdf".into(),
                content: "not base64!".into(),
                mime_type: String::new()
            },
            &sample()
        )
        .is_err()
    );
}
#[test]
fn destination_policy_cannot_be_bypassed_with_mapped_ipv4() {
    let policy = Policy::default();
    for ip in [
        "127.0.0.1",
        "10.0.0.1",
        "172.16.0.1",
        "192.168.0.1",
        "::1",
        "fc00::1",
        "::ffff:127.0.0.1",
        "169.254.169.254",
        "::ffff:169.254.169.254",
        "fe80::1",
    ] {
        assert!(!policy.permits(ip.parse().unwrap()));
    }
    assert!(!policy.permits("127.0.0.1".parse().unwrap()));
    let restricted = Policy::new(&["10.1.0.0/16".into()]).unwrap();
    assert!(restricted.permits("10.1.2.3".parse().unwrap()));
    assert!(!restricted.permits("::ffff:10.2.2.3".parse().unwrap()));
    assert!(!restricted.permits("127.0.0.1".parse().unwrap()));
    assert!(Policy::new(&["broken".into()]).is_err());
    // All original dialer table guarantees, without making network requests.
    assert!(policy.permits("1.1.1.1".parse().unwrap()));
    let custom = Policy::new(&["127.0.0.1/32".into()]).unwrap();
    assert!(custom.permits("127.0.0.1".parse().unwrap()));
    assert!(!custom.permits("192.168.1.2".parse().unwrap()));
    let single = Policy::new(&["127.0.0.1".into(), "::1".into()]).unwrap();
    assert!(single.permits("127.0.0.1".parse().unwrap()));
    assert!(single.permits("::1".parse().unwrap()));
    assert!(!single.permits("192.168.1.2".parse().unwrap()));
}

#[test]
fn campaign_mime_preserves_address_headers_and_inline_attachment_contracts() {
    use mail_parser::MimeHeaders;
    use ruphish_application::{
        delivery::{MessageInput, Sender},
        resources::{Header, Profile, Recipient, Template},
    };
    let sender =
        ruphish_adapters::smtp::Smtp::new(Default::default(), "contact@example.test".into());
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    archive
        .start_file(
            "word/document.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    archive.write_all(b"<p>Training</p>").unwrap();
    let office = archive.finish().unwrap().into_inner();
    for (name, inline) in [
        (".png", true),
        (".jpg", true),
        (".jpeg", true),
        (".gif", true),
        (".docx", false),
        (".txt", false),
        (".jar", false),
        (".exe", false),
        ("", false),
        ("png", false),
        ("photo.PNG", true),
    ] {
        let payload = if name == ".docx" {
            office.clone()
        } else {
            b"Training attachment".to_vec()
        };
        let mut request = MessageInput {
            message_id: None,
            recipient: Recipient {
                email: "john@example.test".into(),
                first_name: "John".into(),
                last_name: "Doe".into(),
                ..Default::default()
            },
            template: Template {
                envelope_sender: "visible@example.test".into(),
                subject: "{{.RId}} - Subject".into(),
                text: "{{.RId}} - Text".into(),
                html: "{{.RId}} - HTML".into(),
                attachments: vec![Attachment {
                    name: name.into(),
                    mime_type: "application/octet-stream".into(),
                    content: STANDARD.encode(&payload),
                }],
            },
            template_id: 1,
            page_id: 0,
            profile: Profile {
                from_address: "transport@example.test".into(),
                ..Default::default()
            },
            url: "https://example.test".into(),
            rid: "fixture-rid".into(),
        };
        for named in [true, false] {
            if !named {
                request.recipient.first_name.clear();
                request.recipient.last_name.clear();
                request.profile.headers = vec![
                    Header {
                        key: "X-Mailer".into(),
                        value: String::new(),
                    },
                    Header {
                        key: "X-Gophish-Contact".into(),
                        value: String::new(),
                    },
                    Header {
                        key: "From".into(),
                        value: "Training <override@example.test>".into(),
                    },
                ];
            }
            let mail = sender.compose(&request).unwrap();
            assert_eq!(mail.from, "transport@example.test");
            assert_eq!(mail.to, "john@example.test");
            let parsed = mail_parser::MessageParser::default()
                .parse(&mail.bytes)
                .unwrap();
            assert!(parsed.header_raw("From").unwrap().contains(if named {
                "visible@example.test"
            } else {
                "override@example.test"
            }));
            let review = sender.review(&request).unwrap();
            assert_eq!(review.transport_sender, "transport@example.test");
            assert!(review.visible_sender.contains(if named {
                "visible@example.test"
            } else {
                "Training <override@example.test>"
            }));
            assert_eq!(parsed.subject().unwrap(), "fixture-rid - Subject");
            assert_eq!(parsed.body_text(0).unwrap().trim(), "fixture-rid - Text");
            let html = parsed.body_html(0).unwrap();
            let document = scraper::Html::parse_document(&html);
            let selector = scraper::Selector::parse("body").unwrap();
            assert_eq!(
                document
                    .select(&selector)
                    .next()
                    .unwrap()
                    .text()
                    .collect::<String>(),
                "fixture-rid - HTML"
            );
            assert_eq!(parsed.header_raw("To").unwrap().contains("John Doe"), named);
            assert_eq!(
                parsed.header_raw("X-Mailer").unwrap().trim(),
                if named { "gophish" } else { "" }
            );
            assert_eq!(
                parsed.header_raw("X-Gophish-Contact").unwrap().trim(),
                if named { "contact@example.test" } else { "" }
            );
            let part = parsed.attachments().next().unwrap();
            assert_eq!(
                part.content_disposition().unwrap().ctype(),
                if inline { "inline" } else { "attachment" },
                "{name}"
            );
            if name != ".docx" {
                assert_eq!(part.contents(), payload);
            }
        }
    }
}

#[allow(
    clippy::unwrap_used,
    reason = "Build an isolated in-memory ZIP fixture"
)]
fn page_bundle(files: &[(&str, &[u8])]) -> String {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in files {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(bytes).unwrap();
    }
    STANDARD.encode(zip.finish().unwrap().into_inner())
}

#[test]
fn page_bundle_embeds_nested_css_fonts_images_and_scripts_without_fetching_remote_resources() {
    let input = page_bundle(&[
        ("index.html", br#"<!doctype html><link rel="stylesheet" href="css/main.css" integrity="sha256-original"><img src="images/logo.png"><script src="app.js"></script><img src="https://remote.invalid/image"><style>.inline { background: url('images/logo.png') }</style>"#),
        ("css/main.css", br#"@import "nested/theme.css" screen; .hero { background: image-set("../images/logo.png" 1x, url(../images/logo.png) 2x) }"#),
        ("css/nested/theme.css", br#"@font-face { font-family: exercise; src: url('../../fonts/exercise.woff2') }"#),
        ("images/logo.png", b"synthetic-image"),
        ("fonts/exercise.woff2", b"synthetic-font"),
        ("app.js", b"document.body.dataset.loaded='yes'"),
    ]);
    let tools = Tools {
        policy: Policy::default(),
    };
    let output = tools.page_bundle(&input).unwrap();
    assert!(output.contains("data:image/png;base64,c3ludGhldGljLWltYWdl"));
    assert!(output.contains("data:text/javascript;base64,"));
    assert!(!output.contains("sha256-original"));
    assert!(output.contains("https://remote.invalid/image"));
    let document = scraper::Html::parse_document(&output);
    let stylesheet = document
        .select(&scraper::Selector::parse("link").unwrap())
        .next()
        .unwrap()
        .value()
        .attr("href")
        .unwrap();
    let css = String::from_utf8(
        STANDARD
            .decode(stylesheet.strip_prefix("data:text/css;base64,").unwrap())
            .unwrap(),
    )
    .unwrap();
    assert!(css.contains("data:image/png;base64,c3ludGhldGljLWltYWdl"));
    assert!(css.contains(" screen;"));
    let nested = css.split('"').nth(1).unwrap();
    let nested = String::from_utf8(
        STANDARD
            .decode(nested.strip_prefix("data:text/css;base64,").unwrap())
            .unwrap(),
    )
    .unwrap();
    assert!(nested.contains("data:font/woff2;base64,c3ludGhldGljLWZvbnQ="));
}

#[test]
fn page_bundle_rejects_missing_assets_cycles_traversal_and_oversized_expansion() {
    let tools = Tools {
        policy: Policy::default(),
    };
    for files in [
        vec![("index.html", b"<img src='missing.png'>".as_slice())],
        vec![
            (
                "index.html",
                b"<link rel='stylesheet' href='a.css'>".as_slice(),
            ),
            ("a.css", b"@import 'b.css';"),
            ("b.css", b"@import 'a.css';"),
        ],
        vec![
            ("index.html", b"<p>Page</p>".as_slice()),
            ("../outside.txt", b"blocked"),
        ],
        vec![("other.html", b"<p>No entry point</p>".as_slice())],
        vec![
            (
                "index.html",
                b"<script type='module' src='module.js'></script>".as_slice(),
            ),
            ("module.js", b"import './nested.js'"),
        ],
    ] {
        assert!(tools.page_bundle(&page_bundle(&files)).is_err());
    }
    let huge = vec![b' '; 8 * 1024 * 1024 + 1];
    assert!(
        tools
            .page_bundle(&page_bundle(&[("index.html", &huge)]))
            .is_err()
    );
}
