use axc_eval::{Issue, validate_condition, validate_decision_plan};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Fixture {
    catalog: Value,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    #[serde(default)]
    catalog: Option<Value>,
    #[serde(default)]
    options: Options,
    value: Value,
    issues: Vec<Issue>,
}

#[derive(Default, Deserialize)]
struct Options {
    #[serde(default, rename = "allowEmpty")]
    allow_empty: bool,
}

fn merge(base: &Value, changes: Option<&Value>) -> Value {
    let mut result = base.as_object().unwrap().clone();
    if let Some(changes) = changes.and_then(Value::as_object) {
        result.extend(changes.clone());
    }
    Value::Object(result)
}

#[test]
fn condition_protocol_fixtures_match_exactly() {
    let fixture: Fixture = serde_json::from_str(include_str!("../../../fixtures/condition.fixtures.json")).unwrap();
    for case in fixture.cases {
        let catalog = merge(&fixture.catalog, case.catalog.as_ref());
        let actual = match validate_condition(&case.value, &catalog, case.options.allow_empty) {
            Ok(actual) => actual,
            Err(error) => panic!("{}: {error}", case.name),
        };
        assert_eq!(actual, case.issues, "{}", case.name);
    }
}

#[test]
fn decision_protocol_fixtures_match_exactly() {
    let fixture: Fixture = serde_json::from_str(include_str!("../../../fixtures/decision.fixtures.json")).unwrap();
    for case in fixture.cases {
        let catalog = merge(&fixture.catalog, case.catalog.as_ref());
        let actual = match validate_decision_plan(&case.value, &catalog) {
            Ok(actual) => actual,
            Err(error) => panic!("{}: {error}", case.name),
        };
        assert_eq!(actual, case.issues, "{}", case.name);
    }
}
