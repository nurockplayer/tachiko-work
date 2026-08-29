#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
runtime_manifest="${repo_root}/apps/designer/runtime/Cargo.toml"
runtime_artifact="${repo_root}/apps/designer/runtime/target/wasm32-unknown-unknown/release/tachiko_designer_runtime.wasm"
public_dir="${repo_root}/apps/designer/public"

cargo build \
  --manifest-path "${runtime_manifest}" \
  --target wasm32-unknown-unknown \
  --release \
  --locked

mkdir -p "${public_dir}"
cp "${runtime_artifact}" "${public_dir}/designer_runtime.wasm"
