#!/usr/bin/env bash
# Proposed repository replacement: scripts/experimental-designer-client-source-boundary-test.sh
set -euo pipefail

repo_root="$(cd "$(dirname "$BASH_SOURCE")/.." && pwd)"
smoke="${TACHIKO_SMOKE_UNDER_TEST:-$repo_root/scripts/experimental-designer-client-smoke.sh}"

make_fixture() {
  local root="$1"
  mkdir -p "$root/scripts" "$root/examples/experimental-designer-client/src" "$root/bin"
  cp "$smoke" "$root/scripts/experimental-designer-client-smoke.sh"
  printf '#!/usr/bin/env bash\nexit 0\n' >"$root/scripts/export-experimental-designer-client.sh"
  printf '#!/usr/bin/env bash\nexit 0\n' >"$root/scripts/experimental-designer-client-acceptance.sh"
  chmod +x "$root/scripts/export-experimental-designer-client.sh"
  chmod +x "$root/scripts/experimental-designer-client-acceptance.sh"
  for tool in bash dirname mktemp rm sed sort; do
    ln -s "$(command -v "$tool")" "$root/bin/$tool"
  done
  printf '#!/usr/bin/env bash\nexit 0\n' >"$root/bin/diff"
  cat >"$root/bin/find" <<'EOF_FIND'
#!/usr/bin/env bash
printf '%s\n' \
  'README.md' \
  'artifact-manifest.json' \
  'designer_runtime.wasm' \
  'experimental-client.d.ts' \
  'experimental-client.js' \
  'experimental-client.worker.d.ts' \
  'experimental-client.worker.js' \
  'host/project-transfer.d.ts' \
  'host/project-transfer.js' \
  'notices/LICENSE-APACHE' \
  'notices/LICENSE-MIT' \
  'notices/THIRD_PARTY_LICENSES.md' \
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
EOF_FIND
  # The fake exporter emits no files. These commands deliberately pass only so
  # this test remains isolated to rg status semantics; real integrity checks
  # run in the smoke and immutable-source regressions.
  printf '#!/usr/bin/env bash\nexit 0\n' >"$root/bin/pnpm"
  cat >"$root/bin/git" <<'EOF_GIT'
#!/usr/bin/env bash
if [[ "$*" == *"rev-parse --verify HEAD^{commit}"* ]]; then
  printf '%s\n' '0123456789abcdef0123456789abcdef01234567'
  exit 0
fi
exit 64
EOF_GIT
  chmod +x "$root/bin/diff" "$root/bin/find" "$root/bin/pnpm" "$root/bin/git"
}

run_case() {
  local private_source_status="$1"
  local emitted_js_status="$2"
  local expected="$3"
  local root output scan_result
  root="$(mktemp -d /tmp/tachiko-source-boundary-test.XXXXXX)"
  make_fixture "$root"
  cat >"$root/bin/rg" <<EOF_RG
#!/usr/bin/env bash
scan_result=2
for argument in "\$@"; do
  case "\$argument" in
    */examples/experimental-designer-client/src) scan_result=$private_source_status ;;
    */examples/experimental-designer-client/vendor/tachiko) scan_result=$emitted_js_status ;;
  esac
done
if [[ \$scan_result -eq 0 ]]; then printf '%s\n' 'fixture:1:forbidden source import'; fi
exit \$scan_result
EOF_RG
  chmod +x "$root/bin/rg"
  set +e
  output="$(PATH="$root/bin" /bin/bash "$root/scripts/experimental-designer-client-smoke.sh" 2>&1)"
  scan_result=$?
  set -e
  rm -rf -- "$root"
  case "$expected" in
    success)
      [[ $scan_result -eq 0 ]] || { echo "source-boundary test: clean scans should succeed, got $scan_result" >&2; return 1; }
      ;;
    failure)
      [[ $scan_result -ne 0 ]] || { echo "source-boundary test: private=$private_source_status emitted=$emitted_js_status must not be clean" >&2; return 1; }
      if [[ $private_source_status -gt 1 && "$output" != *"source-boundary scan failed (rg exit $private_source_status)"* ]]; then
        echo "source-boundary test: private-source scan failure must be actionable" >&2; return 1
      fi
      if [[ $emitted_js_status -gt 1 && "$output" != *"source-boundary scan failed (rg exit $emitted_js_status)"* ]]; then
        echo "source-boundary test: emitted-JavaScript scan failure must be actionable" >&2; return 1
      fi
      ;;
    *) echo "source-boundary test: invalid expectation $expected" >&2; return 1 ;;
  esac
}

# ripgrep: 0 = forbidden match, 1 = clean no-match, >1 = scan/tool failure.
run_case 1 1 success
run_case 0 1 failure
run_case 1 0 failure
run_case 2 1 failure
run_case 1 2 failure
run_case 127 1 failure
echo "experimental Designer source-boundary scanner semantics passed"
