import { readFileSync } from 'node:fs';

const cargoManifest = readFileSync(new URL('../Cargo.toml', import.meta.url), 'utf8');
const pythonManifest = readFileSync(new URL('../crates/axc-eval-python/pyproject.toml', import.meta.url), 'utf8');
const cargoVersion = cargoManifest.match(/^version = "([^"]+)"$/m)?.[1];
const pythonVersion = pythonManifest.match(/^version = "([^"]+)"$/m)?.[1];
const tag = process.env.GITHUB_REF_NAME;

if (!cargoVersion || !pythonVersion || cargoVersion !== pythonVersion || tag !== `v${cargoVersion}`) {
	throw new Error(
		`Release tag ${tag ?? '(missing)'} does not match Cargo ${cargoVersion ?? '(missing)'} and Python ${pythonVersion ?? '(missing)'}`,
	);
}
