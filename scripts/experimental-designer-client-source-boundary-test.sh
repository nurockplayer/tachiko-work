#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
smoke="${repo_root}/scripts/experimental-designer-client-smoke.sh"

make_fixture() {
  local root="$1"
  mkdir -p "${root}/scripts" "${root}/examples/experimental-designer-client/src" "${root}/bin"
  cp "${smoke}" "${root}/scripts/experimental-designer-client-smoke.sh"
  cat >"${root}/scripts/export-experimental-designer-client.sh" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
  chmod +x "${root}/scripts/export-experimental-designer-client.sh"

  for tool in bash dirname mktemp rm sed sort; do
    ln -s "$(command -v "${tool}")" "${root}/bin/${tool}"
  done
  cat >"${root}/bin/diff" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
  cat >"${root}/bin/find" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' \
  'README.md' \
  'designer_runtime.wasm' \
  'experimental-client.d.ts' \
  'experimental-client.js' \
  'experimental-client.worker.d.ts' \
  'experimental-client.worker.js' \
  'host/project-transfer.d.ts' \
  'host/project-transfer.js' \
  'package.json' \
  'runtime/client.d.ts' \
  'runtime/client.js' \
  'runtime/interop-protocol.d.ts' \
  'runtime/interop-protocol.js' \
  'runtime/protocol.d.ts' \
  'runtime/protocol.js' \
  'runtime/wasm-bridge.d.ts' \
  'runtime/wasm-bridge.js' \
  'runtime/worker-client.d.ts' \
  'runtime/worker-client.js' \
  'runtime/worker-runtime.d.ts' \
  'runtime/worker-runtime.js'
EOF
  cat >"${root}/bin/pnpm" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
  chmod +x "${root}/bin/diff" "${root}/bin/find" "${root}/bin/pnpm"
}

run_case() {
  local rg_status="$1"
  local expected="$2"
  local root
  root="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-source-boundary-test.XXXXXX")"
  make_fixture "${root}"

  cat >"${root}/bin/rg" <<EOF
#!/usr/bin/env bash
if [[ ${rg_status} -eq 0 ]]; then
  printf '%s\n' 'fixture:1:forbidden source import'
fi
exit ${rg_status}
EOF
  chmod +x "${root}/bin/rg"

  set +e
  PATH="${root}/bin" /bin/bash "${root}/scripts/experimental-designer-client-smoke.sh" >/dev/null 2>&1
  local status=$?
  set -e
  rm -rf -- "${root}"

  case "${expected}" in
    success)
      if [[ ${status} -ne 0 ]]; then
        echo "source-boundary test: rg=${rg_status} should mean clean no-match, got ${status}" >&2
        return 1
      fi
      ;;
    failure)
      if [[ ${status} -eq 0 ]]; then
        echo "source-boundary test: rg=${rg_status} must not be treated as clean no-match" >&2
        return 1
      fi
      ;;
    *)
      echo "source-boundary test: invalid expectation ${expected}" >&2
      return 1
      ;;
  esac
}

# ripgrep: 0 = match, 1 = clean no-match, >1 = scan/tool failure.
run_case 1 success
run_case 0 failure
run_case 2 failure
run_case 127 failure

echo "experimental Designer source-boundary scanner semantics passed"
