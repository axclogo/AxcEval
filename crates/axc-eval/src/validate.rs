use crate::catalog::{field_map, operators_for, validate_catalog};
use crate::date_time::{parse_date, parse_datetime};
use crate::error::{AxcError, Issue, PathItem};
use crate::model::{Catalog, DataType, Field, Operator, OperatorShape};
use regex::Regex;
use serde_json::{Map, Value};
use std::collections::HashSet;

pub fn validate_condition(value: &Value, catalog_value: &Value, allow_empty: bool) -> Result<Vec<Issue>, AxcError> {
    let catalog = validate_catalog(catalog_value)?;
    Ok(validate_condition_typed(value, &catalog, allow_empty))
}

pub(crate) fn validate_condition_typed(value: &Value, catalog: &Catalog, allow_empty: bool) -> Vec<Issue> {
    let mut issues = Vec::new();
    check_group(value, catalog, 1, Vec::new(), allow_empty, &mut issues);
    issues
}

fn object<'a>(
    value: &'a Value,
    keys: &[&str],
    path: &[PathItem],
    issues: &mut Vec<Issue>,
) -> Option<&'a Map<String, Value>> {
    let Some(map) = value.as_object() else {
        issues.push(Issue::new("type", path.to_vec()).param("expected", "object"));
        return None;
    };
    for key in map.keys() {
        if !keys.contains(&key.as_str()) {
            issues.push(Issue::new("unknownKey", extend(path, key.as_str())).param("key", key.clone()));
        }
    }
    Some(map)
}

fn check_group(
    value: &Value,
    catalog: &Catalog,
    depth: usize,
    path: Vec<PathItem>,
    allow_empty: bool,
    issues: &mut Vec<Issue>,
) {
    let Some(map) = object(value, &["combinator", "not", "rules"], &path, issues) else {
        return;
    };
    if depth > catalog.max_levels {
        issues.push(Issue::new("tooDeep", path).param("max", catalog.max_levels));
        return;
    }
    if !matches!(map.get("combinator").and_then(Value::as_str), Some("and" | "or")) {
        issues.push(Issue::new("combinator", extend(&path, "combinator")));
    }
    match map.get("not") {
        Some(Value::Bool(true)) if !catalog.allow_not => {
            issues.push(Issue::new("negationDisabled", extend(&path, "not")));
        }
        Some(Value::Bool(_)) => {}
        _ => issues.push(Issue::new("type", extend(&path, "not")).param("expected", "boolean")),
    }
    let Some(rules) = map.get("rules").and_then(Value::as_array) else {
        issues.push(Issue::new("type", extend(&path, "rules")).param("expected", "array"));
        return;
    };
    if rules.is_empty() && !(allow_empty && depth == 1) {
        issues.push(Issue::new("empty", extend(&path, "rules")));
    }
    for (index, child) in rules.iter().enumerate() {
        let child_path = extend(&extend(&path, "rules"), index);
        if child.as_object().is_some_and(|node| node.contains_key("rules")) {
            check_group(child, catalog, depth + 1, child_path, false, issues);
        } else {
            check_rule(child, catalog, child_path, issues);
        }
    }
}

fn check_rule(value: &Value, catalog: &Catalog, path: Vec<PathItem>, issues: &mut Vec<Issue>) {
    let Some(map) = object(value, &["field", "operator", "value", "valueSource"], &path, issues) else {
        return;
    };
    let fields = field_map(catalog);
    let Some(field_value) = map.get("field") else {
        issues.push(Issue::new("required", extend(&path, "field")));
        return;
    };
    let Some(field_name) = field_value.as_str() else {
        issues.push(Issue::new("type", extend(&path, "field")).param("expected", "string"));
        return;
    };
    let Some(field) = fields.get(field_name).copied().filter(|field| !field.disabled) else {
        issues.push(Issue::new("unknownField", extend(&path, "field")).param("field", field_name));
        return;
    };
    let Some(operator_value) = map.get("operator") else {
        issues.push(Issue::new("required", extend(&path, "operator")));
        return;
    };
    let Some(operator_name) = operator_value.as_str() else {
        issues.push(Issue::new("type", extend(&path, "operator")).param("expected", "string"));
        return;
    };
    let Some(operator) = operators_for(field)
        .into_iter()
        .find(|candidate| candidate.name == operator_name)
    else {
        issues.push(Issue::new("unknownOperator", extend(&path, "operator")).param("operator", operator_name));
        return;
    };
    let Some(value) = map.get("value") else {
        issues.push(Issue::new("required", extend(&path, "value")));
        return;
    };
    if !map.contains_key("valueSource") {
        if field_ref_only(field, &operator) {
            issues.push(Issue::new("fieldRefRequired", extend(&path, "value")));
        } else {
            check_value(value, field, &operator, extend(&path, "value"), issues);
        }
        return;
    }
    let field_source_allowed = field
        .value_sources
        .as_ref()
        .is_some_and(|sources| sources.iter().any(|source| source == "field"));
    if map.get("valueSource").and_then(Value::as_str) != Some("field")
        || !field_source_allowed
        || operator.value_shape != OperatorShape::Scalar
    {
        issues.push(Issue::new("fieldRefInvalid", extend(&path, "valueSource")));
        return;
    }
    let valid_target = value
        .as_str()
        .and_then(|name| fields.get(name).copied())
        .is_some_and(|target| !target.disabled && target.data_type == field.data_type);
    if !valid_target {
        issues.push(Issue::new("fieldRefInvalid", extend(&path, "value")));
    }
}

fn field_ref_only(field: &Field, operator: &Operator) -> bool {
    operator.value_shape != OperatorShape::None
        && field
            .value_sources
            .as_deref()
            .is_some_and(|sources| sources == ["field"])
}

fn check_value(value: &Value, field: &Field, operator: &Operator, path: Vec<PathItem>, issues: &mut Vec<Issue>) {
    match operator.value_shape {
        OperatorShape::None => {
            if !value.is_null() {
                issues.push(Issue::new("type", path).param("expected", "null"));
            }
        }
        OperatorShape::Scalar => {
            check_item(value, field, operator, path, issues);
        }
        OperatorShape::List | OperatorShape::Range => {
            let Some(items) = value.as_array() else {
                issues.push(Issue::new("type", path).param("expected", "array"));
                return;
            };
            if items.is_empty() {
                issues.push(Issue::new("empty", path));
                return;
            }
            if operator.value_shape == OperatorShape::Range && items.len() != 2 {
                issues.push(Issue::new("rangeLength", path));
                return;
            }
            let mut valid = true;
            let mut seen = HashSet::new();
            for (index, item) in items.iter().enumerate() {
                let before = issues.len();
                check_item(item, field, operator, extend(&path, index), issues);
                if before != issues.len() {
                    valid = false;
                } else if operator.value_shape == OperatorShape::List {
                    let key = serde_json::to_string(item).expect("serializing JSON value cannot fail");
                    if !seen.insert(key) {
                        issues.push(Issue::new("duplicate", extend(&path, index)).param("value", display_value(item)));
                    }
                }
            }
            if operator.value_shape == OperatorShape::Range && valid && compare_items(&items[0], &items[1], field) > 0 {
                issues.push(Issue::new("rangeOrder", path));
            }
        }
    }
}

fn check_item(value: &Value, field: &Field, operator: &Operator, path: Vec<PathItem>, issues: &mut Vec<Issue>) {
    if field.data_type == DataType::TypedArray && matches!(operator.name.as_str(), "hasAny" | "hasAll" | "hasNone") {
        check_typed_item(value, field, path, issues);
        return;
    }
    let expected = match field.data_type {
        DataType::Number => "number",
        DataType::Boolean => "boolean",
        _ => "string",
    };
    let valid_type = match expected {
        "number" => value.as_f64().is_some_and(f64::is_finite),
        "boolean" => value.is_boolean(),
        _ => value.is_string(),
    };
    if !valid_type {
        issues.push(Issue::new("type", path).param("expected", expected));
        return;
    }
    let Some(text) = value.as_str() else { return };
    if text.trim().is_empty() {
        issues.push(Issue::new("required", path));
        return;
    }
    if let Some(options) = &field.values
        && !matches!(field.data_type, DataType::TypedArray)
        && !options.iter().any(|option| !option.disabled && option.value == text)
    {
        issues.push(Issue::new("option", path).param("value", text));
        return;
    }
    if field.data_type == DataType::Date && parse_date(text).is_none() {
        issues.push(Issue::new("date", path));
    } else if field.data_type == DataType::Datetime && parse_datetime(text).is_none() {
        issues.push(Issue::new("datetime", path));
    } else if matches!(operator.name.as_str(), "regex" | "notRegex") && Regex::new(text).is_err() {
        issues.push(Issue::new("regex", path));
    }
}

fn check_typed_item(value: &Value, field: &Field, path: Vec<PathItem>, issues: &mut Vec<Issue>) {
    let Some(map) = object(value, &["type", "value"], &path, issues) else {
        return;
    };
    for key in ["type", "value"] {
        let key_path = extend(&path, key);
        match map.get(key) {
            Some(Value::String(text)) if !text.trim().is_empty() => {
                if key == "type"
                    && field
                        .values
                        .as_ref()
                        .is_some_and(|options| !options.iter().any(|option| !option.disabled && option.value == *text))
                {
                    issues.push(Issue::new("option", key_path).param("value", text.as_str()));
                }
            }
            Some(Value::String(_)) => issues.push(Issue::new("required", key_path)),
            Some(_) => issues.push(Issue::new("type", key_path).param("expected", "string")),
            None => issues.push(Issue::new("required", key_path)),
        }
    }
}

fn compare_items(left: &Value, right: &Value, field: &Field) -> i8 {
    let order = match field.data_type {
        DataType::Number => left.as_f64().partial_cmp(&right.as_f64()),
        DataType::Date => left
            .as_str()
            .and_then(parse_date)
            .partial_cmp(&right.as_str().and_then(parse_date)),
        DataType::Datetime => left
            .as_str()
            .and_then(parse_datetime)
            .partial_cmp(&right.as_str().and_then(parse_datetime)),
        _ => left.as_str().partial_cmp(&right.as_str()),
    };
    match order {
        Some(std::cmp::Ordering::Less) => -1,
        Some(std::cmp::Ordering::Greater) => 1,
        _ => 0,
    }
}

pub(crate) fn extend(path: &[PathItem], item: impl Into<PathItem>) -> Vec<PathItem> {
    let mut result = path.to_vec();
    result.push(item.into());
    result
}

fn display_value(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        value => value.to_string(),
    }
}
