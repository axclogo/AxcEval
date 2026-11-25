use crate::error::AxcError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// A deterministic snapshot emitted for one condition or decision evaluation.
///
/// The snapshot contains no timestamp or storage metadata. Hosts that persist it
/// should add their own time, actor, retention, and redaction information.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationEvidence {
    pub snapshot_id: String,
    pub kind: EvidenceKind,
    pub input: Value,
    pub outcome: EvidenceOutcome,
}

/// Identifies which evaluator produced an evidence snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EvidenceKind {
    Condition,
    Decision,
}

/// Captures either the serialized result or the structured evaluation error.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum EvidenceOutcome {
    Succeeded { result: Value },
    Failed { error: AxcError },
}

impl EvaluationEvidence {
    pub(crate) fn from_result<T: Serialize>(kind: EvidenceKind, input: Value, result: &Result<T, AxcError>) -> Self {
        let outcome = match result {
            Ok(value) => EvidenceOutcome::Succeeded {
                result: serde_json::to_value(value).expect("serializing an evaluation result cannot fail"),
            },
            Err(error) => EvidenceOutcome::Failed { error: error.clone() },
        };
        let identity = serde_json::json!({"kind": kind, "input": &input, "outcome": &outcome});
        let snapshot_id = format!("{:x}", Sha256::digest(canonical_json(&identity).as_bytes()));
        Self {
            snapshot_id,
            kind,
            input,
            outcome,
        }
    }
}

pub(crate) fn emit_evidence<T: Serialize>(
    kind: EvidenceKind,
    input: impl FnOnce() -> Value,
    result: &Result<T, AxcError>,
    callback: Option<&mut dyn FnMut(&EvaluationEvidence)>,
) {
    if let Some(callback) = callback {
        callback(&EvaluationEvidence::from_result(kind, input(), result));
    }
}

pub(crate) fn canonical_json(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let sorted = map.iter().collect::<BTreeMap<_, _>>();
            let entries = sorted
                .into_iter()
                .map(|(key, value)| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(key).expect("serializing a JSON object key cannot fail"),
                        canonical_json(value)
                    )
                })
                .collect::<Vec<_>>();
            format!("{{{}}}", entries.join(","))
        }
        Value::Array(values) => format!("[{}]", values.iter().map(canonical_json).collect::<Vec<_>>().join(",")),
        _ => serde_json::to_string(value).expect("JSON serialization cannot fail"),
    }
}
