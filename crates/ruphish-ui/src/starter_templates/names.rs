//! Ordinary resource names for repeat additions; no installed-template registry.
use std::collections::HashSet;

/// Choose the first unused display name without overwriting an existing resource.
pub fn available_name(base: &str, used: &HashSet<String>) -> String {
    let mut name = base.to_owned();
    let mut copy = 2u64;
    while used.contains(&name) {
        name = format!("{base} ({copy})");
        copy += 1;
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeat_additions_preserve_existing_names_and_reuse_available_gaps() {
        let mut used =
            HashSet::from(["Example".into(), "Example (2)".into(), "Example (4)".into()]);
        let name = available_name("Example", &used);
        assert_eq!(name, "Example (3)");
        used.insert(name);
        assert_eq!(available_name("Example", &used), "Example (5)");
        assert_eq!(available_name("Other", &used), "Other");
    }
}
