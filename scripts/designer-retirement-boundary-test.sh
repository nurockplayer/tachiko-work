#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

fail=0
if [[ -e apps/designer ]]; then
  echo "FAIL: legacy human-facing Designer app still exists" >&2
  fail=1
fi

for path in \
  README.md \
  CONTRIBUTING.md \
  scripts/release-check.sh \
  .github/workflows/ci.yml \
  scripts/experimental-designer-client-smoke.sh \
  scripts/experimental-designer-client-acceptance.sh; do
  if [[ ! -f "${path}" ]]; then
    echo "FAIL: required release/consumer route missing: ${path}" >&2
    fail=1
    continue
  fi
  if [[ "${path}" == scripts/experimental-designer-client-* ]]; then
    pattern='pnpm --dir.*apps/designer|playwright\.experimental-client\.config\.ts'
  else
    pattern='apps/designer|scripts/designer-check\.sh|Web Designer runtime and browser journey|Run the browser Designer slice|first-party Web Designer'
  fi
  if rg -n "${pattern}" "${path}"; then
    echo "FAIL: active route still points at legacy Designer: ${path}" >&2
    fail=1
  fi
done

for path in \
  packages/browser-client/src/experimental-client.ts \
  packages/browser-client/src/experimental-client.worker.ts \
  packages/browser-client/runtime/src/lib.rs \
  packages/browser-client/runtime/tests/local_ro_document_ingress.rs \
  packages/browser-client/runtime/tests/local_ro_launch_preflight.rs \
  scripts/export-experimental-designer-client.sh \
  scripts/experimental-designer-client-smoke.sh \
  scripts/experimental-designer-client-acceptance.sh \
  tests/consumer-kit/seed/tests/browser.mjs; do
  if [[ ! -f "${path}" ]]; then
    echo "FAIL: retained producer/runtime/consumer evidence missing: ${path}" >&2
    fail=1
  fi
done

if [[ "${fail}" -ne 0 ]]; then
  exit 1
fi

echo "PASS: Designer product is absent and active Work routes retain the technical producer/consumer boundary"
