#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
consumer_dir="${repo_root}/examples/experimental-designer-client"
vendor_dir="${consumer_dir}/vendor/tachiko"
check_dir="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-experimental-client-smoke.XXXXXX")"

rm -rf -- "${consumer_dir}/vendor"
cleanup() {
  rm -rf -- "${consumer_dir}/vendor" "${check_dir}"
}
trap cleanup EXIT

bash "${repo_root}/scripts/export-experimental-designer-client.sh" "${vendor_dir}"
bash "${repo_root}/scripts/export-experimental-designer-client.sh" "${check_dir}/kit"
diff -qr "${vendor_dir}" "${check_dir}/kit"

expected_files=$'README.md\ndesigner_runtime.wasm\nexperimental-client.d.ts\nexperimental-client.js\nexperimental-client.worker.d.ts\nexperimental-client.worker.js\nhost/project-transfer.d.ts\nhost/project-transfer.js\npackage.json\nruntime/client.d.ts\nruntime/client.js\nruntime/protocol.d.ts\nruntime/protocol.js\nruntime/wasm-bridge.d.ts\nruntime/wasm-bridge.js\nruntime/worker-client.d.ts\nruntime/worker-client.js\nruntime/worker-runtime.d.ts\nruntime/worker-runtime.js'
actual_files="$(find "${vendor_dir}" -type f -print | sed "s#${vendor_dir}/##" | LC_ALL=C sort)"
if [[ "${actual_files}" != "${expected_files}" ]]; then
  echo "experimental-designer-client-smoke: exported kit shape changed unexpectedly" >&2
  diff -u <(printf '%s\n' "${expected_files}") <(printf '%s\n' "${actual_files}") || true
  exit 1
fi

if rg -n 'apps/designer|src/runtime|src/host' "${consumer_dir}/src"; then
  echo "experimental-designer-client-smoke: consumer imports private Designer source" >&2
  exit 1
fi
if rg -n '\.ts"' "${vendor_dir}" -g '*.js'; then
  echo "experimental-designer-client-smoke: emitted JavaScript retains source-only imports" >&2
  exit 1
fi
pnpm --dir "${repo_root}/apps/designer" exec node \
  --eval 'const manifest = JSON.parse(require("node:fs").readFileSync(process.argv[1], "utf8")); if (manifest.private !== true || manifest.packageManager !== "pnpm@11.25.0") process.exit(1);' \
  "${vendor_dir}/package.json"

pnpm --dir "${repo_root}/apps/designer" exec tsc \
  --project "${consumer_dir}/tsconfig.json" \
  --noEmit \
  --pretty false
pnpm --dir "${repo_root}/apps/designer" exec playwright test \
  --config playwright.experimental-client.config.ts

echo "experimental Designer client smoke passed"
