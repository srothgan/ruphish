//! Accumulating dropdown options; paging is an internal loading detail.
#[derive(Clone, Debug, PartialEq)]
/// Identifies one search page and rejects responses superseded by a newer request.
pub struct Request {
    /// Search used for this request.
    pub search: String,
    /// Backend offset, independent of deduplicated visible rows.
    pub offset: usize,
    generation: u64,
}
/// Searchable options with late-response rejection and retryable append failures.
#[derive(Clone, Default)]
pub struct Options {
    /// Loaded identities and their display labels.
    pub rows: Vec<(i64, String)>,
    /// Current request status.
    pub loading: bool,
    /// Safe error shown beside the retry action.
    pub error: String,
    search: String,
    offset: usize,
    total: usize,
    generation: u64,
}
impl Options {
    /// Begin a new search, discarding only its obsolete options.
    pub fn search(&mut self, search: String) -> Request {
        self.search = search;
        self.offset = 0;
        self.total = 0;
        self.rows.clear();
        self.begin()
    }
    /// Whether another scroll load is available.
    pub fn has_more(&self) -> bool {
        self.offset < self.total
    }
    fn begin(&mut self) -> Request {
        self.generation += 1;
        self.loading = true;
        self.error.clear();
        Request {
            search: self.search.clone(),
            offset: self.offset,
            generation: self.generation,
        }
    }
    /// Coalesce scroll requests; retry uses the same uncommitted offset.
    pub fn more(&mut self, retry: bool) -> Option<Request> {
        if self.loading || (!retry && (!self.has_more() || !self.error.is_empty())) {
            return None;
        }
        Some(self.begin())
    }
    /// Append a matching response without losing previous choices or accepting an old search.
    pub fn finish(
        &mut self,
        request: Request,
        result: Result<(Vec<(i64, String)>, usize), String>,
    ) {
        if request.generation != self.generation {
            return;
        }
        self.loading = false;
        match result {
            Ok((rows, total)) => {
                self.total = total;
                self.offset += rows.len();
                if rows.is_empty() {
                    self.total = self.offset;
                }
                for (id, label) in rows {
                    if let Some(existing) = self.rows.iter_mut().find(|row| row.0 == id) {
                        existing.1 = label;
                    } else {
                        self.rows.push((id, label));
                    }
                }
            }
            Err(error) => self.error = error,
        }
    }
}
/// Show the person's name first; unnamed recipients retain an unambiguous address.
pub fn recipient_label(name: &str, email: &str) -> String {
    if name.trim().is_empty() {
        email.into()
    } else {
        format!("{} · {email}", name.trim())
    }
}
#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Workflow assertions")]
mod tests {
    use super::*;
    #[test]
    fn scrolling_appends_retries_and_ignores_a_late_previous_search() {
        let mut options = Options::default();
        let initial = options.search(String::new());
        assert!(options.more(false).is_none());
        options.finish(
            initial,
            Ok((vec![(1, "Alice".into()), (2, "Bob".into())], 4)),
        );
        let next = options.more(false).unwrap();
        assert_eq!(next.offset, 2);
        options.finish(next, Err("Connection failed".into()));
        assert_eq!(options.rows.len(), 2);
        assert!(options.more(false).is_none());
        let retry = options.more(true).unwrap();
        assert_eq!(retry.offset, 2);
        options.finish(
            retry,
            Ok((vec![(2, "Bob renamed".into()), (3, "Clara".into())], 4)),
        );
        assert_eq!(options.rows.len(), 3);
        assert!(!options.has_more());
        let old = options.search("Alice".into());
        let new = options.search("Clara".into());
        options.finish(old, Ok((vec![(1, "Alice".into())], 1)));
        assert!(options.loading && options.rows.is_empty());
        options.finish(new, Ok((vec![(3, "Clara".into())], 1)));
        assert_eq!(options.rows[0].0, 3);
        assert!(!options.loading);
    }
    #[test]
    fn empty_tail_stops_loading_and_names_precede_addresses() {
        let mut options = Options::default();
        let initial = options.search(String::new());
        options.finish(initial, Ok((vec![(1, "A".into())], 2)));
        let tail = options.more(false).unwrap();
        options.finish(tail, Ok((vec![], 2)));
        assert!(options.more(false).is_none());
        assert_eq!(
            recipient_label(" Alice ", "alice@example.test"),
            "Alice · alice@example.test"
        );
        assert_eq!(
            recipient_label(" ", "alice@example.test"),
            "alice@example.test"
        );
    }
}
