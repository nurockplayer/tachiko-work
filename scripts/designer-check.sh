#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
designer_dir="${repo_root}/apps/designer"
runtime_manifest="${designer_dir}/runtime/Cargo.toml"
runtime_root="${designer_dir}/runtime"

command -v pnpm >/dev/null 2>&1 || {
  echo "designer-check: pnpm 11.25.0 is required" >&2
  exit 1
}
command -v rg >/dev/null 2>&1 || {
  echo "designer-check: ripgrep (rg) is required for exported-client boundary checks" >&2
  exit 1
}
command -v node >/dev/null 2>&1 || {
  echo "designer-check: Node.js is required for Designer unsafe-surface checks" >&2
  exit 1
}
designer_pnpm_version="$(pnpm --dir "${designer_dir}" --version)"
if [[ "${designer_pnpm_version}" != "11.25.0" ]]; then
  echo "designer-check: pnpm 11.25.0 is required; found ${designer_pnpm_version}" >&2
  exit 1
fi

# The standalone runtime permits Rust 2024 `#[unsafe(no_mangle)]` only for the
# private WASM C ABI. The lexical scanner rejects every other real `unsafe`
# token, including multiline forms, while ignoring comments and literals.
node "${repo_root}/scripts/designer-unsafe-surface-check.mjs" "${runtime_root}"
node "${repo_root}/scripts/designer-unsafe-surface-check-test.mjs"

pnpm --dir "${designer_dir}" install --frozen-lockfile
pnpm --dir "${designer_dir}" peers check

cargo fmt --manifest-path "${runtime_manifest}" --all -- --check
cargo clippy --manifest-path "${runtime_manifest}" --all-targets --locked -- -D warnings
cargo test --manifest-path "${runtime_manifest}" --all-targets --locked
# Re-scan after Cargo has run build scripts so generated or rewritten sources
# are checked at the same post-build state that subsequent validation observes.
node "${repo_root}/scripts/designer-unsafe-surface-check.mjs" "${runtime_root}"

pnpm --dir "${designer_dir}" lint
pnpm --dir "${designer_dir}" test
pnpm --dir "${designer_dir}" build
# The frontend build runs another Cargo/WASM build; re-scan after it as well.
node "${repo_root}/scripts/designer-unsafe-surface-check.mjs" "${runtime_root}"
pnpm --dir "${designer_dir}" exec playwright test
bash "${repo_root}/scripts/export-experimental-designer-client-immutable-source-test.sh"
bash "${repo_root}/scripts/experimental-designer-client-source-boundary-test.sh"
bash "${repo_root}/scripts/experimental-designer-client-smoke-lifecycle-test.sh"
bash "${repo_root}/scripts/experimental-designer-client-smoke.sh"
