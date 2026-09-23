#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
producer_dir="${repo_root}/packages/browser-client"
runtime_manifest="${producer_dir}/runtime/Cargo.toml"
runtime_root="${producer_dir}/runtime"

command -v pnpm >/dev/null 2>&1 || {
  echo "browser-client-check: pnpm 11.25.0 is required" >&2
  exit 1
}
command -v rg >/dev/null 2>&1 || {
  echo "browser-client-check: ripgrep (rg) is required for exported-kit boundary checks" >&2
  exit 1
}
command -v node >/dev/null 2>&1 || {
  echo "browser-client-check: Node.js is required for runtime unsafe-surface checks" >&2
  exit 1
}
pnpm_version="$(pnpm --dir "${producer_dir}" --version)"
if [[ "${pnpm_version}" != "11.25.0" ]]; then
  echo "browser-client-check: pnpm 11.25.0 is required; found ${pnpm_version}" >&2
  exit 1
fi

# The standalone runtime permits Rust 2024 `#[unsafe(no_mangle)]` only for the
# private WASM C ABI. The scanner rejects every other real `unsafe` token.
node "${repo_root}/scripts/designer-unsafe-surface-check.mjs" "${runtime_root}"
node "${repo_root}/scripts/designer-unsafe-surface-check-test.mjs"

pnpm --dir "${producer_dir}" install --frozen-lockfile

cargo fmt --manifest-path "${runtime_manifest}" --all -- --check
cargo clippy --manifest-path "${runtime_manifest}" --all-targets --locked -- -D warnings
cargo test --manifest-path "${runtime_manifest}" --all-targets --locked
node "${repo_root}/scripts/designer-unsafe-surface-check.mjs" "${runtime_root}"

pnpm --dir "${producer_dir}" lint
pnpm --dir "${producer_dir}" test
pnpm --dir "${producer_dir}" build
node "${repo_root}/scripts/designer-unsafe-surface-check.mjs" "${runtime_root}"
bash "${repo_root}/scripts/export-experimental-designer-client-immutable-source-test.sh"
bash "${repo_root}/scripts/experimental-designer-client-source-boundary-test.sh"
bash "${repo_root}/scripts/experimental-designer-client-smoke-lifecycle-test.sh"
bash "${repo_root}/scripts/experimental-designer-client-smoke.sh"
