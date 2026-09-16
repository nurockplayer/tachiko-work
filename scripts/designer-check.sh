#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
designer_dir="${repo_root}/apps/designer"
runtime_manifest="${designer_dir}/runtime/Cargo.toml"
runtime_src="${designer_dir}/runtime/src"

command -v pnpm >/dev/null 2>&1 || {
  echo "designer-check: pnpm 11.25.0 is required" >&2
  exit 1
}
command -v rg >/dev/null 2>&1 || {
  echo "designer-check: ripgrep (rg) is required for exported-client boundary checks" >&2
  exit 1
}
designer_pnpm_version="$(pnpm --dir "${designer_dir}" --version)"
if [[ "${designer_pnpm_version}" != "11.25.0" ]]; then
  echo "designer-check: pnpm 11.25.0 is required; found ${designer_pnpm_version}" >&2
  exit 1
fi

# The standalone runtime permits Rust 2024 `#[unsafe(no_mangle)]` only for the
# private WASM C ABI. Do not let that crate-level lint exception become a
# general unsafe escape hatch.
if rg -n --glob '*.rs' '\bunsafe[[:space:]]*\{|\bunsafe[[:space:]]+(fn|impl|trait)\b' "${runtime_src}"; then
  echo "designer-check: unsafe blocks/functions/impls/traits are outside the approved Designer runtime boundary" >&2
  exit 1
fi
if rg -n -P --glob '*.rs' '#\[unsafe\((?!no_mangle\))' "${runtime_src}"; then
  echo "designer-check: only #[unsafe(no_mangle)] is approved for Designer runtime unsafe attributes" >&2
  exit 1
fi
if rg -n --glob '*.rs' '#\[unsafe\(no_mangle\)\]' "${runtime_src}" -g '!wasm.rs'; then
  echo "designer-check: #[unsafe(no_mangle)] is allowed only in runtime/src/wasm.rs" >&2
  exit 1
fi

pnpm --dir "${designer_dir}" install --frozen-lockfile
pnpm --dir "${designer_dir}" peers check

cargo fmt --manifest-path "${runtime_manifest}" --all -- --check
cargo clippy --manifest-path "${runtime_manifest}" --all-targets --locked -- -D warnings
cargo test --manifest-path "${runtime_manifest}" --all-targets --locked

pnpm --dir "${designer_dir}" lint
pnpm --dir "${designer_dir}" typecheck
pnpm --dir "${designer_dir}" test
pnpm --dir "${designer_dir}" build
pnpm --dir "${designer_dir}" exec playwright test
bash "${repo_root}/scripts/export-experimental-designer-client-immutable-source-test.sh"
bash "${repo_root}/scripts/experimental-designer-client-source-boundary-test.sh"
bash "${repo_root}/scripts/experimental-designer-client-smoke-lifecycle-test.sh"
bash "${repo_root}/scripts/experimental-designer-client-smoke.sh"
