#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cargo_script="${repo_root}/scripts/codex-cargo.sh"
test_dir="$(mktemp -d "${repo_root}/.codex-cargo-check.XXXXXX")"

cleanup() {
  rm -rf -- "${test_dir}"
}
trap cleanup EXIT

assert_contains() {
  local file="$1"
  local expected="$2"
  if ! grep -F -- "${expected}" "${file}" >/dev/null; then
    echo "codex-cargo-check: missing '${expected}' in ${file}" >&2
    sed -n '1,160p' "${file}" >&2
    exit 1
  fi
}

mkdir -p "${test_dir}/src"
cat >"${test_dir}/Cargo.toml" <<'EOF'
[package]
name = "codex-cargo-fixture"
version = "0.1.0"
edition = "2024"

[workspace]
EOF
cat >"${test_dir}/src/main.rs" <<'EOF'
fn main() {}
EOF

fallback_sccache="${test_dir}/sccache-server-fallback"
cat >"${fallback_sccache}" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[[ "${SCCACHE_IGNORE_SERVER_IO_ERROR:-}" == "1" ]]
exec "$@"
EOF
chmod +x "${fallback_sccache}"

fallback_output="${test_dir}/fallback.out"
fallback_error="${test_dir}/fallback.err"
env -u CARGO_INCREMENTAL -u SCCACHE_IGNORE_SERVER_IO_ERROR \
  CARGO_TARGET_DIR="${test_dir}/fallback-target" \
  TACHIKO_CODEX_SCCACHE=1 \
  TACHIKO_CODEX_SCCACHE_BIN="${fallback_sccache}" \
  bash "${cargo_script}" check --manifest-path "${test_dir}/Cargo.toml" \
  >"${fallback_output}" 2>"${fallback_error}"
assert_contains "${fallback_error}" "incremental=0"
assert_contains "${fallback_error}" "sccache=enabled"
[[ -x "${test_dir}/fallback-target/debug/codex-cargo-fixture" ||
  -e "${test_dir}/fallback-target/debug/deps" ]]

direct_output="${test_dir}/direct.out"
direct_error="${test_dir}/direct.err"
env -u CARGO_INCREMENTAL \
  CARGO_TARGET_DIR="${test_dir}/direct-target" \
  TACHIKO_CODEX_SCCACHE=1 \
  TACHIKO_CODEX_SCCACHE_BIN="${test_dir}/missing-sccache" \
  bash "${cargo_script}" check --manifest-path "${test_dir}/Cargo.toml" \
  >"${direct_output}" 2>"${direct_error}"
assert_contains "${direct_error}" "sccache=unavailable; direct rustc"

incremental_output="${test_dir}/incremental.out"
incremental_error="${test_dir}/incremental.err"
CARGO_INCREMENTAL=1 CARGO_TARGET_DIR="${test_dir}/incremental-target" \
  TACHIKO_CODEX_SCCACHE=0 \
  bash "${cargo_script}" check --manifest-path "${test_dir}/Cargo.toml" \
  >"${incremental_output}" 2>"${incremental_error}"
assert_contains "${incremental_error}" "incremental=1"
assert_contains "${incremental_error}" "sccache=disabled"

compiler_count="${test_dir}/compiler-count"
compiler_fixture="${test_dir}/failing-compiler"
cat >"${compiler_fixture}" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
count=0
if [[ -f "${CODEX_TEST_COMPILER_COUNT}" ]]; then
  count="$(<"${CODEX_TEST_COMPILER_COUNT}")"
fi
printf '%s\n' "$((count + 1))" >"${CODEX_TEST_COMPILER_COUNT}"
exit 17
EOF
chmod +x "${compiler_fixture}"

passthrough_sccache="${test_dir}/sccache-passthrough"
cat >"${passthrough_sccache}" <<'EOF'
#!/usr/bin/env bash
set -u
exec "$@"
EOF
chmod +x "${passthrough_sccache}"

compiler_error="${test_dir}/compiler-error.err"
compiler_status=0
CODEX_TEST_COMPILER_COUNT="${compiler_count}" \
  TACHIKO_CODEX_SCCACHE_BIN="${passthrough_sccache}" \
  bash "${repo_root}/scripts/codex-rustc-wrapper.sh" "${compiler_fixture}" \
  >"${test_dir}/compiler-error.out" 2>"${compiler_error}" || compiler_status="$?"
[[ "${compiler_status}" -eq 17 ]]
[[ "$(<"${compiler_count}")" -eq 1 ]]
if grep -F -- "falling back" "${compiler_error}" >/dev/null; then
  echo "codex-cargo-check: compiler failure was retried as a cache failure" >&2
  exit 1
fi

foreign_repo="${test_dir}/foreign-repo"
git init --quiet "${foreign_repo}"
inherited_output="${test_dir}/inherited.out"
inherited_error="${test_dir}/inherited.err"
GIT_DIR="${foreign_repo}/.git" GIT_WORK_TREE="${foreign_repo}" \
  GIT_COMMON_DIR="${foreign_repo}/.git" GIT_CEILING_DIRECTORIES="${foreign_repo}" \
  CARGO_TARGET_DIR="${test_dir}/inherited-target" \
  bash "${cargo_script}" check --manifest-path "${test_dir}/Cargo.toml" \
  >"${inherited_output}" 2>"${inherited_error}"
assert_contains "${inherited_error}" "target=${test_dir}/inherited-target"
[[ -e "${test_dir}/inherited-target/debug/deps" ]]

outside_error="${test_dir}/outside.err"
outside_status=0
CARGO_TARGET_DIR="${repo_root}/../shared-codex-target" \
  bash "${cargo_script}" check --manifest-path "${test_dir}/Cargo.toml" \
  >"${test_dir}/outside.out" 2>"${outside_error}" || outside_status="$?"
[[ "${outside_status}" -eq 1 ]]
assert_contains "${outside_error}" "CARGO_TARGET_DIR must resolve below the current worktree"
[[ ! -e "${repo_root}/../shared-codex-target" ]]

symlink_target="${test_dir}/symlink-target"
ln -s "${test_dir}/outside-target" "${symlink_target}"
symlink_error="${test_dir}/symlink.err"
symlink_status=0
CARGO_TARGET_DIR="${symlink_target}" \
  bash "${cargo_script}" check --manifest-path "${test_dir}/Cargo.toml" \
  >"${test_dir}/symlink.out" 2>"${symlink_error}" || symlink_status="$?"
[[ "${symlink_status}" -eq 1 ]]
assert_contains "${symlink_error}" "CARGO_TARGET_DIR must not be a symlink"
[[ ! -e "${test_dir}/outside-target" ]]

cli_target_error="${test_dir}/cli-target.err"
cli_target_status=0
CARGO_TARGET_DIR="${repo_root}/target" \
  bash "${cargo_script}" check --manifest-path "${test_dir}/Cargo.toml" \
  --target-dir "${repo_root}/../shared-codex-target-cli" \
  >"${test_dir}/cli-target.out" 2>"${cli_target_error}" || cli_target_status="$?"
[[ "${cli_target_status}" -eq 1 ]]
assert_contains "${cli_target_error}" "--target-dir is not allowed"
[[ ! -e "${repo_root}/../shared-codex-target-cli" ]]

delimiter_error="${test_dir}/delimiter.err"
env -u CARGO_INCREMENTAL CARGO_TARGET_DIR="${test_dir}/delimiter-target" \
  TACHIKO_CODEX_SCCACHE=0 bash "${cargo_script}" run \
  --manifest-path "${test_dir}/Cargo.toml" -- --target-dir \
  >"${test_dir}/delimiter.out" 2>"${delimiter_error}"
assert_contains "${delimiter_error}" "sccache=disabled"
[[ -x "${test_dir}/delimiter-target/debug/codex-cargo-fixture" ]]

echo "codex Cargo check passed: private target guard, incremental default, server-I/O fallback, and single compiler failure"
