'use strict';

const { readFileSync } = require('node:fs');
const { resolve } = require('node:path');

const root = resolve(__dirname, '..');
const modulePath = process.argv[2];
if (!modulePath) {
	throw new Error('usage: node scripts/smoke-wasm.cjs <generated-node-module>');
}

const api = require(resolve(modulePath));
const fixture = JSON.parse(readFileSync(resolve(root, 'fixtures/simulation.fixtures.json'), 'utf8'));

for (const testCase of fixture.cases) {
	const actual = api.simulateCondition(
		testCase.condition,
		testCase.catalog,
		testCase.record,
		testCase.allowEmpty,
	);
	if (JSON.stringify(actual) !== JSON.stringify(testCase.expected)) {
		throw new Error(`simulation output differs for ${testCase.name}`);
	}
	const matched = api.matches(
		testCase.condition,
		testCase.catalog,
		testCase.record,
		testCase.allowEmpty,
	);
	if (matched !== testCase.expected.matched) {
		throw new Error(`match result differs for ${testCase.name}`);
	}
}

const evidence = [];
const first = fixture.cases[0];
api.simulateCondition(first.condition, first.catalog, first.record, first.allowEmpty, (snapshot) => {
	evidence.push(snapshot);
});
if (evidence.length !== 1 || evidence[0].input.operation !== 'simulate') {
	throw new Error('simulation must emit exactly one explicit evidence snapshot');
}

console.log(`WebAssembly binding: ${fixture.cases.length} simulation fixtures passed`);
