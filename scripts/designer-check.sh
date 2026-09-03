#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
designer_dir="${repo_root}/apps/designer"
runtime_manifest="${designer_dir}/runtime/Cargo.toml"

command -v pnpm >/dev/null 2>&1 || {
  echo "designer-check: pnpm 11.25.0 is required" >&2
  exit 1
}
designer_pnpm_version="$(pnpm --dir "${designer_dir}" --version)"
if [[ "${designer_pnpm_version}" != "11.25.0" ]]; then
  echo "designer-check: pnpm 11.25.0 is required; found ${designer_pnpm_version}" >&2
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
bash "${repo_root}/scripts/experimental-designer-client-smoke.sh"
