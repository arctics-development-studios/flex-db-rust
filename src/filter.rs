use serde::Serialize;
use serde_json::Value;

/// Filter operator used in [`SearchFilter`].
///
/// Operators are sent as lowercase strings in the request body and match
/// the server's `op` field exactly.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterOp {
    /// `field == value`
    Eq,
    /// `field != value`
    Ne,
    /// `field > value`
    Gt,
    /// `field >= value`
    Gte,
    /// `field < value`
    Lt,
    /// `field <= value`
    Lte,
    /// `value` is a substring of `field`
    Contains,
    /// `field` starts with `value`
    StartsWith,
}

/// A single filter condition for [`Namespace::search`](crate::Namespace::search).
///
/// Multiple filters passed to `search` are AND-ed together on the server.
/// Filters operate on the `sp` (search properties) map stored with each object.
///
/// All `sp` keys follow the rules: alphabet `A-Za-z0-9_`, max 64 chars.
///
/// # Examples
///
/// ```rust
/// use flex_db::{SearchFilter, FilterOp};
///
/// let f1 = SearchFilter::eq("status", "active");
/// let f2 = SearchFilter::gte("score", 80u32);
/// let f3 = SearchFilter::starts_with("label", "prod-");
/// let f4 = SearchFilter::new("tag", FilterOp::Contains, "rust");
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct SearchFilter {
    pub field: String,
    pub op: FilterOp,
    pub value: Value,
}

impl SearchFilter {
    /// General constructor. `value` is any JSON-serializable type.
    pub fn new(field: impl Into<String>, op: FilterOp, value: impl Serialize) -> Self {
        Self {
            field: field.into(),
            op,
            value: serde_json::to_value(value).unwrap_or(Value::Null),
        }
    }

    /// `field == value`
    pub fn eq(field: impl Into<String>, value: impl Serialize) -> Self {
        Self::new(field, FilterOp::Eq, value)
    }

    /// `field != value`
    pub fn neq(field: impl Into<String>, value: impl Serialize) -> Self {
        Self::new(field, FilterOp::Ne, value)
    }

    /// `field > value`
    pub fn gt(field: impl Into<String>, value: impl Serialize) -> Self {
        Self::new(field, FilterOp::Gt, value)
    }

    /// `field >= value`
    pub fn gte(field: impl Into<String>, value: impl Serialize) -> Self {
        Self::new(field, FilterOp::Gte, value)
    }

    /// `field < value`
    pub fn lt(field: impl Into<String>, value: impl Serialize) -> Self {
        Self::new(field, FilterOp::Lt, value)
    }

    /// `field <= value`
    pub fn lte(field: impl Into<String>, value: impl Serialize) -> Self {
        Self::new(field, FilterOp::Lte, value)
    }

    /// `field` contains `substring` (case-sensitive).
    pub fn contains(field: impl Into<String>, substring: impl Into<String>) -> Self {
        Self::new(field, FilterOp::Contains, substring.into())
    }

    /// `field` starts with `prefix`.
    pub fn starts_with(field: impl Into<String>, prefix: impl Into<String>) -> Self {
        Self::new(field, FilterOp::StartsWith, prefix.into())
    }
}
