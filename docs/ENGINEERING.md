# Core engineering principles

These rules are release requirements, not preferences.

## One semantic path

Each behavior has one implementation. Rust, Python, and WebAssembly bindings delegate to the same core. APIs for
different use cases may return different views, but they must not duplicate operator, validation, short-circuit, or
decision semantics. Condition matching and simulation therefore use the same execution engine.

Do not keep legacy aliases, compatibility entry points, or parallel evaluators. Make intentional public changes through
Semantic Versioning, fixtures, documentation, and migration notes instead of preserving hidden alternate behavior.

## Explicit contracts

Unknown input is validated at an explicit boundary. Invalid values produce structured errors. Do not coerce types,
guess delimiters or locales, infer missing configuration, substitute empty values, or treat errors as negative matches.
Documented protocol defaults are part of the contract and are not error recovery.

Hosts explicitly supply runtime model allowlists, records, model answers, locales, SQL mappings, and optional callbacks.
The core never discovers or substitutes them.

## Errors remain visible

Do not catch a significant error and return a generic value, stale value, partial success, or fabricated result.
Serialization and adapter failures propagate to the caller. Internal `expect` or `unreachable` assertions are permitted
only after validation has proven the invariant, and their messages must name that invariant.

## Optional work is truly optional

An absent evidence callback performs no snapshot construction and no external work. The core performs no network,
filesystem, database, clock, environment, or model I/O. Hosts own persistence, timestamps, actors, redaction, access
control, and retention.

## Diagnostics are truthful

Simulation reports the exact production execution path. It preserves short-circuit behavior and marks nodes that were
not evaluated. It must never evaluate extra rules and present them as production results. Trace paths address protocol
locations; persisted protocols do not gain editor node IDs.

## Repository hygiene

Source modules, tests, fixtures, bindings, documentation, and release scripts have separate directories and clear
ownership. Generated packages, wheels, WebAssembly bundles, caches, virtual environments, and build directories do not
belong in source handoff. Remove temporary artifacts after verification.

Before handoff, run formatting, strict Clippy, all workspace tests, the coverage gate, binding smoke tests, artifact-size
checks, diff checks, and residual-pattern scans. Do not weaken a gate to make a change pass.
