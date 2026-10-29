//! Closed repository SQL transformations, independent of a particular feature.

/// Number positional placeholders for PostgreSQL, preserving bind order.
/// Callers supply internal SQL with question marks only at parameter positions,
/// never in literals/comments/operators; this is deliberately not an SQL parser.
pub(crate) fn placeholders(statement: impl Into<String>, postgres: bool) -> String {
    let statement = statement.into();
    if !postgres {
        return statement;
    }
    let mut output = String::with_capacity(statement.len());
    let mut index = 0;
    for character in statement.chars() {
        if character == '?' {
            index += 1;
            output.push('$');
            output.push_str(&index.to_string());
        } else {
            output.push(character);
        }
    }
    output
}
