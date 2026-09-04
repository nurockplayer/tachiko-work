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
normal_materialization_template="${test_dir}/normal-materialization-template"
native_target="x86_64-pc-windows-msvc"
normal_tmp="${test_dir}/normal-tmp"
relative_tmp_root="${test_dir}/relative-tmp-root"
relative_course_tmpdir="../relative-tmp-root/nested"
relative_course_cargo_home="../normal-cargo-home"
ancestor_tmp_root="${test_dir}/ancestor-tmp-root"
ancestor_tmpdir="${ancestor_tmp_root}/nested"
normal_cargo_home="${test_dir}/normal-cargo-home"
normal_cargo_registry="${normal_cargo_home}/registry"
normal_cargo_git="${normal_cargo_home}/git"
persistent_target_dir="${normal_repo}/target/obstacle-course"
stale_tachiko_bin="${persistent_target_dir}/${native_target}/release/tachiko.exe"
tracked_raw_file="${normal_repo}/tracked-fixture"
ignored_cargo_input_relative="crates/workspace-engine/build.rs"
ignored_cargo_input="${normal_repo}/${ignored_cargo_input_relative}"
mkdir -p \
  "${normal_repo}/.cargo" \
  "${normal_repo}/scripts" \
  "${normal_bin_dir}" \
  "${normal_materialization_template}" \
  "${normal_cargo_registry}" \
  "${normal_cargo_git}" \
  "${normal_tmp}" \
  "${relative_tmp_root}/nested" \
  "${ancestor_tmp_root}/.cargo" \
  "${ancestor_tmpdir}" \
  "$(dirname "${stale_tachiko_bin}")"
normal_tmp="$(cd "${normal_tmp}" && pwd -P)"
ancestor_tmp_root="$(cd "${ancestor_tmp_root}" && pwd -P)"
ancestor_tmpdir="${ancestor_tmp_root}/nested"
normal_cargo_home="$(cd "${normal_cargo_home}" && pwd -P)"
normal_cargo_registry="${normal_cargo_home}/registry"
normal_cargo_git="${normal_cargo_home}/git"
: >"${normal_log}"
printf 'registry cache fixture\n' >"${normal_cargo_registry}/cache-marker"
printf 'git source cache fixture\n' >"${normal_cargo_git}/cache-marker"
cat >"${normal_cargo_home}/config.toml" <<EOF
[profile.release]
opt-level = 0

[build]
target-dir = "${test_dir}/hostile-cargo-target"
EOF
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
printf '/%s\n' "${ignored_cargo_input_relative}" >"${normal_repo}/.gitignore"

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

cat >"${ancestor_tmp_root}/.cargo/config.toml" <<'EOF'
[profile.release]
opt-level = 0
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

cp -R "${normal_repo}/." "${normal_materialization_template}"
rm -rf -- "${normal_materialization_template}/target"
mkdir -p "$(dirname "${ignored_cargo_input}")"
printf '%s\n' \
  'compile_error!("ignored live-checkout build script must stay isolated");' \
  >"${ignored_cargo_input}"

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

if [[ "${1:-}" != "-C" ]]; then
  echo "fake git: query is not repository-bound: $*" >&2
  exit 2
fi
query_root="${2:-}"
case "${query_root}" in
  "${FAKE_REPO_ROOT}") ;;
  "${FAKE_COURSE_TMP_ROOT}"/tachiko-obstacle.*/source) ;;
  *)
    echo "fake git: query is not bound to the live or isolated source: $*" >&2
    exit 2
    ;;
esac
shift 2

git_config_args=()

while [[ "${1:-}" == "-c" ]]; do
  if [[ "$#" -lt 2 ]]; then
    echo "fake git: unexpected command-scoped configuration: $*" >&2
    exit 2
  fi
  case "${2:-}" in
    core.hooksPath=/dev/null) ;;
    *)
      echo "fake git: unexpected command-scoped configuration: $*" >&2
      exit 2
      ;;
  esac
  git_config_args+=("${2}")
  shift 2
done

if [[ "${1:-}" == "worktree" && "${2:-}" == "add" ]]; then
  if [[ "${query_root}" != "${FAKE_REPO_ROOT}" || "$#" -ne 5 || \
    "${3:-}" != "--detach" || "${5:-}" != "0123456789abcdef0123456789abcdef01234567" || \
    "${#git_config_args[@]}" -ne 1 || \
    "${git_config_args[0]:-}" != "core.hooksPath=/dev/null" ]]; then
    echo "fake git: isolated worktree was not created with hook suppression: $*" >&2
    exit 2
  fi
  mkdir -p "${4}"
  cp -R "${FAKE_MATERIALIZATION_TEMPLATE}/." "${4}"
  exit 0
elif [[ "${1:-}" == "worktree" && "${2:-}" == "remove" ]]; then
  if [[ "${query_root}" != "${FAKE_REPO_ROOT}" || "$#" -ne 4 || \
    "${3:-}" != "--force" ]]; then
    echo "fake git: malformed isolated worktree cleanup: $*" >&2
    exit 2
  fi
  rm -rf -- "${4}"
  exit 0
elif [[ "${1:-}" == "rev-parse" && "${2:-}" == "HEAD" ]]; then
  echo "0123456789abcdef0123456789abcdef01234567"
elif [[ "${1:-}" == "rev-parse" && "${2:-}" == "--git-dir" ]]; then
  if [[ "${query_root}" == "${FAKE_REPO_ROOT}" ]]; then
    echo "${FAKE_REPO_ROOT}/.git"
  else
    echo "${FAKE_REPO_ROOT}/.git/worktrees/isolated-source"
  fi
elif [[ "${1:-}" == "rev-parse" && "${2:-}" == "--git-common-dir" ]]; then
  echo "${FAKE_REPO_ROOT}/.git"
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
elif [[ "${1:-}" == "ls-tree" ]]; then
  if [[ "${2:-}" != "-r" || "${3:-}" != "-z" || \
    "${4:-}" != "--full-tree" || "$#" -ne 5 ]]; then
    echo "fake git: malformed Git tree query: $*" >&2
    exit 2
  fi
  while IFS= read -r -d '' fake_path; do
    relative="${fake_path#${FAKE_MATERIALIZATION_TEMPLATE}/}"
    if [[ -L "${fake_path}" ]]; then
      mode=120000
    elif [[ -f "${fake_path}" ]]; then
      if [[ -x "${fake_path}" ]]; then
        mode=100755
      else
        mode=100644
      fi
    else
      continue
    fi
    printf '%s blob 1111111111111111111111111111111111111111\t%s\0' \
      "${mode}" "${relative}"
  done < <(find "${FAKE_MATERIALIZATION_TEMPLATE}" \
    \( -type f -o -type l \) -print0)
  exit 0
elif [[ "${1:-}" == "hash-object" ]]; then
  if [[ "${2:-}" == "--no-filters" && "${3:-}" == "--" ]]; then
    if [[ ! -f "${!#}" && ! -L "${!#}" ]]; then
      echo "fake git: missing hash-object path: $*" >&2
      exit 2
    fi
  elif [[ "${2:-}" == "--no-filters" && "${3:-}" == "--stdin" ]]; then
    cat >/dev/null
  else
    echo "fake git: malformed hash-object query: $*" >&2
    exit 2
  fi
  echo 1111111111111111111111111111111111111111
  exit 0
elif [[ "${1:-}" == "ls-files" ]]; then
  if [[ " $* " == *" --cached "* ]]; then
    printf '.gitignore\0tracked-fixture\0'
  elif [[ " $* " == *" --stage "* ]]; then
    printf '100644 fake-ignore-object 0\t.gitignore\0'
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
    "${candidate}" == "${FAKE_GIT_IGNORED_PATH}" ]] || \
    [[ "${candidate}" == "${FAKE_IGNORED_CARGO_INPUT_RELATIVE:-}" ]]; then
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
  printf 'raw tracked bytes after\r\n' \
    >"${TACHIKO_OBSTACLE_SOURCE_ROOT}/${FAKE_TRACKED_RAW_RELATIVE}"
fi
if [[ "${FAKE_TRIGGER_IGNORED_DRIFT:-0}" == "1" ]]; then
  printf 'ignored host artifact\n' \
    >"${TACHIKO_OBSTACLE_SOURCE_ROOT}/${FAKE_IGNORED_RAW_RELATIVE}"
fi
if [[ "${FAKE_TRIGGER_EMPTY_DIRECTORY_DRIFT:-0}" == "1" ]]; then
  mkdir -p "${TACHIKO_OBSTACLE_SOURCE_ROOT}/${FAKE_EMPTY_DIRECTORY_RELATIVE}"
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

source_root="${TACHIKO_OBSTACLE_SOURCE_ROOT:?}"
case "${source_root}" in
  "${FAKE_COURSE_TMP_ROOT}"/tachiko-obstacle.*/source) ;;
  *)
    echo "fake cargo: source is not run-scoped and isolated: ${source_root}" >&2
    exit 94
    ;;
esac
course_cargo_home="${CARGO_HOME:?}"
case "${course_cargo_home}" in
  "${FAKE_COURSE_TMP_ROOT}"/tachiko-obstacle.*/cargo-home) ;;
  *)
    echo "fake cargo: Cargo home is not run-scoped: ${course_cargo_home}" >&2
    exit 94
    ;;
esac
if [[ -e "${course_cargo_home}/config.toml" ]]; then
  echo "fake cargo: user Cargo configuration entered course home" >&2
  exit 94
fi
if [[ ! -L "${course_cargo_home}/registry" || \
  "$(readlink "${course_cargo_home}/registry")" != "${FAKE_USER_CARGO_HOME}/registry" || \
  ! -L "${course_cargo_home}/git" || \
  "$(readlink "${course_cargo_home}/git")" != "${FAKE_USER_CARGO_HOME}/git" ]]; then
  echo "fake cargo: offline dependency/source caches were not preserved" >&2
  exit 94
fi
if ! grep -Fx 'registry cache fixture' "${course_cargo_home}/registry/cache-marker" >/dev/null || \
  ! grep -Fx 'git source cache fixture' "${course_cargo_home}/git/cache-marker" >/dev/null; then
  echo "fake cargo: preserved offline cache trees are not readable" >&2
  exit 94
fi
if [[ -e "${source_root}/${FAKE_IGNORED_CARGO_INPUT_RELATIVE}" ]]; then
  echo "fake cargo: ignored live-checkout Cargo input entered isolated source" >&2
  exit 94
fi
if ! git -C "${source_root}" check-ignore -q -- \
  "${FAKE_IGNORED_CARGO_INPUT_RELATIVE}"; then
  echo "fake cargo: hostile Cargo input is not classified as ignored" >&2
  exit 94
fi
printf 'cargo-source root=%s ignored_live_input=absent\n' \
  "${source_root}" >>"${FAKE_TOOLCHAIN_LOG}"
printf 'cargo-home root=%s config=absent registry_cache=preserved git_cache=preserved\n' \
  "${course_cargo_home}" >>"${FAKE_TOOLCHAIN_LOG}"

grep -Fx 'target = "conflicting-config-target"' \
  "${source_root}/.cargo/config.toml" >/dev/null
grep -Fx 'target-dir = "conflicting-config-target-dir"' \
  "${source_root}/.cargo/config.toml" >/dev/null
grep -Fx 'rustc = "conflicting-config-rustc"' \
  "${source_root}/.cargo/config.toml" >/dev/null
grep -Fx 'rustc-wrapper = "conflicting-config-rustc-wrapper"' \
  "${source_root}/.cargo/config.toml" >/dev/null
grep -Fx 'rustc-workspace-wrapper = "conflicting-config-workspace-wrapper"' \
  "${source_root}/.cargo/config.toml" >/dev/null
grep -Fx 'rustflags = ["--cfg", "hostile_build_rustflags"]' \
  "${source_root}/.cargo/config.toml" >/dev/null
grep -Fx 'runner = ["conflicting-config-runner", "--from-config"]' \
  "${source_root}/.cargo/config.toml" >/dev/null
grep -Fx 'rustflags = ["--cfg", "hostile_target_rustflags"]' \
  "${source_root}/.cargo/config.toml" >/dev/null
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
  local course_cargo_home="${9:-${normal_cargo_home}}"
  local course_tmp_root course_cargo_root
  if ! course_tmp_root="$(cd "${course_tmpdir}" && pwd -P)"; then
    echo "obstacle-course test: could not resolve course TMPDIR '${course_tmpdir}'" >&2
    exit 1
  fi
  if ! course_cargo_root="$(cd "${course_cargo_home}" && pwd -P)"; then
    echo "obstacle-course test: could not resolve course CARGO_HOME '${course_cargo_home}'" >&2
    exit 1
  fi
  PATH="${normal_bin_dir}:${PATH}" \
    TMPDIR="${course_tmpdir}" \
    CARGO_HOME="${course_cargo_home}" \
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
    FAKE_TRACKED_RAW_RELATIVE=tracked-fixture \
    FAKE_IGNORED_RAW_RELATIVE=crates/workspace-engine/tests/common/.DS_Store \
    FAKE_EMPTY_DIRECTORY_RELATIVE=crates/workspace-engine/tests/common/empty-host-directory \
    FAKE_GIT_IGNORED_PATH=crates/workspace-engine/tests/common/.DS_Store \
    FAKE_NATIVE_TARGET="${native_target}" \
    FAKE_CARGO_RUSTC="${normal_bin_dir}/cargo-rustc" \
    FAKE_REPO_ROOT="${normal_repo}" \
    FAKE_COURSE_TMP_ROOT="${course_tmp_root}" \
    FAKE_USER_CARGO_HOME="${course_cargo_root}" \
    FAKE_MATERIALIZATION_TEMPLATE="${normal_materialization_template}" \
    FAKE_IGNORED_CARGO_INPUT_RELATIVE="${ignored_cargo_input_relative}" \
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

require_normal_log_contains() {
  local expected="$1"
  if ! grep -F "${expected}" "${normal_log}" >/dev/null; then
    echo "obstacle-course test: missing normal-mode evidence containing: ${expected}" >&2
    sed 's/^/  /' "${normal_log}" >&2
    exit 1
  fi
}

observed_target_dir="$(sed -n \
  's/^cargo command=build target_dir=\([^ ]*\) build_target=.*/\1/p' \
  "${normal_log}")"
observed_source_root="$(sed -n \
  's/^cargo-source root=\([^ ]*\) ignored_live_input=absent$/\1/p' \
  "${normal_log}" | head -n 1)"
observed_cargo_home="$(sed -n \
  's/^cargo-home root=\([^ ]*\) config=absent registry_cache=preserved git_cache=preserved$/\1/p' \
  "${normal_log}" | head -n 1)"
case "${observed_target_dir}" in
  "${normal_tmp}"/tachiko-obstacle.*/cargo-target) ;;
  *)
    echo "obstacle-course test: Cargo target is not run-scoped: ${observed_target_dir}" >&2
    sed 's/^/  /' "${normal_log}" >&2
    exit 1
    ;;
esac
case "${observed_source_root}" in
  "${normal_tmp}"/tachiko-obstacle.*/source) ;;
  *)
    echo "obstacle-course test: Cargo source is not isolated: ${observed_source_root}" >&2
    sed 's/^/  /' "${normal_log}" >&2
    exit 1
    ;;
esac
case "${observed_cargo_home}" in
  "${normal_tmp}"/tachiko-obstacle.*/cargo-home) ;;
  *)
    echo "obstacle-course test: Cargo home is not run-scoped: ${observed_cargo_home}" >&2
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
  "cargo-home root=${observed_cargo_home} config=absent registry_cache=preserved git_cache=preserved"
require_normal_log \
  "stage-bin path=${expected_tachiko_bin} target_dir=${observed_target_dir} build_target=${native_target}"
require_normal_log_contains \
  "git args=-C ${normal_repo} -c core.hooksPath=/dev/null worktree add --detach"
require_normal_log_contains \
  "git args=-C ${normal_repo} -c core.hooksPath=/dev/null worktree remove --force"
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
if [[ -e "${observed_source_root}" ]]; then
  echo "obstacle-course test: isolated source survived cleanup" >&2
  exit 1
fi
grep -F \
  "native_target=${native_target} source=isolated-exact-head cargo_target=run-scoped/${native_target} cargo_home=run-scoped-config-free offline_caches=registry,git native_runner=env-passthrough release_profile_env=neutralized cargo_rustflags=neutralized" \
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
if (
  cd "${normal_repo}"
  run_normal_course 0 0 0 0 0 \
    "${test_dir}/relative-tmpdir.out" \
    "${test_dir}/relative-tmpdir.err" \
    "${relative_course_tmpdir}"
); then
  echo "obstacle-course test: relative TMPDIR fake stage unexpectedly passed" >&2
  exit 1
fi
if grep -Fq "obstacle-course: TMPDIR is not a directory" \
  "${test_dir}/relative-tmpdir.err"; then
  echo "obstacle-course test: valid relative TMPDIR was not preserved across isolated re-exec" >&2
  sed 's/^/  /' "${test_dir}/relative-tmpdir.err" >&2
  exit 1
fi
if ! grep -F "cargo command=build" "${normal_log}" >/dev/null; then
  echo "obstacle-course test: relative TMPDIR did not reach the isolated Cargo stage" >&2
  sed 's/^/  /' "${normal_log}" >&2
  exit 1
fi

: >"${normal_log}"
if (
  cd "${normal_repo}"
  run_normal_course 0 0 0 0 0 \
    "${test_dir}/relative-cargo-home.out" \
    "${test_dir}/relative-cargo-home.err" \
    "${normal_tmp}" \
    "${relative_course_cargo_home}"
); then
  echo "obstacle-course test: relative CARGO_HOME fake stage unexpectedly passed" >&2
  exit 1
fi
if grep -Fq "fake cargo: offline dependency/source caches were not preserved" \
  "${test_dir}/relative-cargo-home.err"; then
  echo "obstacle-course test: valid relative CARGO_HOME was not preserved across isolated re-exec" >&2
  sed 's/^/  /' "${test_dir}/relative-cargo-home.err" >&2
  exit 1
fi
if ! grep -F "cargo-home root=" "${normal_log}" >/dev/null; then
  echo "obstacle-course test: relative CARGO_HOME did not reach the isolated Cargo stage" >&2
  sed 's/^/  /' "${normal_log}" >&2
  exit 1
fi

: >"${normal_log}"
if run_normal_course 0 0 0 0 0 \
  "${test_dir}/ancestor-cargo-config.out" \
  "${test_dir}/ancestor-cargo-config.err" \
  "${ancestor_tmpdir}"; then
  echo "obstacle-course test: ancestor Cargo configuration unexpectedly produced evidence" >&2
  exit 1
fi
grep -F "obstacle-course: ambient Cargo configuration outside isolated source" \
  "${test_dir}/ancestor-cargo-config.err" >/dev/null
grep -F "${ancestor_tmp_root}/.cargo/config.toml" \
  "${test_dir}/ancestor-cargo-config.err" >/dev/null
if grep -Eq "^(cargo command|cargo-source|cargo-home)" "${normal_log}"; then
  echo "obstacle-course test: Cargo produced evidence before rejecting ancestor configuration" >&2
  sed 's/^/  /' "${normal_log}" >&2
  exit 1
fi

: >"${normal_log}"
printf 'raw tracked bytes before\n' >"${tracked_raw_file}"
if run_normal_course 0 0 1 0 0 \
  "${test_dir}/fingerprint-drift.out" \
  "${test_dir}/fingerprint-drift.err"; then
  echo "obstacle-course test: isolated source mutation unexpectedly produced stable evidence" >&2
  exit 1
fi
grep -F "EVIDENCE FAIL: source identity changed checkpoint=after-repository-dogfood" \
  "${test_dir}/fingerprint-drift.err" >/dev/null
grep -F "expected_worktree=clean observed_worktree=clean" \
  "${test_dir}/fingerprint-drift.err" >/dev/null
if grep -Fq "EVIDENCE source_identity=stable" \
  "${test_dir}/fingerprint-drift.out"; then
  echo "obstacle-course test: changed isolated source was reported stable" >&2
  exit 1
fi

: >"${normal_log}"
rm -f -- "${normal_repo}/crates/workspace-engine/tests/common/.DS_Store"
printf 'preexisting ignored host artifact\n' \
  >"${normal_repo}/crates/workspace-engine/tests/common/.DS_Store"
if run_normal_course 0 0 0 0 0 \
  "${test_dir}/ignored-initial.out" \
  "${test_dir}/ignored-initial.err"; then
  echo "obstacle-course test: intentionally failing fake stage unexpectedly passed" >&2
  exit 1
fi
if grep -Fq "ignored entry cannot be a workload input" \
  "${test_dir}/ignored-initial.err"; then
  echo "obstacle-course test: live ignored workload input entered isolated source" >&2
  exit 1
fi
if ! grep -Fq "cargo command=build" "${normal_log}"; then
  echo "obstacle-course test: setup did not run from isolated source" >&2
  exit 1
fi

: >"${normal_log}"
rm -f -- "${normal_repo}/crates/workspace-engine/tests/common/.DS_Store"
ln -s ignored-host-target \
  "${normal_repo}/crates/workspace-engine/tests/common/.DS_Store"
if run_normal_course 0 0 0 0 0 \
  "${test_dir}/ignored-symlink.out" \
  "${test_dir}/ignored-symlink.err"; then
  echo "obstacle-course test: intentionally failing fake stage unexpectedly passed" >&2
  exit 1
fi
if grep -Fq "ignored entry cannot be a workload input" \
  "${test_dir}/ignored-symlink.err"; then
  echo "obstacle-course test: live ignored workload symlink entered isolated source" >&2
  exit 1
fi
if ! grep -Fq "cargo command=build" "${normal_log}"; then
  echo "obstacle-course test: setup did not run from isolated source" >&2
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

filter_repo="${test_dir}/filter-repo"
filter_bin_dir="${test_dir}/filter-bin"
filter_tmp="${test_dir}/filter-tmp"
filter_cargo_home="${test_dir}/filter-cargo-home"
filter_log="${test_dir}/filter.log"
filter_cargo_log="${test_dir}/filter-cargo.log"
filter_command="${test_dir}/roundtrip-filter"
filter_out="${test_dir}/filter.out"
filter_err="${test_dir}/filter.err"
real_git="$(command -v git)"
filter_git=(env GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 "${real_git}")

mkdir -p "${filter_repo}" "${filter_bin_dir}" "${filter_tmp}" \
  "${filter_cargo_home}"
cp -R "${normal_materialization_template}/." "${filter_repo}"
printf 'filtered fixture canonical\n' >"${filter_repo}/filtered-fixture"
printf 'filtered-fixture filter=roundtrip\n' >"${filter_repo}/.gitattributes"

"${filter_git[@]}" -C "${filter_repo}" init -q
"${filter_git[@]}" -C "${filter_repo}" config user.name obstacle-course-test
"${filter_git[@]}" -C "${filter_repo}" config user.email obstacle-course-test@invalid
"${filter_git[@]}" -C "${filter_repo}" add --all
"${filter_git[@]}" -C "${filter_repo}" commit -q -m 'filter regression fixture'

cat >"${filter_command}" <<EOF
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "\${1:-}" >>"${filter_log}"
case "\${1:-}" in
  clean) sed 's/^filtered fixture materialized$/filtered fixture canonical/' ;;
  smudge) sed 's/^filtered fixture canonical$/filtered fixture materialized/' ;;
  *) echo "unexpected filter mode: \${1:-}" >&2; exit 2 ;;
esac
EOF
chmod +x "${filter_command}"
"${filter_git[@]}" -C "${filter_repo}" config filter.roundtrip.clean \
  "${filter_command} clean"
"${filter_git[@]}" -C "${filter_repo}" config filter.roundtrip.smudge \
  "${filter_command} smudge"
"${filter_git[@]}" -C "${filter_repo}" config filter.roundtrip.required true
expected_filter_blob="$("${filter_git[@]}" -C "${filter_repo}" rev-parse HEAD:filtered-fixture)"
observed_filter_blob="$(printf 'filtered fixture materialized\n' | \
  "${filter_git[@]}" -C "${filter_repo}" hash-object --no-filters --stdin)"

if [[ -n "$("${filter_git[@]}" -C "${filter_repo}" status --porcelain)" ]]; then
  echo "obstacle-course test: filter fixture is not clean before execution" >&2
  exit 1
fi

# Git records executable status, not the umask-dependent non-executable bits.
umask 077

cat >"${filter_bin_dir}/cargo" <<EOF
#!/usr/bin/env bash
set -euo pipefail
printf 'cargo invoked\n' >>"${filter_cargo_log}"
exit 99
EOF
chmod +x "${filter_bin_dir}/cargo"
cat >"${filter_bin_dir}/rustc" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
case "${1:-}" in
  --version) echo "rustc 1.85.0 (filter regression)" ;;
  -vV) printf 'rustc 1.85.0 (filter regression)\nhost: x86_64-apple-darwin\n' ;;
  *) echo "unexpected rustc arguments: $*" >&2; exit 2 ;;
esac
EOF
chmod +x "${filter_bin_dir}/rustc"

if PATH="${filter_bin_dir}:${PATH}" \
  TMPDIR="${filter_tmp}" \
  CARGO_HOME="${filter_cargo_home}" \
  RUSTC="${filter_bin_dir}/rustc" \
  CARGO_BUILD_RUSTC='' \
  bash "${filter_repo}/scripts/obstacle-course.sh" \
  >"${filter_out}" 2>"${filter_err}"; then
  echo "obstacle-course test: clean/smudge filter unexpectedly passed exact-head verification" >&2
  exit 1
fi
expected_filter_error="obstacle-course: blob-exact source mismatch checkpoint=before-execution path=filtered-fixture expected_type=regular-file expected_mode=100644 expected_blob=${expected_filter_blob} observed_type=regular-file observed_mode=100644 observed_blob=${observed_filter_blob}"
if ! grep -Fqx "${expected_filter_error}" \
  "${filter_err}" >/dev/null; then
  sed 's/^/  /' "${filter_err}" >&2
  echo "obstacle-course test: filter materialization mismatch was not rejected" >&2
  exit 1
fi
if ! grep -Fx 'smudge' "${filter_log}" >/dev/null; then
  echo "obstacle-course test: repository-local smudge filter did not run" >&2
  exit 1
fi
if [[ -s "${filter_cargo_log}" ]]; then
  echo "obstacle-course test: Cargo ran before blob-exact source verification" >&2
  exit 1
fi
if [[ -n "$("${filter_git[@]}" -C "${filter_repo}" status --porcelain)" ]]; then
  echo "obstacle-course test: reversible filter left the source repository dirty" >&2
  exit 1
fi

echo "obstacle-course test passed: registry + exact-test execution + blob-exact filter rejection + fail-closed run identity"
