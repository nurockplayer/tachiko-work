#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$BASH_SOURCE")/.." && pwd)"
smoke="${TACHIKO_SMOKE_UNDER_TEST:-$repo_root/scripts/experimental-designer-client-smoke.sh}"

make_fixture() {
  local root="$1"
  mkdir -p "$root/scripts" "$root/examples/experimental-designer-client/src" "$root/packages/browser-client" "$root/bin" "$root/tmp"
  cp "$smoke" "$root/scripts/experimental-designer-client-smoke.sh"
  cat >"$root/scripts/export-experimental-designer-client.sh" <<'EOF_EXPORT'
#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
status="$(git -C "$root" status --porcelain=v1 --untracked-files=all)"
[[ -z "$status" ]] || { echo "source checkout must be clean" >&2; exit 1; }
mkdir -p "$1"
printf 'complete exported kit\n' >"$1/payload"
EOF_EXPORT
  printf '#!/usr/bin/env bash\nexit 0\n' >"$root/scripts/experimental-designer-client-acceptance.sh"
  chmod +x "$root/scripts/export-experimental-designer-client.sh" "$root/scripts/experimental-designer-client-acceptance.sh"
  for tool in bash dirname mkdir mktemp mv rm sed sort; do
    ln -s "$(command -v "$tool")" "$root/bin/$tool"
  done
  cat >"$root/bin/git" <<'EOF_GIT'
#!/usr/bin/env bash
case " $* " in
  *" status --porcelain=v1 --untracked-files=all "*)
    if [[ -f "$FIXTURE_ROOT/packages/browser-client/test-results/.last-run.json" ]] ||
      compgen -G "$FIXTURE_ROOT/packages/browser-client/tmp/*/playwright-results/.last-run.json" >/dev/null; then
      printf '%s\n' '?? Playwright .last-run.json under source checkout'
    fi
    exit 0 ;;
esac
printf '%s\n' '0123456789abcdef0123456789abcdef01234567'
EOF_GIT
  cat >"$root/bin/find" <<'EOF_FIND'
#!/usr/bin/env bash
printf '%s\n' \
  'README.md' 'artifact-manifest.json' 'designer_runtime.wasm' \
  'experimental-client.d.ts' 'experimental-client.js' 'experimental-client.worker.d.ts' 'experimental-client.worker.js' \
  'host/project-transfer.d.ts' 'host/project-transfer.js' \
  'notices/LICENSE-APACHE' 'notices/LICENSE-MIT' 'notices/THIRD_PARTY_LICENSES.md' \
  'package.json' 'runtime/client.d.ts' 'runtime/client.js' 'runtime/interop-protocol.d.ts' \
  'runtime/interop-protocol.js' 'runtime/protocol.d.ts' 'runtime/protocol.js' \
  'runtime/wasm-bridge.d.ts' 'runtime/wasm-bridge.js' 'runtime/worker-client.d.ts' \
  'runtime/worker-client.js' 'runtime/worker-runtime.d.ts' 'runtime/worker-runtime.js'
EOF_FIND
  cat >"$root/bin/diff" <<'EOF_DIFF'
#!/usr/bin/env bash
exit "${SMOKE_DIFF_STATUS:-0}"
EOF_DIFF
  cat >"$root/bin/pnpm" <<'EOF_PNPM'
#!/usr/bin/env bash
set -euo pipefail
[[ "$1" == --dir ]] || exit 2
cd "$2"
shift 2
[[ "$1" == exec ]] || exit 2
shift
if [[ "${1:-}" != playwright ]]; then exit 0; fi
shift
[[ "${1:-}" == test ]] || exit 2
shift
output=""
while (($#)); do
  if [[ "$1" == --output ]]; then output="$2"; shift 2; else shift; fi
done
[[ -n "$output" ]] || output="test-results"
mkdir -p "$output"
printf '{"status":"complete"}\n' >"$output/.last-run.json"
exit "${SMOKE_PLAYWRIGHT_STATUS:-0}"
EOF_PNPM
  printf '#!/usr/bin/env bash\nexit 1\n' >"$root/bin/rg"
  chmod +x "$root/bin/git" "$root/bin/find" "$root/bin/diff" "$root/bin/pnpm" "$root/bin/rg"
}

run_smoke() {
  local root="$1"
  local diff_status="$2"
  local tmpdir="${SMOKE_TMPDIR:-$root/tmp}"
  local playwright_status="${SMOKE_PLAYWRIGHT_STATUS:-0}"
  set +e
  RUN_OUTPUT="$(cd "$root" && FIXTURE_ROOT="$root" TMPDIR="$tmpdir" PATH="$root/bin" SMOKE_DIFF_STATUS="$diff_status" SMOKE_PLAYWRIGHT_STATUS="$playwright_status" /bin/bash "$root/scripts/experimental-designer-client-smoke.sh" 2>&1)"
  RUN_STATUS=$?
  set -e
}

success_root="$(mktemp -d /tmp/tachiko-smoke-lifecycle-success.XXXXXX)"
failure_root="$(mktemp -d /tmp/tachiko-smoke-lifecycle-failure.XXXXXX)"
trap 'rm -rf -- "$success_root" "$failure_root"' EXIT
make_fixture "$success_root"
run_smoke "$success_root" 0
[[ "$RUN_STATUS" -eq 0 ]] || { echo "lifecycle test: success smoke returned $RUN_STATUS" >&2; exit 1; }
[[ "$RUN_OUTPUT" == *"experimental Designer client smoke passed"* ]] || { echo "lifecycle test: success acknowledgement missing" >&2; exit 1; }
[[ ! -e "$success_root/examples/experimental-designer-client/vendor" ]] || { echo "lifecycle test: success outputs were retained" >&2; exit 1; }
SMOKE_TMPDIR=tmp run_smoke "$success_root" 0
[[ "$RUN_STATUS" -eq 0 ]] || { echo "lifecycle test: relative-TMPDIR success or clean-checkout re-export returned $RUN_STATUS: $RUN_OUTPUT" >&2; exit 1; }
[[ ! -e "$success_root/examples/experimental-designer-client/vendor" ]] || { echo "lifecycle test: relative-TMPDIR outputs were retained" >&2; exit 1; }
SMOKE_TMPDIR=tmp run_smoke "$success_root" 0
[[ "$RUN_STATUS" -eq 0 ]] || { echo "lifecycle test: repeated relative-TMPDIR export returned $RUN_STATUS: $RUN_OUTPUT" >&2; exit 1; }
[[ ! -e "$success_root/examples/experimental-designer-client/vendor" ]] || { echo "lifecycle test: repeated relative-TMPDIR outputs were retained" >&2; exit 1; }

make_fixture "$failure_root"
run_smoke "$failure_root" 23
[[ "$RUN_STATUS" -eq 23 ]] || { echo "lifecycle test: diff failure status changed to $RUN_STATUS" >&2; exit 1; }
primary_path="$(printf '%s\n' "$RUN_OUTPUT" | sed -n 's#^experimental-designer-client-smoke: retained primary export: ##p')"
comparison_path="$(printf '%s\n' "$RUN_OUTPUT" | sed -n 's#^experimental-designer-client-smoke: retained comparison export: ##p')"
diagnostic_path="$(printf '%s\n' "$RUN_OUTPUT" | sed -n 's#^experimental-designer-client-smoke: retained diagnostic directory: ##p')"
[[ -n "$primary_path" && -n "$comparison_path" && -n "$diagnostic_path" ]] || { echo "lifecycle test: retained paths were not reported" >&2; exit 1; }
[[ "$primary_path" == "$diagnostic_path/primary-vendor" && "$comparison_path" == "$diagnostic_path/kit" ]] || { echo "lifecycle test: exports were not retained under one diagnostic directory" >&2; exit 1; }
[[ -f "$primary_path/payload" && -f "$comparison_path/payload" ]] || { echo "lifecycle test: complete compared exports were not retained" >&2; exit 1; }
[[ ! -e "$failure_root/examples/experimental-designer-client/vendor/tachiko" ]] || { echo "lifecycle test: primary export remained vulnerable to next smoke startup" >&2; exit 1; }

run_smoke "$failure_root" 0
[[ "$RUN_STATUS" -eq 0 ]] || { echo "lifecycle test: post-failure success smoke returned $RUN_STATUS" >&2; exit 1; }
[[ -f "$primary_path/payload" && -f "$comparison_path/payload" ]] || { echo "lifecycle test: later startup erased retained evidence" >&2; exit 1; }
[[ ! -e "$failure_root/examples/experimental-designer-client/vendor" ]] || { echo "lifecycle test: post-failure successful outputs were retained" >&2; exit 1; }

SMOKE_PLAYWRIGHT_STATUS=37 run_smoke "$failure_root" 0
[[ "$RUN_STATUS" -eq 37 ]] || { echo "lifecycle test: Playwright failure status changed to $RUN_STATUS" >&2; exit 1; }
playwright_diagnostic="$(printf '%s\n' "$RUN_OUTPUT" | sed -n 's#^experimental-designer-client-smoke: retained diagnostic directory: ##p')"
[[ -f "$playwright_diagnostic/playwright-results/.last-run.json" ]] || { echo "lifecycle test: Playwright failure output was not retained at --output" >&2; exit 1; }
[[ ! -e "$failure_root/packages/browser-client/test-results/.last-run.json" ]] || { echo "lifecycle test: Playwright wrote into source checkout default output" >&2; exit 1; }
run_smoke "$failure_root" 0
[[ "$RUN_STATUS" -eq 0 ]] || { echo "lifecycle test: clean-checkout re-export after Playwright failure returned $RUN_STATUS: $RUN_OUTPUT" >&2; exit 1; }
[[ -f "$playwright_diagnostic/playwright-results/.last-run.json" ]] || { echo "lifecycle test: later startup erased Playwright diagnostics" >&2; exit 1; }
echo "experimental Designer smoke lifecycle passed"
