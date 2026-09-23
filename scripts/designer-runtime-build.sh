#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bash "${repo_root}/packages/browser-client/scripts/build-runtime.sh" \
  "${repo_root}/apps/designer/public"
