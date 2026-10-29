//! Compare the compatibility view with its embedded definition, not just its columns.
//! Catalog printers add qualification, casts and parentheses to this fixed expression.

pub(crate) fn matches(actual: &str, migration: &str, schema: &str) -> bool {
    let Some((_, definition)) = migration.split_once("CREATE VIEW event_history AS ") else {
        return false;
    };
    tokens(actual, schema) == tokens(definition.split(';').next().unwrap_or_default(), schema)
}

fn tokens(sql: &str, schema: &str) -> Vec<String> {
    let mut chars = sql.chars().peekable();
    let mut raw = Vec::new();
    while let Some(c) = chars.next() {
        if c.is_whitespace() || matches!(c, '(' | ')' | ';') {
            continue;
        }
        let mut token = String::new();
        if matches!(c, '\'' | '"' | '`') {
            if c == '\'' {
                token.push(c);
            }
            while let Some(next) = chars.next() {
                if next == c {
                    if chars.peek() == Some(&c) {
                        token.push(next);
                        token.push(chars.next().unwrap_or(c));
                        continue;
                    }
                    break;
                }
                token.push(next);
            }
            if c == '\'' {
                token.push(c);
            }
        } else if c.is_alphanumeric() || c == '_' {
            token.push(c.to_ascii_lowercase());
            while chars
                .peek()
                .is_some_and(|c| c.is_alphanumeric() || *c == '_')
            {
                if let Some(c) = chars.next() {
                    token.push(c.to_ascii_lowercase());
                }
            }
        } else {
            token.push(c);
        }
        raw.push(token);
    }
    let mut normalized: Vec<String> = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        let current = raw[i].as_str();
        let next = raw.get(i + 1).map(String::as_str);
        if (current == schema || current == "events") && next == Some(".") {
            i += 2;
            continue;
        }
        if current == "_utf8mb4" && next.is_some_and(|value| value.starts_with('\'')) {
            i += 1;
            continue;
        }
        if current == ":" && next == Some(":") {
            match raw.get(i + 2).map(String::as_str) {
                Some("text") => {
                    i += 3;
                    continue;
                }
                Some("character") if raw.get(i + 3).map(String::as_str) == Some("varying") => {
                    i += 4;
                    continue;
                }
                _ => (),
            }
        }
        if current == "as" && next == normalized.last().map(String::as_str) {
            i += 2;
            continue;
        }
        normalized.push(raw[i].clone());
        i += 1;
    }
    normalized
}
