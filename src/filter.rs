use serde::Serialize;
use serde_json::Value;

/// Filter operator for `POST /v1/search` and `POST /v1/update`.
///
/// See Section 9 of the API definition for full semantics.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FilterOp {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
    /// Starts-with — `begins_with(field, value)`.
    #[serde(rename = "sw")]
    Sw,
    /// Exists — `attribute_exists(field)`. The value is ignored by the server.
    #[serde(rename = "ex")]
    Ex,
}

/// A single filter condition. Multiple filters in a request are AND-ed.
///
/// Build with the convenience constructors or construct directly.
///
/// ```rust
/// use flex_db::{SearchFilter, FilterOp};
///
/// let f1 = SearchFilter::eq("status", "active");
/// let f2 = SearchFilter::new("score", FilterOp::Gte, 10u32);
/// let f3 = SearchFilter::starts_with("label", "prod-");
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
        Self::new(field, FilterOp::Neq, value)
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

    /// `field` starts with `prefix`.
    pub fn starts_with(field: impl Into<String>, prefix: impl Into<String>) -> Self {
        Self::new(field, FilterOp::Sw, prefix.into())
    }

    /// `field` exists in `metadata.sp`.
    pub fn exists(field: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            op: FilterOp::Ex,
            value: Value::Bool(true),
        }
    }
}
