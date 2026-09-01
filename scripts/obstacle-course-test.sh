#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
runner="${repo_root}/scripts/obstacle-course.sh"
test_dir="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-obstacle-test.XXXXXX")"
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
    bash "${runner}" --internal-run-stage "${stage}" \
    >"${test_dir}/${stage}-polarity.out" \
    2>"${test_dir}/${stage}-polarity.err"; then
    sed 's/^/  /' "${test_dir}/${stage}-polarity.err" >&2
    echo "obstacle-course test: ${stage} is not ignore-polarity safe" >&2
    exit 1
  fi
done

echo "obstacle-course test passed: registry + fail-closed exact-test selection and execution"
