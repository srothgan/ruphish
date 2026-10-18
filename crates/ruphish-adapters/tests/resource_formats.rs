//! Real CSV/MIME/Office/HTML transformations with independent content expectations.
use base64::{Engine, engine::general_purpose::STANDARD};
use ruphish_adapters::{
    imports::{Tools, landing_html},
    outbound::Policy,
    resources::{attachment, sample},
};
use ruphish_application::resources::{Attachment, ContentTools, Page};
use std::io::{Cursor, Read, Write};

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
        delivery::{Sender, TestMessage},
        resources::{Profile, Recipient, Template},
    };
    let sender = ruphish_adapters::smtp::Smtp::new(Policy::default(), "test@test.com".into());
    for variant in ["normal", "empty-subject", "url"] {
        let message = TestMessage {
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
        assert_eq!(
            parsed.body_html(0).unwrap().trim_end_matches(['\r', '\n']),
            if variant == "url" {
                url
            } else {
                "firstlast@example.com - HTML"
            }
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
        delivery::TestMessage,
        resources::{Profile, Recipient, Template},
    };
    let mut message = TestMessage {
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
fn page_policy_is_reversible_on_the_authored_draft_and_shields_template_actions() {
    let html = "<form action='/outside'><input name='user'><input type='PASSWORD' name='secret'><p>{{printf \"<b>%s</b>\" .FirstName}}</p></form>";
    let mut page = Page {
        html: html.into(),
        ..Page::default()
    };
    let disabled = landing_html(&page).unwrap();
    assert!(!disabled.contains("name="));
    assert!(disabled.contains("action=\"\""));
    assert!(disabled.contains("{{printf \"<b>%s</b>\" .FirstName}}"));
    page.capture_credentials = true;
    let ordinary = landing_html(&page).unwrap();
    assert!(ordinary.contains("name='user'"));
    assert!(!ordinary.contains("name='secret'"));
    page.capture_passwords = true;
    let passwords = landing_html(&page).unwrap();
    assert!(passwords.contains("name=\"password\""));
    assert_eq!(page.html, html);
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
    for ip in ["169.254.169.254", "::ffff:169.254.169.254", "fe80::1"] {
        assert!(!policy.permits(ip.parse().unwrap()));
    }
    assert!(policy.permits("127.0.0.1".parse().unwrap()));
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
