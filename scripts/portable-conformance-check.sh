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

cargo build --locked --package tachiko-formula-engine \
  --message-format json-render-diagnostics >"${check_dir}/native-build.jsonl"
native_formula="$(node scripts/cargo-artifact-path.mjs \
  "${check_dir}/native-build.jsonl" tachiko_formula_engine)"
native_semantic="$(node scripts/cargo-artifact-path.mjs \
  "${check_dir}/native-build.jsonl" tachiko_semantic_core)"
native_deps="$(dirname "${native_semantic}")"
rustc --edition=2024 \
  -L "dependency=${native_deps}" \
  --extern "tachiko_formula_engine=${native_formula}" \
  --extern "tachiko_semantic_core=${native_semantic}" \
  scripts/portable-conformance-check.rs \
  -o "${check_dir}/portable-conformance-native"
"${check_dir}/portable-conformance-native" >"${check_dir}/native.out"

cargo build --locked --target wasm32-unknown-unknown \
  --package tachiko-formula-engine \
  --message-format json-render-diagnostics >"${check_dir}/wasm-build.jsonl"
wasm_formula="$(node scripts/cargo-artifact-path.mjs \
  "${check_dir}/wasm-build.jsonl" tachiko_formula_engine)"
wasm_semantic="$(node scripts/cargo-artifact-path.mjs \
  "${check_dir}/wasm-build.jsonl" tachiko_semantic_core)"
wasm_deps="$(dirname "${wasm_semantic}")"
rustc --edition=2024 --target wasm32-unknown-unknown --crate-type cdylib \
  -L "dependency=${wasm_deps}" \
  -L "dependency=${native_deps}" \
  --extern "tachiko_formula_engine=${wasm_formula}" \
  --extern "tachiko_semantic_core=${wasm_semantic}" \
  scripts/portable-conformance-check.rs \
  -o "${check_dir}/portable-conformance.wasm"
node scripts/portable-conformance-check.mjs \
  "${check_dir}/portable-conformance.wasm" \
  >"${check_dir}/wasm.out"
cmp "${check_dir}/native.out" "${check_dir}/wasm.out"

echo "portable conformance check passed: production semantic records match native/WASM"
