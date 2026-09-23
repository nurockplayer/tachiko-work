#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -ne 1 ]]; then
  echo "build-runtime: usage: build-runtime.sh OUTPUT_DIRECTORY" >&2
  exit 1
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
runtime_manifest="${script_dir}/../runtime/Cargo.toml"
runtime_artifact="${script_dir}/../runtime/target/wasm32-unknown-unknown/release/tachiko_designer_runtime.wasm"
output_dir="$1"
if [[ "${output_dir}" != /* ]]; then
  output_dir="${PWD}/${output_dir}"
fi

cargo build \
  --manifest-path "${runtime_manifest}" \
  --target wasm32-unknown-unknown \
  --release \
  --locked

mkdir -p "${output_dir}"
cp "${runtime_artifact}" "${output_dir}/designer_runtime.wasm"
