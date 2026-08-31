#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tool_dir="${repo_root}/tools/operational-evidence"

cd "${tool_dir}"

command -v pnpm >/dev/null 2>&1 || {
  echo "operational-evidence-check: pnpm 11.25.0 is required" >&2
  exit 1
}
tool_pnpm_version="$(pnpm --version)"
if [[ "${tool_pnpm_version}" != "11.25.0" ]]; then
  echo "operational-evidence-check: pnpm 11.25.0 is required; found ${tool_pnpm_version}" >&2
  exit 1
fi

pnpm install --frozen-lockfile
pnpm peers check
pnpm typecheck
pnpm test
pnpm build
