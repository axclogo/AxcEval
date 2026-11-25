use crate::error::PathItem;
use serde::{Deserialize, Serialize};

/// The exact result and execution trace for one condition evaluation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConditionSimulation {
    pub matched: bool,
    pub trace: ConditionTrace,
    pub decisive_rules: Vec<DecisiveRule>,
}

/// A rule whose actual result contributed to the root result.
///
/// A decisive rule may be matched when it sits below a negated group.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisiveRule {
    pub path: Vec<PathItem>,
    pub matched: bool,
}

/// One evaluated or short-circuited node in a condition tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ConditionTrace {
    Group {
        path: Vec<PathItem>,
        combinator: String,
        negated: bool,
        raw_matched: bool,
        matched: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        short_circuit_at: Option<usize>,
        children: Vec<ConditionTrace>,
    },
    Rule {
        path: Vec<PathItem>,
        field: String,
        operator: String,
        matched: bool,
    },
    Skipped {
        path: Vec<PathItem>,
        reason: SkipReason,
    },
}

/// Explains why a condition node was not evaluated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkipReason {
    AndShortCircuit,
    OrShortCircuit,
}

impl ConditionSimulation {
    pub(crate) fn from_trace(trace: ConditionTrace) -> Self {
        let matched = trace.matched().expect("the root trace is always evaluated");
        let mut decisive_rules = Vec::new();
        trace.collect_decisive(matched, &mut decisive_rules);
        Self {
            matched,
            trace,
            decisive_rules,
        }
    }
}

impl ConditionTrace {
    pub(crate) fn matched(&self) -> Option<bool> {
        match self {
            Self::Group { matched, .. } | Self::Rule { matched, .. } => Some(*matched),
            Self::Skipped { .. } => None,
        }
    }

    fn collect_decisive(&self, desired: bool, target: &mut Vec<DecisiveRule>) {
        match self {
            Self::Rule { path, matched, .. } => target.push(DecisiveRule {
                path: path.clone(),
                matched: *matched,
            }),
            Self::Skipped { .. } => {}
            Self::Group {
                combinator,
                negated,
                children,
                ..
            } => {
                let desired_raw = if *negated { !desired } else { desired };
                let select_all = (combinator == "and" && desired_raw) || (combinator == "or" && !desired_raw);
                if select_all {
                    for child in children.iter().filter(|child| child.matched() == Some(desired_raw)) {
                        child.collect_decisive(desired_raw, target);
                    }
                } else if let Some(child) = children.iter().find(|child| child.matched() == Some(desired_raw)) {
                    child.collect_decisive(desired_raw, target);
                }
            }
        }
    }
}
