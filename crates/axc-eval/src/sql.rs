use crate::catalog::{field_map, validate_catalog};
use crate::error::AxcError;
use crate::model::{
    Catalog, Column, ConditionGroup, ConditionNode, ConditionRule, DataType, Dialect, Field, SqlOutput,
};
use crate::validate::validate_condition_typed;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub fn compile_sql(
    condition_value: &Value,
    catalog_value: &Value,
    dialect: Dialect,
    columns: &BTreeMap<String, Column>,
    allow_empty: bool,
) -> Result<SqlOutput, AxcError> {
    let catalog = validate_catalog(catalog_value)?;
    let issues = validate_condition_typed(condition_value, &catalog, allow_empty);
    if !issues.is_empty() {
        return Err(AxcError::ConditionInvalid { issues });
    }
    for field in referenced_fields(condition_value) {
        if !columns.contains_key(field) {
            return Err(AxcError::Sql {
                message: format!("missing SQL column mapping for field {field:?}"),
            });
        }
    }
    let condition: ConditionGroup = serde_json::from_value(condition_value.clone()).expect("validated condition");
    let mut compiler = Compiler {
        catalog: &catalog,
        dialect,
        columns,
        params: Map::new(),
        next_param: 0,
    };
    let sql = compiler.group(&condition)?;
    Ok(SqlOutput {
        sql,
        params: compiler.params,
    })
}

fn referenced_fields(value: &Value) -> Vec<&str> {
    fn walk<'a>(value: &'a Value, fields: &mut Vec<&'a str>) {
        if let Some(rules) = value.get("rules").and_then(Value::as_array) {
            for node in rules {
                if node.get("rules").is_some() {
                    walk(node, fields);
                } else {
                    if let Some(field) = node.get("field").and_then(Value::as_str) {
                        fields.push(field);
                    }
                    if node.get("valueSource").and_then(Value::as_str) == Some("field")
                        && let Some(field) = node.get("value").and_then(Value::as_str)
                    {
                        fields.push(field);
                    }
                }
            }
        }
    }
    let mut result = Vec::new();
    walk(value, &mut result);
    result
}

struct Compiler<'a> {
    catalog: &'a Catalog,
    dialect: Dialect,
    columns: &'a BTreeMap<String, Column>,
    params: Map<String, Value>,
    next_param: usize,
}

impl Compiler<'_> {
    fn group(&mut self, group: &ConditionGroup) -> Result<String, AxcError> {
        if group.rules.is_empty() {
            return Ok("TRUE".to_owned());
        }
        let mut parts = Vec::new();
        for node in &group.rules {
            parts.push(match node {
                ConditionNode::Group(child) => self.group(child)?,
                ConditionNode::Rule(rule) => self.rule(rule)?,
            });
        }
        let joined = format!(
            "({})",
            parts.join(if group.combinator == "and" { " AND " } else { " OR " })
        );
        Ok(if group.not { format!("NOT {joined}") } else { joined })
    }

    fn rule(&mut self, rule: &ConditionRule) -> Result<String, AxcError> {
        let fields = field_map(self.catalog);
        let field = fields[rule.field.as_str()];
        let left = &self.columns[&rule.field].expression;
        let empty = self.empty(left, field);
        if rule.operator == "null" {
            return Ok(empty);
        }
        if rule.operator == "notNull" {
            return Ok(format!("NOT ({empty})"));
        }
        if self.dialect == Dialect::Duckdb && matches!(rule.operator.as_str(), "regex" | "notRegex") {
            return Err(AxcError::Sql {
                message: "DuckDB SQL compilation does not support regex operators".to_owned(),
            });
        }
        let right = if rule.value_source.as_deref() == Some("field") {
            SqlValue::Expression(
                self.columns[rule.value.as_str().expect("validated reference")]
                    .expression
                    .clone(),
            )
        } else {
            SqlValue::Value(rule.value.clone())
        };
        let negative = matches!(
            rule.operator.as_str(),
            "!=" | "notIn" | "notBetween" | "doesNotContain" | "notRegex" | "hasNone"
        );
        let core = self.operation(left, field, &rule.operator, right)?;
        let null_result = if negative { "TRUE" } else { "FALSE" };
        Ok(match self.dialect {
            Dialect::Clickhouse => format!("ifNull(({core}), {null_result})"),
            Dialect::Duckdb => format!("COALESCE(({core}), {null_result})"),
        })
    }

    fn operation(&mut self, left: &str, field: &Field, operator: &str, right: SqlValue) -> Result<String, AxcError> {
        let scalar = |compiler: &mut Self, right: SqlValue| compiler.render(right, field);
        Ok(match operator {
            "=" => format!("{left} = {}", scalar(self, right)),
            "!=" => format!("{left} <> {}", scalar(self, right)),
            ">" | ">=" | "<" | "<=" => format!("{left} {operator} {}", scalar(self, right)),
            "between" | "notBetween" => {
                let values = match right {
                    SqlValue::Value(Value::Array(values)) => values,
                    _ => unreachable!("field references cannot be ranges"),
                };
                let lower = self.bind(&values[0], field);
                let upper = self.bind(&values[1], field);
                format!(
                    "{left} {}BETWEEN {lower} AND {upper}",
                    if operator == "notBetween" { "NOT " } else { "" }
                )
            }
            "in" | "notIn" => {
                let values = match right {
                    SqlValue::Value(Value::Array(values)) => values,
                    _ => unreachable!("field references cannot be lists"),
                };
                let rendered = values
                    .iter()
                    .map(|value| self.bind(value, field))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "{left} {}IN ({rendered})",
                    if operator == "notIn" { "NOT " } else { "" }
                )
            }
            "contains" | "doesNotContain" => {
                let needle = self.pattern(right, field);
                let contains = self.contains(left, &needle, field);
                if operator == "doesNotContain" {
                    format!("NOT ({contains})")
                } else {
                    contains
                }
            }
            "containsAny" | "containsAll" => {
                let values = match right {
                    SqlValue::Value(Value::Array(values)) => values,
                    _ => unreachable!("word-list field references are rejected"),
                };
                let glue = if operator == "containsAll" { " AND " } else { " OR " };
                let parts = values
                    .iter()
                    .map(|value| {
                        let needle = self.pattern(SqlValue::Value(value.clone()), field);
                        self.contains(left, &needle, field)
                    })
                    .collect::<Vec<_>>();
                format!("({})", parts.join(glue))
            }
            "beginsWith" => {
                let right = self.pattern(right, field);
                self.like(left, &right, "prefix")
            }
            "endsWith" => {
                let right = self.pattern(right, field);
                self.like(left, &right, "suffix")
            }
            "regex" | "notRegex" => {
                let expression = format!("match({left}, {})", scalar(self, right));
                if operator == "notRegex" {
                    format!("NOT ({expression})")
                } else {
                    expression
                }
            }
            "isZero" => format!("{left} = 0"),
            "isNotZero" => format!("{left} <> 0"),
            "hasAny" | "hasAll" | "hasNone" => {
                let values = match right {
                    SqlValue::Value(Value::Array(values)) => values,
                    _ => unreachable!("array membership field references are rejected"),
                };
                let parts = values
                    .iter()
                    .map(|value| match (self.dialect, field.data_type) {
                        (Dialect::Clickhouse, DataType::TypedArray) => {
                            let item = value.as_object().expect("validated typed-array value");
                            let item_type = self.bind_clickhouse(&item["type"], "String");
                            let item_value = self.bind_clickhouse(&item["value"], "String");
                            format!("arrayExists(item -> item.1 = {item_type} AND item.2 = {item_value}, {left})")
                        }
                        (Dialect::Clickhouse, _) => {
                            let bound = self.bind(value, field);
                            format!("has({left}, {bound})")
                        }
                        (Dialect::Duckdb, _) => {
                            let bound = self.bind(value, field);
                            format!("list_contains({left}, {bound})")
                        }
                    })
                    .collect::<Vec<_>>();
                let expression = format!("({})", parts.join(if operator == "hasAll" { " AND " } else { " OR " }));
                if operator == "hasNone" {
                    format!("NOT {expression}")
                } else {
                    expression
                }
            }
            _ => {
                return Err(AxcError::Sql {
                    message: format!("operator {operator:?} cannot be compiled"),
                });
            }
        })
    }

    fn contains(&self, left: &str, needle: &str, field: &Field) -> String {
        match (self.dialect, field.data_type) {
            (Dialect::Clickhouse, DataType::Array) => {
                format!("arrayExists(item -> positionCaseInsensitiveUTF8(item, {needle}) > 0, {left})")
            }
            (Dialect::Clickhouse, DataType::TypedArray) => {
                format!("arrayExists(item -> positionCaseInsensitiveUTF8(item.2, {needle}) > 0, {left})")
            }
            (Dialect::Clickhouse, _) => format!("positionCaseInsensitiveUTF8({left}, {needle}) > 0"),
            (Dialect::Duckdb, DataType::Array) => {
                format!(
                    "list_has_any(list_transform({left}, item -> lower(item) LIKE '%' || lower({needle}) || '%'), [TRUE])"
                )
            }
            (Dialect::Duckdb, DataType::TypedArray) => {
                format!(
                    "list_has_any(list_transform({left}, item -> lower(item.value) LIKE '%' || lower({needle}) || '%'), [TRUE])"
                )
            }
            (Dialect::Duckdb, _) => format!("lower({left}) LIKE '%' || lower({needle}) || '%' ESCAPE '\\\\'"),
        }
    }

    fn like(&self, left: &str, right: &str, kind: &str) -> String {
        match (self.dialect, kind) {
            (Dialect::Clickhouse, "prefix") => format!("startsWith({left}, {right})"),
            (Dialect::Clickhouse, _) => format!("endsWith({left}, {right})"),
            (Dialect::Duckdb, "prefix") => format!("{left} LIKE {right} || '%' ESCAPE '\\\\'"),
            (Dialect::Duckdb, _) => format!("{left} LIKE '%' || {right} ESCAPE '\\\\'"),
        }
    }

    fn empty(&self, expression: &str, field: &Field) -> String {
        match field.data_type {
            DataType::String | DataType::Enum => format!("({expression} IS NULL OR {expression} = '')"),
            DataType::Array | DataType::TypedArray => match self.dialect {
                Dialect::Clickhouse => format!("({expression} IS NULL OR length({expression}) = 0)"),
                Dialect::Duckdb => format!("({expression} IS NULL OR len({expression}) = 0)"),
            },
            _ => format!("{expression} IS NULL"),
        }
    }

    fn render(&mut self, value: SqlValue, field: &Field) -> String {
        match value {
            SqlValue::Expression(expression) => expression,
            SqlValue::Value(value) => self.bind(&value, field),
        }
    }

    fn pattern(&mut self, value: SqlValue, field: &Field) -> String {
        match (self.dialect, value) {
            (Dialect::Duckdb, SqlValue::Expression(expression)) => {
                format!("replace(replace(replace({expression}, '\\\\', '\\\\\\\\'), '%', '\\\\%'), '_', '\\\\_')")
            }
            (Dialect::Duckdb, SqlValue::Value(Value::String(value))) => {
                let escaped = value.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
                self.bind(&Value::String(escaped), field)
            }
            (_, value) => self.render(value, field),
        }
    }

    fn bind(&mut self, value: &Value, field: &Field) -> String {
        match self.dialect {
            Dialect::Clickhouse => self.bind_clickhouse(value, clickhouse_type(value, field)),
            Dialect::Duckdb => literal(value),
        }
    }

    fn bind_clickhouse(&mut self, value: &Value, value_type: &str) -> String {
        let name = format!("p{}", self.next_param);
        self.next_param += 1;
        self.params.insert(name.clone(), value.clone());
        format!("{{{name}:{value_type}}}")
    }
}

enum SqlValue {
    Expression(String),
    Value(Value),
}

fn clickhouse_type(value: &Value, field: &Field) -> &'static str {
    if value.is_array() {
        return match field.data_type {
            DataType::Number => "Array(Float64)",
            _ => "Array(String)",
        };
    }
    match field.data_type {
        DataType::Number => "Float64",
        DataType::Boolean => "UInt8",
        DataType::Date => "Date",
        DataType::Datetime => "DateTime64(9)",
        _ => "String",
    }
}

fn literal(value: &Value) -> String {
    match value {
        Value::Null => "NULL".to_owned(),
        Value::Bool(value) => if *value { "TRUE" } else { "FALSE" }.to_owned(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => format!("'{}'", value.replace('\'', "''")),
        Value::Array(values) => format!("[{}]", values.iter().map(literal).collect::<Vec<_>>().join(", ")),
        Value::Object(map) => format!(
            "{{'type': {}, 'value': {}}}",
            literal(&map["type"]),
            literal(&map["value"])
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn emits_bound_clickhouse_and_escaped_duckdb_sql() {
        let catalog = json!({"fields":[{"name":"title","label":"Title"}]});
        let condition = json!({"combinator":"and","not":false,"rules":[
            {"field":"title","operator":"contains","value":"a%b"}
        ]});
        let columns = BTreeMap::from([(
            "title".to_owned(),
            Column {
                expression: "title_col".to_owned(),
                column_type: "String".to_owned(),
            },
        )]);
        let clickhouse = compile_sql(&condition, &catalog, Dialect::Clickhouse, &columns, false).unwrap();
        assert!(clickhouse.sql.contains("{p0:String}"));
        assert_eq!(clickhouse.params["p0"], "a%b");
        let duckdb = compile_sql(&condition, &catalog, Dialect::Duckdb, &columns, false).unwrap();
        assert!(duckdb.sql.contains("a\\%b"));

        let equality = json!({"combinator":"and","not":false,"rules":[
            {"field":"title","operator":"=","value":"a%b"}
        ]});
        let duckdb = compile_sql(&equality, &catalog, Dialect::Duckdb, &columns, false).unwrap();
        assert!(duckdb.sql.contains("= 'a%b'"));
    }

    #[test]
    fn defensive_sql_helpers_cover_all_json_and_type_forms() {
        let field = Field {
            name: "value".to_owned(),
            label: "Value".to_owned(),
            data_type: DataType::Number,
            description: None,
            disabled: false,
            values: None,
            operators: None,
            value_sources: None,
        };
        assert_eq!(clickhouse_type(&json!([1, 2]), &field), "Array(Float64)");
        assert_eq!(literal(&Value::Null), "NULL");
        assert_eq!(literal(&json!([true, 1, "x"])), "[TRUE, 1, 'x']");

        let catalog =
            validate_catalog(&json!({"fields":[{"name":"value","label":"Value","dataType":"number"}]})).unwrap();
        let columns = BTreeMap::from([(
            "value".to_owned(),
            Column {
                expression: "value".to_owned(),
                column_type: "Float64".to_owned(),
            },
        )]);
        let mut compiler = Compiler {
            catalog: &catalog,
            dialect: Dialect::Duckdb,
            columns: &columns,
            params: Map::new(),
            next_param: 0,
        };
        assert!(matches!(
            compiler.operation("value", &field, "unsupported", SqlValue::Value(json!(1))),
            Err(AxcError::Sql { .. })
        ));
    }
}
