import { copyFileSync, readFileSync, writeFileSync } from "node:fs";

const path = new URL("../dist/package/package.json", import.meta.url);
const manifest = JSON.parse(readFileSync(path, "utf8"));
const cargoManifest = readFileSync(
  new URL("../Cargo.toml", import.meta.url),
  "utf8",
);
const version = cargoManifest.match(/^version = "([^"]+)"$/m)?.[1];
const tag =
  process.env.GITHUB_REF_TYPE === "tag"
    ? process.env.GITHUB_REF_NAME
    : undefined;

if (!version || !/^\d+\.\d+\.\d+$/.test(version))
  throw new Error("Cargo workspace version must be stable SemVer");
if (tag && tag !== `v${version}`)
  throw new Error(`Release tag ${tag} does not match Cargo ${version}`);

manifest.name = "@axclogo/eval";
manifest.version = version;
manifest.description =
  "Deterministic WebAssembly evaluator for typed condition trees and decision plans";
manifest.license = "Apache-2.0";
manifest.repository = {
  type: "git",
  url: "git+https://github.com/axclogo/AxcEval.git",
};
manifest.homepage = "https://github.com/axclogo/AxcEval#readme";
manifest.bugs = { url: "https://github.com/axclogo/AxcEval/issues" };
manifest.keywords = [
  "condition",
  "decision",
  "evaluator",
  "rules",
  "typescript",
  "webassembly",
];
manifest.exports = {
  ".": {
    types: "./axc_eval_wasm.d.ts",
    import: "./axc_eval_wasm.js",
    default: "./axc_eval_wasm.js",
  },
};
manifest.publishConfig = {
  access: "public",
  registry: "https://registry.npmjs.org",
};
writeFileSync(path, `${JSON.stringify(manifest, null, 2)}\n`);
copyFileSync(
  new URL("../LICENSE", import.meta.url),
  new URL("../dist/package/LICENSE", import.meta.url),
);
copyFileSync(
  new URL("../crates/axc-eval-wasm/README.md", import.meta.url),
  new URL("../dist/package/README.md", import.meta.url),
);
