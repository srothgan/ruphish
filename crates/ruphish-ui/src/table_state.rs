//! Table ordering uses typed values, never formatted DOM text or row positions.
use ruphish_domain::reporting::SortDirection;

/// Column identity and direction; page owners retain this across refreshes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sort {
    /// Stable field key, independent of translated header text.
    pub column: String,
    /// Direction applies to values, never to the identity tie-breaker.
    pub order: SortDirection,
}
impl Sort {
    /// Switch columns ascending, or toggle the current column.
    pub fn select(&self, column: &str) -> Self {
        Self {
            column: column.into(),
            order: if self.column == column && self.order == SortDirection::Asc {
                SortDirection::Desc
            } else {
                SortDirection::Asc
            },
        }
    }
    /// Stable ordering of the complete supplied collection. Call before slicing pages.
    pub fn rows<T>(
        &self,
        mut rows: Vec<T>,
        value: impl Fn(&T, &str) -> Value,
        identity: impl Fn(&T) -> Value,
    ) -> Vec<T> {
        rows.sort_by(|a, b| {
            let order = value(a, &self.column).cmp(&value(b, &self.column));
            let order = if self.order == SortDirection::Desc {
                order.reverse()
            } else {
                order
            };
            order.then_with(|| identity(a).cmp(&identity(b)))
        });
        rows
    }
    /// WAI-ARIA state for a sortable header.
    pub fn aria(&self, column: &str) -> &'static str {
        if self.column != column {
            "none"
        } else if self.order == SortDirection::Asc {
            "ascending"
        } else {
            "descending"
        }
    }
}
/// Explicit comparable values: absent first, numbers numerically, text by Unicode scalar order.
#[derive(Clone, Debug)]
pub enum Value {
    /// Missing data is distinct from zero or an empty string.
    Missing,
    /// Counts, booleans and UTC timestamps.
    Number(i128),
    /// Unformatted text, ordered identically to UTF-8 binary SQL collation.
    Text(String),
    /// Exact ratio, compared by cross multiplication rather than rounded display percentages.
    Ratio(i64, i64),
    /// Composite row identities.
    Tuple(Vec<Value>),
}
impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::Text(value.into())
    }
}
impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}
impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self::Number(value.into())
    }
}
impl From<usize> for Value {
    fn from(value: usize) -> Self {
        Self::Number(value as i128)
    }
}
impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Number(i128::from(value))
    }
}
/// Parse an actual instant instead of sorting a localized date string.
pub fn timestamp(value: &str) -> Value {
    ruphish_domain::datetime::parse(value)
        .map_or(Value::Missing, |time| Value::Number(time.as_nanosecond()))
}
/// JSON scalar accessor for wire-backed tables; timestamps use `timestamp` explicitly.
pub fn scalar(value: &serde_json::Value) -> Value {
    match value {
        serde_json::Value::String(v) => v.clone().into(),
        serde_json::Value::Number(v) => v.as_i64().map_or(Value::Missing, Into::into),
        serde_json::Value::Bool(v) => (*v).into(),
        _ => Value::Missing,
    }
}

impl Ord for Value {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use Value::{Missing, Number, Ratio, Text, Tuple};
        let rank = |value: &Self| match value {
            Missing => 0,
            Number(_) => 1,
            Text(_) => 2,
            Ratio(_, _) => 3,
            Tuple(_) => 4,
        };
        match (self, other) {
            (Missing, Missing) => std::cmp::Ordering::Equal,
            (Number(a), Number(b)) => a.cmp(b),
            (Text(a), Text(b)) => a.cmp(b),
            (Ratio(a, b), Ratio(c, d)) => {
                (i128::from(*a) * i128::from(*d)).cmp(&(i128::from(*c) * i128::from(*b)))
            }
            (Tuple(a), Tuple(b)) => a.cmp(b),
            _ => rank(self).cmp(&rank(other)),
        }
    }
}
impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl Eq for Value {}
/// Undefined rates remain distinct from a measured zero.
pub fn ratio(numerator: i64, denominator: i64) -> Value {
    if denominator <= 0 {
        Value::Missing
    } else {
        Value::Ratio(numerator, denominator)
    }
}
