use axc_eval::{
    AxcError, CatalogError, Column, Dialect, PathItem, RecordTypeError, compile_sql, matches, validate_catalog,
    validate_condition,
};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

fn catalog() -> Value {
    json!({"fields":[
        {"name":"text","label":"Text","valueSources":["value","field"]},
        {"name":"text2","label":"Text 2"},
        {"name":"number","label":"Number","dataType":"number"},
        {"name":"flag","label":"Flag","dataType":"boolean"},
        {"name":"date","label":"Date","dataType":"date"},
        {"name":"time","label":"Time","dataType":"datetime"},
        {"name":"kind","label":"Kind","dataType":"enum","values":[{"value":"a","label":"A"}]},
        {"name":"tags","label":"Tags","dataType":"array","values":[{"value":"a","label":"A"}]},
        {"name":"words","label":"Words","dataType":"array"},
        {"name":"typed","label":"Typed","dataType":"typedArray","values":[{"value":"email","label":"Email"}]},
        {"name":"data","label":"Data","dataType":"object"}
    ]})
}

fn condition(rule: Value) -> Value {
    json!({"combinator":"and","not":false,"rules":[rule]})
}

fn columns() -> BTreeMap<String, Column> {
    [
        ("text", "String"),
        ("text2", "String"),
        ("number", "Float64"),
        ("flag", "UInt8"),
        ("date", "Date"),
        ("time", "DateTime64(9)"),
        ("kind", "String"),
        ("tags", "Array(String)"),
        ("words", "Array(String)"),
        ("typed", "Array(Tuple(String, String))"),
        ("data", "Object('json')"),
    ]
    .into_iter()
    .map(|(name, column_type)| {
        (
            name.to_owned(),
            Column {
                expression: format!("col_{name}"),
                column_type: column_type.to_owned(),
            },
        )
    })
    .collect()
}

#[test]
fn sql_compiles_all_supported_operator_shapes_and_dialects() {
    let rules = vec![
        json!({"field":"text","operator":"=","value":"x"}),
        json!({"field":"text","operator":"!=","value":"x"}),
        json!({"field":"text","operator":"in","value":["x","y"]}),
        json!({"field":"text","operator":"notIn","value":["x"]}),
        json!({"field":"text","operator":"contains","value":"x%_\\"}),
        json!({"field":"text","operator":"doesNotContain","value":"x"}),
        json!({"field":"text","operator":"beginsWith","value":"x"}),
        json!({"field":"text","operator":"endsWith","value":"x"}),
        json!({"field":"text","operator":"regex","value":"^x"}),
        json!({"field":"text","operator":"notRegex","value":"^x"}),
        json!({"field":"text","operator":"contains","value":"text2","valueSource":"field"}),
        json!({"field":"number","operator":">","value":1}),
        json!({"field":"number","operator":">=","value":1}),
        json!({"field":"number","operator":"<","value":2}),
        json!({"field":"number","operator":"<=","value":2}),
        json!({"field":"number","operator":"between","value":[1,2]}),
        json!({"field":"number","operator":"notBetween","value":[1,2]}),
        json!({"field":"number","operator":"isZero","value":null}),
        json!({"field":"number","operator":"isNotZero","value":null}),
        json!({"field":"flag","operator":"=","value":true}),
        json!({"field":"date","operator":"=","value":"2024-01-01"}),
        json!({"field":"time","operator":"=","value":"2024-01-01T00:00:00Z"}),
        json!({"field":"tags","operator":"hasAny","value":["a"]}),
        json!({"field":"tags","operator":"hasAll","value":["a"]}),
        json!({"field":"tags","operator":"hasNone","value":["a"]}),
        json!({"field":"words","operator":"contains","value":"x"}),
        json!({"field":"words","operator":"containsAny","value":["x","y"]}),
        json!({"field":"words","operator":"containsAll","value":["x","y"]}),
        json!({"field":"typed","operator":"hasAny","value":[{"type":"email","value":"a@example.test"}]}),
        json!({"field":"typed","operator":"contains","value":"example"}),
        json!({"field":"typed","operator":"containsAny","value":["a","b"]}),
        json!({"field":"text","operator":"null","value":null}),
        json!({"field":"tags","operator":"notNull","value":null}),
        json!({"field":"number","operator":"null","value":null}),
    ];
    for rule in rules {
        for dialect in [Dialect::Clickhouse, Dialect::Duckdb] {
            let result = compile_sql(&condition(rule.clone()), &catalog(), dialect, &columns(), false);
            let regex = matches!(rule["operator"].as_str(), Some("regex" | "notRegex"));
            if dialect == Dialect::Duckdb && regex {
                assert!(matches!(result, Err(AxcError::Sql { .. })));
            } else {
                assert!(result.is_ok(), "{dialect:?}: {rule}: {result:?}");
            }
        }
    }

    let nested = json!({"combinator":"or","not":true,"rules":[
        {"combinator":"and","not":false,"rules":[{"field":"text","operator":"=","value":"x"}]},
        {"field":"number","operator":"=","value":2}
    ]});
    assert!(
        compile_sql(&nested, &catalog(), Dialect::Clickhouse, &columns(), false)
            .unwrap()
            .sql
            .starts_with("NOT")
    );
    assert_eq!(
        compile_sql(
            &json!({"combinator":"and","not":false,"rules":[]}),
            &catalog(),
            Dialect::Duckdb,
            &columns(),
            true
        )
        .unwrap()
        .sql,
        "TRUE"
    );

    let mut missing = columns();
    missing.remove("text");
    assert!(matches!(
        compile_sql(
            &condition(json!({"field":"text","operator":"=","value":"x"})),
            &catalog(),
            Dialect::Clickhouse,
            &missing,
            false
        ),
        Err(AxcError::Sql { .. })
    ));
}

#[test]
fn catalog_rejects_every_invalid_configuration_class() {
    let invalid = vec![
        json!(null),
        json!({"maxLevels":0,"fields":[]}),
        json!({"fields":[{"name":"x","label":"X"},{"name":"x","label":"X"}]}),
        json!({"fields":[{"name":"x","label":"X","dataType":"number","values":[]}]}),
        json!({"fields":[{"name":"x","label":"X","dataType":"enum","values":[{"value":"a","label":"A"},{"value":"a","label":"A"}]}]}),
        json!({"fields":[{"name":"x","label":"X","operators":[{"name":"=","valueShape":"scalar"},{"name":"=","valueShape":"scalar"}]}]}),
        json!({"fields":[{"name":"x","label":"X","operators":[{"name":"custom","valueShape":"scalar"}]}]}),
        json!({"fields":[{"name":"x","label":"X","operators":[{"name":"=","valueShape":"list"}]}]}),
        json!({"fields":[{"name":"x","label":"X","valueSources":[]}]}),
        json!({"fields":[{"name":"x","label":"X","valueSources":["other"]}]}),
    ];
    for value in invalid {
        assert!(validate_catalog(&value).is_err(), "{value}");
    }
}

#[test]
fn invalid_calendar_and_record_shapes_are_rejected_without_panics() {
    let date_condition = condition(json!({"field":"date","operator":"=","value":"2024-01-01"}));
    for bad in ["bad", "2023-02-29", "2024-00-01", "2024-13-01", "2024-04-31"] {
        let record = json!({"date":bad}).as_object().unwrap().clone();
        assert!(matches!(
            matches(&date_condition, &catalog(), &record, false, None),
            Err(AxcError::RecordType { .. })
        ));
    }
    let time_condition = condition(json!({"field":"time","operator":"=","value":"2024-01-01T00:00:00Z"}));
    for bad in [
        "bad",
        "2024-01-01 00:00:00Z",
        "2024-01-01T24:00:00Z",
        "2024-01-01T00:60:00Z",
        "2024-01-01T00:00:60Z",
        "2024-01-01T00:00:00.",
        "2024-01-01T00:00:00.1234567890Z",
        "2024-01-01T00:00:00+24:00",
        "2024-01-01T00:00:00+01:60",
    ] {
        let record = json!({"time":bad}).as_object().unwrap().clone();
        assert!(
            matches!(
                matches(&time_condition, &catalog(), &record, false, None),
                Err(AxcError::RecordType { .. })
            ),
            "{bad}"
        );
    }
    let typed_condition = condition(json!({"field":"typed","operator":"notNull","value":null}));
    for bad in [
        json!([1]),
        json!([{"type":"email"}]),
        json!([{"type":"email","value":"x","extra":true}]),
    ] {
        let record = json!({"typed":bad}).as_object().unwrap().clone();
        assert!(matches!(
            matches(&typed_condition, &catalog(), &record, false, None),
            Err(AxcError::RecordType { .. })
        ));
    }
}

#[test]
fn public_errors_and_paths_have_stable_display_and_serialization() {
    assert_eq!(PathItem::from("x"), PathItem::Key("x".to_owned()));
    assert_eq!(PathItem::from(String::from("x")), PathItem::Key("x".to_owned()));
    assert_eq!(PathItem::from(2usize), PathItem::Index(2));
    assert_eq!(
        CatalogError {
            message: "bad".to_owned()
        }
        .to_string(),
        "bad"
    );
    assert_eq!(
        RecordTypeError {
            field: "x".to_owned(),
            expected: "string".to_owned()
        }
        .to_string(),
        "record field \"x\" must be string"
    );
    let errors = [
        AxcError::Catalog {
            message: "bad".to_owned(),
        },
        AxcError::Sql {
            message: "sql".to_owned(),
        },
        AxcError::ConditionInvalid { issues: vec![] },
        AxcError::DecisionInvalid { issues: vec![] },
        AxcError::RecordType {
            field: "x".to_owned(),
            expected: "number".to_owned(),
        },
    ];
    for error in errors {
        assert!(!error.to_string().is_empty());
    }
}

#[test]
fn validator_handles_non_object_and_unknown_shape_inputs() {
    for value in [
        json!(null),
        json!({"combinator":"xor","not":"no","rules":"bad","extra":true}),
        json!({"combinator":"and","not":false,"rules":[null]}),
    ] {
        assert!(!validate_condition(&value, &catalog(), false).unwrap().is_empty());
    }
    for rule in [
        json!({"operator":"=","value":"x"}),
        json!({"field":1,"operator":"=","value":"x"}),
        json!({"field":"text","value":"x"}),
        json!({"field":"text","operator":1,"value":"x"}),
    ] {
        assert!(
            !validate_condition(&condition(rule), &catalog(), false)
                .unwrap()
                .is_empty()
        );
    }
    let mut deep = condition(json!({"field":"text","operator":"=","value":"x"}));
    for _ in 0..6 {
        deep = json!({"combinator":"and","not":false,"rules":[deep]});
    }
    assert!(
        validate_condition(&deep, &catalog(), false)
            .unwrap()
            .iter()
            .any(|issue| issue.code == "tooDeep")
    );

    let invalid_rules = [
        json!({"field":"text","operator":"in","value":["x", 1]}),
        json!({"field":"typed","operator":"hasAny","value":[null]}),
        json!({"field":"typed","operator":"hasAny","value":[{"type":"unknown","value":"x"}]}),
        json!({"field":"typed","operator":"hasAny","value":[{"type":"","value":""}]}),
        json!({"field":"typed","operator":"hasAny","value":[{"type":1,"value":false}]}),
        json!({"field":"typed","operator":"hasAny","value":[{}]}),
        json!({"field":"date","operator":"between","value":["2024-02-01","2024-01-01"]}),
        json!({"field":"time","operator":"between","value":["2024-02-01T00:00:00Z","2024-01-01T00:00:00Z"]}),
        json!({"field":"text","operator":"between","value":["z","a"]}),
        json!({"field":"text","operator":"=","value":null,"valueSource":"field"}),
    ];
    for rule in invalid_rules {
        assert!(
            !validate_condition(&condition(rule), &catalog(), false)
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn evaluator_covers_nested_or_short_circuit_and_validation_errors() {
    let invalid = condition(json!({"field":"missing","operator":"=","value":"x"}));
    assert!(matches!(
        matches(&invalid, &catalog(), &Map::new(), false, None),
        Err(AxcError::ConditionInvalid { .. })
    ));
    let nested = json!({"combinator":"or","not":false,"rules":[
        {"combinator":"and","not":false,"rules":[{"field":"text","operator":"=","value":"no"}]},
        {"field":"text","operator":"=","value":"yes"},
        {"field":"text","operator":"=","value":"unreached"}
    ]});
    let record = json!({"text":"yes"}).as_object().unwrap().clone();
    assert_eq!(matches(&nested, &catalog(), &record, false, None), Ok(true));
}
