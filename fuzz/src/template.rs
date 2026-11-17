//! Arbitrary Go templates and bounded rendering with independent recovery expectations.

use ruphish_adapters::template::{Context, ErrorKind, RenderLimits, render_with_limits};

/// Check successful and partial output budgets, then render a fresh independent message.
pub fn check(data: &[u8]) {
    if data.len() > 64 * 1024 {
        return;
    }
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    let limit = 32 + usize::from(data.first().copied().unwrap_or(0)) * 16;
    let context = Context {
        first_name: source.chars().take(64).collect(),
        ..Context::default()
    };
    match render_with_limits(
        source,
        &context,
        RenderLimits::default().with_output_bytes(limit),
    ) {
        Ok(output) => assert!(output.len() <= limit),
        Err(error) => {
            assert!(error.output.len() <= limit);
            if error.kind == ErrorKind::Parse {
                assert!(error.output.is_empty());
            }
        }
    }
    // Template expressions in recipient values remain data, including braces and Unicode.
    assert_eq!(
        render_with_limits(
            "Hello {{.FirstName}}!",
            &context,
            RenderLimits::default().with_output_bytes(1024)
        )
        .expect("fresh rendering succeeds"),
        format!("Hello {}!", context.first_name)
    );
    let count = usize::from(data.get(1).copied().unwrap_or(0));
    let source = format!("{{{{range {count}}}}}x{{{{end}}}}");
    let result = render_with_limits(
        &source,
        &context,
        RenderLimits::default().with_output_bytes(64),
    );
    if count <= 64 {
        assert_eq!(result.expect("output fits"), "x".repeat(count));
    } else {
        let error = result.expect_err("output exceeds budget");
        assert_eq!(error.kind, ErrorKind::Execution);
        assert_eq!(error.output, "x".repeat(64));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn templates_cover_success_parse_execution_and_utf8() {
        for source in [
            "Hello {{.FirstName}}",
            "{{if}}",
            "{{range 1000}}x{{end}}",
            "{{slice \"é\" 1}}",
            "{{define \"r\"}}x{{template \"r\" .}}{{end}}{{template \"r\" .}}",
        ] {
            super::check(source.as_bytes());
        }
        super::check(include_bytes!("../seeds/template_render/unicode.txt"));
    }
}
