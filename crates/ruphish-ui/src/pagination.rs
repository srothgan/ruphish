//! Cursor navigation uses server-confirmed continuation and visited page boundaries.

/// History belongs to the table, not to the number of rows currently displayed.
#[derive(Clone, Default)]
pub struct CursorPages {
    history: Vec<i64>,
    next: Option<i64>,
    loaded: bool,
}
impl CursorPages {
    /// Cursor of the requested page; zero starts at the first row.
    pub fn after(&self) -> i64 {
        self.history.last().copied().unwrap_or(0)
    }
    /// Whether the displayed response belongs to this page.
    pub fn loaded(&self) -> bool {
        self.loaded
    }
    /// Previous is available even when the last page is partial or empty.
    pub fn has_previous(&self) -> bool {
        !self.history.is_empty()
    }
    /// A full page alone does not establish that more rows exist.
    pub fn has_next(&self) -> bool {
        self.next.is_some()
    }
    /// Ignore responses for other pages and non-advancing cursors.
    pub fn received(&mut self, after: i64, next: Option<i64>) -> bool {
        if self.after() != after {
            return false;
        }
        self.loaded = true;
        self.next = next.filter(|next| *next > after);
        true
    }
    /// Move forward once; another click must wait for the new page's response.
    pub fn advance(&mut self) {
        if let Some(next) = self.next.take() {
            self.history.push(next);
            self.loaded = false;
        }
    }
    /// Return to the actual previous boundary, not always the first page.
    pub fn previous(&mut self) {
        if self.history.pop().is_some() {
            self.next = None;
            self.loaded = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_or_exact_final_pages_cannot_advance_and_previous_retraces_visits() {
        let mut pages = CursorPages::default();
        assert!(!pages.has_previous());
        assert!(!pages.has_next());
        pages.received(0, None);
        pages.advance();
        assert_eq!(pages.after(), 0);
        pages.received(0, Some(50));
        pages.advance();
        pages.advance(); // Double-click cannot skip an unrequested page.
        assert_eq!(pages.after(), 50);
        assert!(!pages.loaded());
        pages.received(50, Some(100));
        pages.advance();
        pages.received(100, None);
        assert!(!pages.has_next());
        assert!(pages.has_previous());
        pages.previous();
        assert_eq!(pages.after(), 50);
        assert!(!pages.loaded());
        pages.received(50, Some(100));
        pages.previous();
        assert_eq!(pages.after(), 0);
        assert!(!pages.has_previous());
        pages.previous();
        assert_eq!(pages.after(), 0);
    }

    #[test]
    fn failed_or_deleted_pages_allow_back_and_stale_responses_cannot_enable_next() {
        let mut pages = CursorPages::default();
        pages.received(0, Some(2));
        pages.advance();
        assert!(!pages.received(0, Some(99)));
        assert!(!pages.has_next());
        assert!(!pages.loaded()); // Failed load has no successful response.
        assert!(pages.has_previous());
        pages.received(2, None); // Rows may have been deleted since the earlier request.
        assert!(!pages.has_next());
        pages.previous();
        assert!(!pages.received(2, Some(3)));
        pages.received(0, None);
        assert!(!pages.has_previous());
        assert!(!pages.has_next());
        for invalid in [0, -1] {
            pages.received(0, Some(invalid));
            assert!(!pages.has_next());
        }
    }
}
