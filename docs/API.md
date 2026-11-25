# API reference

The Rust crate is the normative API. Python uses `snake_case`; WebAssembly uses the corresponding `camelCase` names.

## `validate_catalog(catalog)`

Returns a typed catalog or raises a catalog error. Catalog errors identify invalid configuration such as duplicate field
names, illegal option placement, unsupported operators, or invalid value sources.

## `validate_condition(condition, catalog, allow_empty)`

Returns all structured issues in stable traversal order. Invalid catalog configuration is returned separately.

## `matches(condition, catalog, record, allow_empty, evidence_callback)`

Validates the condition and referenced record values, then returns a boolean. Invalid conditions and record type errors
are distinct error variants. When `evidence_callback` is present, it receives exactly one snapshot for either success or
error. When absent, the evaluator does not construct a snapshot.

## `simulate_condition(condition, catalog, record, allow_empty, evidence_callback)`

Uses the same validator, record checks, operators, and short-circuit engine as `matches`, then returns:

- `matched`: the root result.
- `trace`: a tree of `group`, `rule`, and `skipped` nodes addressed by protocol paths.
- `decisiveRules`: evaluated leaf rules that produced the actual root result.

Group traces contain `rawMatched`, the result before `not`; `matched`, the result after `not`; and optional
`shortCircuitAt`, the child index that stopped evaluation. Skipped nodes report `andShortCircuit` or `orShortCircuit`.
A decisive rule may be matched when it contributes through a negated group. Traces do not duplicate record values; the
host already owns the selected record and can resolve each rule by path.

Python exposes `simulate_condition`; WebAssembly exposes `simulateCondition`. Invalid conditions and records return the
same structured errors as `matches`. The optional evidence callback follows the same contract as other evaluators.

## `validate_decision(decision, catalog, models)`

Validates a decision value including its runtime model. `validate_decision_plan` performs the same checks without the
runtime model field.

## `evaluate_decision(decision, catalog, models, record, answer, locale, evidence_callback)`

`models` is the host's explicit runtime allowlist. A decision whose model is not in that list is rejected before model
request construction.

Returns one of:

- `{status: "skipped"}`
- `{status: "needModel", request_id, model, prompt, response_schema}`
- `{status: "decided", request_id, matched, answer, gates}`

`answer` is absent or null for the first phase and is an already-parsed JSON object for the second phase. Supported
locales are exactly `en` and `zh-CN`. Unknown locales are structured errors. When `evidence_callback` is present, it
receives one snapshot for skipped, model-request, decided, and error outcomes. A decided result repeats the deterministic
`request_id` so the model answer can be linked to its request.

## Evidence snapshots

Snapshots serialize as `{snapshotId, kind, input, outcome}`. `kind` is `condition` or `decision`. `outcome` is either
`{status: "succeeded", result}` or `{status: "failed", error}`. The snapshot ID is a SHA-256 digest of canonical JSON
for the other fields. Snapshots deliberately omit timestamps, actors, storage identifiers, and retention policy; hosts
add those when persisting. Because inputs may contain sensitive data, hosts also own redaction and access control.

The Python and WebAssembly bindings expose the callback as the optional final argument of `matches`,
`simulateCondition`/`simulate_condition`, and `evaluateDecision`/`evaluate_decision`. Python exceptions and JavaScript
throws from a callback are propagated to the caller; the evaluation has already completed when the callback runs.

## `compile_sql(condition, catalog, dialect, columns, allow_empty)`

Returns `{sql, params}`. `dialect` is `clickhouse` or `duckdb`. `columns` maps field names to
`{expression, column_type}`.

## `operators_for(field)`

Returns the executable operators for one field. An explicit field operator list is returned after catalog validation;
otherwise the built-in list is selected by data type.

## Errors

Errors serialize with a `kind` discriminator:

- `catalog`
- `conditionInvalid` with `issues`
- `decisionInvalid` with `issues`
- `recordType` with `field` and `expected`
- `sql` with `message`
