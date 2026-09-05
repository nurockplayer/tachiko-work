#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
smoke="${repo_root}/scripts/experimental-designer-client-smoke.sh"
expected_files=$'README.md\ndesigner_runtime.wasm\nexperimental-client.d.ts\nexperimental-client.js\nexperimental-client.worker.d.ts\nexperimental-client.worker.js\nhost/project-transfer.d.ts\nhost/project-transfer.js\npackage.json\nruntime/client.d.ts\nruntime/client.js\nruntime/interop-protocol.d.ts\nruntime/interop-protocol.js\nruntime/protocol.d.ts\nruntime/protocol.js\nruntime/wasm-bridge.d.ts\nruntime/wasm-bridge.js\nruntime/worker-client.d.ts\nruntime/worker-client.js\nruntime/worker-runtime.d.ts\nruntime/worker-runtime.js'

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
  cat >"${root}/bin/find" <<EOF
#!/usr/bin/env bash
cat <<'FILES'
${expected_files}
FILES
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
  trap 'rm -rf -- "${root}"' RETURN
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
