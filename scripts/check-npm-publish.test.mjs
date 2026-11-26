import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  getPublishStatus,
  validateReleaseMetadata,
} from "./check-npm-publish.mjs";

const manifest = {
  name: "@axclogo/eval",
  version: "0.3.1",
  license: "Apache-2.0",
  repository: { url: "git+https://github.com/axclogo/AxcEval.git" },
  publishConfig: { access: "public", registry: "https://registry.npmjs.org" },
  files: [
    "axc_eval_wasm_bg.wasm",
    "axc_eval_wasm.js",
    "axc_eval_wasm_bg.js",
    "axc_eval_wasm.d.ts",
  ],
  main: "axc_eval_wasm.js",
  types: "axc_eval_wasm.d.ts",
  exports: {
    ".": {
      types: "./axc_eval_wasm.d.ts",
      import: "./axc_eval_wasm.js",
      default: "./axc_eval_wasm.js",
    },
  },
};

function response(status, body) {
  return {
    status,
    ok: status >= 200 && status < 300,
    json: async () => body,
  };
}

test("accepts the exact public npm package metadata", () => {
  assert.equal(validateReleaseMetadata(manifest), manifest);
});

for (const [name, changed, message] of [
  [
    "name",
    { ...manifest, name: "@example/wrong" },
    /name must be @axclogo\/eval/,
  ],
  ["version", { ...manifest, version: "0.3.1-beta.1" }, /stable SemVer/],
  ["private flag", { ...manifest, private: true }, /must not be private/],
  ["license", { ...manifest, license: "MIT" }, /license must be Apache-2.0/],
  [
    "repository",
    { ...manifest, repository: { url: "https://example.com" } },
    /repository\.url must be/,
  ],
  [
    "registry",
    {
      ...manifest,
      publishConfig: {
        ...manifest.publishConfig,
        registry: "https://example.com",
      },
    },
    /registry must be/,
  ],
  [
    "access",
    {
      ...manifest,
      publishConfig: { ...manifest.publishConfig, access: "restricted" },
    },
    /must be public/,
  ],
  [
    "binding file",
    { ...manifest, files: manifest.files.slice(1) },
    /every generated/,
  ],
  [
    "root export",
    {
      ...manifest,
      exports: { ".": { ...manifest.exports["."], import: "./wrong.js" } },
    },
    /root export/,
  ],
]) {
  test(`rejects invalid ${name}`, () => {
    assert.throws(() => validateReleaseMetadata(changed), message);
  });
}

test("reports bootstrap only for an explicit Registry 404", async () => {
  assert.equal(
    await getPublishStatus(manifest, async () => response(404)),
    "bootstrap",
  );
});

test("reports an existing immutable version as published", async () => {
  assert.equal(
    await getPublishStatus(manifest, async () =>
      response(200, { versions: { "0.3.1": {} } }),
    ),
    "published",
  );
});

test("reports a missing version of an existing package as unpublished", async () => {
  assert.equal(
    await getPublishStatus(manifest, async () =>
      response(200, { versions: { "0.2.0": {} } }),
    ),
    "unpublished",
  );
});

test("uses only the public Registry endpoint and the configured timeout signal", async () => {
  let request;
  const timeoutSignal = new AbortController().signal;
  await getPublishStatus(
    manifest,
    async (url, options) => {
      request = { url, options };
      return response(404);
    },
    () => timeoutSignal,
  );
  assert.equal(request.url, "https://registry.npmjs.org/%40axclogo%2Feval");
  assert.equal(request.options.signal, timeoutSignal);
});

test("does not reinterpret Registry or response errors as a missing version", async () => {
  await assert.rejects(
    () => getPublishStatus(manifest, async () => response(503)),
    /HTTP 503/,
  );
  await assert.rejects(
    () =>
      getPublishStatus(manifest, async () => response(200, { versions: [] })),
    /npm registry versions must be an object/,
  );
});

test("keeps the privileged release workflow tag-bound, pinned and token-free", async () => {
  const workflow = await readFile(
    new URL("../.github/workflows/release.yml", import.meta.url),
    "utf8",
  );
  assert.match(workflow, /tags: \["v\*\.\*\.\*"\]/);
  assert.match(workflow, /id-token: write/);
  assert.match(workflow, /persist-credentials: false/);
  assert.match(workflow, /package-manager-cache: false/);
  assert.match(workflow, /node scripts\/check-npm-publish\.mjs/);
  assert.match(
    workflow,
    /npm publish "\.\/dist\/axclogo-eval-\$\{\{ steps\.package\.outputs\.version \}\}\.tgz"/,
  );
  assert.match(workflow, /status == 'bootstrap'[\s\S]*exit 1/);
  assert.doesNotMatch(workflow, /uses: [^\n]+@v\d/);
  assert.doesNotMatch(workflow, /NPM_TOKEN|NODE_AUTH_TOKEN|secrets\./);
});
