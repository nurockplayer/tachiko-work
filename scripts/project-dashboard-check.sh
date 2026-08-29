#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dashboard_dir="${repo_root}/tools/project-dashboard"

command -v pnpm >/dev/null 2>&1 || {
  echo "project-dashboard-check: pnpm 11.25.0 is required" >&2
  exit 1
}
dashboard_pnpm_version="$(pnpm --dir "${dashboard_dir}" --version)"
if [[ "${dashboard_pnpm_version}" != "11.25.0" ]]; then
  echo "project-dashboard-check: pnpm 11.25.0 is required; found ${dashboard_pnpm_version}" >&2
  exit 1
fi

pnpm --dir "${dashboard_dir}" install --frozen-lockfile
pnpm --dir "${dashboard_dir}" peers check
pnpm --dir "${dashboard_dir}" lint
pnpm --dir "${dashboard_dir}" typecheck
pnpm --dir "${dashboard_dir}" test
pnpm --dir "${dashboard_dir}" build
pnpm --dir "${dashboard_dir}" exec playwright test
