#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
backup_script="${repo_root}/scripts/github-dr-backup.sh"
real_git="$(command -v git)"
real_node="$(command -v node)"
test_dir="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-github-dr.XXXXXX")"

cleanup() {
  rm -rf -- "${test_dir}"
}
trap cleanup EXIT

fail() {
  echo "github-dr-backup-check: $*" >&2
  exit 1
}

hash_file() {
  "${real_node}" - "$1" <<'EOF_HASH'
const crypto = require('crypto');
const fs = require('fs');
const file = process.argv[2];
process.stdout.write(crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex'));
EOF_HASH
}

[[ -f "${backup_script}" ]] || fail "missing ${backup_script}; expected on the implementation branch"

source_work="${test_dir}/source-work"
source_bare="${test_dir}/source.git"
target_bare="${test_dir}/target.git"
mkdir -p "${source_work}"
"${real_git}" -C "${source_work}" init --quiet
"${real_git}" -C "${source_work}" config user.name "DR Fixture"
"${real_git}" -C "${source_work}" config user.email "dr-fixture@example.invalid"
printf 'main-v1\n' >"${source_work}/tracked.txt"
"${real_git}" -C "${source_work}" add tracked.txt
"${real_git}" -C "${source_work}" commit --quiet -m "fixture main"
"${real_git}" -C "${source_work}" branch -M main
main_head="$("${real_git}" -C "${source_work}" rev-parse HEAD)"
"${real_git}" -C "${source_work}" tag v0.1.0
"${real_git}" -C "${source_work}" checkout --quiet -b feature/dr-fixture
printf 'feature\n' >>"${source_work}/tracked.txt"
"${real_git}" -C "${source_work}" commit --quiet -am "fixture feature"
feature_head="$("${real_git}" -C "${source_work}" rev-parse HEAD)"
"${real_git}" -C "${source_work}" checkout --quiet main
"${real_git}" init --quiet --bare "${source_bare}"
"${real_git}" -C "${source_work}" remote add origin "${source_bare}"
"${real_git}" -C "${source_work}" push --quiet origin main feature/dr-fixture --tags
"${real_git}" --git-dir="${source_bare}" symbolic-ref HEAD refs/heads/main

"${real_git}" init --quiet --bare "${target_bare}"
metadata_seed="${test_dir}/metadata-seed"
mkdir -p "${metadata_seed}"
"${real_git}" -C "${metadata_seed}" init --quiet
"${real_git}" -C "${metadata_seed}" config user.name "DR Fixture"
"${real_git}" -C "${metadata_seed}" config user.email "dr-fixture@example.invalid"
printf 'preserve-me\n' >"${metadata_seed}/snapshot.txt"
"${real_git}" -C "${metadata_seed}" add snapshot.txt
"${real_git}" -C "${metadata_seed}" commit --quiet -m "metadata seed"
"${real_git}" -C "${metadata_seed}" branch -M dr-metadata
"${real_git}" -C "${metadata_seed}" remote add target "${target_bare}"
"${real_git}" -C "${metadata_seed}" push --quiet target dr-metadata
"${real_git}" -C "${metadata_seed}" checkout --quiet -b stale-source-ref
printf 'stale\n' >"${metadata_seed}/stale.txt"
"${real_git}" -C "${metadata_seed}" add stale.txt
"${real_git}" -C "${metadata_seed}" commit --quiet -m "stale source ref"
"${real_git}" -C "${metadata_seed}" push --quiet target stale-source-ref

replicate_log="${test_dir}/replicate.log"
SECRET_MARKER="glpat-DO-NOT-LEAK-309"
export GITHUB_DR_GIT="${real_git}"
export GITHUB_DR_GITLAB_TOKEN="${SECRET_MARKER}"
if ! bash "${backup_script}" replicate \
  --source "${source_bare}" \
  --target "${target_bare}" \
  --metadata-branch dr-metadata \
  >"${replicate_log}" 2>&1; then
  cat "${replicate_log}" >&2
  fail "replicate command failed"
fi
if grep -F -- "${SECRET_MARKER}" "${replicate_log}" >/dev/null; then
  fail "replicate output leaked the GitLab credential marker"
fi
[[ "$("${real_git}" --git-dir="${target_bare}" rev-parse refs/heads/main)" == "${main_head}" ]] ||
  fail "target main does not match source main"
[[ "$("${real_git}" --git-dir="${target_bare}" rev-parse refs/heads/feature/dr-fixture)" == "${feature_head}" ]] ||
  fail "target feature branch does not match source"
[[ "$("${real_git}" --git-dir="${target_bare}" rev-parse refs/tags/v0.1.0)" == "${main_head}" ]] ||
  fail "target tag does not match source"
"${real_git}" --git-dir="${target_bare}" show-ref --verify --quiet refs/heads/dr-metadata ||
  fail "replication pruned the independent metadata branch"
if "${real_git}" --git-dir="${target_bare}" show-ref --verify --quiet refs/heads/stale-source-ref; then
  fail "replication left a stale source branch in the target"
fi

fixture_gh="${test_dir}/fake-gh"
gh_log="${test_dir}/gh.log"
cat >"${fixture_gh}" <<'EOF_GH'
#!/usr/bin/env bash
set -euo pipefail

printf '%s\n' "$*" >>"${GITHUB_DR_FIXTURE_LOG}"
[[ "${1:-}" == "api" ]] || exit 64
shift

paginate=0
endpoint=""
for argument in "$@"; do
  case "${argument}" in
    --paginate) paginate=1 ;;
    repos/*|/repos/*|graphql)
      if [[ -z "${endpoint}" ]]; then
        endpoint="${argument}"
      fi
      ;;
  esac
done
endpoint="${endpoint#/}"

if [[ -n "${GITHUB_DR_FIXTURE_FAIL_MATCH:-}" && "${endpoint}" == *"${GITHUB_DR_FIXTURE_FAIL_MATCH}"* ]]; then
  exit 23
fi

repo="repos/nurockplayer/tachiko-work"
case "${endpoint}" in
  "${repo}")
    printf '{"full_name":"nurockplayer/tachiko-work","default_branch":"main"}\n'
    ;;
  "${repo}/branches"*)
    printf '[{"name":"main","commit":{"sha":"%s"}},{"name":"feature/dr-fixture","commit":{"sha":"%s"}}]\n' \
      "${GITHUB_DR_FIXTURE_MAIN_HEAD}" "${GITHUB_DR_FIXTURE_FEATURE_HEAD}"
    ;;
  "${repo}/tags"*)
    printf '[{"name":"v0.1.0","commit":{"sha":"%s"}}]\n' "${GITHUB_DR_FIXTURE_MAIN_HEAD}"
    ;;
  "${repo}/issues"*)
    if [[ "${endpoint}" == *"/comments"* ]]; then
      printf '[{"id":1101,"body":"ISSUE_COMMENT_SENTINEL"}]\n'
    elif [[ "${paginate}" -eq 1 ]]; then
      printf '[{"number":1,"title":"Issue one","state":"open","labels":[{"name":"state:ready"}],"milestone":null,"assignees":[]},{"number":3,"title":"Issue page two sentinel","state":"closed","labels":[],"milestone":{"title":"M1"},"assignees":[]}]\n'
    else
      printf '[{"number":1,"title":"Issue one","state":"open","labels":[{"name":"state:ready"}],"milestone":null,"assignees":[]}]\n'
    fi
    ;;
  "${repo}/pulls"*)
    if [[ "${endpoint}" == *"/reviews"* ]]; then
      printf '[{"id":2201,"state":"APPROVED","body":"PR_REVIEW_SENTINEL"}]\n'
    elif [[ "${endpoint}" == *"/comments"* ]]; then
      printf '[{"id":2202,"body":"PR_REVIEW_COMMENT_SENTINEL"}]\n'
    else
      printf '[{"number":2,"state":"open","head":{"ref":"feature/dr-fixture","sha":"%s"},"base":{"ref":"main","sha":"%s"}}]\n' \
        "${GITHUB_DR_FIXTURE_FEATURE_HEAD}" "${GITHUB_DR_FIXTURE_MAIN_HEAD}"
    fi
    ;;
  "${repo}/labels"*)
    printf '[{"name":"state:ready","color":"ededed"}]\n'
    ;;
  "${repo}/milestones"*)
    printf '[{"number":1,"title":"M1","state":"open"}]\n'
    ;;
  "${repo}/releases"*)
    printf '[{"id":3301,"tag_name":"v0.1.0","name":"Fixture release"}]\n'
    ;;
  "${repo}/rulesets"*)
    printf '[{"id":4401,"name":"main protection","enforcement":"active"}]\n'
    ;;
  *)
    # Permit a backup implementation to query common collection-level endpoints.
    case "${endpoint}" in
      "${repo}/issues/comments"*) printf '[{"id":1101,"body":"ISSUE_COMMENT_SENTINEL"}]\n' ;;
      "${repo}/pulls/comments"*) printf '[{"id":2202,"body":"PR_REVIEW_COMMENT_SENTINEL"}]\n' ;;
      *) printf '[]\n' ;;
    esac
    ;;
esac
EOF_GH
chmod +x "${fixture_gh}"

snapshot_root="${test_dir}/snapshots"
mkdir -p "${snapshot_root}"
export GITHUB_DR_GH="${fixture_gh}"
export GITHUB_DR_FIXTURE_LOG="${gh_log}"
export GITHUB_DR_FIXTURE_MAIN_HEAD="${main_head}"
export GITHUB_DR_FIXTURE_FEATURE_HEAD="${feature_head}"
export GITHUB_TOKEN="ghp-DO-NOT-LEAK-309"

snapshot_log="${test_dir}/snapshot.log"
if ! bash "${backup_script}" snapshot \
  --repository nurockplayer/tachiko-work \
  --output "${snapshot_root}" \
  --source-head "${main_head}" \
  >"${snapshot_log}" 2>&1; then
  cat "${snapshot_log}" >&2
  fail "snapshot command failed"
fi

manifest="${snapshot_root}/latest/manifest.json"
[[ -f "${manifest}" ]] || fail "snapshot did not publish latest/manifest.json"

"${real_node}" - "${snapshot_root}/latest" "${main_head}" <<'EOF_NODE'
const fs = require('fs');
const path = require('path');
const root = process.argv[2];
const expectedHead = process.argv[3];
const manifestPath = path.join(root, 'manifest.json');
const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
if (manifest.source_repository !== 'nurockplayer/tachiko-work') throw new Error('wrong source_repository');
if (manifest.default_branch !== 'main') throw new Error('wrong default_branch');
if (manifest.source_head !== expectedHead) throw new Error('wrong source_head');
if (!manifest.schema_version) throw new Error('missing schema_version');
if (!manifest.snapshot_at || !String(manifest.snapshot_at).endsWith('Z')) throw new Error('snapshot_at must be UTC');
if (!manifest.counts || Number(manifest.counts.issues) < 2) throw new Error('pagination fixture was truncated');
if (!manifest.counts || Number(manifest.counts.pull_requests) < 1) throw new Error('missing PR count');
if (!Array.isArray(manifest.omitted_or_unavailable)) throw new Error('missing omissions disclosure');
if (!manifest.checksums || Object.keys(manifest.checksums).length === 0) throw new Error('missing checksums');

const files = [];
function walk(dir) {
  for (const entry of fs.readdirSync(dir, {withFileTypes: true})) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full);
    else files.push(full);
  }
}
walk(root);
const corpus = files.map((file) => fs.readFileSync(file, 'utf8')).join('\n');
for (const sentinel of ['ISSUE_COMMENT_SENTINEL', 'PR_REVIEW_SENTINEL', 'PR_REVIEW_COMMENT_SENTINEL']) {
  if (!corpus.includes(sentinel)) throw new Error(`missing ${sentinel}`);
}
for (const secret of ['ghp-DO-NOT-LEAK-309', 'glpat-DO-NOT-LEAK-309']) {
  if (corpus.includes(secret)) throw new Error(`secret leaked into snapshot: ${secret}`);
}
EOF_NODE

if ! grep -F -- "--paginate" "${gh_log}" >/dev/null; then
  fail "snapshot never requested paginated GitHub API traversal"
fi
if grep -F -- "${GITHUB_TOKEN}" "${snapshot_log}" >/dev/null ||
   grep -F -- "${GITHUB_DR_GITLAB_TOKEN}" "${snapshot_log}" >/dev/null; then
  fail "snapshot output leaked a credential marker"
fi

if ! bash "${backup_script}" verify --snapshot "${snapshot_root}/latest" \
  >"${test_dir}/verify-ok.log" 2>&1; then
  cat "${test_dir}/verify-ok.log" >&2
  fail "fresh snapshot did not verify"
fi

payload_to_corrupt="$(find "${snapshot_root}/latest" -type f ! -name manifest.json | head -1)"
[[ -n "${payload_to_corrupt}" ]] || fail "no snapshot payload exists to corrupt"
printf '\ncorrupt\n' >>"${payload_to_corrupt}"
corrupt_status=0
bash "${backup_script}" verify --snapshot "${snapshot_root}/latest" \
  >"${test_dir}/verify-corrupt.log" 2>&1 || corrupt_status="$?"
[[ "${corrupt_status}" -ne 0 ]] || fail "checksum verification accepted a corrupted payload"

rm -rf -- "${snapshot_root}/latest"
unset GITHUB_DR_FIXTURE_FAIL_MATCH
bash "${backup_script}" snapshot \
  --repository nurockplayer/tachiko-work \
  --output "${snapshot_root}" \
  --source-head "${main_head}" \
  >"${test_dir}/snapshot-good-again.log" 2>&1
prior_manifest_hash="$(hash_file "${snapshot_root}/latest/manifest.json")"
export GITHUB_DR_FIXTURE_FAIL_MATCH="pulls"
failed_status=0
bash "${backup_script}" snapshot \
  --repository nurockplayer/tachiko-work \
  --output "${snapshot_root}" \
  --source-head "${main_head}" \
  >"${test_dir}/snapshot-failed.log" 2>&1 || failed_status="$?"
[[ "${failed_status}" -ne 0 ]] || fail "partial GitHub API failure was reported as success"
after_failed_hash="$(hash_file "${snapshot_root}/latest/manifest.json")"
[[ "${prior_manifest_hash}" == "${after_failed_hash}" ]] ||
  fail "failed capture replaced the prior known-good snapshot"

printf 'github-dr-backup-check: PASS\n'
