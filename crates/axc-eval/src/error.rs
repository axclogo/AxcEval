use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PathItem {
    Key(String),
    Index(usize),
}

impl From<&str> for PathItem {
    fn from(value: &str) -> Self {
        Self::Key(value.to_owned())
    }
}

impl From<String> for PathItem {
    fn from(value: String) -> Self {
        Self::Key(value)
    }
}

impl From<usize> for PathItem {
    fn from(value: usize) -> Self {
        Self::Index(value)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Issue {
    pub code: String,
    pub path: Vec<PathItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Map<String, Value>>,
}

impl Issue {
    pub(crate) fn new(code: &str, path: Vec<PathItem>) -> Self {
        Self {
            code: code.to_owned(),
            path,
            params: None,
        }
    }

    pub(crate) fn param(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.params
            .get_or_insert_with(Map::new)
            .insert(key.to_owned(), value.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogError {
    pub message: String,
}

impl CatalogError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for CatalogError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CatalogError {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordTypeError {
    pub field: String,
    pub expected: String,
}

impl Display for RecordTypeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "record field {:?} must be {}", self.field, self.expected)
    }
}

impl std::error::Error for RecordTypeError {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AxcError {
    Catalog { message: String },
    ConditionInvalid { issues: Vec<Issue> },
    DecisionInvalid { issues: Vec<Issue> },
    RecordType { field: String, expected: String },
    Sql { message: String },
}

impl Display for AxcError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Catalog { message } | Self::Sql { message } => f.write_str(message),
            Self::ConditionInvalid { issues } => write!(f, "condition has {} issue(s)", issues.len()),
            Self::DecisionInvalid { issues } => write!(f, "decision has {} issue(s)", issues.len()),
            Self::RecordType { field, expected } => write!(f, "record field {field:?} must be {expected}"),
        }
    }
}

impl std::error::Error for AxcError {}

impl From<CatalogError> for AxcError {
    fn from(value: CatalogError) -> Self {
        Self::Catalog { message: value.message }
    }
}

impl From<RecordTypeError> for AxcError {
    fn from(value: RecordTypeError) -> Self {
        Self::RecordType {
            field: value.field,
            expected: value.expected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_error_conversion_preserves_contract() {
        assert_eq!(
            AxcError::from(CatalogError {
                message: "invalid".to_owned(),
            }),
            AxcError::Catalog {
                message: "invalid".to_owned(),
            }
        );
        let error = AxcError::from(RecordTypeError {
            field: "count".to_owned(),
            expected: "number".to_owned(),
        });
        assert_eq!(
            error,
            AxcError::RecordType {
                field: "count".to_owned(),
                expected: "number".to_owned(),
            }
        );
    }
}
