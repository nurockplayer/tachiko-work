#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
runner="${repo_root}/scripts/obstacle-course.sh"
test_dir="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-obstacle-test.XXXXXX")"
test_dir="$(cd "${test_dir}" && pwd)"
cleanup() {
  rm -rf -- "${test_dir}"
}
trap cleanup EXIT

cat >"${test_dir}/expected-list.txt" <<'EOF'
tachiko-obstacle/v0
correctness repository-dogfood
correctness git-review-roundtrip
correctness semantic-runtime
correctness retained-workspace
performance retained-workspace samples=3 thresholds=none
EOF

bash "${runner}" --list >"${test_dir}/actual-list.txt"
diff -u "${test_dir}/expected-list.txt" "${test_dir}/actual-list.txt"

if bash "${runner}" --unknown-option \
  >"${test_dir}/unknown.out" 2>"${test_dir}/unknown.err"; then
  echo "obstacle-course test: unknown option unexpectedly succeeded" >&2
  exit 1
fi
grep -F "usage: bash scripts/obstacle-course.sh [--list]" \
  "${test_dir}/unknown.err" >/dev/null

if bash "${runner}" "" --unknown-option \
  >"${test_dir}/empty-arg.out" 2>"${test_dir}/empty-arg.err"; then
  echo "obstacle-course test: empty first argument unexpectedly selected a run" >&2
  exit 1
fi
grep -F "usage: bash scripts/obstacle-course.sh [--list]" \
  "${test_dir}/empty-arg.err" >/dev/null

mkdir -p "${test_dir}/fake-bin"
cat >"${test_dir}/fake-bin/cargo" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
chmod +x "${test_dir}/fake-bin/cargo"

for stage in semantic-runtime retained-workspace; do
  if PATH="${test_dir}/fake-bin:${PATH}" \
    TACHIKO_OBSTACLE_INTERNAL=1 TACHIKO_BIN=/bin/true \
    CARGO_BUILD_TARGET=fake-native-target \
    bash "${runner}" --internal-run-stage "${stage}" \
    >"${test_dir}/${stage}-missing-test.out" \
    2>"${test_dir}/${stage}-missing-test.err"; then
    echo "obstacle-course test: ${stage} accepted a missing exact test" >&2
    exit 1
  fi
  grep -F "${stage}: expected exact workspace test" \
    "${test_dir}/${stage}-missing-test.err" >/dev/null
done

mkdir -p "${test_dir}/polarity-bin"
cat >"${test_dir}/polarity-bin/cargo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

test_target=""
previous=""
list_mode=0
include_ignored=0
for argument in "$@"; do
  if [[ "${previous}" == "--test" ]]; then
    test_target="${argument}"
  fi
  if [[ "${argument}" == "--list" ]]; then
    list_mode=1
  elif [[ "${argument}" == "--include-ignored" ]]; then
    include_ignored=1
  fi
  previous="${argument}"
done

if [[ "${list_mode}" -eq 1 ]]; then
  case "${test_target}" in
    analysis_operations)
      echo "repeated_equal_query_is_exactly_reproducible_with_structured_lineage: test"
      ;;
    patch_lifecycle)
      echo "approved_one_field_patch_previews_applies_verifies_and_records_provenance: test"
      ;;
    resident_session)
      echo "scalar_mutation_invalidates_changed_field_and_downstream_projection_at_new_revision: test"
      echo "field_query_preserves_formula_failure_and_stable_subject_diagnostics: test"
      ;;
    retained_state_benchmark)
      echo "repeated_local_edits_reuse_material_calculation_work: test"
      ;;
    *)
      echo "fake cargo: unexpected test target ${test_target}" >&2
      exit 2
      ;;
  esac
  exit 0
fi

if [[ "${include_ignored}" -ne 1 ]]; then
  echo "fake cargo: exact test invocation can skip after ignore-polarity drift" >&2
  exit 86
fi
EOF
chmod +x "${test_dir}/polarity-bin/cargo"

for stage in semantic-runtime retained-workspace; do
  if ! PATH="${test_dir}/polarity-bin:${PATH}" \
    TACHIKO_OBSTACLE_INTERNAL=1 TACHIKO_BIN=/bin/true \
    CARGO_BUILD_TARGET=fake-native-target \
    bash "${runner}" --internal-run-stage "${stage}" \
    >"${test_dir}/${stage}-polarity.out" \
    2>"${test_dir}/${stage}-polarity.err"; then
    sed 's/^/  /' "${test_dir}/${stage}-polarity.err" >&2
    echo "obstacle-course test: ${stage} is not ignore-polarity safe" >&2
    exit 1
  fi
done

normal_repo="${test_dir}/normal-repo"
normal_bin_dir="${test_dir}/normal-bin"
normal_log="${test_dir}/normal-toolchain.log"
native_target="x86_64-pc-windows-msvc"
normal_tmp="${test_dir}/normal-tmp"
persistent_target_dir="${normal_repo}/target/obstacle-course"
stale_tachiko_bin="${persistent_target_dir}/${native_target}/release/tachiko.exe"
tracked_raw_file="${normal_repo}/tracked-fixture"
mkdir -p \
  "${normal_repo}/.cargo" \
  "${normal_repo}/scripts" \
  "${normal_bin_dir}" \
  "${test_dir}/normal-cargo-home" \
  "${normal_tmp}" \
  "$(dirname "${stale_tachiko_bin}")"
normal_tmp="$(cd "${normal_tmp}" && pwd -P)"
: >"${normal_log}"
cp "${runner}" "${normal_repo}/scripts/obstacle-course.sh"
cp "${repo_root}/scripts/release-lib.sh" \
  "${normal_repo}/scripts/release-lib.sh"

for fixture in \
  dogfood/product-gaps.roproj \
  .gitattributes \
  examples/game-balance/game-balance.ro \
  crates/workspace-engine/tests/common/fixture.txt \
  crates/workspace-engine/tests/analysis_operations.rs \
  crates/workspace-engine/tests/patch_lifecycle.rs \
  crates/workspace-engine/tests/resident_session.rs \
  crates/workspace-engine/tests/retained_state_benchmark.rs; do
  mkdir -p "$(dirname "${normal_repo}/${fixture}")"
  printf 'fake obstacle fixture: %s\n' "${fixture}" \
    >"${normal_repo}/${fixture}"
done
printf 'raw tracked bytes before\n' >"${tracked_raw_file}"

cat >"${normal_repo}/.cargo/config.toml" <<'EOF'
[build]
target = "conflicting-config-target"
target-dir = "conflicting-config-target-dir"
rustc = "conflicting-config-rustc"
rustc-wrapper = "conflicting-config-rustc-wrapper"
rustc-workspace-wrapper = "conflicting-config-workspace-wrapper"
rustflags = ["--cfg", "hostile_build_rustflags"]

[target.x86_64-pc-windows-msvc]
runner = ["conflicting-config-runner", "--from-config"]
rustflags = ["--cfg", "hostile_target_rustflags"]
EOF

cat >"${normal_repo}/scripts/git-ci-smoke.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

printf 'git-smoke config_global=%s config_nosystem=%s config_count=%s config_parameters=%s author_name=%s author_email=%s author_date=%s committer_name=%s committer_email=%s committer_date=%s\n' \
  "${GIT_CONFIG_GLOBAL:-unset}" "${GIT_CONFIG_NOSYSTEM:-unset}" \
  "${GIT_CONFIG_COUNT:-unset}" "${GIT_CONFIG_PARAMETERS:-unset}" \
  "${GIT_AUTHOR_NAME:-unset}" "${GIT_AUTHOR_EMAIL:-unset}" \
  "${GIT_AUTHOR_DATE:-unset}" "${GIT_COMMITTER_NAME:-unset}" \
  "${GIT_COMMITTER_EMAIL:-unset}" "${GIT_COMMITTER_DATE:-unset}" \
  >>"${FAKE_TOOLCHAIN_LOG}"
exit 99
EOF
chmod +x "${normal_repo}/scripts/git-ci-smoke.sh"

cat >"${normal_bin_dir}/git" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

printf 'git args=%s git_dir=%s git_work_tree=%s\n' \
  "$*" "${GIT_DIR:-unset}" "${GIT_WORK_TREE:-unset}" \
  >>"${FAKE_TOOLCHAIN_LOG}"

for variable in \
  GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY \
  GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_COMMON_DIR GIT_NAMESPACE \
  GIT_CONFIG_COUNT GIT_CONFIG_PARAMETERS GIT_CONFIG_SYSTEM GIT_TEMPLATE_DIR \
  GIT_CEILING_DIRECTORIES GIT_EXTERNAL_DIFF GIT_DIFF_OPTS \
  GIT_REPLACE_REF_BASE GIT_AUTHOR_NAME GIT_AUTHOR_EMAIL GIT_AUTHOR_DATE \
  GIT_COMMITTER_NAME GIT_COMMITTER_EMAIL GIT_COMMITTER_DATE; do
  if [[ -n "${!variable:-}" ]]; then
    echo "fake git: inherited ${variable}" >&2
    exit 93
  fi
done
if [[ "${GIT_CONFIG_GLOBAL:-}" != "/dev/null" || \
  "${GIT_CONFIG_NOSYSTEM:-}" != "1" || \
  "${GIT_ATTR_NOSYSTEM:-}" != "1" || \
  "${GIT_NO_REPLACE_OBJECTS:-}" != "1" ]]; then
  echo "fake git: user/system configuration is not isolated" >&2
  exit 93
fi

if [[ "${1:-}" != "-C" || "${2:-}" != "${FAKE_REPO_ROOT}" ]]; then
  echo "fake git: query is not bound to ${FAKE_REPO_ROOT}: $*" >&2
  exit 2
fi
shift 2

if [[ "${1:-}" == "rev-parse" && "${2:-}" == "HEAD" ]]; then
  echo "0123456789abcdef0123456789abcdef01234567"
elif [[ "${1:-}" == "status" ]]; then
  if [[ "${FAKE_GIT_STATUS_FAIL:-0}" == "1" ]]; then
    echo "fake git: intentional status failure" >&2
    exit 95
  fi
  if [[ "${FAKE_GIT_DIRTY:-0}" == "1" ]]; then
    echo " M tracked-fixture"
  fi
  exit 0
elif [[ "${1:-}" == "diff" ]]; then
  echo "normalized tracked diff"
elif [[ "${1:-}" == "ls-files" ]]; then
  if [[ " $* " == *" --cached "* ]]; then
    printf 'tracked-fixture\0'
  elif [[ " $* " == *" --stage "* ]]; then
    printf '100644 fake-index-object 0\ttracked-fixture\0'
  fi
  exit 0
elif [[ "${1:-}" == "check-ignore" ]]; then
  if [[ "$#" -ne 4 || "${2:-}" != "-q" || "${3:-}" != "--" ]]; then
    echo "fake git: malformed check-ignore query: $*" >&2
    exit 2
  fi
  candidate="${!#}"
  if [[ -n "${FAKE_GIT_IGNORED_PATH:-}" && \
    "${candidate}" == "${FAKE_GIT_IGNORED_PATH}" ]]; then
    exit 0
  fi
  exit 1
else
  echo "fake git: unexpected arguments: $*" >&2
  exit 2
fi
EOF
chmod +x "${normal_bin_dir}/git"

cat >"${normal_bin_dir}/cargo-rustc" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

case "${1:-}" in
  --version)
    echo "rustc 1.85.0 (fake obstacle test)"
    ;;
  -vV)
    printf 'rustc 1.85.0 (fake obstacle test)\nhost: %s\n' \
      "${FAKE_NATIVE_TARGET}"
    ;;
  *)
    echo "fake rustc: unexpected arguments: $*" >&2
    exit 2
    ;;
esac
EOF
chmod +x "${normal_bin_dir}/cargo-rustc"

cat >"${normal_bin_dir}/rustc" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

case "${1:-}" in
  --version)
    echo "rustc 0.0.0 (wrong PATH compiler)"
    ;;
  -vV)
    printf 'rustc 0.0.0 (wrong PATH compiler)\nhost: wrong-path-target\n'
    ;;
  *) exit 2 ;;
esac
EOF
chmod +x "${normal_bin_dir}/rustc"

cat >"${test_dir}/fake-tachiko" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

printf 'stage-bin path=%s target_dir=%s build_target=%s\n' \
  "$0" "${CARGO_TARGET_DIR:-unset}" "${CARGO_BUILD_TARGET:-unset}" \
  >>"${FAKE_TOOLCHAIN_LOG}"
if [[ "${FAKE_TRIGGER_FINGERPRINT_DRIFT:-0}" == "1" ]]; then
  printf 'raw tracked bytes after\r\n' >"${FAKE_TRACKED_RAW_FILE}"
fi
if [[ "${FAKE_TRIGGER_IGNORED_DRIFT:-0}" == "1" ]]; then
  printf 'ignored host artifact\n' >"${FAKE_IGNORED_RAW_FILE}"
fi
if [[ "${FAKE_TRIGGER_EMPTY_DIRECTORY_DRIFT:-0}" == "1" ]]; then
  mkdir -p "${FAKE_EMPTY_DIRECTORY}"
fi
exit 97
EOF
chmod +x "${test_dir}/fake-tachiko"

cat >"${stale_tachiko_bin}" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

printf 'stale-stage-bin path=%s target_dir=%s build_target=%s\n' \
  "$0" "${CARGO_TARGET_DIR:-unset}" "${CARGO_BUILD_TARGET:-unset}" \
  >>"${FAKE_TOOLCHAIN_LOG}"
exit 96
EOF
chmod +x "${stale_tachiko_bin}"

cat >"${normal_bin_dir}/cargo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

grep -Fx 'target = "conflicting-config-target"' \
  "${FAKE_REPO_ROOT}/.cargo/config.toml" >/dev/null
grep -Fx 'target-dir = "conflicting-config-target-dir"' \
  "${FAKE_REPO_ROOT}/.cargo/config.toml" >/dev/null
grep -Fx 'rustc = "conflicting-config-rustc"' \
  "${FAKE_REPO_ROOT}/.cargo/config.toml" >/dev/null
grep -Fx 'rustc-wrapper = "conflicting-config-rustc-wrapper"' \
  "${FAKE_REPO_ROOT}/.cargo/config.toml" >/dev/null
grep -Fx 'rustc-workspace-wrapper = "conflicting-config-workspace-wrapper"' \
  "${FAKE_REPO_ROOT}/.cargo/config.toml" >/dev/null
grep -Fx 'rustflags = ["--cfg", "hostile_build_rustflags"]' \
  "${FAKE_REPO_ROOT}/.cargo/config.toml" >/dev/null
grep -Fx 'runner = ["conflicting-config-runner", "--from-config"]' \
  "${FAKE_REPO_ROOT}/.cargo/config.toml" >/dev/null
grep -Fx 'rustflags = ["--cfg", "hostile_target_rustflags"]' \
  "${FAKE_REPO_ROOT}/.cargo/config.toml" >/dev/null
if [[ "${RUSTC:-}" != "${FAKE_CARGO_RUSTC}" || \
  -n "${CARGO_BUILD_RUSTC:-}" ]]; then
  echo "fake cargo: compiler selection is not normalized" >&2
  exit 94
fi
if [[ "${RUSTC_WRAPPER-unset}" != "" || \
  "${RUSTC_WORKSPACE_WRAPPER-unset}" != "" || \
  -n "${CARGO_BUILD_RUSTC_WRAPPER+x}" || \
  -n "${CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER+x}" ]]; then
  echo "fake cargo: compiler wrappers are not neutralized" >&2
  exit 94
fi
if [[ "${CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUNNER:-}" != "env" ]]; then
  echo "fake cargo: native target runner is not normalized" >&2
  exit 94
fi
release_profile_overrides=("${!CARGO_PROFILE_RELEASE_@}")
if [[ "${#release_profile_overrides[@]}" -ne 0 ]]; then
  echo "fake cargo: inherited release profile override ${release_profile_overrides[*]}" >&2
  exit 94
fi
if [[ "${CARGO_ENCODED_RUSTFLAGS+x}" != "x" ]]; then
  echo "fake cargo: encoded Rust flags precedence is not pinned" >&2
  exit 94
elif [[ -n "${CARGO_ENCODED_RUSTFLAGS}" ]]; then
  echo "fake cargo: encoded Rust flags were not neutralized" >&2
  exit 94
fi

command_name="${1:-}"
explicit_target="unset"
explicit_target_count=0
no_run=0
previous=""
for argument in "$@"; do
  if [[ "${previous}" == "--target" ]]; then
    explicit_target="${argument}"
    explicit_target_count=$((explicit_target_count + 1))
  fi
  case "${argument}" in
    --target=*)
      explicit_target="${argument#--target=}"
      explicit_target_count=$((explicit_target_count + 1))
      ;;
    --no-run) no_run=1 ;;
  esac
  previous="${argument}"
done

printf 'cargo command=%s target_dir=%s build_target=%s explicit_target_count=%s explicit_target=%s no_run=%s\n' \
  "${command_name}" "${CARGO_TARGET_DIR:-unset}" \
  "${CARGO_BUILD_TARGET:-unset}" "${explicit_target_count}" \
  "${explicit_target}" "${no_run}" >>"${FAKE_TOOLCHAIN_LOG}"

case "${command_name}" in
  build)
    if [[ "${CARGO_TARGET_DIR:?}" != "${FAKE_PERSISTENT_TARGET_DIR}" ]]; then
      artifact_dir="${CARGO_TARGET_DIR}/${CARGO_BUILD_TARGET:?}/release"
      executable_name=tachiko
      if [[ "${CARGO_BUILD_TARGET}" == *-windows-* ]]; then
        executable_name=tachiko.exe
      fi
      mkdir -p "${artifact_dir}"
      cp "${FAKE_TACHIKO_TEMPLATE}" "${artifact_dir}/${executable_name}"
      chmod +x "${artifact_dir}/${executable_name}"
    fi
    ;;
  test)
    if [[ "${no_run}" -eq 1 ]]; then
      exit 0
    fi
    exit 98
    ;;
  *)
    echo "fake cargo: unexpected command ${command_name}" >&2
    exit 2
    ;;
esac
EOF
chmod +x "${normal_bin_dir}/cargo"

run_normal_course() {
  local status_fail="$1"
  local git_dirty="$2"
  local trigger_fingerprint_drift="$3"
  local trigger_ignored_drift="$4"
  local trigger_empty_directory_drift="$5"
  local stdout_file="$6"
  local stderr_file="$7"
  local course_tmpdir="${8:-${normal_tmp}}"
  PATH="${normal_bin_dir}:${PATH}" \
    TMPDIR="${course_tmpdir}" \
    CARGO_HOME="${test_dir}/normal-cargo-home" \
    CARGO_TARGET_DIR="${test_dir}/conflicting-env-target" \
    CARGO_BUILD_TARGET=conflicting-env-target \
    CARGO_BUILD_RUSTC="${normal_bin_dir}/cargo-rustc" \
    RUSTC_WRAPPER="${test_dir}/hostile-rustc-wrapper" \
    RUSTC_WORKSPACE_WRAPPER="${test_dir}/hostile-workspace-wrapper" \
    CARGO_BUILD_RUSTC_WRAPPER="${test_dir}/hostile-cargo-rustc-wrapper" \
    CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER="${test_dir}/hostile-cargo-workspace-wrapper" \
    CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUNNER="conflicting-env-runner --from-env" \
    CARGO_PROFILE_RELEASE_OPT_LEVEL=0 \
    CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true \
    CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS=true \
    CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_OPT_LEVEL=3 \
    RUSTFLAGS="--cfg hostile_rustflags" \
    CARGO_ENCODED_RUSTFLAGS=$'--cfg\x1fhostile_encoded_rustflags' \
    GIT_DIR="${test_dir}/hostile.git" \
    GIT_WORK_TREE="${test_dir}/hostile-worktree" \
    GIT_INDEX_FILE="${test_dir}/hostile-index" \
    GIT_OBJECT_DIRECTORY="${test_dir}/hostile-objects" \
    GIT_ALTERNATE_OBJECT_DIRECTORIES="${test_dir}/hostile-alternates" \
    GIT_COMMON_DIR="${test_dir}/hostile-common" \
    GIT_NAMESPACE=hostile-namespace \
    GIT_CONFIG_GLOBAL="${test_dir}/hostile-global-config" \
    GIT_CONFIG_SYSTEM="${test_dir}/hostile-system-config" \
    GIT_CONFIG_NOSYSTEM=0 \
    GIT_CONFIG_COUNT=1 \
    GIT_CONFIG_KEY_0=commit.gpgSign \
    GIT_CONFIG_VALUE_0=true \
    GIT_CONFIG_PARAMETERS=hostile-parameters \
    GIT_TEMPLATE_DIR="${test_dir}/hostile-template" \
    GIT_ATTR_NOSYSTEM=0 \
    GIT_CEILING_DIRECTORIES="${test_dir}" \
    GIT_EXTERNAL_DIFF="${test_dir}/hostile-diff" \
    GIT_DIFF_OPTS=--unified=99 \
    GIT_REPLACE_REF_BASE=refs/replace/hostile \
    GIT_NO_REPLACE_OBJECTS=0 \
    GIT_AUTHOR_NAME="Hostile Author" \
    GIT_AUTHOR_EMAIL=hostile-author@invalid \
    GIT_AUTHOR_DATE=not-a-date \
    GIT_COMMITTER_NAME="Hostile Committer" \
    GIT_COMMITTER_EMAIL=hostile-committer@invalid \
    GIT_COMMITTER_DATE=not-a-date \
    FAKE_GIT_STATUS_FAIL="${status_fail}" \
    FAKE_GIT_DIRTY="${git_dirty}" \
    FAKE_TRIGGER_FINGERPRINT_DRIFT="${trigger_fingerprint_drift}" \
    FAKE_TRIGGER_IGNORED_DRIFT="${trigger_ignored_drift}" \
    FAKE_TRIGGER_EMPTY_DIRECTORY_DRIFT="${trigger_empty_directory_drift}" \
    FAKE_TRACKED_RAW_FILE="${tracked_raw_file}" \
    FAKE_IGNORED_RAW_FILE="${normal_repo}/crates/workspace-engine/tests/common/.DS_Store" \
    FAKE_EMPTY_DIRECTORY="${normal_repo}/crates/workspace-engine/tests/common/empty-host-directory" \
    FAKE_GIT_IGNORED_PATH=crates/workspace-engine/tests/common/.DS_Store \
    FAKE_NATIVE_TARGET="${native_target}" \
    FAKE_CARGO_RUSTC="${normal_bin_dir}/cargo-rustc" \
    FAKE_REPO_ROOT="${normal_repo}" \
    FAKE_PERSISTENT_TARGET_DIR="${persistent_target_dir}" \
    FAKE_TACHIKO_TEMPLATE="${test_dir}/fake-tachiko" \
    FAKE_TOOLCHAIN_LOG="${normal_log}" \
    bash "${normal_repo}/scripts/obstacle-course.sh" \
    >"${stdout_file}" 2>"${stderr_file}"
}

if run_normal_course 0 0 0 0 0 \
  "${test_dir}/normal.out" "${test_dir}/normal.err"; then
  echo "obstacle-course test: intentionally failing fake stage unexpectedly passed" >&2
  exit 1
fi

require_normal_log() {
  local expected="$1"
  if ! grep -Fqx "${expected}" "${normal_log}"; then
    echo "obstacle-course test: missing normal-mode evidence: ${expected}" >&2
    sed 's/^/  /' "${normal_log}" >&2
    exit 1
  fi
}

observed_target_dir="$(sed -n \
  's/^cargo command=build target_dir=\([^ ]*\) build_target=.*/\1/p' \
  "${normal_log}")"
case "${observed_target_dir}" in
  "${normal_tmp}"/tachiko-obstacle.*/cargo-target) ;;
  *)
    echo "obstacle-course test: Cargo target is not run-scoped: ${observed_target_dir}" >&2
    sed 's/^/  /' "${normal_log}" >&2
    exit 1
    ;;
esac
expected_tachiko_bin="${observed_target_dir}/${native_target}/release/tachiko.exe"

require_normal_log \
  "cargo command=build target_dir=${observed_target_dir} build_target=${native_target} explicit_target_count=1 explicit_target=${native_target} no_run=0"
require_normal_log \
  "cargo command=test target_dir=${observed_target_dir} build_target=${native_target} explicit_target_count=1 explicit_target=${native_target} no_run=1"
require_normal_log \
  "stage-bin path=${expected_tachiko_bin} target_dir=${observed_target_dir} build_target=${native_target}"
require_normal_log \
  "git args=-C ${normal_repo} rev-parse HEAD git_dir=unset git_work_tree=unset"
require_normal_log \
  "git-smoke config_global=/dev/null config_nosystem=1 config_count=unset config_parameters=unset author_name=unset author_email=unset author_date=unset committer_name=unset committer_email=unset committer_date=unset"
if grep -Fq "stale-stage-bin" "${normal_log}"; then
  echo "obstacle-course test: persistent stale CLI was executed" >&2
  exit 1
fi
if [[ -e "${observed_target_dir}" ]]; then
  echo "obstacle-course test: run-scoped Cargo target survived cleanup" >&2
  exit 1
fi
grep -F \
  "native_target=${native_target} cargo_target=run-scoped/${native_target} native_runner=env-passthrough release_profile_env=neutralized cargo_rustflags=neutralized" \
  "${test_dir}/normal.out" >/dev/null

: >"${normal_log}"
if run_normal_course 1 0 0 0 0 \
  "${test_dir}/status-fail.out" "${test_dir}/status-fail.err"; then
  echo "obstacle-course test: failed Git status unexpectedly produced evidence" >&2
  exit 1
fi
grep -F "obstacle-course: could not determine worktree state" \
  "${test_dir}/status-fail.err" >/dev/null
if grep -Fq "cargo command=" "${normal_log}"; then
  echo "obstacle-course test: setup ran after Git identity failure" >&2
  exit 1
fi

: >"${normal_log}"
mkdir -p "${normal_repo}/hostile-tmp"
if run_normal_course 0 0 0 0 0 \
  "${test_dir}/in-repository-tmp.out" \
  "${test_dir}/in-repository-tmp.err" \
  "${normal_repo}/hostile-tmp"; then
  echo "obstacle-course test: in-repository TMPDIR unexpectedly produced evidence" >&2
  exit 1
fi
grep -F "obstacle-course: TMPDIR must resolve outside repository" \
  "${test_dir}/in-repository-tmp.err" >/dev/null
if grep -Eq "^(cargo command|git args)=" "${normal_log}"; then
  echo "obstacle-course test: setup ran with an in-repository TMPDIR" >&2
  exit 1
fi

: >"${normal_log}"
printf 'raw tracked bytes before\n' >"${tracked_raw_file}"
if run_normal_course 0 1 1 0 0 \
  "${test_dir}/fingerprint-drift.out" \
  "${test_dir}/fingerprint-drift.err"; then
  echo "obstacle-course test: dirty source mutation unexpectedly produced stable evidence" >&2
  exit 1
fi
grep -F "EVIDENCE FAIL: source identity changed checkpoint=after-repository-dogfood" \
  "${test_dir}/fingerprint-drift.err" >/dev/null
grep -F "expected_worktree=dirty observed_worktree=dirty" \
  "${test_dir}/fingerprint-drift.err" >/dev/null
if grep -Fq "EVIDENCE source_identity=stable" \
  "${test_dir}/fingerprint-drift.out"; then
  echo "obstacle-course test: changed dirty source was reported stable" >&2
  exit 1
fi

: >"${normal_log}"
rm -f -- "${normal_repo}/crates/workspace-engine/tests/common/.DS_Store"
printf 'preexisting ignored host artifact\n' \
  >"${normal_repo}/crates/workspace-engine/tests/common/.DS_Store"
if run_normal_course 0 0 0 0 0 \
  "${test_dir}/ignored-initial.out" \
  "${test_dir}/ignored-initial.err"; then
  echo "obstacle-course test: preexisting ignored workload input unexpectedly passed" >&2
  exit 1
fi
grep -F "obstacle-course: ignored entry cannot be a workload input 'crates/workspace-engine/tests/common/.DS_Store'" \
  "${test_dir}/ignored-initial.err" >/dev/null
if grep -Fq "cargo command=" "${normal_log}"; then
  echo "obstacle-course test: setup ran after ignored workload rejection" >&2
  exit 1
fi

: >"${normal_log}"
rm -f -- "${normal_repo}/crates/workspace-engine/tests/common/.DS_Store"
ln -s ignored-host-target \
  "${normal_repo}/crates/workspace-engine/tests/common/.DS_Store"
if run_normal_course 0 0 0 0 0 \
  "${test_dir}/ignored-symlink.out" \
  "${test_dir}/ignored-symlink.err"; then
  echo "obstacle-course test: ignored workload symlink unexpectedly passed" >&2
  exit 1
fi
grep -F "obstacle-course: ignored entry cannot be a workload input 'crates/workspace-engine/tests/common/.DS_Store'" \
  "${test_dir}/ignored-symlink.err" >/dev/null
if grep -Fq "cargo command=" "${normal_log}"; then
  echo "obstacle-course test: setup ran after ignored symlink rejection" >&2
  exit 1
fi

: >"${normal_log}"
rm -f -- "${normal_repo}/crates/workspace-engine/tests/common/.DS_Store"
if run_normal_course 0 0 0 1 0 \
  "${test_dir}/ignored-drift.out" \
  "${test_dir}/ignored-drift.err"; then
  echo "obstacle-course test: ignored workload mutation unexpectedly produced stable evidence" >&2
  exit 1
fi
grep -F "obstacle-course: ignored entry cannot be a workload input 'crates/workspace-engine/tests/common/.DS_Store'" \
  "${test_dir}/ignored-drift.err" >/dev/null
grep -F "EVIDENCE FAIL: could not fingerprint workload identity checkpoint=after-repository-dogfood stage=semantic-runtime" \
  "${test_dir}/ignored-drift.err" >/dev/null
if grep -Fq "EVIDENCE source_identity=stable" \
  "${test_dir}/ignored-drift.out"; then
  echo "obstacle-course test: ignored workload mutation was reported stable" >&2
  exit 1
fi

: >"${normal_log}"
rm -f -- "${normal_repo}/crates/workspace-engine/tests/common/.DS_Store"
rm -rf -- "${normal_repo}/crates/workspace-engine/tests/common/empty-host-directory"
if run_normal_course 0 0 0 0 1 \
  "${test_dir}/empty-directory-drift.out" \
  "${test_dir}/empty-directory-drift.err"; then
  echo "obstacle-course test: empty workload directory mutation unexpectedly produced stable evidence" >&2
  exit 1
fi
if ! grep -F "EVIDENCE FAIL: workload identity changed checkpoint=after-repository-dogfood stage=semantic-runtime" \
  "${test_dir}/empty-directory-drift.err" >/dev/null; then
  sed 's/^/  /' "${test_dir}/empty-directory-drift.err" >&2
  echo "obstacle-course test: empty workload directory drift was not identified" >&2
  exit 1
fi
if grep -Fq "EVIDENCE source_identity=stable" \
  "${test_dir}/empty-directory-drift.out"; then
  echo "obstacle-course test: empty workload directory mutation was reported stable" >&2
  exit 1
fi

echo "obstacle-course test passed: registry + exact-test execution + fail-closed run identity"
