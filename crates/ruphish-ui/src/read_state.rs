//! Presentation policy for a query's current selection, including successful empty reads.

/// Current selection visibility derived solely from its query snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadState {
    /// No usable snapshot yet, including the first render before fetching starts.
    Loading,
    /// The first read failed and is not being retried.
    Failed,
    /// A successful snapshot, including a valid empty result.
    Ready,
    /// A confirmed snapshot remains usable during refresh.
    Refreshing,
    /// A refresh failed; retain the last confirmed snapshot.
    Stale,
}

impl ReadState {
    /// Classify query state without treating missing data as a successful empty read.
    pub fn from_query(has_data: bool, fetching: bool, has_error: bool) -> Self {
        match (has_data, fetching, has_error) {
            (false, false, true) => Self::Failed,
            (false, _, _) => Self::Loading,
            (true, _, true) => Self::Stale,
            (true, true, false) => Self::Refreshing,
            (true, false, false) => Self::Ready,
        }
    }

    /// Whether confirmed content may remain visible and interactive.
    pub fn shows_content(self) -> bool {
        matches!(self, Self::Ready | Self::Refreshing | Self::Stale)
    }
}

#[cfg(test)]
mod tests {
    use super::ReadState::*;
    use super::*;

    #[test]
    fn first_render_and_selection_changes_never_masquerade_as_empty_results() {
        assert_eq!(ReadState::from_query(false, false, false), Loading);
        assert_eq!(ReadState::from_query(false, true, false), Loading);
        assert_eq!(ReadState::from_query(false, false, true), Failed);
        assert_eq!(ReadState::from_query(false, true, true), Loading);
        // A successful empty response is data, just like a populated response.
        assert_eq!(ReadState::from_query(true, false, false), Ready);
        assert!(!Loading.shows_content());
        assert!(!Failed.shows_content());
    }

    #[test]
    fn refresh_and_retry_preserve_confirmed_content_until_access_is_lost() {
        for (fetching, error, expected) in [
            (true, false, Refreshing),
            (false, true, Stale),
            (true, true, Stale),
        ] {
            let state = ReadState::from_query(true, fetching, error);
            assert_eq!(state, expected);
            assert!(state.shows_content());
        }
        // Query policy discards data on an authorization/not-found response.
        assert!(!ReadState::from_query(false, false, true).shows_content());
    }
}
