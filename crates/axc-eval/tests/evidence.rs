use axc_eval::{
    AxcError, DecisionResult, EvaluationEvidence, EvidenceKind, EvidenceOutcome, evaluate_decision, matches,
    validate_decision,
};
use serde_json::{Map, Value, json};

fn catalog() -> Value {
    json!({"fields":[
        {"name":"text","label":"Text"},
        {"name":"score","label":"Score","dataType":"number"}
    ]})
}

fn decision() -> Value {
    json!({
        "model":"model-a",
        "precondition":{"combinator":"and","not":false,"rules":[
            {"field":"score","operator":">=","value":1}
        ]},
        "input_fields":["text"],
        "instruction":"Classify the supplied text.",
        "outputs":[{"name":"category","label":"Category","type":"enum","options":["allowed","blocked"]}],
        "postcondition":{"combinator":"and","not":false,"rules":[
            {"field":"ai.category","operator":"=","value":"allowed"}
        ]}
    })
}

fn record() -> Map<String, Value> {
    json!({"text":"example","score":2}).as_object().unwrap().clone()
}

fn capture<T>(run: impl FnOnce(&mut dyn FnMut(&EvaluationEvidence)) -> T) -> (T, Vec<EvaluationEvidence>) {
    let mut evidence = Vec::new();
    let result = run(&mut |item| evidence.push(item.clone()));
    (result, evidence)
}

#[test]
fn fixed_conditions_emit_success_and_error_snapshots_once() {
    let condition = json!({"combinator":"and","not":false,"rules":[
        {"field":"score","operator":">=","value":1}
    ]});
    let (result, evidence) = capture(|sink| matches(&condition, &catalog(), &record(), false, Some(sink)));
    assert_eq!(result, Ok(true));
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].kind, EvidenceKind::Condition);
    assert!(matches!(
        evidence[0].outcome,
        EvidenceOutcome::Succeeded {
            result: Value::Bool(true)
        }
    ));

    let (_, repeated) = capture(|sink| matches(&condition, &catalog(), &record(), false, Some(sink)));
    assert_eq!(evidence[0].snapshot_id, repeated[0].snapshot_id);

    let invalid = json!({"combinator":"and","not":false,"rules":[
        {"field":"missing","operator":"=","value":"x"}
    ]});
    let (result, evidence) = capture(|sink| matches(&invalid, &catalog(), &record(), false, Some(sink)));
    assert!(matches!(result, Err(AxcError::ConditionInvalid { .. })));
    assert!(matches!(evidence[0].outcome, EvidenceOutcome::Failed { .. }));
}

#[test]
fn ai_decision_simulates_request_compliant_negative_and_invalid_answers() {
    let (request, request_evidence) = capture(|sink| {
        evaluate_decision(
            &decision(),
            &catalog(),
            &["model-a".to_owned()],
            &record(),
            None,
            "en",
            Some(sink),
        )
    });
    let request_id = match request.unwrap() {
        DecisionResult::NeedModel { request_id, .. } => request_id,
        other => panic!("unexpected result: {other:?}"),
    };
    assert_eq!(request_evidence.len(), 1);
    assert_eq!(request_evidence[0].kind, EvidenceKind::Decision);
    assert_eq!(request_evidence[0].input["decision"]["model"], "model-a");

    let mut other_model = decision();
    other_model["model"] = json!("model-b");
    let (other_request, _) = capture(|sink| {
        evaluate_decision(
            &other_model,
            &catalog(),
            &["model-b".to_owned()],
            &record(),
            None,
            "en",
            Some(sink),
        )
    });
    let other_request_id = match other_request.unwrap() {
        DecisionResult::NeedModel { request_id, .. } => request_id,
        other => panic!("unexpected result: {other:?}"),
    };
    assert_ne!(request_id, other_request_id);

    let compliant = json!({"matched":true,"reason":"meets the criteria","category":"allowed"});
    let (decided, evidence) = capture(|sink| {
        evaluate_decision(
            &decision(),
            &catalog(),
            &["model-a".to_owned()],
            &record(),
            Some(&compliant),
            "en",
            Some(sink),
        )
    });
    assert!(matches!(
        decided,
        Ok(DecisionResult::Decided { request_id: id, matched: true, .. }) if id == request_id
    ));
    assert!(matches!(evidence[0].outcome, EvidenceOutcome::Succeeded { .. }));

    let valid_negative = json!({"matched":false,"reason":"does not meet the criteria","category":"blocked"});
    let (decided, _) = capture(|sink| {
        evaluate_decision(
            &decision(),
            &catalog(),
            &["model-a".to_owned()],
            &record(),
            Some(&valid_negative),
            "en",
            Some(sink),
        )
    });
    assert!(matches!(decided, Ok(DecisionResult::Decided { matched: false, .. })));

    for invalid in [
        json!({"matched":"true","reason":"wrong type","category":"allowed"}),
        json!({"matched":true,"reason":"missing output"}),
        json!({"matched":true,"reason":"bad option","category":"unknown"}),
        json!({"matched":true,"reason":"extra key","category":"allowed","extra":1}),
        json!("not an object"),
    ] {
        let (result, evidence) = capture(|sink| {
            evaluate_decision(
                &decision(),
                &catalog(),
                &["model-a".to_owned()],
                &record(),
                Some(&invalid),
                "en",
                Some(sink),
            )
        });
        assert!(matches!(result, Err(AxcError::DecisionInvalid { .. })), "{invalid}");
        assert_eq!(evidence.len(), 1);
        assert!(matches!(evidence[0].outcome, EvidenceOutcome::Failed { .. }));
    }
}

#[test]
fn unknown_locale_and_unlisted_model_are_explicit_errors() {
    for (models, locale) in [(vec!["model-b".to_owned()], "en"), (vec!["model-a".to_owned()], "fr")] {
        let (result, evidence) =
            capture(|sink| evaluate_decision(&decision(), &catalog(), &models, &record(), None, locale, Some(sink)));
        assert!(matches!(result, Err(AxcError::DecisionInvalid { .. })));
        assert!(matches!(evidence[0].outcome, EvidenceOutcome::Failed { .. }));
    }
}

#[test]
fn runtime_model_allowlist_is_enforced_before_execution() {
    let issues = validate_decision(&decision(), &catalog(), &["model-b".to_owned()]).unwrap();
    assert!(
        issues
            .iter()
            .any(|issue| issue.code == "option" && issue.path == vec!["model".into()])
    );
}
