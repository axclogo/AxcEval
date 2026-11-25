use crate::error::CatalogError;
use crate::model::{Catalog, DataType, Field, Operator, OperatorShape};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

fn op(name: &str, value_shape: OperatorShape) -> Operator {
    Operator {
        name: name.to_owned(),
        value_shape,
        label: None,
    }
}

fn scalar(name: &str) -> Operator {
    op(name, OperatorShape::Scalar)
}

fn none(name: &str) -> Operator {
    op(name, OperatorShape::None)
}

fn list(name: &str) -> Operator {
    op(name, OperatorShape::List)
}

fn range(name: &str) -> Operator {
    op(name, OperatorShape::Range)
}

pub fn operators_for(field: &Field) -> Vec<Operator> {
    if let Some(operators) = &field.operators {
        return operators.clone();
    }
    let empty = || vec![none("null"), none("notNull")];
    let equality = || vec![scalar("="), scalar("!=")];
    let ordered = || {
        let mut result = equality();
        result.extend([
            scalar(">"),
            scalar(">="),
            scalar("<"),
            scalar("<="),
            range("between"),
            range("notBetween"),
        ]);
        result
    };
    let mut result = match field.data_type {
        DataType::Number => {
            let mut values = ordered();
            values.extend([none("isZero"), none("isNotZero")]);
            values
        }
        DataType::Date | DataType::Datetime => ordered(),
        DataType::Boolean => equality(),
        DataType::Enum => {
            let mut values = equality();
            values.extend([list("in"), list("notIn")]);
            values
        }
        DataType::Array if field.values.is_some() => vec![list("hasAny"), list("hasAll"), list("hasNone")],
        DataType::Array => vec![
            scalar("contains"),
            scalar("doesNotContain"),
            list("containsAny"),
            list("containsAll"),
        ],
        DataType::TypedArray => vec![
            list("hasAny"),
            list("hasAll"),
            list("hasNone"),
            scalar("contains"),
            scalar("doesNotContain"),
            list("containsAny"),
            list("containsAll"),
        ],
        DataType::Object => Vec::new(),
        DataType::String => vec![
            scalar("="),
            scalar("!="),
            scalar("contains"),
            scalar("doesNotContain"),
            scalar("beginsWith"),
            scalar("endsWith"),
            scalar("regex"),
            scalar("notRegex"),
            list("in"),
            list("notIn"),
        ],
    };
    result.extend(empty());
    result
}

pub fn validate_catalog(value: &Value) -> Result<Catalog, CatalogError> {
    let catalog: Catalog = serde_json::from_value(value.clone())
        .map_err(|error| CatalogError::new(format!("invalid catalog: {error}")))?;
    validate_catalog_typed(&catalog)?;
    Ok(catalog)
}

pub(crate) fn validate_catalog_typed(catalog: &Catalog) -> Result<(), CatalogError> {
    if catalog.max_levels == 0 {
        return Err(CatalogError::new("maxLevels must be a positive integer"));
    }
    let mut names = HashSet::new();
    for field in &catalog.fields {
        if !names.insert(field.name.as_str()) {
            return Err(CatalogError::new(format!("duplicate field name {:?}", field.name)));
        }
        if field.values.is_some()
            && !matches!(
                field.data_type,
                DataType::String | DataType::Enum | DataType::Array | DataType::TypedArray
            )
        {
            return Err(CatalogError::new(format!(
                "field {:?}: values cannot be declared on dataType {:?}",
                field.name,
                field.data_type.as_str()
            )));
        }
        if let Some(values) = &field.values {
            let mut seen = HashSet::new();
            for option in values {
                if !seen.insert(option.value.as_str()) {
                    return Err(CatalogError::new(format!(
                        "field {:?}: duplicate option value {:?}",
                        field.name, option.value
                    )));
                }
            }
        }
        let mut operator_names = HashSet::new();
        for operator in operators_for(field) {
            if !operator_names.insert(operator.name.clone()) {
                return Err(CatalogError::new(format!(
                    "field {:?}: duplicate operator {:?}",
                    field.name, operator.name
                )));
            }
            let expected = executable_operators()
                .get(operator.name.as_str())
                .copied()
                .ok_or_else(|| {
                    CatalogError::new(format!(
                        "field {:?}: operator {:?} is not executable",
                        field.name, operator.name
                    ))
                })?;
            if expected != operator.value_shape {
                return Err(CatalogError::new(format!(
                    "field {:?}: operator {:?} must use valueShape {:?}",
                    field.name, operator.name, expected
                )));
            }
        }
        if let Some(sources) = &field.value_sources {
            if sources.is_empty() || sources.iter().any(|source| source != "value" && source != "field") {
                return Err(CatalogError::new(format!(
                    "field {:?}: invalid valueSources",
                    field.name
                )));
            }
        }
    }
    Ok(())
}

fn executable_operators() -> HashMap<&'static str, OperatorShape> {
    use OperatorShape::{List, None, Range, Scalar};
    HashMap::from([
        ("=", Scalar),
        ("!=", Scalar),
        (">", Scalar),
        (">=", Scalar),
        ("<", Scalar),
        ("<=", Scalar),
        ("between", Range),
        ("notBetween", Range),
        ("contains", Scalar),
        ("doesNotContain", Scalar),
        ("beginsWith", Scalar),
        ("endsWith", Scalar),
        ("regex", Scalar),
        ("notRegex", Scalar),
        ("in", List),
        ("notIn", List),
        ("containsAny", List),
        ("containsAll", List),
        ("isZero", None),
        ("isNotZero", None),
        ("null", None),
        ("notNull", None),
        ("hasAny", List),
        ("hasAll", List),
        ("hasNone", List),
    ])
}

pub(crate) fn field_map(catalog: &Catalog) -> HashMap<&str, &Field> {
    catalog
        .fields
        .iter()
        .map(|field| (field.name.as_str(), field))
        .collect()
}
