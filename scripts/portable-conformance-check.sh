#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

command -v node >/dev/null 2>&1 || {
  echo "portable-conformance-check: node is required" >&2
  exit 1
}
rustup target list --installed | grep -qx 'wasm32-unknown-unknown' || {
  echo "portable-conformance-check: wasm32-unknown-unknown is not installed" >&2
  exit 1
}

check_dir="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-portable-check.XXXXXX")"
cleanup() {
  rm -rf -- "${check_dir}"
}
trap cleanup EXIT

cargo build --locked \
  --package tachiko-ai-api \
  --package tachiko-storage \
  --message-format json-render-diagnostics >"${check_dir}/native-build.jsonl"
native_ai="$(node scripts/cargo-artifact-path.mjs \
  "${check_dir}/native-build.jsonl" tachiko_ai_api)"
native_formula="$(node scripts/cargo-artifact-path.mjs \
  "${check_dir}/native-build.jsonl" tachiko_formula_engine)"
native_sha2="$(node scripts/cargo-artifact-path.mjs \
  "${check_dir}/native-build.jsonl" sha2)"
native_semantic="$(node scripts/cargo-artifact-path.mjs \
  "${check_dir}/native-build.jsonl" tachiko_semantic_core)"
native_storage="$(node scripts/cargo-artifact-path.mjs \
  "${check_dir}/native-build.jsonl" tachiko_storage)"
native_workspace="$(node scripts/cargo-artifact-path.mjs \
  "${check_dir}/native-build.jsonl" tachiko_workspace_engine)"
native_deps="$(dirname "${native_semantic}")"
rustc --edition=2024 \
  -L "dependency=${native_deps}" \
  --extern "tachiko_ai_api=${native_ai}" \
  --extern "tachiko_formula_engine=${native_formula}" \
  --extern "sha2=${native_sha2}" \
  --extern "tachiko_semantic_core=${native_semantic}" \
  --extern "tachiko_storage=${native_storage}" \
  --extern "tachiko_workspace_engine=${native_workspace}" \
  scripts/portable-conformance-check.rs \
  -o "${check_dir}/portable-conformance-native"
"${check_dir}/portable-conformance-native" >"${check_dir}/native.out"

cargo build --locked --target wasm32-unknown-unknown \
  --package tachiko-ai-api \
  --package tachiko-storage \
  --message-format json-render-diagnostics >"${check_dir}/wasm-build.jsonl"
wasm_ai="$(node scripts/cargo-artifact-path.mjs \
  "${check_dir}/wasm-build.jsonl" tachiko_ai_api)"
wasm_formula="$(node scripts/cargo-artifact-path.mjs \
  "${check_dir}/wasm-build.jsonl" tachiko_formula_engine)"
wasm_sha2="$(node scripts/cargo-artifact-path.mjs \
  "${check_dir}/wasm-build.jsonl" sha2)"
wasm_semantic="$(node scripts/cargo-artifact-path.mjs \
  "${check_dir}/wasm-build.jsonl" tachiko_semantic_core)"
wasm_storage="$(node scripts/cargo-artifact-path.mjs \
  "${check_dir}/wasm-build.jsonl" tachiko_storage)"
wasm_workspace="$(node scripts/cargo-artifact-path.mjs \
  "${check_dir}/wasm-build.jsonl" tachiko_workspace_engine)"
wasm_deps="$(dirname "${wasm_semantic}")"
rustc --edition=2024 --target wasm32-unknown-unknown --crate-type cdylib \
  -L "dependency=${wasm_deps}" \
  -L "dependency=${native_deps}" \
  --extern "tachiko_ai_api=${wasm_ai}" \
  --extern "tachiko_formula_engine=${wasm_formula}" \
  --extern "sha2=${wasm_sha2}" \
  --extern "tachiko_semantic_core=${wasm_semantic}" \
  --extern "tachiko_storage=${wasm_storage}" \
  --extern "tachiko_workspace_engine=${wasm_workspace}" \
  scripts/portable-conformance-check.rs \
  -o "${check_dir}/portable-conformance.wasm"
node scripts/portable-conformance-check.mjs \
  "${check_dir}/portable-conformance.wasm" \
  >"${check_dir}/wasm.out"
cmp "${check_dir}/native.out" "${check_dir}/wasm.out"

if grep -q '|255|' "${check_dir}/native.out"; then
  echo "portable-conformance-check: fixed oracle mismatch" >&2
  cat "${check_dir}/native.out" >&2
  exit 1
fi

echo "portable conformance check passed: production semantic/storage/workspace/AI records match native/WASM and fixed oracles"
