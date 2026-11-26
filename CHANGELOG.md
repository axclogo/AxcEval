# Changelog

All notable changes are documented here. This project follows Semantic Versioning.

## [Unreleased]

## [0.3.2] - 2026-11-26

- Activate token-free npm publication through the verified GitHub Actions Trusted Publisher.
- Correct the bootstrap documentation to build and inspect the first npm archive from the reviewed release commit.

## [0.3.1] - 2026-11-26

- Publish the WebAssembly binding as `@axclogo/eval` after the originally planned npm scope was unavailable.
- Add a token-free npm Trusted Publisher release gate for the `@axclogo/eval` WebAssembly package.
- Publish npm-specific installation, runtime and package-contract documentation.

## [0.3.0] - 2026-11-25

- Add exact condition simulation traces for frontend record testing, including negation and short-circuit visibility.
- Add stable decisive-rule paths without adding node identifiers to persisted condition protocols.
- Expose simulation through Rust, Python, and WebAssembly using the same core execution engine as matching.
- Establish mandatory engineering principles covering single semantic paths, explicit contracts, visible errors, and
  clean source handoff.
- Avoid all evidence-input construction when the optional callback is absent.

## [0.2.0] - 2026-11-25

- Add optional evidence callbacks for condition and AI decision evaluations, including structured error attempts.
- Add deterministic evidence snapshot identifiers and repeat the model request ID in final decision results.
- Add simulated compliant, negative, schema-invalid, and malformed model-answer tests.
- Require an explicit runtime model allowlist and reject unsupported locales without substitution.
- Keep evidence optional on the primary evaluation APIs instead of exposing parallel entry points.
- Fail release metadata generation when the version tag is missing or malformed.

## [0.1.0] - 2026-11-25

- Add strict catalog, condition and decision validation with structured issues.
- Add deterministic in-memory condition and two-phase decision evaluation.
- Add ClickHouse and DuckDB boolean-expression compilation.
- Add Python 3.11 abi3 and WebAssembly bindings.
