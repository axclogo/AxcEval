# AxcEval

[![CI](https://github.com/axclogo/AxcEval/actions/workflows/ci.yml/badge.svg)](https://github.com/axclogo/AxcEval/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

AxcEval is a pure, deterministic evaluator for typed condition trees and two-phase AI decision plans. The Rust core
validates and evaluates JSON protocols and compiles conditions to ClickHouse or DuckDB boolean expressions. Python and
WebAssembly bindings expose the same behavior without duplicating the rules.

The library performs no network, database, filesystem, clock, or model I/O. Applications supply records, model answers,
and SQL column expressions explicitly.

## Features

- Strict field catalogs and structured validation issues (`{code, path, params}`)
- Nested `and` / `or` / `not` groups with scalar, list, range, array and typed-array operators
- Strict record typing with no coercion or implicit path lookup
- Two-phase decisions: precondition, model request, strict answer validation, postcondition
- Optional evidence callbacks for successful and failed condition or decision attempts
- Exact condition simulation traces with decisive rules and short-circuit visibility
- Parameterized ClickHouse output and escaped DuckDB output
- Rust, Python 3.11+ abi3, and WebAssembly targets

## Distributions

| Runtime                 | Distribution                        | Status                                                |
| ----------------------- | ----------------------------------- | ----------------------------------------------------- |
| JavaScript / TypeScript | `npm install @axc/eval`             | Official ESM WebAssembly package for browser bundlers |
| Python 3.11+            | Versioned wheels on GitHub Releases | Native abi3 binding; not published to PyPI            |
| Rust                    | Workspace source in this repository | Normative core; not published to crates.io            |

The npm package is optional and independent of `@axc/uikit`. UIKit does not bundle Eval or WASM; applications that use
the condition simulator explicitly pass Eval's `simulateCondition` export to the component.

## Rust

```rust
use axc_eval::matches;
use serde_json::json;

let catalog = json!({"fields": [{"name": "score", "label": "Score", "dataType": "number"}]});
let condition = json!({
    "combinator": "and",
    "not": false,
    "rules": [{"field": "score", "operator": ">=", "value": 80}]
});
let record = json!({"score": 92}).as_object().unwrap().clone();

assert!(matches(&condition, &catalog, &record, false, None)?);
# Ok::<(), axc_eval::AxcError>(())
```

## Python

Release wheels expose ordinary Python dictionaries and lists:

```python
import axc_eval

catalog = {"fields": [{"name": "score", "label": "Score", "dataType": "number"}]}
condition = {
    "combinator": "and",
    "not": False,
    "rules": [{"field": "score", "operator": ">=", "value": 80}],
}

assert axc_eval.matches(condition, catalog, {"score": 92})
```

Evidence is opt-in. The callback receives a serializable snapshot that the host may persist or discard:

```python
snapshots = []
matched = axc_eval.matches(condition, catalog, {"score": 92}, evidence_callback=snapshots.append)
```

Frontends can simulate the same execution path and map trace paths back to their condition editor:

```python
simulation = axc_eval.simulate_condition(condition, catalog, {"score": 92})
assert simulation["matched"] is True
```

## WebAssembly

Install the official JavaScript/WebAssembly package from npm:

```sh
npm install @axc/eval
```

```ts
import { simulateCondition } from "@axc/eval";

const result = simulateCondition(condition, catalog, record, false);
```

The package is ESM for browser bundlers with WebAssembly module support. The bundler loads the included `.wasm` asset;
callers do not run a separate initialization function. It exports `validateCatalog`, `validateCondition`, `matches`,
`simulateCondition`, `validateDecision`, `evaluateDecision`, `compileSql`, and `operatorsFor`. Inputs and outputs are
plain JavaScript values. The npm package is not required by Rust or Python consumers.

## Protocol and API

See [execution semantics](docs/SEMANTICS.md), the [API reference](docs/API.md), and the mandatory
[engineering principles](docs/ENGINEERING.md). Validation issue codes and protocol fixtures are versioned under
`fixtures/` so integrations can verify protocol stability.

## Development

Rust 1.85 or newer is required.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo llvm-cov -p axc-eval --all-targets --ignore-filename-regex 'decision\.rs' --fail-under-lines 98
cargo build --release --target wasm32-unknown-unknown -p axc-eval-wasm
```

The coverage gate measures the deterministic condition, validation, and SQL core. Decision orchestration has dedicated
protocol and runtime tests but is excluded from the percentage gate because model behavior is supplied by the host.
CI also builds the Python 3.11 binding with `uv` and executes both binding smoke scripts against the shared simulation
fixtures.

Read [CONTRIBUTING.md](CONTRIBUTING.md) before submitting changes. Security reports follow [SECURITY.md](SECURITY.md).
Release maintainers follow [docs/RELEASING.md](docs/RELEASING.md).

Licensed under [Apache License 2.0](LICENSE).
