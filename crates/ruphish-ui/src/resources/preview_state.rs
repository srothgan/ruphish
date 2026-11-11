//! A preview belongs to one source snapshot and never replaces the editable document.

/// Request/result state shared by template and landing-page previews.
#[derive(Clone, Default)]
pub struct ContentPreview {
    generation: u64,
    source: String,
    /// Whether the current source is being rendered.
    pub loading: bool,
    /// Rendered HTML for the current source only.
    pub html: Option<String>,
    /// An error for the current request only.
    pub error: Option<String>,
}
impl ContentPreview {
    /// Hide old output before rendering a new source snapshot.
    pub fn begin(&mut self, source: String) -> u64 {
        self.clear();
        self.source = source;
        self.loading = true;
        self.generation
    }
    /// Returning to editing invalidates outstanding requests.
    pub fn clear(&mut self) {
        self.generation += 1;
        self.loading = false;
        self.html = None;
        self.error = None;
        self.source.clear();
    }
    /// Ignore responses for another tab visit or an edited source snapshot.
    pub fn finish(&mut self, generation: u64, source: &str, result: Result<String, String>) {
        if generation != self.generation || source != self.source {
            return;
        }
        self.loading = false;
        match result {
            Ok(html) => self.html = Some(html),
            Err(error) => self.error = Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editing_and_reopening_never_show_an_old_preview_or_its_late_error() {
        let mut preview = ContentPreview::default();
        let old = preview.begin("<p>Before</p>".into());
        preview.clear();
        let current = preview.begin("<p>After</p>".into());
        preview.finish(old, "<p>Before</p>", Ok("Old rendered content".into()));
        preview.finish(old, "<p>Before</p>", Err("Old failure".into()));
        assert!(preview.loading);
        assert!(preview.html.is_none());
        assert!(preview.error.is_none());
        preview.finish(current, "<p>After</p>", Ok("<p>After</p>".into()));
        assert_eq!(preview.html.as_deref(), Some("<p>After</p>"));
        assert!(!preview.loading);
        let invalid = preview.begin("{{.Missing}}".into());
        assert!(preview.html.is_none());
        preview.finish(
            invalid,
            "{{.Missing}}",
            Err("Unknown template field".into()),
        );
        assert!(preview.html.is_none());
        assert_eq!(preview.error.as_deref(), Some("Unknown template field"));
        let retry = preview.begin("<p>Fixed</p>".into());
        preview.finish(retry, "different snapshot", Ok("Wrong content".into()));
        assert!(preview.loading);
        preview.finish(retry, "<p>Fixed</p>", Ok("<p>Fixed</p>".into()));
        assert_eq!(preview.html.as_deref(), Some("<p>Fixed</p>"));
        assert!(preview.error.is_none());
    }
}
