#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dashboard_dir="${repo_root}/tools/project-dashboard"

if [[ "${TACHIKO_OPERATIONAL_EVIDENCE_CHECKED:-0}" != "1" ]]; then
  bash "${repo_root}/scripts/operational-evidence-check.sh"
fi

cd "${dashboard_dir}"

command -v pnpm >/dev/null 2>&1 || {
  echo "project-dashboard-check: pnpm 11.25.0 is required" >&2
  exit 1
}
dashboard_pnpm_version="$(pnpm --version)"
if [[ "${dashboard_pnpm_version}" != "11.25.0" ]]; then
  echo "project-dashboard-check: pnpm 11.25.0 is required; found ${dashboard_pnpm_version}" >&2
  exit 1
fi

pnpm install --frozen-lockfile
pnpm peers check
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm exec playwright test
