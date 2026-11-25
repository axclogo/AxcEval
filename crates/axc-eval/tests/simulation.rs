use axc_eval::{AxcError, EvidenceOutcome, matches, simulate_condition};
use serde::Deserialize;
use serde_json::{Map, Value};

#[derive(Deserialize)]
struct Fixture {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    catalog: Value,
    condition: Value,
    record: Map<String, Value>,
    #[serde(rename = "allowEmpty")]
    allow_empty: bool,
    expected: Value,
}

#[test]
fn simulation_fixtures_match_exactly_and_share_match_semantics() {
    let fixture: Fixture = serde_json::from_str(include_str!("../../../fixtures/simulation.fixtures.json")).unwrap();
    for case in fixture.cases {
        let simulation =
            simulate_condition(&case.condition, &case.catalog, &case.record, case.allow_empty, None).unwrap();
        assert_eq!(
            serde_json::to_value(&simulation).unwrap(),
            case.expected,
            "{}",
            case.name
        );
        assert_eq!(
            matches(&case.condition, &case.catalog, &case.record, case.allow_empty, None),
            Ok(simulation.matched),
            "{}",
            case.name
        );
    }
}

#[test]
fn simulation_evidence_is_optional_and_includes_errors() {
    let catalog = serde_json::json!({"fields":[{"name":"score","label":"Score","dataType":"number"}]});
    let condition = serde_json::json!({"combinator":"and","not":false,"rules":[
        {"field":"score","operator":">=","value":80}
    ]});
    let record = serde_json::json!({"score":90}).as_object().unwrap().clone();
    let mut evidence = Vec::new();
    let simulation = simulate_condition(
        &condition,
        &catalog,
        &record,
        false,
        Some(&mut |item| {
            evidence.push(item.clone());
        }),
    )
    .unwrap();
    assert!(simulation.matched);
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].input["operation"], "simulate");
    assert!(matches!(evidence[0].outcome, EvidenceOutcome::Succeeded { .. }));

    let invalid = serde_json::json!({"combinator":"and","not":false,"rules":[
        {"field":"missing","operator":"=","value":"x"}
    ]});
    let mut errors = Vec::new();
    let result = simulate_condition(
        &invalid,
        &catalog,
        &record,
        false,
        Some(&mut |item| {
            errors.push(item.clone());
        }),
    );
    assert!(matches!(result, Err(AxcError::ConditionInvalid { .. })));
    assert_eq!(errors.len(), 1);
    assert!(matches!(errors[0].outcome, EvidenceOutcome::Failed { .. }));
}
