#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
runner="${repo_root}/scripts/pre-push-check.sh"
test_dir="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-pre-push-test.XXXXXX")"
trap 'rm -rf -- "${test_dir}"' EXIT

bash "${runner}" --help >"${test_dir}/help.out"
grep -F 'Usage: bash scripts/pre-push-check.sh [--fast] [--help]' \
  "${test_dir}/help.out" >/dev/null
grep -F 'Skip the Rust 1.85.0 workspace and wasm32 runtime checks.' \
  "${test_dir}/help.out" >/dev/null
grep -F 'An existing CARGO_TARGET_DIR is honored.' "${test_dir}/help.out" >/dev/null

bash "${runner}" --fast --help >"${test_dir}/fast-help.out"
grep -F 'Checks run in this order:' "${test_dir}/fast-help.out" >/dev/null

mkdir -p "${test_dir}/fake-bin"
cat >"${test_dir}/fake-bin/rustup" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ "$1" == "check" ]]; then
  echo "${PRE_PUSH_TEST_CHECK:-stable-aarch64-apple-darwin - up to date : 1.99.0 (rustc 1.99.0)}"
  exit "${PRE_PUSH_TEST_CHECK_STATUS:-0}"
elif [[ "$1" == "run" && "$3" == "rustc" ]]; then
  echo "rustc ${PRE_PUSH_TEST_LOCAL:-1.99.0} (test build)"
else
  printf 'rustup %s\n' "$*" >>"${PRE_PUSH_TEST_CALLS}"
fi
EOF
cat >"${test_dir}/fake-bin/cargo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf 'cargo %s\n' "$*" >>"${PRE_PUSH_TEST_CALLS}"
EOF
chmod +x "${test_dir}/fake-bin/rustup" "${test_dir}/fake-bin/cargo"
PRE_PUSH_TEST_CALLS="${test_dir}/calls.log" \
  PATH="${test_dir}/fake-bin:${PATH}" \
  bash "${runner}" --fast >"${test_dir}/fast.out"
grep -F 'cargo fmt --all -- --check' "${test_dir}/calls.log" >/dev/null
grep -F 'cargo fmt --manifest-path packages/browser-client/runtime/Cargo.toml --all -- --check' \
  "${test_dir}/calls.log" >/dev/null
grep -F 'clippy --workspace --all-targets --locked -- -D warnings' \
  "${test_dir}/calls.log" >/dev/null
if grep -F '1.85.0' "${test_dir}/calls.log" >/dev/null; then
  echo "pre-push-check test: --fast ran a Rust 1.85.0 check" >&2
  exit 1
fi
grep -F -- '--fast: skipping Rust 1.85.0 and wasm32 checks' \
  "${test_dir}/fast.out" >/dev/null

# rustup's real wording and status: the newest version is the one after the
# arrow, and rustup check exits 100 when an update is available.
PRE_PUSH_TEST_CALLS="${test_dir}/calls-old.log" \
  PRE_PUSH_TEST_CHECK_STATUS=100 \
  PRE_PUSH_TEST_CHECK='stable-aarch64-apple-darwin - update available: 1.97.1 (8bab26f4f 2026-07-14) -> 1.99.1 (58cf8f9c3 2026-10-13)' \
  PRE_PUSH_TEST_LOCAL=1.97.1 \
  PATH="${test_dir}/fake-bin:${PATH}" \
  bash "${runner}" --fast >/dev/null 2>"${test_dir}/old.err"
grep -F 'is Rust 1.97.1; newest stable is 1.99.1.' "${test_dir}/old.err" >/dev/null
grep -F 'rustup toolchain install 1.99.1 --profile minimal -c clippy' \
  "${test_dir}/old.err" >/dev/null

if bash "${runner}" --unknown >"${test_dir}/unknown.out" 2>"${test_dir}/unknown.err"; then
  echo "pre-push-check test: unknown argument unexpectedly succeeded" >&2
  exit 1
fi
grep -F 'unknown argument: --unknown' "${test_dir}/unknown.err" >/dev/null

echo "pre-push-check argument and help tests passed"
