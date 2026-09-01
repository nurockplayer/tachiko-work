#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
runner_path="${repo_root}/scripts/obstacle-course.sh"
course_version="tachiko-obstacle/v0"
correctness_stage_count=4
TACHIKO_BIN="${TACHIKO_BIN:-}"

# shellcheck source=scripts/release-lib.sh
source "${repo_root}/scripts/release-lib.sh"

usage() {
  echo "usage: bash scripts/obstacle-course.sh [--list]" >&2
}

list_course() {
  printf '%s\n' \
    "${course_version}" \
    "correctness repository-dogfood" \
    "correctness git-review-roundtrip" \
    "correctness semantic-runtime" \
    "correctness retained-workspace" \
    "performance retained-workspace samples=3 thresholds=none"
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

for required_command in cargo git rustc; do
  if ! command -v "${required_command}" >/dev/null 2>&1; then
    echo "obstacle-course: ${required_command} is required" >&2
    exit 1
  fi
done

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-obstacle.XXXXXX")"
cleanup() {
  rm -rf -- "${run_dir}"
}
trap cleanup EXIT
performance_log="${run_dir}/performance.log"
: >"${performance_log}"

head_commit="$(git rev-parse HEAD)"
if [[ -n "$(git status --porcelain --untracked-files=normal)" ]]; then
  worktree_state="dirty"
else
  worktree_state="clean"
fi
rust_identity="$(rustc --version | tr ' ' '_')"
os_identity="$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)"
tachiko_bin="${repo_root}/target/release/tachiko"

record_fixture_file() {
  local file="$1"
  local manifest="$2"
  local relative bytes digest
  relative="${file#"${repo_root}/"}"
  if [[ ! -f "${file}" || "${relative}" == *$'\t'* || "${relative}" == *$'\n'* ]]; then
    echo "obstacle-course: invalid fixture manifest path '${relative}'" >&2
    return 1
  fi
  if ! bytes="$(wc -c <"${file}" | tr -d ' ')"; then
    echo "obstacle-course: could not measure fixture '${relative}'" >&2
    return 1
  fi
  if ! digest="$(tachiko_sha256_digest "${file}")"; then
    echo "obstacle-course: could not hash fixture '${relative}'" >&2
    return 1
  fi
  printf '%s\t%s\t%s\n' "${relative}" "${bytes}" "${digest}" >>"${manifest}"
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

dogfood_digest="$(workload_digest repository-dogfood dogfood/product-gaps.roproj)"
git_review_digest="$(workload_digest git-review-roundtrip \
  .gitattributes examples/game-balance/game-balance.ro scripts/git-ci-smoke.sh)"
semantic_digest="$(workload_digest semantic-runtime \
  crates/workspace-engine/tests/analysis_operations.rs \
  crates/workspace-engine/tests/patch_lifecycle.rs \
  crates/workspace-engine/tests/resident_session.rs)"
retained_digest="$(workload_digest retained-workspace \
  crates/workspace-engine/tests/retained_state_benchmark.rs)"

echo "COURSE ${course_version} commit=${head_commit} worktree=${worktree_state} profile=release network=offline correctness_stages=${correctness_stage_count}"
echo "ENV os=${os_identity} rustc=${rust_identity}"
echo "WORKLOAD stage=repository-dogfood id=product-gaps-roproj/v1 sha256=${dogfood_digest}"
echo "WORKLOAD stage=git-review-roundtrip id=game-balance-git-review/v0 sha256=${git_review_digest}"
echo "WORKLOAD stage=semantic-runtime id=focused-semantic-runtime/v0 sha256=${semantic_digest}"
echo "WORKLOAD stage=retained-workspace id=formula-per-entity/v0 entities=10,100,1000 edits=20 sha256=${retained_digest}"

echo "SETUP release artifacts (excluded from performance samples)"
if ! cargo build --quiet --release --locked --offline -p tachiko-cli; then
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
  --no-run; then
  echo "SETUP FAIL: release test artifact build failed" >&2
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
        env TACHIKO_OBSTACLE_INTERNAL=1 TACHIKO_BIN="${tachiko_bin}" \
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
        env TACHIKO_OBSTACLE_INTERNAL=1 TACHIKO_BIN="${tachiko_bin}" \
          CARGO_TERM_COLOR=never bash "${runner_path}" \
          --internal-run-stage "${stage}" \
        >"${stdout_file}" 2>"${metrics_file}"
      status=$?
      ;;
    *)
      started=${SECONDS}
      env TACHIKO_OBSTACLE_INTERNAL=1 TACHIKO_BIN="${tachiko_bin}" \
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
for stage in repository-dogfood git-review-roundtrip semantic-runtime retained-workspace; do
  samples=1
  if [[ "${stage}" == "retained-workspace" ]]; then
    samples=3
  fi
  if run_stage "${stage}" "${samples}"; then
    passed=$((passed + 1))
  else
    failed=$((failed + 1))
  fi
done

echo "${passed}/${correctness_stage_count} correctness stages passed"
echo "PERFORMANCE evidence=informational thresholds=none correctness_independent=true"
cat "${performance_log}"

if [[ "${failed}" -ne 0 ]]; then
  exit 1
fi
