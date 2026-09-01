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

echo "obstacle-course test passed: registry + fail-closed options and exact-test selection"
