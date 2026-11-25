# Execution semantics

## Catalogs and records

A catalog defines every field's name, type, options, allowed operators, and value sources. Field names are opaque keys;
dots have no path meaning. Records are flat JSON objects and must contain every field referenced by a condition. Values
must match the declared type exactly. A number is never parsed from a string, and arrays are never inferred from text.

Empty values are `null`, an empty string for string and enum fields, and an empty array for array fields. Other values
retain their ordinary JSON meaning.

## Groups

An `and` group matches when every child matches. An `or` group matches when any child matches. `not: true` negates the
group result. An empty root group is accepted only when `allow_empty` is enabled and then matches. Nested empty groups
are invalid.

## Simulation

Simulation is an instrumented execution, not a second evaluator. It validates the same inputs and calls the same rule
and group engine as ordinary matching. `and` stops at the first false child and `or` stops at the first true child.
Remaining siblings are represented as skipped nodes and are never evaluated. Group traces expose results before and
after negation. `decisiveRules` recursively identifies the evaluated leaves that produced the root result, including
matched leaves that cause a failure through `not`.

Trace paths use the condition protocol's `rules` keys and array indexes. They are snapshot locations, not persisted node
identifiers. The trace omits record values so callers can apply their own display and redaction policy.

## Operators

Comparisons on numbers use numeric order. Dates use calendar order and datetimes use their UTC instant. `between` is a
closed interval. Text `contains` and word-list operators compare Unicode lowercase strings. Prefix, suffix, equality and
regular-expression operators remain case-sensitive. Regular expressions use Rust `regex` syntax without Unicode
character-class tables; text containment still uses Unicode lowercase comparison.

Array membership compares whole elements. Free-text array searches compare each string element. Typed arrays contain
objects shaped as `{type, value}`: membership compares both properties, while text searches inspect `value` only.

Positive operators do not match an empty field. Negative operators (`!=`, `notIn`, `notBetween`, `doesNotContain`,
`notRegex`, and `hasNone`) do match an empty field. `null` and `notNull` test emptiness directly.

With `valueSource: "field"`, the rule reads its comparison value from another field of the same declared type. Both
fields are taken from the same record.

## Decisions

Decision evaluation is split around the application's model call:

1. A failing precondition returns `skipped`.
2. A passing precondition without an answer returns `needModel`, including a deterministic request ID, prompt and JSON
   Schema.
3. Calling again with the parsed answer validates every key and type, evaluates the optional postcondition, and returns
   `decided`.

The final result requires both the model's `matched` value and the postcondition to pass. A missing postcondition passes.
Model failures and invalid answers are errors rather than negative matches.

The host supplies a runtime model allowlist. The selected model must be in that list and is part of the deterministic
request identity. The same `request_id` is returned in both `needModel` and `decided` results. Model answers must be JSON
objects with exactly the generated schema keys; missing keys, extra keys, wrong types, and unknown enum values are
structured decision errors.

## Evidence hooks

The condition and decision APIs accept one optional synchronous evidence callback. When present, it receives one
deterministic snapshot after each attempt, including failed attempts. The snapshot records the exact protocol inputs and
the serialized result or structured error. When absent, no snapshot is constructed. The evaluator performs no storage or
network I/O. Hosts decide whether to retain a snapshot and must supply timestamps, actor identity, redaction, access
control, and retention policy when they do.

## SQL

SQL compilation produces a boolean expression only. Callers provide the SQL expression and physical type for each
field. ClickHouse values use named typed parameters. DuckDB values use escaped literals. Unsupported dialect/operator
combinations return an error. Input column expressions are trusted configuration and are not parsed or rewritten.
