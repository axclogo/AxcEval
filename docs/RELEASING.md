# Release process

AxcEval publishes three artifacts from one version tag:

- Python wheels and the `@axclogo/eval` WebAssembly package are attached to the GitHub Release.
- `@axclogo/eval` is published to the public npm Registry through npm Trusted Publisher.
- Rust source remains installable from the repository; crates.io publication is not part of this workflow.

The release workflow is `.github/workflows/release.yml`. It contains no npm token, secret fallback or automatic version
selection. A version is immutable after publication.

## One-time npm bootstrap

npm requires a package to exist before a Trusted Publisher can be attached. For the first version only:

1. Check out the reviewed release commit and run the complete engineering gate.
2. Build the versioned npm archive and run the WebAssembly smoke test against that exact archive.
3. Inspect its file list with `npm pack --dry-run <archive>`.
4. Sign in interactively with an npm maintainer account protected by 2FA.
5. Publish the inspected archive with `npm publish <archive> --access public`.
6. Sign out locally. Do not create, paste, store or commit an npm token.

The automatic workflow deliberately reports `bootstrap` and fails while the package does not exist. It never converts a
404, timeout or Registry error into permission to publish.

After bootstrap, configure the publisher with npm 11.15.0 or newer:

```sh
npm trust github @axclogo/eval \
  --repository axclogo/AxcEval \
  --file release.yml \
  --allow-publish \
  --yes
npm trust list @axclogo/eval --json
```

The account must have 2FA enabled and use interactive account authentication. A Granular Access Token with bypass 2FA
cannot create or query trust configuration; do not add a workflow token fallback when npm returns `E403`.

The equivalent npmjs.com Settings → Trusted publishing values are:

| Setting              | Value               |
| -------------------- | ------------------- |
| Provider             | GitHub Actions      |
| Organization or user | `axclogo`           |
| Repository           | `AxcEval`           |
| Workflow filename    | `release.yml`       |
| Environment          | blank               |
| Allowed action       | allow `npm publish` |

Complete the next successful OIDC publication within npm's activation window. Then set publishing access to require 2FA
and disallow traditional tokens. Trusted Publisher continues to work because it uses short-lived OIDC credentials.

## Prepare a release

1. Choose a stable Semantic Version. Prerelease tags are not accepted by the current workflow.
2. Update the shared version in `Cargo.toml` and the Python version in `crates/axc-eval-python/pyproject.toml`.
3. Update workspace dependency versions and `Cargo.lock` when Cargo requires it.
4. Move user-visible changes from `Unreleased` into a dated `CHANGELOG.md` section.
5. Run the complete engineering gate from `docs/ENGINEERING.md`, including Rust tests, coverage, Python smoke tests,
   WebAssembly smoke tests and artifact-size checks.
6. Run `node --test scripts/check-npm-publish.test.mjs`.
7. Review the diff for credentials, absolute paths, generated artifacts, caches and unrelated changes.
8. Merge or push the reviewed release commit to `main`.
9. Create and push the exact tag `v<version>` from that commit.

`scripts/check-release-tag.mjs` rejects disagreement between the tag, Cargo version and Python version.

## Automatic release gate

The tag workflow:

1. Builds the platform wheels and browser-bundler WebAssembly package from the tagged commit.
2. Rewrites and validates the npm manifest, copies the license and npm README, and enforces the 1 MiB WebAssembly
   budget.
3. Packs the exact npm archive and passes that immutable archive to the npm job.
4. Queries only the public npm Registry with a fixed timeout.
5. Publishes an absent version through OIDC, safely skips an existing version, and fails if the package is uninitialized
   or the Registry response is invalid.
6. Creates the GitHub Release only after the npm gate succeeds.

Only the npm job has `id-token: write`; only the GitHub Release job has `contents: write`. Release Actions are pinned to
reviewed commit SHAs. No job reads an npm secret, restores an npm dependency cache or persists checkout credentials.

## Post-release checks

Verify all of the following:

1. The GitHub Release contains the expected wheels and one `axc-eval-<version>.tgz` archive.
2. npm shows `@axclogo/eval@<version>`, the expected files, Apache-2.0 license and provenance for OIDC-published versions.
3. `latest` points to the intended stable version.
4. A clean browser-bundler project can run the documented `simulateCondition` import.
5. The source tag, Cargo version, Python version, npm version and changelog section all agree.

If npm publication succeeds but a later GitHub Release step fails, rerun the workflow. The Registry gate recognizes the
existing immutable version and performs a safe no-op before recreating the GitHub Release. Never bump, overwrite, unpublish
or change dist-tags as an automatic recovery step.
