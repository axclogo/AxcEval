use axc_eval::{Column, Dialect};
use serde_json::Value;
use std::collections::BTreeMap;
use wasm_bindgen::prelude::*;

fn value(input: &JsValue) -> Result<Value, JsValue> {
    let text = js_sys::JSON::stringify(input)?
        .as_string()
        .ok_or_else(|| JsValue::from_str("value is not JSON serializable"))?;
    serde_json::from_str(&text).map_err(|error| JsValue::from_str(&error.to_string()))
}

fn output(input: &impl serde::Serialize) -> Result<JsValue, JsValue> {
    let text = serde_json::to_string(input).map_err(|error| JsValue::from_str(&error.to_string()))?;
    js_sys::JSON::parse(&text)
}

fn error(error: impl serde::Serialize) -> JsValue {
    let text = serde_json::to_string(&error).expect("serializing an evaluation error cannot fail");
    js_sys::JSON::parse(&text).expect("serialized JSON must parse in JavaScript")
}

fn evaluate_with_callback<T>(
    evidence_callback: Option<js_sys::Function>,
    evaluate: impl FnOnce(Option<&mut dyn FnMut(&axc_eval::EvaluationEvidence)>) -> Result<T, axc_eval::AxcError>,
) -> Result<T, JsValue> {
    let mut callback_error = None;
    let mut sink = |evidence: &axc_eval::EvaluationEvidence| {
        if let Some(callback) = evidence_callback.as_ref() {
            match output(evidence) {
                Ok(value) => {
                    if callback_error.is_none()
                        && let Err(error) = callback.call1(&JsValue::UNDEFINED, &value)
                    {
                        callback_error = Some(error);
                    }
                }
                Err(error) => callback_error = Some(error),
            }
        }
    };
    let sink = evidence_callback
        .is_some()
        .then_some(&mut sink as &mut dyn FnMut(&axc_eval::EvaluationEvidence));
    let result = evaluate(sink);
    if let Some(error) = callback_error {
        return Err(error);
    }
    result.map_err(error)
}

#[wasm_bindgen(js_name = validateCatalog)]
pub fn validate_catalog(catalog: JsValue) -> Result<JsValue, JsValue> {
    let parsed = axc_eval::validate_catalog(&value(&catalog)?).map_err(error)?;
    output(&parsed)
}

#[wasm_bindgen(js_name = validateCondition)]
pub fn validate_condition(condition: JsValue, catalog: JsValue, allow_empty: bool) -> Result<JsValue, JsValue> {
    let issues = axc_eval::validate_condition(&value(&condition)?, &value(&catalog)?, allow_empty).map_err(error)?;
    output(&issues)
}

#[wasm_bindgen(js_name = matches)]
pub fn matches_js(
    condition: JsValue,
    catalog: JsValue,
    record: JsValue,
    allow_empty: bool,
    evidence_callback: Option<js_sys::Function>,
) -> Result<bool, JsValue> {
    let condition = value(&condition)?;
    let catalog = value(&catalog)?;
    let record = value(&record)?;
    let record = record
        .as_object()
        .ok_or_else(|| JsValue::from_str("record must be an object"))?;
    evaluate_with_callback(evidence_callback, |sink| {
        axc_eval::matches(&condition, &catalog, record, allow_empty, sink)
    })
}

#[wasm_bindgen(js_name = simulateCondition)]
pub fn simulate_condition(
    condition: JsValue,
    catalog: JsValue,
    record: JsValue,
    allow_empty: bool,
    evidence_callback: Option<js_sys::Function>,
) -> Result<JsValue, JsValue> {
    let condition = value(&condition)?;
    let catalog = value(&catalog)?;
    let record = value(&record)?;
    let record = record
        .as_object()
        .ok_or_else(|| JsValue::from_str("record must be an object"))?;
    let result = evaluate_with_callback(evidence_callback, |sink| {
        axc_eval::simulate_condition(&condition, &catalog, record, allow_empty, sink)
    })?;
    output(&result)
}

#[wasm_bindgen(js_name = validateDecision)]
pub fn validate_decision(decision: JsValue, catalog: JsValue, models: JsValue) -> Result<JsValue, JsValue> {
    let models: Vec<String> =
        serde_json::from_value(value(&models)?).map_err(|error| JsValue::from_str(&error.to_string()))?;
    let issues = axc_eval::validate_decision(&value(&decision)?, &value(&catalog)?, &models).map_err(error)?;
    output(&issues)
}

#[wasm_bindgen(js_name = evaluateDecision)]
pub fn evaluate_decision(
    decision: JsValue,
    catalog: JsValue,
    models: JsValue,
    record: JsValue,
    answer: JsValue,
    locale: &str,
    evidence_callback: Option<js_sys::Function>,
) -> Result<JsValue, JsValue> {
    let decision = value(&decision)?;
    let catalog = value(&catalog)?;
    let models: Vec<String> =
        serde_json::from_value(value(&models)?).map_err(|error| JsValue::from_str(&error.to_string()))?;
    let record = value(&record)?;
    let record = record
        .as_object()
        .ok_or_else(|| JsValue::from_str("record must be an object"))?;
    let answer = if answer.is_null() || answer.is_undefined() {
        None
    } else {
        Some(value(&answer)?)
    };
    let result = evaluate_with_callback(evidence_callback, |sink| {
        axc_eval::evaluate_decision(&decision, &catalog, &models, record, answer.as_ref(), locale, sink)
    })?;
    output(&result)
}

#[wasm_bindgen(js_name = compileSql)]
pub fn compile_sql(
    condition: JsValue,
    catalog: JsValue,
    dialect: &str,
    columns: JsValue,
    allow_empty: bool,
) -> Result<JsValue, JsValue> {
    let dialect = match dialect {
        "clickhouse" => Dialect::Clickhouse,
        "duckdb" => Dialect::Duckdb,
        _ => return Err(JsValue::from_str("dialect must be clickhouse or duckdb")),
    };
    let columns: BTreeMap<String, Column> =
        serde_json::from_value(value(&columns)?).map_err(|error| JsValue::from_str(&error.to_string()))?;
    let result =
        axc_eval::compile_sql(&value(&condition)?, &value(&catalog)?, dialect, &columns, allow_empty).map_err(error)?;
    output(&result)
}

#[wasm_bindgen(js_name = operatorsFor)]
pub fn operators_for(field: JsValue) -> Result<JsValue, JsValue> {
    let field = serde_json::from_value(value(&field)?).map_err(|error| JsValue::from_str(&error.to_string()))?;
    output(&axc_eval::operators_for(&field))
}
