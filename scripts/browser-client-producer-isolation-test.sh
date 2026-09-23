#!/usr/bin/env bash
set -euo pipefail

# A producer qualifies only if its exact committed source can build the
# consumer artifact without the retiring human-facing Designer application.
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
test_root="$(mktemp -d)"
trap 'rm -rf -- "${test_root}"' EXIT
mkdir -p "${test_root}/source" "${test_root}/scratch"

git -C "${repo_root}" archive --format=tar HEAD | tar -xf - -C "${test_root}/source"
rm -rf -- "${test_root}/source/apps/designer"

git -C "${test_root}/source" init --quiet
git -C "${test_root}/source" config user.name "Tachiko producer isolation test"
git -C "${test_root}/source" config user.email "producer-isolation@invalid.example"
git -C "${test_root}/source" add --all
git -C "${test_root}/source" commit --quiet -m "Temporary pruned producer source"
pruned_commit="$(git -C "${test_root}/source" rev-parse HEAD)"

TMPDIR="${test_root}/scratch" \
  bash "${test_root}/source/scripts/export-experimental-designer-client.sh" \
  "${test_root}/kit"

node - "${test_root}/kit/artifact-manifest.json" "${pruned_commit}" <<'NODE'
const fs = require("node:fs");
const [manifestPath, prunedCommit] = process.argv.slice(2);
const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
if (manifest.sourceCommit !== prunedCommit) {
  throw new Error("producer artifact does not identify the actual pruned source commit");
}
for (const required of [
  "experimental-client.js",
  "experimental-client.d.ts",
  "experimental-client.worker.js",
  "designer_runtime.wasm",
]) {
  if (!manifest.files.some((entry) => entry.path === required)) {
    throw new Error(`producer artifact is missing ${required}`);
  }
}
NODE

echo "browser-client producer isolation passed at temporary pruned source ${pruned_commit}"
