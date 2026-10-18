//! Execute independent Go expectations, including partial-output and syntax failures.

use ruphish_adapters::template::{self, Context};
use serde_json::Value;

#[test]
fn captured_go_template_cases_match_outputs_and_error_classes() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/gophish-v0.12.1/templates.json"
    ))
    .unwrap();
    for case in cases {
        let data = &case["context"];
        let context = Context {
            from: data["From"].as_str().unwrap().into(),
            url: data["URL"].as_str().unwrap().into(),
            tracker: data["Tracker"].as_str().unwrap().into(),
            tracking_url: data["TrackingURL"].as_str().unwrap().into(),
            rid: data["RId"].as_str().unwrap().into(),
            base_url: data["BaseURL"].as_str().unwrap().into(),
            email: data["email"].as_str().unwrap().into(),
            first_name: data["first_name"].as_str().unwrap().into(),
            last_name: data["last_name"].as_str().unwrap().into(),
            position: data["position"].as_str().unwrap().into(),
        };
        let result = template::render(case["text"].as_str().unwrap(), &context);
        let (output, kind) = match result {
            Ok(output) => (output, String::new()),
            Err(error) => (error.output, format!("{:?}", error.kind).to_lowercase()),
        };
        assert_eq!(
            output,
            case["expected_output"].as_str().unwrap(),
            "{}",
            case["id"]
        );
        assert_eq!(
            kind,
            case["expected_error_kind"].as_str().unwrap(),
            "{}",
            case["id"]
        );
    }
}

#[test]
fn extended_go_corpus_preserves_scoping_bytes_composition_and_partial_failures() {
    let mut cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/compatibility/templates.json"
    ))
    .unwrap();
    cases.extend(
        serde_json::from_str::<Vec<Value>>(include_str!(
            "../../../tests/fixtures/compatibility/templates-edge.json"
        ))
        .unwrap(),
    );
    let context = Context {
        from: "Sender".into(),
        first_name: "Ada".into(),
        last_name: "Example".into(),
        email: "ada@example.test".into(),
        position: "Engineer".into(),
        url: "https://training.example.test/?rid=1".into(),
        ..Context::default()
    };
    let deviations: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/compatibility/template-deviations.json"
    ))
    .unwrap();
    for case in cases {
        let result = template::render(case["text"].as_str().unwrap(), &context);
        let (output, kind) = match result {
            Ok(output) => (output, String::new()),
            Err(error) => (error.output, format!("{:?}", error.kind).to_lowercase()),
        };
        if let Some(decision) = deviations
            .iter()
            .find(|decision| decision["text"] == case["text"])
        {
            assert_eq!(decision["status"], "approved");
            assert_eq!(kind, decision["rust_error_kind"].as_str().unwrap());
            assert_eq!(output, decision["rust_partial_output"].as_str().unwrap());
            continue;
        }
        assert_eq!(
            output,
            case["expected_output"].as_str().unwrap(),
            "{}",
            case["text"]
        );
        assert_eq!(
            kind,
            case["expected_error_kind"].as_str().unwrap(),
            "{}",
            case["text"]
        );
    }
}

#[test]
fn approved_utf8_policy_accepts_accented_mail_and_valid_slices_but_rejects_split_characters() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/compatibility/templates-edge.json"
    ))
    .unwrap();
    let deviations: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/compatibility/template-deviations.json"
    ))
    .unwrap();
    for decision in deviations {
        let reference = cases
            .iter()
            .find(|case| case["text"] == decision["text"])
            .unwrap();
        assert_eq!(reference["expected_error_kind"], "");
        assert_eq!(decision["status"], "approved");
        let error =
            template::render(decision["text"].as_str().unwrap(), &Context::default()).unwrap_err();
        assert_eq!(error.kind, template::ErrorKind::Execution);
        assert_ne!(error.output, reference["expected_output"].as_str().unwrap());
    }
    let context = Context {
        first_name: "José".into(),
        last_name: "Muñoz".into(),
        ..Context::default()
    };
    assert_eq!(
        template::render(
            "<p>Bonjour {{.FirstName}} {{.LastName}}. Ça va? ¿Cómo estás?</p>",
            &context
        )
        .unwrap(),
        "<p>Bonjour José Muñoz. Ça va? ¿Cómo estás?</p>"
    );
    for (source, expected) in [
        (r#"{{slice "é" 0 2}}"#, "é"),
        (r#"{{slice "José" 3 5}}"#, "é"),
        (r#"{{slice "français" 4 6}}"#, "ç"),
        (r#"{{slice "👩" 0 4}}"#, "👩"),
    ] {
        assert_eq!(template::render(source, &context).unwrap(), expected);
    }
    for source in [
        r#"{{slice "é" 1}}"#,
        r#"{{slice "José" 0 4}}"#,
        r#"{{slice "👩" 1}}"#,
    ] {
        let error = template::render(source, &context).unwrap_err();
        assert_eq!(error.kind, template::ErrorKind::Execution);
        assert!(error.output.is_empty());
    }
}

#[test]
fn rendering_budgets_recursion_and_failures_do_not_poison_subsequent_or_concurrent_jobs() {
    let context = Context {
        first_name: "Ada".into(),
        ..Context::default()
    };
    let error = template::render_with_limits(
        "{{range 1000}}x{{end}}",
        &context,
        template::RenderLimits::default().with_output_bytes(64),
    )
    .unwrap_err();
    assert_eq!(error.kind, template::ErrorKind::Execution);
    assert_eq!(error.output, "x".repeat(64));
    let recursive = "{{define \"loop\"}}x{{template \"loop\" .}}{{end}}{{template \"loop\" .}}";
    let error = template::render(recursive, &context).unwrap_err();
    assert_eq!(error.kind, template::ErrorKind::Execution);
    assert!(error.output.len() <= 100);
    let nested = format!("{}x{}", "{{if true}}".repeat(65), "{{end}}".repeat(65));
    let error = template::render(&nested, &context).unwrap_err();
    assert_eq!(error.kind, template::ErrorKind::Parse);
    assert!(error.output.is_empty());
    let jobs: Vec<_> = (0..8)
        .map(|index| {
            std::thread::spawn(move || {
                let context = Context {
                    first_name: format!("Recipient {index}"),
                    ..Context::default()
                };
                let error = template::render_with_limits(
                    "{{range 1000}}x{{end}}",
                    &context,
                    template::RenderLimits::default().with_output_bytes(16),
                )
                .unwrap_err();
                assert_eq!(error.output, "x".repeat(16));
                assert_eq!(
                    template::render("Hello {{.FirstName}}", &context).unwrap(),
                    format!("Hello Recipient {index}")
                );
            })
        })
        .collect();
    for job in jobs {
        job.join().unwrap();
    }
    assert_eq!(
        template::render("Hello {{.FirstName}}", &context).unwrap(),
        "Hello Ada"
    );
}

#[test]
fn actual_visual_editor_outputs_execute_like_go_without_breaking_saved_template_expressions() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/compatibility/editor-rendering.json"
    ))
    .unwrap();
    let data = &fixture["context"];
    let context = Context {
        from: data["From"].as_str().unwrap().into(),
        url: data["URL"].as_str().unwrap().into(),
        tracker: data["Tracker"].as_str().unwrap().into(),
        tracking_url: data["TrackingURL"].as_str().unwrap().into(),
        rid: data["RId"].as_str().unwrap().into(),
        base_url: data["BaseURL"].as_str().unwrap().into(),
        email: data["email"].as_str().unwrap().into(),
        first_name: data["first_name"].as_str().unwrap().into(),
        last_name: data["last_name"].as_str().unwrap().into(),
        position: data["position"].as_str().unwrap().into(),
    };
    for case in fixture["cases"].as_array().unwrap() {
        assert_eq!(
            template::render(case["original_source"].as_str().unwrap(), &context).unwrap(),
            case["original_rendered"].as_str().unwrap(),
            "original {}",
            case["id"]
        );
        assert_eq!(
            template::render(case["edited_source"].as_str().unwrap(), &context).unwrap(),
            case["edited_rendered"].as_str().unwrap(),
            "edited {}",
            case["id"]
        );
    }
}
