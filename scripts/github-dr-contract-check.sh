#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
backup_script="${repo_root}/scripts/github-dr-backup.sh"
real_git="$(command -v git)"
real_node="$(command -v node)"
test_dir="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-github-dr-contract.XXXXXX")"

cleanup() {
  rm -rf -- "${test_dir}"
}
trap cleanup EXIT

fail() {
  echo "github-dr-contract-check: $*" >&2
  exit 1
}

tree_digest() {
  "${real_node}" - "$1" <<'EOF_NODE'
const crypto = require('crypto');
const fs = require('fs');
const path = require('path');
const root = process.argv[2];
const entries = [];
function walk(dir, relative = '') {
  for (const entry of fs.readdirSync(dir, {withFileTypes: true})) {
    const rel = relative ? `${relative}/${entry.name}` : entry.name;
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, rel);
    else if (entry.isFile()) entries.push([rel, full]);
  }
}
walk(root);
const hash = crypto.createHash('sha256');
for (const [rel, full] of entries.sort((a, b) => a[0].localeCompare(b[0]))) {
  hash.update(rel);
  hash.update('\0');
  hash.update(fs.readFileSync(full));
  hash.update('\0');
}
process.stdout.write(hash.digest('hex'));
EOF_NODE
}

manifest_head() {
  "${real_node}" - "$1" <<'EOF_NODE'
const fs = require('fs');
const manifest = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
process.stdout.write(String(manifest.source_head || ''));
EOF_NODE
}

manifest_issue_count() {
  "${real_node}" - "$1" <<'EOF_NODE'
const fs = require('fs');
const manifest = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
process.stdout.write(String(manifest.counts?.issues ?? ''));
EOF_NODE
}

[[ -f "${backup_script}" ]] || fail "missing ${backup_script}; expected on the implementation branch"

source_work="${test_dir}/source-work"
source_bare="${test_dir}/source.git"
target_bare="${test_dir}/target.git"
mkdir -p "${source_work}/.github/workflows"
"${real_git}" -C "${source_work}" init --quiet
"${real_git}" -C "${source_work}" config user.name "DR Contract Fixture"
"${real_git}" -C "${source_work}" config user.email "dr-contract-fixture@example.invalid"
printf 'main-v1\n' >"${source_work}/tracked.txt"
printf 'name: WORKFLOW_DEFINITION_SENTINEL\n"on": workflow_dispatch\njobs: {}\n' \
  >"${source_work}/.github/workflows/dr-fixture.yml"
"${real_git}" -C "${source_work}" add tracked.txt .github/workflows/dr-fixture.yml
"${real_git}" -C "${source_work}" commit --quiet -m "fixture main v1"
"${real_git}" -C "${source_work}" branch -M main
main_head_v1="$("${real_git}" -C "${source_work}" rev-parse HEAD)"
"${real_git}" -C "${source_work}" tag v0.1.0
"${real_git}" -C "${source_work}" checkout --quiet -b feature/contract-fixture
printf 'feature\n' >>"${source_work}/tracked.txt"
"${real_git}" -C "${source_work}" commit --quiet -am "fixture feature"
feature_head="$("${real_git}" -C "${source_work}" rev-parse HEAD)"
"${real_git}" -C "${source_work}" checkout --quiet main
"${real_git}" init --quiet --bare "${source_bare}"
"${real_git}" -C "${source_work}" remote add origin "${source_bare}"
"${real_git}" -C "${source_work}" push --quiet origin main feature/contract-fixture --tags
"${real_git}" --git-dir="${source_bare}" symbolic-ref HEAD refs/heads/main

"${real_git}" init --quiet --bare "${target_bare}"
metadata_work="${test_dir}/metadata-work"
mkdir -p "${metadata_work}"
"${real_git}" -C "${metadata_work}" init --quiet
"${real_git}" -C "${metadata_work}" config user.name "DR Contract Fixture"
"${real_git}" -C "${metadata_work}" config user.email "dr-contract-fixture@example.invalid"
printf 'metadata-history\n' >"${metadata_work}/README.txt"
"${real_git}" -C "${metadata_work}" add README.txt
"${real_git}" -C "${metadata_work}" commit --quiet -m "metadata seed"
"${real_git}" -C "${metadata_work}" branch -M dr-metadata
"${real_git}" -C "${metadata_work}" remote add target "${target_bare}"
"${real_git}" -C "${metadata_work}" push --quiet target dr-metadata

export GITHUB_DR_GIT="${real_git}"
export GITHUB_DR_GITLAB_TOKEN="glpat-DO-NOT-LEAK-CONTRACT-309"
if ! bash "${backup_script}" replicate \
  --source "${source_bare}" \
  --target "${target_bare}" \
  --metadata-branch dr-metadata \
  >"${test_dir}/replicate-v1.log" 2>&1; then
  cat "${test_dir}/replicate-v1.log" >&2
  fail "initial replicate command failed"
fi

[[ "$("${real_git}" --git-dir="${target_bare}" symbolic-ref HEAD)" == "refs/heads/main" ]] ||
  fail "backup target did not preserve the source default-branch identity"

restored_default="${test_dir}/restored-default"
"${real_git}" clone --quiet "${target_bare}" "${restored_default}"
[[ "$("${real_git}" -C "${restored_default}" branch --show-current)" == "main" ]] ||
  fail "default restore did not check out main"
grep -F -- 'WORKFLOW_DEFINITION_SENTINEL' \
  "${restored_default}/.github/workflows/dr-fixture.yml" >/dev/null ||
  fail "workflow definition did not survive Git replication and restore"

fixture_gh="${test_dir}/fake-gh"
cat >"${fixture_gh}" <<'EOF_GH'
#!/usr/bin/env bash
set -euo pipefail

[[ "${1:-}" == "api" ]] || exit 64
shift
paginate=0
slurp=0
endpoint=""
for argument in "$@"; do
  case "${argument}" in
    --paginate) paginate=1 ;;
    --slurp) slurp=1 ;;
    repos/*|/repos/*)
      if [[ -z "${endpoint}" ]]; then
        endpoint="${argument#/}"
      fi
      ;;
  esac
done

if [[ -n "${GITHUB_DR_CONTRACT_FAIL_MATCH:-}" && "${endpoint}" == *"${GITHUB_DR_CONTRACT_FAIL_MATCH}"* ]]; then
  exit 23
fi

emit_one_page() {
  local payload="$1"
  if ((paginate == 1 && slurp == 1)); then
    printf '[%s]\n' "${payload}"
  else
    printf '%s\n' "${payload}"
  fi
}

repo="repos/nurockplayer/tachiko-work"
case "${endpoint}" in
  "${repo}")
    printf '{"full_name":"nurockplayer/tachiko-work","default_branch":"main"}\n'
    ;;
  "${repo}/branches"*)
    emit_one_page "[{\"name\":\"main\",\"commit\":{\"sha\":\"${GITHUB_DR_CONTRACT_MAIN_HEAD}\"}},{\"name\":\"feature/contract-fixture\",\"commit\":{\"sha\":\"${GITHUB_DR_CONTRACT_FEATURE_HEAD}\"}}]"
    ;;
  "${repo}/tags"*)
    emit_one_page "[{\"name\":\"v0.1.0\",\"commit\":{\"sha\":\"${GITHUB_DR_CONTRACT_TAG_HEAD}\"}}]"
    ;;
  "${repo}/issues/comments"*)
    emit_one_page '[]'
    ;;
  "${repo}/issues"*)
    if ((paginate != 1)); then
      printf '[{"number":1,"title":"page one"}]\n'
    elif ((slurp == 1)); then
      printf '[[{"number":1,"title":"page one"}],[{"number":2,"title":"page two"}]]\n'
    else
      printf '[{"number":1,"title":"page one"}]\n'
      printf '[{"number":2,"title":"page two"}]\n'
    fi
    ;;
  "${repo}/pulls"*|"${repo}/labels"*|"${repo}/milestones"*|"${repo}/releases"*|"${repo}/rulesets"*)
    emit_one_page '[]'
    ;;
  *)
    emit_one_page '[]'
    ;;
esac
EOF_GH
chmod +x "${fixture_gh}"

snapshot_root="${test_dir}/snapshot-root"
mkdir -p "${snapshot_root}"
export GITHUB_DR_GH="${fixture_gh}"
export GITHUB_DR_CONTRACT_MAIN_HEAD="${main_head_v1}"
export GITHUB_DR_CONTRACT_FEATURE_HEAD="${feature_head}"
export GITHUB_DR_CONTRACT_TAG_HEAD="${main_head_v1}"
export GITHUB_TOKEN="ghp-DO-NOT-LEAK-CONTRACT-309"
unset GITHUB_DR_CONTRACT_FAIL_MATCH

if ! bash "${backup_script}" snapshot \
  --repository nurockplayer/tachiko-work \
  --output "${snapshot_root}" \
  --source-head "${main_head_v1}" \
  >"${test_dir}/snapshot-v1.log" 2>&1; then
  cat "${test_dir}/snapshot-v1.log" >&2
  fail "first snapshot failed"
fi
[[ "$(manifest_issue_count "${snapshot_root}/latest/manifest.json")" == "2" ]] ||
  fail "separate pagination pages were truncated"

first_dated=""
first_count=0
for candidate in "${snapshot_root}/snapshots/"20*T*Z-*; do
  [[ -d "${candidate}" ]] || continue
  first_count=$((first_count + 1))
  [[ -n "${first_dated}" ]] || first_dated="${candidate}"
done
[[ "${first_count}" -eq 1 && -n "${first_dated}" ]] ||
  fail "first successful capture did not publish exactly one dated immutable snapshot"
first_dated_digest="$(tree_digest "${first_dated}")"
[[ "$(manifest_head "${first_dated}/manifest.json")" == "${main_head_v1}" ]] ||
  fail "first dated snapshot recorded the wrong source HEAD"

printf 'main-v2\n' >>"${source_work}/tracked.txt"
"${real_git}" -C "${source_work}" commit --quiet -am "fixture main v2"
main_head_v2="$("${real_git}" -C "${source_work}" rev-parse HEAD)"
"${real_git}" -C "${source_work}" push --quiet origin main
if ! bash "${backup_script}" replicate \
  --source "${source_bare}" \
  --target "${target_bare}" \
  --metadata-branch dr-metadata \
  >"${test_dir}/replicate-v2.log" 2>&1; then
  cat "${test_dir}/replicate-v2.log" >&2
  fail "second replicate command failed"
fi
[[ "$("${real_git}" --git-dir="${target_bare}" symbolic-ref HEAD)" == "refs/heads/main" ]] ||
  fail "second replication lost default-branch identity"
[[ "$("${real_git}" --git-dir="${target_bare}" rev-parse refs/heads/main)" == "${main_head_v2}" ]] ||
  fail "second replication did not advance target main"

export GITHUB_DR_CONTRACT_MAIN_HEAD="${main_head_v2}"
if ! bash "${backup_script}" snapshot \
  --repository nurockplayer/tachiko-work \
  --output "${snapshot_root}" \
  --source-head "${main_head_v2}" \
  >"${test_dir}/snapshot-v2.log" 2>&1; then
  cat "${test_dir}/snapshot-v2.log" >&2
  fail "second snapshot failed"
fi

second_count=0
second_dated=""
for candidate in "${snapshot_root}/snapshots/"20*T*Z-*; do
  [[ -d "${candidate}" ]] || continue
  second_count=$((second_count + 1))
  if [[ "${candidate}" != "${first_dated}" ]]; then
    second_dated="${candidate}"
  fi
done
[[ "${second_count}" -eq 2 && -n "${second_dated}" ]] ||
  fail "second successful capture did not retain both dated snapshots"
[[ "$(tree_digest "${first_dated}")" == "${first_dated_digest}" ]] ||
  fail "second capture mutated the first dated snapshot"
[[ "$(manifest_head "${first_dated}/manifest.json")" == "${main_head_v1}" ]] ||
  fail "first dated snapshot source HEAD changed"
[[ "$(manifest_head "${second_dated}/manifest.json")" == "${main_head_v2}" ]] ||
  fail "second dated snapshot recorded the wrong source HEAD"
[[ "$(manifest_head "${snapshot_root}/latest/manifest.json")" == "${main_head_v2}" ]] ||
  fail "latest did not advance to the second successful source state"

prior_latest_digest="$(tree_digest "${snapshot_root}/latest")"
export GITHUB_DR_CONTRACT_FAIL_MATCH="pulls"
failed_status=0
bash "${backup_script}" snapshot \
  --repository nurockplayer/tachiko-work \
  --output "${snapshot_root}" \
  --source-head "${main_head_v2}" \
  >"${test_dir}/snapshot-failed.log" 2>&1 || failed_status="$?"
[[ "${failed_status}" -ne 0 ]] ||
  fail "partial metadata capture was reported as success"
[[ "$(tree_digest "${snapshot_root}/latest")" == "${prior_latest_digest}" ]] ||
  fail "failed capture changed the complete prior known-good latest snapshot"

printf 'github-dr-contract-check: PASS\n'
