use axc_eval::{AxcError, Column, Dialect, compile_sql, evaluate_decision, validate_decision};
use serde::Deserialize;
use serde_json::json;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct DecisionRequestFixtures {
    cases: Vec<DecisionRequestCase>,
}

#[derive(Deserialize)]
struct DecisionRequestCase {
    name: String,
    catalog: Value,
    decision: Value,
    record: Map<String, Value>,
    locale: String,
    expected: Value,
}

#[test]
fn decision_request_fixtures_match_exactly() {
    let fixtures: DecisionRequestFixtures =
        serde_json::from_str(include_str!("../../../fixtures/decision-request.fixtures.json")).unwrap();
    for case in fixtures.cases {
        let actual = evaluate_decision(
            &case.decision,
            &case.catalog,
            &["m".to_owned()],
            &case.record,
            None,
            &case.locale,
            None,
        )
        .unwrap();
        assert_eq!(serde_json::to_value(actual).unwrap(), case.expected, "{}", case.name);
    }
}

#[derive(Deserialize)]
struct DecisionRunFixtures {
    cases: Vec<DecisionRunCase>,
}

#[derive(Deserialize)]
struct DecisionRunCase {
    name: String,
    catalog: Value,
    decision: Value,
    record: Map<String, Value>,
    answer: Value,
    expected: Value,
}

#[test]
fn decision_run_fixtures_match_exactly() {
    let fixtures: DecisionRunFixtures =
        serde_json::from_str(include_str!("../../../fixtures/decision-run.fixtures.json")).unwrap();
    for case in fixtures.cases {
        let actual = evaluate_decision(
            &case.decision,
            &case.catalog,
            &["m".to_owned()],
            &case.record,
            Some(&case.answer),
            "en",
            None,
        )
        .unwrap();
        assert_eq!(serde_json::to_value(actual).unwrap(), case.expected, "{}", case.name);
    }
}

#[test]
fn decision_rejects_unknown_model_and_invalid_record_input() {
    let fixtures: DecisionRequestFixtures =
        serde_json::from_str(include_str!("../../../fixtures/decision-request.fixtures.json")).unwrap();
    let case = &fixtures.cases[0];
    let issues = validate_decision(&case.decision, &case.catalog, &[]).unwrap();
    assert!(issues.iter().any(|issue| issue.code == "option"));

    let mut record = case.record.clone();
    let input_name = case.decision["input_fields"][0].as_str().unwrap();
    record.insert(input_name.to_owned(), json!({"invalid": true}));
    assert!(matches!(
        evaluate_decision(
            &case.decision,
            &case.catalog,
            &["m".to_owned()],
            &record,
            None,
            &case.locale,
            None,
        ),
        Err(AxcError::RecordType { .. })
    ));
}

#[derive(Deserialize)]
struct SqlFixtures {
    cases: Vec<SqlCase>,
}

#[derive(Deserialize)]
struct SqlCase {
    name: String,
    catalog: Value,
    condition: Value,
    columns: BTreeMap<String, Column>,
    clickhouse: Value,
    duckdb: Value,
}

#[test]
fn sql_fixtures_match_exactly() {
    let fixtures: SqlFixtures = serde_json::from_str(include_str!("../../../fixtures/sql.fixtures.json")).unwrap();
    for case in fixtures.cases {
        for (dialect, expected) in [(Dialect::Clickhouse, case.clickhouse), (Dialect::Duckdb, case.duckdb)] {
            let actual = compile_sql(&case.condition, &case.catalog, dialect, &case.columns, false).unwrap();
            assert_eq!(serde_json::to_value(actual).unwrap(), expected, "{}", case.name);
        }
    }
}

#[test]
fn clickhouse_typed_array_membership_binds_tuple_parts() {
    let catalog = json!({"fields":[{
        "name":"contacts","label":"Contacts","dataType":"typedArray",
        "values":[{"value":"email","label":"Email"}]
    }]});
    let condition = json!({"combinator":"and","not":false,"rules":[{
        "field":"contacts","operator":"hasAny",
        "value":[{"type":"email","value":"a@example.test"}]
    }]});
    let columns = BTreeMap::from([(
        "contacts".to_owned(),
        Column {
            expression: "contacts".to_owned(),
            column_type: "Array(Tuple(String, String))".to_owned(),
        },
    )]);
    let output = compile_sql(&condition, &catalog, Dialect::Clickhouse, &columns, false).unwrap();
    assert!(output.sql.contains("item.1 = {p0:String}"));
    assert!(output.sql.contains("item.2 = {p1:String}"));
    assert_eq!(
        output.params,
        json!({"p0":"email","p1":"a@example.test"}).as_object().unwrap().clone()
    );
}
