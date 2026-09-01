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

echo "obstacle-course test passed: public stage registry + fail-closed option parsing"
