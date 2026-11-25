import { copyFileSync, readFileSync, writeFileSync } from 'node:fs';

const path = new URL('../dist/package/package.json', import.meta.url);
const manifest = JSON.parse(readFileSync(path, 'utf8'));
const tag = process.env.GITHUB_REF_NAME;

if (!tag || !/^v\d+\.\d+\.\d+$/.test(tag)) {
	throw new Error('GITHUB_REF_NAME must be a semantic version tag such as v1.2.3');
}

manifest.name = '@axc/eval';
manifest.version = tag.slice(1);
manifest.description = 'Deterministic WebAssembly evaluator for typed condition trees and decision plans';
manifest.license = 'Apache-2.0';
manifest.repository = { type: 'git', url: 'git+https://github.com/axclogo/AxcEval.git' };
writeFileSync(path, `${JSON.stringify(manifest, null, 2)}\n`);
copyFileSync(new URL('../LICENSE', import.meta.url), new URL('../dist/package/LICENSE', import.meta.url));
