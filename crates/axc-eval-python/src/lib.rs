use ::axc_eval as core;
use core::{Column, Dialect};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde_json::Value;
use std::collections::BTreeMap;

fn to_value(value: &Bound<'_, PyAny>) -> PyResult<Value> {
    let json = value.py().import("json")?;
    let text: String = json.call_method1("dumps", (value,))?.extract()?;
    serde_json::from_str(&text).map_err(|error| PyValueError::new_err(error.to_string()))
}

fn from_value(py: Python<'_>, value: &impl serde::Serialize) -> PyResult<Py<PyAny>> {
    let text = serde_json::to_string(value).map_err(|error| PyValueError::new_err(error.to_string()))?;
    Ok(py.import("json")?.call_method1("loads", (text,))?.unbind())
}

fn core_error(error: core::AxcError) -> PyErr {
    let detail = serde_json::to_string(&error).expect("serializing AxcError cannot fail");
    PyValueError::new_err(detail)
}

fn evaluate_with_callback<T>(
    py: Python<'_>,
    evidence_callback: Option<&Bound<'_, PyAny>>,
    evaluate: impl FnOnce(Option<&mut dyn FnMut(&core::EvaluationEvidence)>) -> Result<T, core::AxcError>,
) -> PyResult<T> {
    let mut callback_error = None;
    let mut sink = |evidence: &core::EvaluationEvidence| {
        if let Some(callback) = evidence_callback
            && callback_error.is_none()
            && let Err(error) = from_value(py, evidence).and_then(|value| {
                callback.call1((value,))?;
                Ok(())
            })
        {
            callback_error = Some(error);
        }
    };
    let sink = evidence_callback
        .is_some()
        .then_some(&mut sink as &mut dyn FnMut(&core::EvaluationEvidence));
    let result = evaluate(sink);
    if let Some(error) = callback_error {
        return Err(error);
    }
    result.map_err(core_error)
}

#[pyfunction]
fn validate_catalog(py: Python<'_>, catalog: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let catalog = to_value(catalog)?;
    let parsed = core::validate_catalog(&catalog).map_err(|error| PyValueError::new_err(error.to_string()))?;
    from_value(py, &parsed)
}

#[pyfunction]
#[pyo3(signature = (condition, catalog, allow_empty=false))]
fn validate_condition(
    py: Python<'_>,
    condition: &Bound<'_, PyAny>,
    catalog: &Bound<'_, PyAny>,
    allow_empty: bool,
) -> PyResult<Py<PyAny>> {
    let issues =
        core::validate_condition(&to_value(condition)?, &to_value(catalog)?, allow_empty).map_err(core_error)?;
    from_value(py, &issues)
}

#[pyfunction(name = "matches")]
#[pyo3(signature = (condition, catalog, record, allow_empty=false, evidence_callback=None))]
fn condition_matches(
    condition: &Bound<'_, PyAny>,
    catalog: &Bound<'_, PyAny>,
    record: &Bound<'_, PyAny>,
    allow_empty: bool,
    evidence_callback: Option<&Bound<'_, PyAny>>,
) -> PyResult<bool> {
    let py = condition.py();
    let condition_value = to_value(condition)?;
    let catalog_value = to_value(catalog)?;
    let record_value = to_value(record)?;
    let record = record_value
        .as_object()
        .ok_or_else(|| PyValueError::new_err("record must be an object"))?;
    evaluate_with_callback(py, evidence_callback, |sink| {
        core::matches(&condition_value, &catalog_value, record, allow_empty, sink)
    })
}

#[pyfunction]
#[pyo3(signature = (condition, catalog, record, allow_empty=false, evidence_callback=None))]
fn simulate_condition(
    py: Python<'_>,
    condition: &Bound<'_, PyAny>,
    catalog: &Bound<'_, PyAny>,
    record: &Bound<'_, PyAny>,
    allow_empty: bool,
    evidence_callback: Option<&Bound<'_, PyAny>>,
) -> PyResult<Py<PyAny>> {
    let condition_value = to_value(condition)?;
    let catalog_value = to_value(catalog)?;
    let record_value = to_value(record)?;
    let record = record_value
        .as_object()
        .ok_or_else(|| PyValueError::new_err("record must be an object"))?;
    let result = evaluate_with_callback(py, evidence_callback, |sink| {
        core::simulate_condition(&condition_value, &catalog_value, record, allow_empty, sink)
    })?;
    from_value(py, &result)
}

#[pyfunction]
fn validate_decision(
    py: Python<'_>,
    decision: &Bound<'_, PyAny>,
    catalog: &Bound<'_, PyAny>,
    models: Vec<String>,
) -> PyResult<Py<PyAny>> {
    let issues = core::validate_decision(&to_value(decision)?, &to_value(catalog)?, &models).map_err(core_error)?;
    from_value(py, &issues)
}

#[pyfunction]
#[pyo3(signature = (decision, catalog, models, record, answer=None, locale="en", evidence_callback=None))]
fn evaluate_decision(
    decision: &Bound<'_, PyAny>,
    catalog: &Bound<'_, PyAny>,
    models: Vec<String>,
    record: &Bound<'_, PyAny>,
    answer: Option<&Bound<'_, PyAny>>,
    locale: &str,
    evidence_callback: Option<&Bound<'_, PyAny>>,
) -> PyResult<Py<PyAny>> {
    let py = decision.py();
    let decision_value = to_value(decision)?;
    let catalog_value = to_value(catalog)?;
    let record_value = to_value(record)?;
    let record = record_value
        .as_object()
        .ok_or_else(|| PyValueError::new_err("record must be an object"))?;
    let answer_value = answer.map(to_value).transpose()?;
    let result = evaluate_with_callback(py, evidence_callback, |sink| {
        core::evaluate_decision(
            &decision_value,
            &catalog_value,
            &models,
            record,
            answer_value.as_ref(),
            locale,
            sink,
        )
    })?;
    from_value(py, &result)
}

#[pyfunction]
#[pyo3(signature = (condition, catalog, dialect, columns, allow_empty=false))]
fn compile_sql(
    py: Python<'_>,
    condition: &Bound<'_, PyAny>,
    catalog: &Bound<'_, PyAny>,
    dialect: &str,
    columns: &Bound<'_, PyAny>,
    allow_empty: bool,
) -> PyResult<Py<PyAny>> {
    let dialect = match dialect {
        "clickhouse" => Dialect::Clickhouse,
        "duckdb" => Dialect::Duckdb,
        _ => return Err(PyValueError::new_err("dialect must be clickhouse or duckdb")),
    };
    let columns: BTreeMap<String, Column> =
        serde_json::from_value(to_value(columns)?).map_err(|error| PyValueError::new_err(error.to_string()))?;
    let output = core::compile_sql(
        &to_value(condition)?,
        &to_value(catalog)?,
        dialect,
        &columns,
        allow_empty,
    )
    .map_err(core_error)?;
    from_value(py, &output)
}

#[pyfunction]
fn operators_for(py: Python<'_>, field: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let field = serde_json::from_value(to_value(field)?).map_err(|error| PyValueError::new_err(error.to_string()))?;
    from_value(py, &core::operators_for(&field))
}

#[pymodule]
fn axc_eval(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(validate_catalog, module)?)?;
    module.add_function(wrap_pyfunction!(validate_condition, module)?)?;
    module.add_function(wrap_pyfunction!(condition_matches, module)?)?;
    module.add_function(wrap_pyfunction!(simulate_condition, module)?)?;
    module.add_function(wrap_pyfunction!(validate_decision, module)?)?;
    module.add_function(wrap_pyfunction!(evaluate_decision, module)?)?;
    module.add_function(wrap_pyfunction!(compile_sql, module)?)?;
    module.add_function(wrap_pyfunction!(operators_for, module)?)?;
    Ok(())
}
