use axc_eval::{AxcError, matches, validate_condition};
use serde::Deserialize;
use serde_json::{Map, Value, json};

fn group(rule: Value) -> Value {
    json!({"combinator":"and","not":false,"rules":[rule]})
}

fn record(value: Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}

fn catalog() -> Value {
    json!({"fields":[
        {"name":"text","label":"Text"},
        {"name":"number","label":"Number","dataType":"number"},
        {"name":"other","label":"Other","dataType":"number"},
        {"name":"flag","label":"Flag","dataType":"boolean"},
        {"name":"date","label":"Date","dataType":"date"},
        {"name":"time","label":"Time","dataType":"datetime"},
        {"name":"kind","label":"Kind","dataType":"enum","values":[{"value":"a","label":"A"},{"value":"b","label":"B"}]},
        {"name":"set","label":"Set","dataType":"array","values":[{"value":"a","label":"A"},{"value":"b","label":"B"}]},
        {"name":"words","label":"Words","dataType":"array"},
        {"name":"typed","label":"Typed","dataType":"typedArray","values":[{"value":"email","label":"Email"}]},
        {"name":"data","label":"Data","dataType":"object"}
    ]})
}

#[test]
fn every_builtin_operator_has_runtime_behavior() {
    let cases = vec![
        (
            json!({"field":"text","operator":"=","value":"Alpha"}),
            json!({"text":"Alpha"}),
            true,
        ),
        (
            json!({"field":"text","operator":"!=","value":"Beta"}),
            json!({"text":"Alpha"}),
            true,
        ),
        (
            json!({"field":"text","operator":"contains","value":"LPH"}),
            json!({"text":"Alpha"}),
            true,
        ),
        (
            json!({"field":"text","operator":"doesNotContain","value":"z"}),
            json!({"text":"Alpha"}),
            true,
        ),
        (
            json!({"field":"text","operator":"beginsWith","value":"Al"}),
            json!({"text":"Alpha"}),
            true,
        ),
        (
            json!({"field":"text","operator":"endsWith","value":"ha"}),
            json!({"text":"Alpha"}),
            true,
        ),
        (
            json!({"field":"text","operator":"regex","value":"^A.+a$"}),
            json!({"text":"Alpha"}),
            true,
        ),
        (
            json!({"field":"text","operator":"notRegex","value":"^B"}),
            json!({"text":"Alpha"}),
            true,
        ),
        (
            json!({"field":"text","operator":"in","value":["Alpha","Beta"]}),
            json!({"text":"Alpha"}),
            true,
        ),
        (
            json!({"field":"text","operator":"notIn","value":["Beta"]}),
            json!({"text":"Alpha"}),
            true,
        ),
        (
            json!({"field":"number","operator":">","value":1}),
            json!({"number":2}),
            true,
        ),
        (
            json!({"field":"number","operator":">=","value":2}),
            json!({"number":2}),
            true,
        ),
        (
            json!({"field":"number","operator":"<","value":3}),
            json!({"number":2}),
            true,
        ),
        (
            json!({"field":"number","operator":"<=","value":2}),
            json!({"number":2}),
            true,
        ),
        (
            json!({"field":"number","operator":"between","value":[1,3]}),
            json!({"number":2}),
            true,
        ),
        (
            json!({"field":"number","operator":"notBetween","value":[3,4]}),
            json!({"number":2}),
            true,
        ),
        (
            json!({"field":"number","operator":"isZero","value":null}),
            json!({"number":0}),
            true,
        ),
        (
            json!({"field":"number","operator":"isNotZero","value":null}),
            json!({"number":2}),
            true,
        ),
        (
            json!({"field":"flag","operator":"=","value":true}),
            json!({"flag":true}),
            true,
        ),
        (
            json!({"field":"date","operator":">","value":"2024-01-01"}),
            json!({"date":"2024-01-02"}),
            true,
        ),
        (
            json!({"field":"time","operator":"=","value":"2024-01-01T00:00:00Z"}),
            json!({"time":"2024-01-01T08:00:00+08:00"}),
            true,
        ),
        (
            json!({"field":"kind","operator":"in","value":["a"]}),
            json!({"kind":"a"}),
            true,
        ),
        (
            json!({"field":"set","operator":"hasAny","value":["b"]}),
            json!({"set":["a","b"]}),
            true,
        ),
        (
            json!({"field":"set","operator":"hasAll","value":["a","b"]}),
            json!({"set":["a","b"]}),
            true,
        ),
        (
            json!({"field":"set","operator":"hasNone","value":["b"]}),
            json!({"set":["a"]}),
            true,
        ),
        (
            json!({"field":"words","operator":"contains","value":"LPH"}),
            json!({"words":["Alpha"]}),
            true,
        ),
        (
            json!({"field":"words","operator":"containsAny","value":["z","bet"]}),
            json!({"words":["Alpha","Beta"]}),
            true,
        ),
        (
            json!({"field":"words","operator":"containsAll","value":["alp","bet"]}),
            json!({"words":["Alpha","Beta"]}),
            true,
        ),
        (
            json!({"field":"typed","operator":"hasAny","value":[{"type":"email","value":"a@example.test"}]}),
            json!({"typed":[{"type":"email","value":"a@example.test"}]}),
            true,
        ),
        (
            json!({"field":"typed","operator":"contains","value":"EXAMPLE"}),
            json!({"typed":[{"type":"email","value":"a@example.test"}]}),
            true,
        ),
        (
            json!({"field":"data","operator":"null","value":null}),
            json!({"data":null}),
            true,
        ),
        (
            json!({"field":"text","operator":"notNull","value":null}),
            json!({"text":"x"}),
            true,
        ),
    ];
    for (rule, input, expected) in cases {
        assert_eq!(
            matches(&group(rule.clone()), &catalog(), &record(input), false, None),
            Ok(expected),
            "{rule}"
        );
    }
}

#[test]
fn field_references_and_group_negation_are_deterministic() {
    let mut catalog = catalog();
    catalog["fields"][1]["valueSources"] = json!(["value", "field"]);
    let condition = json!({"combinator":"and","not":true,"rules":[
        {"field":"number","operator":"<","value":"other","valueSource":"field"}
    ]});
    assert_eq!(
        matches(
            &condition,
            &catalog,
            &record(json!({"number":5,"other":3})),
            false,
            None
        ),
        Ok(true)
    );
}

#[test]
fn field_references_are_rejected_for_non_scalar_operators() {
    let mut catalog = catalog();
    catalog["fields"][1]["valueSources"] = json!(["value", "field"]);
    let condition = group(json!({
        "field":"number",
        "operator":"between",
        "value":"other",
        "valueSource":"field"
    }));
    let issues = validate_condition(&condition, &catalog, false).unwrap();
    assert!(issues.iter().any(|issue| issue.code == "fieldRefInvalid"));
}

#[test]
fn missing_and_wrong_record_values_are_errors() {
    let condition = group(json!({"field":"number","operator":"=","value":1}));
    for input in [json!({}), json!({"number":"1"})] {
        assert!(matches!(
            matches(&condition, &catalog(), &record(input), false, None),
            Err(AxcError::RecordType { .. })
        ));
    }
    assert_eq!(
        matches(
            &condition,
            &catalog(),
            &record(json!({"number":null,"extra":true})),
            false,
            None
        ),
        Ok(false)
    );
}

#[derive(Deserialize)]
struct EvaluationFixture {
    catalog: Value,
    cases: Vec<EvaluationCase>,
}

#[derive(Deserialize)]
struct EvaluationCase {
    name: String,
    condition: Value,
    record: Map<String, Value>,
    matched: bool,
}

#[test]
fn evaluation_fixtures_are_executable() {
    let fixture: EvaluationFixture =
        serde_json::from_str(include_str!("../../../fixtures/evaluation.fixtures.json")).unwrap();
    for case in fixture.cases {
        assert_eq!(
            matches(&case.condition, &fixture.catalog, &case.record, false, None),
            Ok(case.matched),
            "{}",
            case.name
        );
    }
}
