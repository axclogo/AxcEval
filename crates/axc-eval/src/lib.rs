//! Pure, deterministic evaluation for Axc condition trees and decision plans.
//!
//! The crate performs no network, filesystem, clock, environment, or database I/O.

mod catalog;
mod date_time;
mod decision;
mod error;
mod evaluate;
mod evidence;
mod model;
mod simulation;
mod sql;
mod validate;

pub use catalog::{operators_for, validate_catalog};
pub use decision::{evaluate_decision, validate_decision, validate_decision_plan};
pub use error::{AxcError, CatalogError, Issue, PathItem, RecordTypeError};
pub use evaluate::{matches, simulate_condition};
pub use evidence::{EvaluationEvidence, EvidenceKind, EvidenceOutcome};
pub use model::{
    Catalog, Column, ConditionGroup, DataType, DecisionResult, Dialect, Operator, OperatorShape, SqlOutput,
};
pub use simulation::{ConditionSimulation, ConditionTrace, DecisiveRule, SkipReason};
pub use sql::compile_sql;
pub use validate::validate_condition;
