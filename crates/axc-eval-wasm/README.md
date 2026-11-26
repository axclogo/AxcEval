# @axc/eval

Deterministic WebAssembly evaluator for typed condition trees and two-phase decision plans. The package performs no
network, database, filesystem, clock, or model I/O. Applications provide catalogs, records and model answers explicitly.

This is AxcEval's official npm distribution. It is an optional ESM package for browser bundlers and is not a transitive
dependency of `@axc/uikit`. Applications using the UIKit condition simulator install this package explicitly and pass
its `simulateCondition` export through the component's `simulate` prop.

## Install

```sh
npm install @axc/eval
```

The package targets browser bundlers with WebAssembly module support. The bundler loads the included `.wasm` asset;
callers do not run a separate initialization function. The package has one public root entry and no hidden subpaths.

## Condition simulation

```ts
import { simulateCondition } from "@axc/eval";

const catalog = {
  fields: [{ name: "score", label: "Score", dataType: "number" }],
};
const condition = {
  combinator: "and",
  not: false,
  rules: [{ field: "score", operator: ">=", value: 80 }],
};

const result = simulateCondition(condition, catalog, { score: 92 }, false);
console.log(result.matched); // true
```

Exports include `validateCatalog`, `validateCondition`, `matches`, `simulateCondition`, `validateDecision`,
`evaluateDecision`, `compileSql` and `operatorsFor`. Inputs and outputs are plain JavaScript values. Invalid data throws
structured errors; the evaluator does not coerce values or turn errors into negative matches.

See the [repository README](https://github.com/axclogo/AxcEval#readme),
[API reference](https://github.com/axclogo/AxcEval/blob/main/docs/API.md) and
[execution semantics](https://github.com/axclogo/AxcEval/blob/main/docs/SEMANTICS.md).

Licensed under Apache-2.0.
