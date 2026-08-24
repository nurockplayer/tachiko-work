#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: run-portable-observations.sh --candidate-root /abs/repo --crate-set core|full --output /abs/observations.json" >&2
  exit 2
}

candidate_root=""
crate_set=""
output=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --candidate-root) candidate_root="$2"; shift 2 ;;
    --crate-set) crate_set="$2"; shift 2 ;;
    --output) output="$2"; shift 2 ;;
    *) usage ;;
  esac
done
[[ -n "${candidate_root}" && -n "${crate_set}" && -n "${output}" ]] || usage
[[ "${candidate_root}" = /* && "${output}" = /* ]] || usage
[[ -d "${candidate_root}" && -d "$(dirname "${output}")" ]] || usage

case "${crate_set}" in
  core)
    packages=(tachiko-formula-engine tachiko-storage)
    crates=(tachiko_formula_engine tachiko_semantic_core tachiko_storage)
    ;;
  full)
    packages=(tachiko-ai-api tachiko-storage)
    crates=(tachiko_ai_api tachiko_formula_engine tachiko_semantic_core tachiko_storage tachiko_workspace_engine)
    ;;
  *) usage ;;
esac

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${candidate_root}"
rustup target list --installed | grep -qx 'wasm32-unknown-unknown'

check_dir="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-portable-observe.XXXXXX")"
cleanup() {
  rm -rf -- "${check_dir}"
}
trap cleanup EXIT

package_args=()
for package in "${packages[@]}"; do
  package_args+=(--package "${package}")
done

cargo build --locked "${package_args[@]}" \
  --message-format json-render-diagnostics >"${check_dir}/native-build.jsonl"
native_args=()
native_deps=""
for crate in "${crates[@]}"; do
  artifact="$(node scripts/cargo-artifact-path.mjs "${check_dir}/native-build.jsonl" "${crate}")"
  native_args+=(--extern "${crate}=${artifact}")
  if [[ "${crate}" == "tachiko_semantic_core" ]]; then
    native_deps="$(dirname "${artifact}")"
  fi
done
[[ -n "${native_deps}" ]]
rustc --edition=2024 -L "dependency=${native_deps}" "${native_args[@]}" \
  scripts/portable-conformance-check.rs -o "${check_dir}/portable-native"
"${check_dir}/portable-native" >"${check_dir}/native.out"

cargo build --locked --target wasm32-unknown-unknown "${package_args[@]}" \
  --message-format json-render-diagnostics >"${check_dir}/wasm-build.jsonl"
wasm_args=()
wasm_deps=""
for crate in "${crates[@]}"; do
  artifact="$(node scripts/cargo-artifact-path.mjs "${check_dir}/wasm-build.jsonl" "${crate}")"
  wasm_args+=(--extern "${crate}=${artifact}")
  if [[ "${crate}" == "tachiko_semantic_core" ]]; then
    wasm_deps="$(dirname "${artifact}")"
  fi
done
[[ -n "${wasm_deps}" ]]
rustc --edition=2024 --target wasm32-unknown-unknown --crate-type cdylib \
  -L "dependency=${wasm_deps}" -L "dependency=${native_deps}" "${wasm_args[@]}" \
  scripts/portable-conformance-check.rs -o "${check_dir}/portable.wasm"
node "${script_dir}/portable-wasm-records.mjs" "${check_dir}/portable.wasm" \
  >"${check_dir}/wasm.out"

node "${script_dir}/collect-portable-observations.mjs" \
  --native "${check_dir}/native.out" \
  --wasm "${check_dir}/wasm.out" \
  --output "${output}"
