use crate::catalog::{field_map, validate_catalog};
use crate::date_time::{parse_date, parse_datetime};
use crate::error::{AxcError, PathItem, RecordTypeError};
use crate::evidence::{EvaluationEvidence, EvidenceKind, emit_evidence};
use crate::model::{Catalog, ConditionGroup, ConditionNode, ConditionRule, DataType, Field};
use crate::simulation::{ConditionSimulation, ConditionTrace, SkipReason};
use crate::validate::validate_condition_typed;
use regex::Regex;
use serde_json::{Map, Value};
use std::cmp::Ordering;

/// Validates and evaluates a condition against one record.
///
/// When `on_evidence` is present, it receives exactly one deterministic snapshot
/// for either the result or a structured error. No snapshot is constructed when
/// the callback is absent.
pub fn matches(
    condition_value: &Value,
    catalog_value: &Value,
    record: &Map<String, Value>,
    allow_empty: bool,
    on_evidence: Option<&mut dyn FnMut(&EvaluationEvidence)>,
) -> Result<bool, AxcError> {
    let result = validate_catalog(catalog_value)
        .map_err(AxcError::from)
        .and_then(|catalog| matches_typed(condition_value, &catalog, record, allow_empty));
    emit_evidence(
        EvidenceKind::Condition,
        || {
            serde_json::json!({
                "operation": "match",
                "condition": condition_value,
                "catalog": catalog_value,
                "record": record,
                "allowEmpty": allow_empty,
            })
        },
        &result,
        on_evidence,
    );
    result
}

/// Evaluates a condition with an exact trace of evaluated and short-circuited nodes.
///
/// Simulation uses the same validation, record checks, operators, and short-circuit
/// engine as [`matches`]. It never evaluates a node that normal execution would skip.
/// When `on_evidence` is present, it receives exactly one snapshot for the attempt.
pub fn simulate_condition(
    condition_value: &Value,
    catalog_value: &Value,
    record: &Map<String, Value>,
    allow_empty: bool,
    on_evidence: Option<&mut dyn FnMut(&EvaluationEvidence)>,
) -> Result<ConditionSimulation, AxcError> {
    let result = validate_catalog(catalog_value)
        .map_err(AxcError::from)
        .and_then(|catalog| execute_typed(condition_value, &catalog, record, allow_empty, true))
        .map(|execution| {
            ConditionSimulation::from_trace(execution.trace.expect("simulation always captures the root trace"))
        });
    emit_evidence(
        EvidenceKind::Condition,
        || {
            serde_json::json!({
                "operation": "simulate",
                "condition": condition_value,
                "catalog": catalog_value,
                "record": record,
                "allowEmpty": allow_empty,
            })
        },
        &result,
        on_evidence,
    );
    result
}

pub(crate) fn matches_typed(
    condition_value: &Value,
    catalog: &Catalog,
    record: &Map<String, Value>,
    allow_empty: bool,
) -> Result<bool, AxcError> {
    Ok(execute_typed(condition_value, catalog, record, allow_empty, false)?.matched)
}

fn execute_typed(
    condition_value: &Value,
    catalog: &Catalog,
    record: &Map<String, Value>,
    allow_empty: bool,
    capture_trace: bool,
) -> Result<ConditionExecution, AxcError> {
    let issues = validate_condition_typed(condition_value, catalog, allow_empty);
    if !issues.is_empty() {
        return Err(AxcError::ConditionInvalid { issues });
    }
    let condition: ConditionGroup = serde_json::from_value(condition_value.clone())
        .expect("validated condition must deserialize into ConditionGroup");
    validate_record(&condition, catalog, record)?;
    if capture_trace {
        evaluate_group(&condition, catalog, record, Some(&[]))
    } else {
        evaluate_group(&condition, catalog, record, None)
    }
}

fn validate_record(
    group: &ConditionGroup,
    catalog: &Catalog,
    record: &Map<String, Value>,
) -> Result<(), RecordTypeError> {
    let fields = field_map(catalog);
    for node in &group.rules {
        match node {
            ConditionNode::Group(child) => validate_record(child, catalog, record)?,
            ConditionNode::Rule(rule) => {
                let field = fields[rule.field.as_str()];
                validate_record_value(field, record.get(&rule.field))?;
                if rule.value_source.as_deref() == Some("field") {
                    let target = rule.value.as_str().expect("validated field reference must be a string");
                    validate_record_value(fields[target], record.get(target))?;
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_record_value(field: &Field, value: Option<&Value>) -> Result<(), RecordTypeError> {
    let valid = match value {
        Some(Value::Null) => true,
        Some(Value::String(text)) => match field.data_type {
            DataType::String | DataType::Enum => true,
            DataType::Date => parse_date(text).is_some(),
            DataType::Datetime => parse_datetime(text).is_some(),
            _ => false,
        },
        Some(Value::Number(number)) => {
            field.data_type == DataType::Number && number.as_f64().is_some_and(f64::is_finite)
        }
        Some(Value::Bool(_)) => field.data_type == DataType::Boolean,
        Some(Value::Array(items)) if field.data_type == DataType::Array => items.iter().all(Value::is_string),
        Some(Value::Array(items)) if field.data_type == DataType::TypedArray => items.iter().all(|item| {
            item.as_object().is_some_and(|map| {
                map.len() == 2
                    && map.get("type").is_some_and(Value::is_string)
                    && map.get("value").is_some_and(Value::is_string)
            })
        }),
        Some(Value::Object(_)) => field.data_type == DataType::Object,
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(RecordTypeError {
            field: field.name.clone(),
            expected: field.data_type.as_str().to_owned(),
        })
    }
}

struct ConditionExecution {
    matched: bool,
    trace: Option<ConditionTrace>,
}

fn evaluate_group(
    group: &ConditionGroup,
    catalog: &Catalog,
    record: &Map<String, Value>,
    path: Option<&[PathItem]>,
) -> Result<ConditionExecution, AxcError> {
    let capture_trace = path.is_some();
    let is_and = group.combinator == "and";
    let mut raw_matched = is_and;
    let mut short_circuit_at = None;
    let mut children = capture_trace.then(|| Vec::with_capacity(group.rules.len()));

    for (index, node) in group.rules.iter().enumerate() {
        let child_path = path.map(|path| trace_child_path(path, index));
        if short_circuit_at.is_some() {
            if let (Some(children), Some(child_path)) = (children.as_mut(), child_path) {
                children.push(ConditionTrace::Skipped {
                    path: child_path,
                    reason: if is_and {
                        SkipReason::AndShortCircuit
                    } else {
                        SkipReason::OrShortCircuit
                    },
                });
            }
            continue;
        }

        let execution = match node {
            ConditionNode::Group(child) => evaluate_group(child, catalog, record, child_path.as_deref())?,
            ConditionNode::Rule(rule) => {
                let matched = evaluate_rule(rule, catalog, record)?;
                ConditionExecution {
                    matched,
                    trace: child_path.map(|path| ConditionTrace::Rule {
                        path,
                        field: rule.field.clone(),
                        operator: rule.operator.clone(),
                        matched,
                    }),
                }
            }
        };
        if let (Some(children), Some(trace)) = (children.as_mut(), execution.trace) {
            children.push(trace);
        }
        if is_and {
            raw_matched &= execution.matched;
            if !raw_matched {
                short_circuit_at = Some(index);
            }
        } else {
            raw_matched |= execution.matched;
            if raw_matched {
                short_circuit_at = Some(index);
            }
        }
        if short_circuit_at.is_some() && !capture_trace {
            break;
        }
    }

    let matched = if group.not { !raw_matched } else { raw_matched };
    let trace = match (children, path) {
        (Some(children), Some(path)) => Some(ConditionTrace::Group {
            path: path.to_vec(),
            combinator: group.combinator.clone(),
            negated: group.not,
            raw_matched,
            matched,
            short_circuit_at,
            children,
        }),
        (None, None) => None,
        _ => unreachable!("trace storage and trace paths are enabled together"),
    };
    Ok(ConditionExecution { matched, trace })
}

fn trace_child_path(path: &[PathItem], index: usize) -> Vec<PathItem> {
    let mut child = Vec::with_capacity(path.len() + 2);
    child.extend_from_slice(path);
    child.push("rules".into());
    child.push(index.into());
    child
}

fn evaluate_rule(rule: &ConditionRule, catalog: &Catalog, record: &Map<String, Value>) -> Result<bool, AxcError> {
    let fields = field_map(catalog);
    let field = fields[rule.field.as_str()];
    let left = &record[&rule.field];
    let right = if rule.value_source.as_deref() == Some("field") {
        &record[rule.value.as_str().expect("validated reference")]
    } else {
        &rule.value
    };
    let left_empty = is_empty(left, field);
    if rule.operator == "null" {
        return Ok(left_empty);
    }
    if rule.operator == "notNull" {
        return Ok(!left_empty);
    }
    let negative = matches!(
        rule.operator.as_str(),
        "!=" | "notIn" | "notBetween" | "doesNotContain" | "notRegex" | "hasNone"
    );
    if left_empty || (rule.value_source.is_some() && is_empty(right, field)) {
        return Ok(negative);
    }
    let result = match rule.operator.as_str() {
        "=" => equal_typed(left, right, field),
        "!=" => !equal_typed(left, right, field),
        ">" => compare(left, right, field) == Some(Ordering::Greater),
        ">=" => matches!(compare(left, right, field), Some(Ordering::Greater | Ordering::Equal)),
        "<" => compare(left, right, field) == Some(Ordering::Less),
        "<=" => matches!(compare(left, right, field), Some(Ordering::Less | Ordering::Equal)),
        "between" | "notBetween" => {
            let bounds = right.as_array().expect("validated range");
            let inside = !matches!(compare(left, &bounds[0], field), Some(Ordering::Less))
                && !matches!(compare(left, &bounds[1], field), Some(Ordering::Greater));
            if rule.operator == "notBetween" { !inside } else { inside }
        }
        "in" | "notIn" => {
            let found = right
                .as_array()
                .expect("validated list")
                .iter()
                .any(|candidate| equal_typed(left, candidate, field));
            if rule.operator == "notIn" { !found } else { found }
        }
        "contains" | "doesNotContain" => {
            let needle = right.as_str().expect("validated text");
            let found = contains(left, needle, field);
            if rule.operator == "doesNotContain" {
                !found
            } else {
                found
            }
        }
        "beginsWith" => left
            .as_str()
            .is_some_and(|text| text.starts_with(right.as_str().expect("validated text"))),
        "endsWith" => left
            .as_str()
            .is_some_and(|text| text.ends_with(right.as_str().expect("validated text"))),
        "regex" | "notRegex" => {
            let regex = Regex::new(right.as_str().expect("validated regex")).expect("validated regex must compile");
            let found = left.as_str().is_some_and(|text| regex.is_match(text));
            if rule.operator == "notRegex" { !found } else { found }
        }
        "containsAny" | "containsAll" => {
            let needles = right.as_array().expect("validated list");
            let predicate = |needle: &Value| contains(left, needle.as_str().expect("validated text"), field);
            if rule.operator == "containsAll" {
                needles.iter().all(predicate)
            } else {
                needles.iter().any(predicate)
            }
        }
        "isZero" => left.as_f64() == Some(0.0),
        "isNotZero" => left.as_f64().is_some_and(|number| number != 0.0),
        "hasAny" | "hasAll" | "hasNone" => {
            let left = left.as_array().expect("validated array record");
            let wanted = right.as_array().expect("validated list");
            let includes = |needle: &Value| left.iter().any(|item| equal(item, needle));
            match rule.operator.as_str() {
                "hasAny" => wanted.iter().any(includes),
                "hasAll" => wanted.iter().all(includes),
                _ => !wanted.iter().any(includes),
            }
        }
        _ => unreachable!("catalog validation rejects unsupported operators"),
    };
    Ok(result)
}

fn is_empty(value: &Value, field: &Field) -> bool {
    value.is_null()
        || matches!(field.data_type, DataType::String | DataType::Enum) && value.as_str() == Some("")
        || matches!(field.data_type, DataType::Array | DataType::TypedArray)
            && value.as_array().is_some_and(Vec::is_empty)
}

fn equal(left: &Value, right: &Value) -> bool {
    left == right
}

fn equal_typed(left: &Value, right: &Value, field: &Field) -> bool {
    match field.data_type {
        DataType::Number | DataType::Date | DataType::Datetime => compare(left, right, field) == Some(Ordering::Equal),
        _ => equal(left, right),
    }
}

fn compare(left: &Value, right: &Value, field: &Field) -> Option<Ordering> {
    match field.data_type {
        DataType::Number => left.as_f64()?.partial_cmp(&right.as_f64()?),
        DataType::Date => parse_date(left.as_str()?).partial_cmp(&parse_date(right.as_str()?)),
        DataType::Datetime => parse_datetime(left.as_str()?).partial_cmp(&parse_datetime(right.as_str()?)),
        _ => left.as_str()?.partial_cmp(right.as_str()?),
    }
}

fn contains(left: &Value, needle: &str, field: &Field) -> bool {
    let needle = needle.to_lowercase();
    match field.data_type {
        DataType::Array => left.as_array().is_some_and(|items| {
            items
                .iter()
                .any(|item| item.as_str().is_some_and(|text| text.to_lowercase().contains(&needle)))
        }),
        DataType::TypedArray => left.as_array().is_some_and(|items| {
            items.iter().any(|item| {
                item.get("value")
                    .and_then(Value::as_str)
                    .is_some_and(|text| text.to_lowercase().contains(&needle))
            })
        }),
        _ => left.as_str().is_some_and(|text| text.to_lowercase().contains(&needle)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn catalog() -> Value {
        json!({"fields": [
            {"name": "text", "label": "Text"},
            {"name": "n", "label": "N", "dataType": "number"},
            {"name": "tags", "label": "Tags", "dataType": "array"}
        ]})
    }

    #[test]
    fn evaluates_groups_text_numbers_arrays_and_nulls() {
        let condition = json!({"combinator":"and","not":false,"rules":[
            {"field":"text","operator":"contains","value":"WORLD"},
            {"field":"n","operator":"between","value":[1,3]},
            {"field":"tags","operator":"containsAll","value":["a","b"]}
        ]});
        let record = json!({"text":"Hello world","n":2,"tags":["Alpha","Beta"]})
            .as_object()
            .unwrap()
            .clone();
        assert_eq!(matches(&condition, &catalog(), &record, false, None), Ok(true));
        let null_rule = json!({"combinator":"and","not":false,"rules":[
            {"field":"text","operator":"doesNotContain","value":"x"}
        ]});
        let empty = json!({"text":"","n":null,"tags":[]}).as_object().unwrap().clone();
        assert_eq!(matches(&null_rule, &catalog(), &empty, false, None), Ok(true));
    }

    #[test]
    fn rejects_wrong_record_types_without_coercion() {
        let condition = json!({"combinator":"and","not":false,"rules":[{"field":"n","operator":"=","value":2}]});
        let record = json!({"n":"2"}).as_object().unwrap().clone();
        assert!(matches(&condition, &catalog(), &record, false, None).is_err());
    }
}
