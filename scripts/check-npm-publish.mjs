import { appendFileSync, readFileSync, statSync } from "node:fs";
import { pathToFileURL } from "node:url";

const expectedName = "@axc/eval";
const expectedRepository = "git+https://github.com/axclogo/AxcEval.git";
const registryUrl = "https://registry.npmjs.org";
const stableVersion = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
const expectedFiles = [
  "axc_eval_wasm_bg.wasm",
  "axc_eval_wasm.js",
  "axc_eval_wasm_bg.js",
  "axc_eval_wasm.d.ts",
];

function isRecord(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function requireRecord(value, label) {
  if (!isRecord(value)) throw new TypeError(`${label} must be an object`);
  return value;
}

export function validateReleaseMetadata(value) {
  const manifest = requireRecord(value, "npm package manifest");
  const repository = requireRecord(
    manifest.repository,
    "npm package repository",
  );
  const publishConfig = requireRecord(
    manifest.publishConfig,
    "npm package publishConfig",
  );
  const packageExports = requireRecord(manifest.exports, "npm package exports");
  const rootExport = requireRecord(
    packageExports["."],
    "npm package root export",
  );

  if (manifest.name !== expectedName)
    throw new Error(`Package name must be ${expectedName}`);
  if (
    typeof manifest.version !== "string" ||
    !stableVersion.test(manifest.version)
  ) {
    throw new Error("Package version must be stable SemVer");
  }
  if (manifest.private === true)
    throw new Error("The npm package must not be private");
  if (manifest.license !== "Apache-2.0")
    throw new Error("Package license must be Apache-2.0");
  if (repository.url !== expectedRepository) {
    throw new Error(
      `Package repository.url must be ${expectedRepository} for npm provenance`,
    );
  }
  if (publishConfig.registry !== registryUrl)
    throw new Error(`Package registry must be ${registryUrl}`);
  if (publishConfig.access !== "public")
    throw new Error("Package access must be public");
  if (
    manifest.main !== "axc_eval_wasm.js" ||
    manifest.types !== "axc_eval_wasm.d.ts"
  ) {
    throw new Error(
      "Package main and types must reference the generated WebAssembly bindings",
    );
  }
  if (
    rootExport.import !== "./axc_eval_wasm.js" ||
    rootExport.default !== "./axc_eval_wasm.js" ||
    rootExport.types !== "./axc_eval_wasm.d.ts"
  ) {
    throw new Error(
      "Package root export must reference the generated ESM bindings and declarations",
    );
  }
  if (
    !Array.isArray(manifest.files) ||
    expectedFiles.some((file) => !manifest.files.includes(file))
  ) {
    throw new Error(
      "Package files must include every generated WebAssembly binding artifact",
    );
  }

  return manifest;
}

export function validatePackageDirectory(directoryUrl, manifest) {
  for (const file of [...expectedFiles, "LICENSE", "README.md"])
    statSync(new URL(file, directoryUrl));
  const wasmBytes = statSync(
    new URL("axc_eval_wasm_bg.wasm", directoryUrl),
  ).size;
  if (wasmBytes > 1_048_576)
    throw new Error(`WebAssembly artifact exceeds 1 MiB: ${wasmBytes} bytes`);
}

export async function getPublishStatus(
  manifest,
  fetchRegistry = fetch,
  createTimeoutSignal = () => AbortSignal.timeout(15_000),
) {
  const response = await fetchRegistry(
    `${registryUrl}/${encodeURIComponent(manifest.name)}`,
    {
      signal: createTimeoutSignal(),
    },
  );
  if (response.status === 404) return "bootstrap";
  if (!response.ok)
    throw new Error(`npm registry returned HTTP ${response.status}`);

  const metadata = requireRecord(
    await response.json(),
    "npm registry response",
  );
  const versions = requireRecord(metadata.versions, "npm registry versions");
  return Object.hasOwn(versions, manifest.version)
    ? "published"
    : "unpublished";
}

function readJson(url) {
  return JSON.parse(readFileSync(url, "utf8"));
}

function writeActionOutput(manifest, status) {
  if (!process.env.GITHUB_OUTPUT) return;
  appendFileSync(
    process.env.GITHUB_OUTPUT,
    `name=${manifest.name}\nversion=${manifest.version}\nstatus=${status}\npublish=${String(status === "unpublished")}\n`,
  );
}

function writeActionSummary(manifest, status) {
  if (!process.env.GITHUB_STEP_SUMMARY) return;
  const decision = {
    bootstrap:
      "Blocked: create the package once before configuring Trusted Publisher.",
    published: "No-op: this immutable version already exists on npm.",
    unpublished: "Ready: publish this version after the release gate succeeds.",
  }[status];
  appendFileSync(
    process.env.GITHUB_STEP_SUMMARY,
    `## npm release gate\n\n| Package | Registry state | Decision |\n| --- | --- | --- |\n| \`${manifest.name}@${manifest.version}\` | \`${status}\` | ${decision} |\n`,
  );
}

export async function main(
  manifestPath = process.argv[2] ?? "dist/package/package.json",
  packageOnly = process.argv.includes("--package-only"),
) {
  const manifestUrl = pathToFileURL(manifestPath);
  const manifest = validateReleaseMetadata(readJson(manifestUrl));
  if (manifestPath.endsWith("/package.json"))
    validatePackageDirectory(new URL("./", manifestUrl), manifest);
  if (packageOnly) {
    console.log(
      `Package contents checked ${manifest.name}@${manifest.version}.`,
    );
    return;
  }
  const status = await getPublishStatus(manifest);
  writeActionOutput(manifest, status);
  writeActionSummary(manifest, status);
  console.log(
    `Publish gate checked ${manifest.name}@${manifest.version}: ${status}.`,
  );
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url)
  await main();
