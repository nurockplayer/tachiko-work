#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
backup_script="${repo_root}/scripts/github-dr-backup.sh"
real_git="$(command -v git)"
test_dir="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-github-dr-restore.XXXXXX")"

cleanup() {
  rm -rf -- "${test_dir}"
}
trap cleanup EXIT

fail() {
  echo "github-dr-restore-check: $*" >&2
  exit 1
}

[[ -f "${backup_script}" ]] || fail "missing ${backup_script}; expected on the implementation branch"

source_work="${test_dir}/source-work"
source_bare="${test_dir}/source.git"
target_bare="${test_dir}/target.git"
mkdir -p "${source_work}"
"${real_git}" -C "${source_work}" init --quiet
"${real_git}" -C "${source_work}" config user.name "DR Restore Fixture"
"${real_git}" -C "${source_work}" config user.email "dr-restore-fixture@example.invalid"
printf 'main-v1\n' >"${source_work}/tracked.txt"
"${real_git}" -C "${source_work}" add tracked.txt
"${real_git}" -C "${source_work}" commit --quiet -m "fixture main"
"${real_git}" -C "${source_work}" branch -M main
main_head="$("${real_git}" -C "${source_work}" rev-parse HEAD)"
"${real_git}" -C "${source_work}" tag v0.1.0
"${real_git}" -C "${source_work}" checkout --quiet -b feature/restore-fixture
printf 'feature\n' >>"${source_work}/tracked.txt"
"${real_git}" -C "${source_work}" commit --quiet -am "fixture feature"
feature_head="$("${real_git}" -C "${source_work}" rev-parse HEAD)"
"${real_git}" -C "${source_work}" checkout --quiet main
"${real_git}" init --quiet --bare "${source_bare}"
"${real_git}" -C "${source_work}" remote add origin "${source_bare}"
"${real_git}" -C "${source_work}" push --quiet origin main feature/restore-fixture --tags
"${real_git}" --git-dir="${source_bare}" symbolic-ref HEAD refs/heads/main

"${real_git}" init --quiet --bare "${target_bare}"
metadata_work="${test_dir}/metadata-work"
mkdir -p "${metadata_work}"
"${real_git}" -C "${metadata_work}" init --quiet
"${real_git}" -C "${metadata_work}" config user.name "DR Restore Fixture"
"${real_git}" -C "${metadata_work}" config user.email "dr-restore-fixture@example.invalid"
printf 'metadata-history\n' >"${metadata_work}/README.txt"
"${real_git}" -C "${metadata_work}" add README.txt
"${real_git}" -C "${metadata_work}" commit --quiet -m "metadata seed"
"${real_git}" -C "${metadata_work}" branch -M dr-metadata
"${real_git}" -C "${metadata_work}" remote add target "${target_bare}"
"${real_git}" -C "${metadata_work}" push --quiet target dr-metadata

export GITHUB_DR_GIT="${real_git}"
export GITHUB_DR_GITLAB_TOKEN="glpat-DO-NOT-LEAK-RESTORE-309"
replicate_log="${test_dir}/replicate.log"
if ! bash "${backup_script}" replicate \
  --source "${source_bare}" \
  --target "${target_bare}" \
  --metadata-branch dr-metadata \
  >"${replicate_log}" 2>&1; then
  cat "${replicate_log}" >&2
  fail "replicate command failed"
fi
if grep -F -- "${GITHUB_DR_GITLAB_TOKEN}" "${replicate_log}" >/dev/null; then
  fail "replicate output leaked the GitLab credential marker"
fi

fixture_gh="${test_dir}/fake-gh"
cat >"${fixture_gh}" <<'EOF_GH'
#!/usr/bin/env bash
set -euo pipefail

[[ "${1:-}" == "api" ]] || exit 64
shift
endpoint=""
for argument in "$@"; do
  case "${argument}" in
    repos/*|/repos/*|graphql)
      if [[ -z "${endpoint}" ]]; then
        endpoint="${argument}"
      fi
      ;;
  esac
done
endpoint="${endpoint#/}"
repo="repos/nurockplayer/tachiko-work"
case "${endpoint}" in
  "${repo}")
    printf '{"full_name":"nurockplayer/tachiko-work","default_branch":"main"}\n'
    ;;
  "${repo}/branches"*)
    printf '[{"name":"main","commit":{"sha":"%s"}},{"name":"feature/restore-fixture","commit":{"sha":"%s"}}]\n' \
      "${GITHUB_DR_RESTORE_MAIN_HEAD}" "${GITHUB_DR_RESTORE_FEATURE_HEAD}"
    ;;
  "${repo}/tags"*)
    printf '[{"name":"v0.1.0","commit":{"sha":"%s"}}]\n' "${GITHUB_DR_RESTORE_MAIN_HEAD}"
    ;;
  "${repo}/issues"*|"${repo}/pulls"*|"${repo}/labels"*|"${repo}/milestones"*|"${repo}/releases"*|"${repo}/rulesets"*|graphql)
    printf '[]\n'
    ;;
  *)
    printf '[]\n'
    ;;
esac
EOF_GH
chmod +x "${fixture_gh}"

snapshot_root="${test_dir}/snapshots"
mkdir -p "${snapshot_root}"
export GITHUB_DR_GH="${fixture_gh}"
export GITHUB_DR_RESTORE_MAIN_HEAD="${main_head}"
export GITHUB_DR_RESTORE_FEATURE_HEAD="${feature_head}"
export GITHUB_TOKEN="ghp-DO-NOT-LEAK-RESTORE-309"
snapshot_log="${test_dir}/snapshot.log"
if ! bash "${backup_script}" snapshot \
  --repository nurockplayer/tachiko-work \
  --output "${snapshot_root}" \
  --source-head "${main_head}" \
  >"${snapshot_log}" 2>&1; then
  cat "${snapshot_log}" >&2
  fail "snapshot command failed"
fi
[[ -f "${snapshot_root}/latest/manifest.json" ]] || fail "snapshot did not publish latest/manifest.json"
if grep -F -- "${GITHUB_TOKEN}" "${snapshot_log}" >/dev/null ||
   grep -F -- "${GITHUB_DR_GITLAB_TOKEN}" "${snapshot_log}" >/dev/null; then
  fail "snapshot output leaked a credential marker"
fi

rm -rf -- "${metadata_work}/snapshots"
mkdir -p "${metadata_work}/snapshots"
cp -R "${snapshot_root}/latest" "${metadata_work}/snapshots/latest"
"${real_git}" -C "${metadata_work}" add snapshots
"${real_git}" -C "${metadata_work}" commit --quiet -m "store known-good metadata snapshot"
metadata_head="$("${real_git}" -C "${metadata_work}" rev-parse HEAD)"
"${real_git}" -C "${metadata_work}" push --quiet target dr-metadata

# Simulate losing the GitHub-side source and the local working snapshot. From
# here on, recovery is allowed to use only the backup target and the production
# verification command.
rm -rf -- "${source_work}" "${source_bare}" "${snapshot_root}" "${metadata_work}"

restored_source="${test_dir}/restored-source"
"${real_git}" clone --quiet --branch main "${target_bare}" "${restored_source}"
[[ "$("${real_git}" -C "${restored_source}" rev-parse HEAD)" == "${main_head}" ]] ||
  fail "restored main does not match the original source HEAD"
[[ "$("${real_git}" -C "${restored_source}" rev-parse refs/remotes/origin/feature/restore-fixture)" == "${feature_head}" ]] ||
  fail "restored feature branch does not match the original source"
[[ "$("${real_git}" -C "${restored_source}" rev-parse refs/tags/v0.1.0)" == "${main_head}" ]] ||
  fail "restored tag does not match the original source"

restored_metadata="${test_dir}/restored-metadata"
"${real_git}" clone --quiet --single-branch --branch dr-metadata "${target_bare}" "${restored_metadata}"
[[ "$("${real_git}" -C "${restored_metadata}" rev-parse HEAD)" == "${metadata_head}" ]] ||
  fail "restored metadata history does not match the backed-up metadata branch"
restored_snapshot="${restored_metadata}/snapshots/latest"
[[ -f "${restored_snapshot}/manifest.json" ]] || fail "restored metadata branch does not expose the snapshot manifest"
if ! bash "${backup_script}" verify --snapshot "${restored_snapshot}" \
  >"${test_dir}/verify-restored.log" 2>&1; then
  cat "${test_dir}/verify-restored.log" >&2
  fail "restored metadata snapshot did not verify"
fi
if grep -R -F -- "ghp-DO-NOT-LEAK-RESTORE-309" "${restored_metadata}" >/dev/null 2>&1 ||
   grep -R -F -- "glpat-DO-NOT-LEAK-RESTORE-309" "${restored_metadata}" >/dev/null 2>&1; then
  fail "restored metadata contains a credential marker"
fi

printf 'github-dr-restore-check: PASS\n'
