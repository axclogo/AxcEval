use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

fn default_max_levels() -> usize {
    5
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DataType {
    #[default]
    String,
    Number,
    Boolean,
    Date,
    Datetime,
    Enum,
    Array,
    TypedArray,
    Object,
}

impl DataType {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Boolean => "boolean",
            Self::Date => "date",
            Self::Datetime => "datetime",
            Self::Enum => "enum",
            Self::Array => "array",
            Self::TypedArray => "typedArray",
            Self::Object => "object",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_type_names_cover_the_wire_contract() {
        let cases = [
            (DataType::String, "string"),
            (DataType::Number, "number"),
            (DataType::Boolean, "boolean"),
            (DataType::Date, "date"),
            (DataType::Datetime, "datetime"),
            (DataType::Enum, "enum"),
            (DataType::Array, "array"),
            (DataType::TypedArray, "typedArray"),
            (DataType::Object, "object"),
        ];
        for (data_type, name) in cases {
            assert_eq!(data_type.as_str(), name);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptionValue {
    pub value: String,
    pub label: String,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OperatorShape {
    None,
    Scalar,
    List,
    Range,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operator {
    pub name: String,
    #[serde(rename = "valueShape")]
    pub value_shape: OperatorShape,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub name: String,
    pub label: String,
    #[serde(default, rename = "dataType")]
    pub data_type: DataType,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub values: Option<Vec<OptionValue>>,
    #[serde(default)]
    pub operators: Option<Vec<Operator>>,
    #[serde(default, rename = "valueSources")]
    pub value_sources: Option<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub fields: Vec<Field>,
    #[serde(default = "default_max_levels", rename = "maxLevels")]
    pub max_levels: usize,
    #[serde(default = "default_true", rename = "allowNot")]
    pub allow_not: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConditionRule {
    pub field: String,
    pub operator: String,
    pub value: Value,
    #[serde(default, rename = "valueSource")]
    pub value_source: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConditionNode {
    Group(ConditionGroup),
    Rule(ConditionRule),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConditionGroup {
    pub combinator: String,
    pub not: bool,
    pub rules: Vec<ConditionNode>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dialect {
    Clickhouse,
    Duckdb,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Column {
    pub expression: String,
    pub column_type: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SqlOutput {
    pub sql: String,
    pub params: Map<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DecisionGates {
    pub pre: bool,
    pub ai: bool,
    pub post: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum DecisionResult {
    Skipped,
    NeedModel {
        request_id: String,
        model: String,
        prompt: String,
        response_schema: Value,
    },
    Decided {
        request_id: String,
        matched: bool,
        answer: BTreeMap<String, Value>,
        gates: DecisionGates,
    },
}
