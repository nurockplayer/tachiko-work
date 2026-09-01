#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
runner_path="${repo_root}/scripts/obstacle-course.sh"
course_version="tachiko-obstacle/v0"
correctness_stages=(
  repository-dogfood
  git-review-roundtrip
  semantic-runtime
  retained-workspace
)
correctness_stage_count="${#correctness_stages[@]}"
dogfood_workload_inputs=(
  dogfood/product-gaps.roproj
)
git_review_workload_inputs=(
  .gitattributes
  examples/game-balance/game-balance.ro
  scripts/git-ci-smoke.sh
)
semantic_workload_inputs=(
  crates/workspace-engine/tests/common
  crates/workspace-engine/tests/analysis_operations.rs
  crates/workspace-engine/tests/patch_lifecycle.rs
  crates/workspace-engine/tests/resident_session.rs
)
retained_workload_inputs=(
  crates/workspace-engine/tests/retained_state_benchmark.rs
)
TACHIKO_BIN="${TACHIKO_BIN:-}"

# shellcheck source=scripts/release-lib.sh
source "${repo_root}/scripts/release-lib.sh"

usage() {
  echo "usage: bash scripts/obstacle-course.sh [--list]" >&2
}

list_course() {
  local stage
  echo "${course_version}"
  for stage in "${correctness_stages[@]}"; do
    echo "correctness ${stage}"
  done
  echo "performance retained-workspace samples=3 thresholds=none"
}

run_repository_dogfood() {
  local project
  stage_dir="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-obstacle-dogfood.XXXXXX")"
  trap 'rm -rf -- "${stage_dir}"' EXIT
  project="${repo_root}/dogfood/product-gaps.roproj"

  "${TACHIKO_BIN}" validate "${project}" >"${stage_dir}/validate.txt"
  "${TACHIKO_BIN}" calculate "${project}" >"${stage_dir}/calculate-one.json"
  "${TACHIKO_BIN}" calculate "${project}" >"${stage_dir}/calculate-two.json"
  if ! cmp "${stage_dir}/calculate-one.json" "${stage_dir}/calculate-two.json"; then
    echo "repository-dogfood: repeated calculation bytes differ" >&2
    return 1
  fi
  require_dogfood_value \
    '  "browser_save_as_only.priority": 8.0,' "${stage_dir}/calculate-one.json"
  require_dogfood_value \
    '  "designer_profile_bound.priority": 10.0,' "${stage_dir}/calculate-one.json"
  require_dogfood_value \
    '  "schema_authoring_missing.priority": 9.0' "${stage_dir}/calculate-one.json"

  echo "repository-dogfood passed: canonical Product Gaps project + deterministic formula oracle"
}

require_dogfood_value() {
  local expected="$1"
  local calculation="$2"
  if ! grep -Fx "${expected}" "${calculation}" >/dev/null; then
    echo "repository-dogfood: missing pinned calculation ${expected}" >&2
    return 1
  fi
}

run_git_review_roundtrip() {
  TACHIKO_BIN="${TACHIKO_BIN}" bash "${repo_root}/scripts/git-ci-smoke.sh"
}

require_exact_workspace_test() {
  local stage="$1"
  local test_target="$2"
  local test_name="$3"
  local listing match_count

  if ! listing="$(cargo test --quiet --release --locked --offline \
    -p tachiko-workspace-engine --test "${test_target}" \
    -- --list --format terse)"; then
    echo "${stage}: could not enumerate exact workspace test ${test_target}::${test_name}" >&2
    return 1
  fi
  match_count="$(printf '%s\n' "${listing}" | grep -Fxc "${test_name}: test" || true)"
  if [[ "${match_count}" -ne 1 ]]; then
    echo "${stage}: expected exact workspace test ${test_target}::${test_name} once, found ${match_count}" >&2
    return 1
  fi
}

run_exact_workspace_test() {
  local stage="$1"
  local test_target="$2"
  local test_name="$3"

  require_exact_workspace_test "${stage}" "${test_target}" "${test_name}"

  cargo test --quiet --release --locked --offline \
    -p tachiko-workspace-engine --test "${test_target}" "${test_name}" \
    -- --exact --include-ignored
}

run_semantic_runtime() {
  run_exact_workspace_test \
    semantic-runtime \
    analysis_operations \
    repeated_equal_query_is_exactly_reproducible_with_structured_lineage
  run_exact_workspace_test \
    semantic-runtime \
    patch_lifecycle \
    approved_one_field_patch_previews_applies_verifies_and_records_provenance
  run_exact_workspace_test \
    semantic-runtime \
    resident_session \
    scalar_mutation_invalidates_changed_field_and_downstream_projection_at_new_revision
  run_exact_workspace_test \
    semantic-runtime \
    resident_session \
    field_query_preserves_formula_failure_and_stable_subject_diagnostics

  echo "semantic-runtime passed: query lineage + propose/execute + revision invalidation + diagnostics"
}

run_retained_workspace() {
  require_exact_workspace_test \
    retained-workspace \
    retained_state_benchmark \
    repeated_local_edits_reuse_material_calculation_work

  cargo test --quiet --release --locked --offline \
    -p tachiko-workspace-engine \
    --test retained_state_benchmark \
    repeated_local_edits_reuse_material_calculation_work \
    -- --exact --include-ignored --nocapture

  echo "retained-workspace passed: full-oracle equivalence + deterministic incremental work counters"
}

run_internal_stage() {
  case "$1" in
    repository-dogfood) run_repository_dogfood ;;
    git-review-roundtrip) run_git_review_roundtrip ;;
    semantic-runtime) run_semantic_runtime ;;
    retained-workspace) run_retained_workspace ;;
    *)
      echo "obstacle-course: unknown internal stage '$1'" >&2
      return 2
      ;;
  esac
}

if [[ "${1:-}" == "--internal-run-stage" ]]; then
  if [[ "${TACHIKO_OBSTACLE_INTERNAL:-}" != "1" || \
    -z "${TACHIKO_BIN}" || "$#" -ne 2 ]]; then
    usage
    exit 2
  fi
  cd "${repo_root}"
  run_internal_stage "$2"
  exit
fi

if [[ "$#" -eq 0 ]]; then
  :
elif [[ "$#" -eq 1 && "$1" == "--list" ]]; then
  list_course
  exit
else
  usage
  exit 2
fi

cd "${repo_root}"

# Evidence identity must describe this checkout, not an inherited Git context.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY \
  GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_COMMON_DIR GIT_NAMESPACE \
  GIT_CONFIG_COUNT GIT_CONFIG_PARAMETERS GIT_CONFIG_SYSTEM GIT_TEMPLATE_DIR \
  GIT_CEILING_DIRECTORIES GIT_EXTERNAL_DIFF GIT_DIFF_OPTS GIT_REPLACE_REF_BASE \
  GIT_AUTHOR_NAME GIT_AUTHOR_EMAIL GIT_AUTHOR_DATE \
  GIT_COMMITTER_NAME GIT_COMMITTER_EMAIL GIT_COMMITTER_DATE
export GIT_CONFIG_GLOBAL=/dev/null
export GIT_CONFIG_NOSYSTEM=1
export GIT_ATTR_NOSYSTEM=1
export GIT_NO_REPLACE_OBJECTS=1

for required_command in cargo git; do
  if ! command -v "${required_command}" >/dev/null 2>&1; then
    echo "obstacle-course: ${required_command} is required" >&2
    exit 1
  fi
done
rustc_command="${RUSTC:-${CARGO_BUILD_RUSTC:-rustc}}"
export RUSTC="${rustc_command}"
unset CARGO_BUILD_RUSTC
export RUSTC_WRAPPER=
export RUSTC_WORKSPACE_WRAPPER=
unset CARGO_BUILD_RUSTC_WRAPPER CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER
if ! command -v "${rustc_command}" >/dev/null 2>&1; then
  echo "obstacle-course: selected Rust compiler '${rustc_command}' is required" >&2
  exit 1
fi

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-obstacle.XXXXXX")"
cleanup() {
  rm -rf -- "${run_dir}"
}
trap cleanup EXIT
performance_log="${run_dir}/performance.log"
: >"${performance_log}"

current_worktree_state() {
  local status_output
  if ! status_output="$(git -C "${repo_root}" status \
    --porcelain --untracked-files=normal)"; then
    echo "obstacle-course: could not determine worktree state" >&2
    return 1
  fi
  if [[ -n "${status_output}" ]]; then
    echo "dirty"
  else
    echo "clean"
  fi
}

repository_state_fingerprint() {
  local manifest staged_diff unstaged_diff index_entries
  local tracked_paths untracked_paths link_bytes
  local path absolute kind digest
  if ! manifest="$(mktemp "${run_dir}/source-state.XXXXXX")"; then
    echo "obstacle-course: could not create source-state manifest" >&2
    return 1
  fi
  staged_diff="${manifest}.staged"
  unstaged_diff="${manifest}.unstaged"
  index_entries="${manifest}.index"
  tracked_paths="${manifest}.tracked-paths"
  untracked_paths="${manifest}.untracked"
  link_bytes="${manifest}.link"

  if ! git -C "${repo_root}" diff --cached --no-ext-diff --no-textconv \
    --binary --full-index --no-renames "${head_commit}" -- >"${staged_diff}"; then
    echo "obstacle-course: could not fingerprint staged source state" >&2
    return 1
  fi
  if ! git -C "${repo_root}" diff --no-ext-diff --no-textconv \
    --binary --full-index --no-renames -- >"${unstaged_diff}"; then
    echo "obstacle-course: could not fingerprint unstaged source state" >&2
    return 1
  fi
  if ! LC_ALL=C git -C "${repo_root}" ls-files \
    --stage -z >"${index_entries}"; then
    echo "obstacle-course: could not enumerate exact index state" >&2
    return 1
  fi
  if ! LC_ALL=C git -C "${repo_root}" ls-files \
    --cached -z >"${tracked_paths}"; then
    echo "obstacle-course: could not enumerate tracked source state" >&2
    return 1
  fi
  if ! LC_ALL=C git -C "${repo_root}" ls-files \
    --others --exclude-standard -z >"${untracked_paths}"; then
    echo "obstacle-course: could not enumerate untracked source state" >&2
    return 1
  fi
  if ! digest="$(tachiko_sha256_digest "${staged_diff}")"; then
    echo "obstacle-course: could not hash staged source state" >&2
    return 1
  fi
  if ! printf 'staged-diff\0%s\0' "${digest}" >"${manifest}"; then
    echo "obstacle-course: could not record staged source state" >&2
    return 1
  fi
  if ! digest="$(tachiko_sha256_digest "${unstaged_diff}")"; then
    echo "obstacle-course: could not hash unstaged source state" >&2
    return 1
  fi
  if ! printf 'unstaged-diff\0%s\0' "${digest}" >>"${manifest}"; then
    echo "obstacle-course: could not record unstaged source state" >&2
    return 1
  fi
  if ! digest="$(tachiko_sha256_digest "${index_entries}")"; then
    echo "obstacle-course: could not hash exact index state" >&2
    return 1
  fi
  if ! printf 'index-entries\0%s\0' "${digest}" >>"${manifest}"; then
    echo "obstacle-course: could not record exact index state" >&2
    return 1
  fi

  while IFS= read -r -d '' path; do
    absolute="${repo_root}/${path}"
    if [[ -L "${absolute}" ]]; then
      kind="tracked-symlink"
      if ! readlink "${absolute}" >"${link_bytes}"; then
        echo "obstacle-course: could not read tracked symlink '${path}'" >&2
        return 1
      fi
      if ! digest="$(tachiko_sha256_digest "${link_bytes}")"; then
        echo "obstacle-course: could not hash tracked symlink '${path}'" >&2
        return 1
      fi
    elif [[ -f "${absolute}" ]]; then
      if [[ -x "${absolute}" ]]; then
        kind="tracked-file-755"
      else
        kind="tracked-file-644"
      fi
      if ! digest="$(tachiko_sha256_digest "${absolute}")"; then
        echo "obstacle-course: could not hash tracked file '${path}'" >&2
        return 1
      fi
    elif [[ ! -e "${absolute}" ]]; then
      kind="tracked-missing"
      digest="-"
    else
      echo "obstacle-course: unsupported tracked source path '${path}'" >&2
      return 1
    fi
    if ! printf '%s\0%s\0%s\0' "${kind}" "${path}" "${digest}" \
      >>"${manifest}"; then
      echo "obstacle-course: could not record tracked source path '${path}'" >&2
      return 1
    fi
  done <"${tracked_paths}"

  while IFS= read -r -d '' path; do
    absolute="${repo_root}/${path}"
    if [[ -L "${absolute}" ]]; then
      kind="symlink"
      if ! readlink "${absolute}" >"${link_bytes}"; then
        echo "obstacle-course: could not read untracked symlink '${path}'" >&2
        return 1
      fi
      if ! digest="$(tachiko_sha256_digest "${link_bytes}")"; then
        echo "obstacle-course: could not hash untracked symlink '${path}'" >&2
        return 1
      fi
    elif [[ -f "${absolute}" ]]; then
      if [[ -x "${absolute}" ]]; then
        kind="file-755"
      else
        kind="file-644"
      fi
      if ! digest="$(tachiko_sha256_digest "${absolute}")"; then
        echo "obstacle-course: could not hash untracked file '${path}'" >&2
        return 1
      fi
    else
      echo "obstacle-course: unsupported untracked source path '${path}'" >&2
      return 1
    fi
    if ! printf '%s\0%s\0%s\0' "${kind}" "${path}" "${digest}" \
      >>"${manifest}"; then
      echo "obstacle-course: could not record untracked source path '${path}'" >&2
      return 1
    fi
  done <"${untracked_paths}"

  if ! tachiko_sha256_digest "${manifest}"; then
    echo "obstacle-course: could not hash source-state manifest" >&2
    return 1
  fi
}

if ! head_commit="$(git -C "${repo_root}" rev-parse HEAD)"; then
  echo "obstacle-course: could not determine HEAD" >&2
  exit 1
fi
if ! worktree_state="$(current_worktree_state)"; then
  exit 1
fi
if ! source_state_fingerprint="$(repository_state_fingerprint)"; then
  exit 1
fi
rust_identity="$("${rustc_command}" --version | tr ' ' '_')"
os_identity="$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)"
native_target="$("${rustc_command}" -vV | sed -n 's/^host: //p')"
if [[ -z "${native_target}" ]]; then
  echo "obstacle-course: could not determine the native Rust target" >&2
  exit 1
fi
course_target_dir="${run_dir}/cargo-target"
export CARGO_TARGET_DIR="${course_target_dir}"
export CARGO_BUILD_TARGET="${native_target}"
executable_name="$(tachiko_executable_name "${native_target}")"
tachiko_bin="${course_target_dir}/${native_target}/release/${executable_name}"

verify_source_identity() {
  local checkpoint="$1"
  local observed_head observed_worktree_state observed_source_state_fingerprint
  if ! observed_head="$(git -C "${repo_root}" rev-parse HEAD)"; then
    echo "EVIDENCE FAIL: could not determine HEAD checkpoint=${checkpoint}" >&2
    return 1
  fi
  if ! observed_worktree_state="$(current_worktree_state)"; then
    echo "EVIDENCE FAIL: could not determine worktree state checkpoint=${checkpoint}" >&2
    return 1
  fi
  if ! observed_source_state_fingerprint="$(repository_state_fingerprint)"; then
    echo "EVIDENCE FAIL: could not fingerprint source state checkpoint=${checkpoint}" >&2
    return 1
  fi
  if [[ "${observed_head}" != "${head_commit}" || \
    "${observed_worktree_state}" != "${worktree_state}" || \
    "${observed_source_state_fingerprint}" != "${source_state_fingerprint}" ]]; then
    echo "EVIDENCE FAIL: source identity changed checkpoint=${checkpoint} expected_commit=${head_commit} observed_commit=${observed_head} expected_worktree=${worktree_state} observed_worktree=${observed_worktree_state} expected_state=${source_state_fingerprint} observed_state=${observed_source_state_fingerprint}" >&2
    return 1
  fi
  if ! verify_workload_identity "${checkpoint}"; then
    return 1
  fi
}

record_fixture_file() {
  local file="$1"
  local manifest="$2"
  local relative ignored_status bytes digest
  relative="${file#"${repo_root}/"}"
  if [[ ! -f "${file}" || "${relative}" == *$'\t'* || "${relative}" == *$'\n'* ]]; then
    echo "obstacle-course: invalid fixture manifest path '${relative}'" >&2
    return 1
  fi
  if git -C "${repo_root}" check-ignore -q -- "${relative}"; then
    echo "obstacle-course: ignored file cannot be a workload input '${relative}'" >&2
    return 1
  else
    ignored_status=$?
    if [[ "${ignored_status}" -ne 1 ]]; then
      echo "obstacle-course: could not classify workload input '${relative}'" >&2
      return 1
    fi
  fi
  if ! bytes="$(wc -c <"${file}" | tr -d ' ')"; then
    echo "obstacle-course: could not measure fixture '${relative}'" >&2
    return 1
  fi
  if ! digest="$(tachiko_sha256_digest "${file}")"; then
    echo "obstacle-course: could not hash fixture '${relative}'" >&2
    return 1
  fi
  if ! printf '%s\t%s\t%s\n' "${relative}" "${bytes}" "${digest}" \
    >>"${manifest}"; then
    echo "obstacle-course: could not record fixture '${relative}'" >&2
    return 1
  fi
}

workload_digest() {
  local stage="$1"
  shift
  local manifest file_list path file
  manifest="${run_dir}/${stage}.manifest"
  file_list="${run_dir}/${stage}.files"
  : >"${manifest}"
  : >"${file_list}"
  for path in "$@"; do
    if [[ -d "${repo_root}/${path}" ]]; then
      if ! find "${repo_root}/${path}" -type f -print | LC_ALL=C sort >>"${file_list}"; then
        echo "obstacle-course: could not enumerate fixture directory '${path}'" >&2
        return 1
      fi
    else
      printf '%s\n' "${repo_root}/${path}" >>"${file_list}"
    fi
  done
  if [[ ! -s "${file_list}" ]]; then
    echo "obstacle-course: workload '${stage}' has no fixture inputs" >&2
    return 1
  fi
  while IFS= read -r file; do
    if ! record_fixture_file "${file}" "${manifest}"; then
      return 1
    fi
  done <"${file_list}"
  tachiko_sha256_digest "${manifest}"
}

if ! dogfood_digest="$(workload_digest repository-dogfood \
  "${dogfood_workload_inputs[@]}")"; then
  exit 1
fi
if ! git_review_digest="$(workload_digest git-review-roundtrip \
  "${git_review_workload_inputs[@]}")"; then
  exit 1
fi
if ! semantic_digest="$(workload_digest semantic-runtime \
  "${semantic_workload_inputs[@]}")"; then
  exit 1
fi
if ! retained_digest="$(workload_digest retained-workspace \
  "${retained_workload_inputs[@]}")"; then
  exit 1
fi

verify_one_workload_identity() {
  local checkpoint="$1"
  local stage="$2"
  local expected="$3"
  local observed
  shift 3

  if ! observed="$(workload_digest "${stage}" "$@")"; then
    echo "EVIDENCE FAIL: could not fingerprint workload identity checkpoint=${checkpoint} stage=${stage}" >&2
    return 1
  fi
  if [[ "${observed}" != "${expected}" ]]; then
    echo "EVIDENCE FAIL: workload identity changed checkpoint=${checkpoint} stage=${stage} expected=${expected} observed=${observed}" >&2
    return 1
  fi
}

verify_workload_identity() {
  local checkpoint="$1"

  verify_one_workload_identity "${checkpoint}" repository-dogfood \
    "${dogfood_digest}" "${dogfood_workload_inputs[@]}" &&
    verify_one_workload_identity "${checkpoint}" git-review-roundtrip \
      "${git_review_digest}" "${git_review_workload_inputs[@]}" &&
    verify_one_workload_identity "${checkpoint}" semantic-runtime \
      "${semantic_digest}" "${semantic_workload_inputs[@]}" &&
    verify_one_workload_identity "${checkpoint}" retained-workspace \
      "${retained_digest}" "${retained_workload_inputs[@]}"
}

echo "COURSE ${course_version} commit=${head_commit} worktree=${worktree_state} profile=release network=offline correctness_stages=${correctness_stage_count}"
echo "ENV os=${os_identity} rustc=${rust_identity} native_target=${native_target} cargo_target=run-scoped/${native_target}"
echo "WORKLOAD stage=repository-dogfood id=product-gaps-roproj/v1 sha256=${dogfood_digest}"
echo "WORKLOAD stage=git-review-roundtrip id=game-balance-git-review/v0 sha256=${git_review_digest}"
echo "WORKLOAD stage=semantic-runtime id=focused-semantic-runtime/v0 sha256=${semantic_digest}"
echo "WORKLOAD stage=retained-workspace id=formula-per-entity/v0 entities=10,100,1000 edits=20 sha256=${retained_digest}"

echo "SETUP release artifacts (excluded from performance samples)"
if ! cargo build --quiet --release --locked --offline \
  --target "${native_target}" -p tachiko-cli; then
  echo "SETUP FAIL: release CLI build failed" >&2
  echo "0/${correctness_stage_count} correctness stages passed"
  exit 1
fi
if ! cargo test --quiet --release --locked --offline \
  -p tachiko-workspace-engine \
  --test analysis_operations \
  --test patch_lifecycle \
  --test resident_session \
  --test retained_state_benchmark \
  --target "${native_target}" \
  --no-run; then
  echo "SETUP FAIL: release test artifact build failed" >&2
  echo "0/${correctness_stage_count} correctness stages passed"
  exit 1
fi
if ! verify_source_identity "after-setup"; then
  echo "0/${correctness_stage_count} correctness stages passed"
  exit 1
fi

time_mode="none"
if [[ "$(uname -s)" == "Darwin" && -x /usr/bin/time ]] && \
  /usr/bin/time -l /usr/bin/true >"${run_dir}/time-probe.out" \
    2>"${run_dir}/time-probe.err"; then
  time_mode="darwin"
elif [[ -x /usr/bin/time ]] && \
  /usr/bin/time --version 2>/dev/null | grep -F "GNU Time" >/dev/null; then
  time_mode="gnu"
fi

measure_stage_once() {
  local stage="$1"
  local sample="$2"
  local stdout_file stderr_file metrics_file workload_file status started wall_seconds rss_kib rss_bytes summary
  stdout_file="${run_dir}/${stage}-${sample}.out"
  stderr_file="${run_dir}/${stage}-${sample}.err"
  metrics_file="${run_dir}/${stage}-${sample}.metrics"
  : >"${stdout_file}"
  : >"${stderr_file}"
  : >"${metrics_file}"
  wall_seconds="unavailable"
  rss_kib="unavailable"

  set +e
  case "${time_mode}" in
    darwin)
      # shellcheck disable=SC2016 # Expanded by the measured child bash.
      /usr/bin/time -l bash -c \
        'stage_stderr=$1; shift; exec "$@" 2>"${stage_stderr}"' \
        obstacle-metrics "${stderr_file}" \
        env CARGO_TARGET_DIR="${course_target_dir}" CARGO_BUILD_TARGET="${native_target}" \
          TACHIKO_OBSTACLE_INTERNAL=1 TACHIKO_BIN="${tachiko_bin}" \
          CARGO_TERM_COLOR=never bash "${runner_path}" \
          --internal-run-stage "${stage}" \
        >"${stdout_file}" 2>"${metrics_file}"
      status=$?
      ;;
    gnu)
      # shellcheck disable=SC2016 # Expanded by the measured child bash.
      /usr/bin/time -f $'wall_seconds=%e\npeak_rss_kib=%M' bash -c \
        'stage_stderr=$1; shift; exec "$@" 2>"${stage_stderr}"' \
        obstacle-metrics "${stderr_file}" \
        env CARGO_TARGET_DIR="${course_target_dir}" CARGO_BUILD_TARGET="${native_target}" \
          TACHIKO_OBSTACLE_INTERNAL=1 TACHIKO_BIN="${tachiko_bin}" \
          CARGO_TERM_COLOR=never bash "${runner_path}" \
          --internal-run-stage "${stage}" \
        >"${stdout_file}" 2>"${metrics_file}"
      status=$?
      ;;
    *)
      started=${SECONDS}
      env CARGO_TARGET_DIR="${course_target_dir}" CARGO_BUILD_TARGET="${native_target}" \
        TACHIKO_OBSTACLE_INTERNAL=1 TACHIKO_BIN="${tachiko_bin}" \
        CARGO_TERM_COLOR=never bash "${runner_path}" \
        --internal-run-stage "${stage}" \
        >"${stdout_file}" 2>"${stderr_file}"
      status=$?
      wall_seconds="$((SECONDS - started)).000"
      ;;
  esac
  set -e

  case "${time_mode}" in
    darwin)
      wall_seconds="$(awk '/ real[[:space:]]/ { print $1; exit }' "${metrics_file}")"
      rss_bytes="$(awk '/maximum resident set size/ { print $1; exit }' "${metrics_file}")"
      if [[ -n "${rss_bytes}" ]]; then
        rss_kib="$(awk -v bytes="${rss_bytes}" 'BEGIN { printf "%.0f", bytes / 1024 }')"
      fi
      ;;
    gnu)
      wall_seconds="$(sed -n 's/^wall_seconds=//p' "${metrics_file}" | head -n 1)"
      rss_kib="$(sed -n 's/^peak_rss_kib=//p' "${metrics_file}" | head -n 1)"
      ;;
  esac
  wall_seconds="${wall_seconds:-unavailable}"
  rss_kib="${rss_kib:-unavailable}"

  printf 'PERF stage=%s sample=%s wall_seconds=%s peak_rss_kib=%s rss_scope=stage_process_tree cache_state=process-fresh_os-cache-uncontrolled blocking=false\n' \
    "${stage}" "${sample}" "${wall_seconds}" "${rss_kib}" \
    >>"${performance_log}"
  if [[ "${stage}" == "retained-workspace" ]]; then
    workload_file="${run_dir}/${stage}-${sample}.workload"
    if awk -v sample="${sample}" \
      '/^entities,edits,/ || /^[0-9]+,[0-9]+,/ { print "PERF_WORKLOAD sample=" sample " " $0 }' \
      "${stdout_file}" >"${workload_file}" && [[ -s "${workload_file}" ]]; then
      cat "${workload_file}" >>"${performance_log}"
    else
      printf 'PERF_WORKLOAD sample=%s status=unavailable reason=no_rows\n' \
        "${sample}" >>"${performance_log}"
    fi
  fi

  if [[ "${status}" -ne 0 ]]; then
    echo "SAMPLE ${stage}#${sample} FAIL exit=${status}" >&2
    if [[ -s "${stdout_file}" ]]; then
      sed 's/^/  stdout: /' "${stdout_file}" >&2
    fi
    if [[ -s "${stderr_file}" ]]; then
      sed 's/^/  stderr: /' "${stderr_file}" >&2
    fi
    return "${status}"
  fi

  summary="$(tail -n 1 "${stdout_file}")"
  echo "SAMPLE ${stage}#${sample} PASS ${summary}"
}

stage_invariant() {
  case "$1" in
    repository-dogfood) echo "canonical dogfood admission and deterministic calculation" ;;
    git-review-roundtrip) echo "canonical Git review round trip and strict fail-closed rejection" ;;
    semantic-runtime) echo "query, proposal publication, revision invalidation, and diagnostics" ;;
    retained-workspace) echo "full-oracle equivalence and deterministic bounded incremental work" ;;
  esac
}

run_stage() {
  local stage="$1"
  local samples="$2"
  local sample stage_failed
  stage_failed=0
  for ((sample = 1; sample <= samples; sample++)); do
    if ! measure_stage_once "${stage}" "${sample}"; then
      stage_failed=1
    fi
  done
  if [[ "${stage_failed}" -eq 0 ]]; then
    echo "STAGE ${stage} PASS invariant=$(stage_invariant "${stage}")"
    return 0
  fi
  echo "STAGE ${stage} FAIL invariant=$(stage_invariant "${stage}")" >&2
  return 1
}

passed=0
failed=0
evidence_failed=0
for stage in "${correctness_stages[@]}"; do
  samples=1
  if [[ "${stage}" == "retained-workspace" ]]; then
    samples=3
  fi
  if run_stage "${stage}" "${samples}"; then
    passed=$((passed + 1))
  else
    failed=$((failed + 1))
  fi
  if ! verify_source_identity "after-${stage}"; then
    evidence_failed=1
    break
  fi
done

if [[ "${evidence_failed}" -eq 0 ]]; then
  echo "EVIDENCE source_identity=stable commit=${head_commit} worktree=${worktree_state}"
fi

echo "${passed}/${correctness_stage_count} correctness stages passed"
echo "PERFORMANCE evidence=informational thresholds=none correctness_independent=true"
cat "${performance_log}"

if [[ "${failed}" -ne 0 || "${evidence_failed}" -ne 0 ]]; then
  exit 1
fi
