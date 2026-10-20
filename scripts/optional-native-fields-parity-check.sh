#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tests_dir="${repo_root}/packages/browser-client/runtime/tests"
if [[ "$#" -ne 1 || ! -s "$1" ]]; then
  echo "optional-native-fields-parity-check: expected the just-built designer_runtime.wasm" >&2
  exit 1
fi
wasm="$1"
check_dir="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-optional-native-parity.XXXXXX")"
check_dir="$(cd "${check_dir}" && pwd -P)"
cleanup() {
  local status="$?"
  if [[ "${status}" -eq 0 ]]; then
    rm -rf -- "${check_dir}"
  else
    echo "optional-native-fields-parity-check: retained failed captures: ${check_dir}" >&2
  fi
  exit "${status}"
}
trap cleanup EXIT

node --test "${tests_dir}/optional_native_fields.test.mjs" \
  "${tests_dir}/optional_native_persistence.test.mjs"

# Never consume inherited capture paths or a previous run's native evidence.
TACHIKO_OPTIONAL_TRANSCRIPT="${check_dir}/optional-native-fields.json" \
  TACHIKO_OPTIONAL_PERSISTENCE_TRANSCRIPT="${check_dir}/optional-persistence" \
  TACHIKO_IMPORTED_STRUCTURAL_TRANSCRIPT="${check_dir}/imported-structure" \
  cargo test --manifest-path "${repo_root}/packages/browser-client/runtime/Cargo.toml" \
  --locked --test optional_native_fields --test optional_native_persistence --test imported_structural_export

fields_captures=(
  "${check_dir}/optional-native-fields.json"
  "${check_dir}/optional-native-fields.json.capacity"
  "${check_dir}/optional-native-fields.json.formula"
  "${check_dir}/optional-native-fields.json.rowgate"
  "${check_dir}/optional-native-fields.json.editability-boolean"
  "${check_dir}/optional-native-fields.json.editability-date"
  "${check_dir}/optional-native-fields.json.editability-text"
)
persistence_captures=()
for sole in false true; do
  for portable in false true; do
    persistence_captures+=("${check_dir}/optional-persistence.sole-${sole}-portable-${portable}.json")
  done
done
for source in csv xlsx; do
  for operation in rename add_blank add_fill remove insert insert_blank delete; do
    persistence_captures+=("${check_dir}/imported-structure.${source}-${operation}.json")
  done
done
persistence_captures+=("${check_dir}/imported-structure.refusals.json" "${check_dir}/imported-structure.formulas.json")
for repair in explicit-name-csv explicit-name-xlsx cell-compatibility-csv cell-compatibility-xlsx foreign-current header-history-collision untouched-compatibility scalar-compatibility formula-output-policy csv-output-scope presentation-output-policy style-inventory-policy references-headers-policy closed-grammar-policy boolean-output-policy precision-budget-policy; do
  persistence_captures+=("${check_dir}/imported-structure.${repair}.json")
done
for capture in "${fields_captures[@]}" "${persistence_captures[@]}"; do
  [[ -s "${capture}" ]] || {
    echo "optional-native-fields-parity-check: missing or empty native capture: ${capture}" >&2
    exit 1
  }
done

node "${tests_dir}/optional_native_fields.mjs" "${wasm}" "${fields_captures[@]}"
node "${tests_dir}/optional_native_persistence.mjs" "${wasm}" "${persistence_captures[@]}"
echo "optional native-field and persistence native/WASM parity passed"
