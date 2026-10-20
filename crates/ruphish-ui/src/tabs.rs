//! Keyboard navigation for a tab list, independent of rendering.
/// Return the enabled destination for horizontal arrow, Home or End navigation.
pub fn next_tab(enabled: &[usize], selected: usize, key: &str) -> Option<usize> {
    let position = enabled.iter().position(|index| *index == selected);
    match key {
        "Home" => enabled.first().copied(),
        "End" => enabled.last().copied(),
        "ArrowRight" => enabled
            .get(position.map_or(0, |i| (i + 1) % enabled.len()))
            .copied(),
        "ArrowLeft" => enabled
            .get(position.map_or(enabled.len().saturating_sub(1), |i| {
                (i + enabled.len() - 1) % enabled.len()
            }))
            .copied(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::next_tab;
    #[test]
    fn keyboard_skips_disabled_tabs_and_wraps_without_selecting_them() {
        let enabled = [0, 2, 4];
        assert_eq!(next_tab(&enabled, 0, "ArrowRight"), Some(2));
        assert_eq!(next_tab(&enabled, 4, "ArrowRight"), Some(0));
        assert_eq!(next_tab(&enabled, 0, "ArrowLeft"), Some(4));
        assert_eq!(next_tab(&enabled, 2, "Home"), Some(0));
        assert_eq!(next_tab(&enabled, 2, "End"), Some(4));
        assert_eq!(next_tab(&enabled, 2, "Tab"), None);
    }
    #[test]
    fn empty_and_dynamically_disabled_selections_are_safe() {
        assert_eq!(next_tab(&[], 0, "ArrowRight"), None);
        assert_eq!(next_tab(&[], 0, "ArrowLeft"), None);
        assert_eq!(next_tab(&[3], 3, "ArrowRight"), Some(3));
        assert_eq!(next_tab(&[0, 2], 1, "ArrowRight"), Some(0));
        assert_eq!(next_tab(&[0, 2], 1, "ArrowLeft"), Some(2));
    }
}
