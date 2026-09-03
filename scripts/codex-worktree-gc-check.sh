#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
gc_script="${repo_root}/scripts/codex-worktree-gc.sh"
real_git="$(command -v git)"
test_dir="$(cd "$(mktemp -d "${TMPDIR:-/tmp}/tachiko-codex-gc.XXXXXX")" && pwd -P)"

cleanup() {
  rm -rf -- "${test_dir}"
}
trap cleanup EXIT

assert_contains() {
  local file="$1"
  local expected="$2"
  if ! grep -F -- "${expected}" "${file}" >/dev/null; then
    echo "codex-worktree-gc-check: missing '${expected}' in ${file}" >&2
    sed -n '1,220p' "${file}" >&2
    exit 1
  fi
}

assert_not_contains() {
  local file="$1"
  local unexpected="$2"
  if grep -F -- "${unexpected}" "${file}" >/dev/null; then
    echo "codex-worktree-gc-check: unexpected '${unexpected}' in ${file}" >&2
    sed -n '1,220p' "${file}" >&2
    exit 1
  fi
}

run_gc() {
  local repository="$1"
  shift
  CODEX_WORKTREE_GC_GIT="${fake_git}" \
    CODEX_WORKTREE_GC_GH="${fake_gh}" \
    bash "${gc_script}" --repository "${repository}" \
    --codex-root "${codex_root}" "$@"
}

run_gc_from() {
  local working_directory="$1"
  local repository="$2"
  shift 2
  (
    cd "${working_directory}"
    CODEX_WORKTREE_GC_GIT="${fake_git}" \
      CODEX_WORKTREE_GC_GH="${fake_gh}" \
      bash "${gc_script}" --repository "${repository}" \
      --codex-root "${codex_root}" "$@"
  )
}

codex_root="${test_dir}/codex-worktrees"
primary="${codex_root}/primary"
mkdir -p "${primary}"
"${real_git}" -C "${primary}" init --quiet
"${real_git}" -C "${primary}" config user.name "Codex GC Fixture"
"${real_git}" -C "${primary}" config user.email "codex-gc-fixture@example.invalid"
printf 'fixture baseline\n' >"${primary}/tracked.txt"
"${real_git}" -C "${primary}" add tracked.txt
"${real_git}" -C "${primary}" commit --quiet -m "fixture baseline"
"${real_git}" -C "${primary}" branch -M main
"${real_git}" -C "${primary}" remote add origin \
  git@github.com:nurockplayer/tachiko-work.git

add_worktree() {
  local id="$1"
  local branch="$2"
  local path="${codex_root}/${id}/tachiko-work"
  mkdir -p "$(dirname "${path}")"
  "${real_git}" -C "${primary}" worktree add --quiet -b "${branch}" "${path}" HEAD
  printf '%s\n' "${path}"
}

open_path="$(add_worktree open codex/open)"
merged_path="$(add_worktree merged codex/merged)"
tracked_dirty_path="$(add_worktree dirty-tracked codex/dirty-tracked)"
untracked_dirty_path="$(add_worktree dirty-untracked codex/dirty-untracked)"
ambiguous_path="$(add_worktree ambiguous codex/ambiguous)"
head_mismatch_path="$(add_worktree head-mismatch codex/head-mismatch)"
branch_mismatch_path="$(add_worktree branch-mismatch codex/branch-mismatch)"
repo_mismatch_path="$(add_worktree repo-mismatch codex/repo-mismatch)"
github_unavailable_path="$(add_worktree gh-unavailable codex/gh-unavailable)"
closed_path="$(add_worktree closed codex/closed)"
current_path="$(add_worktree current codex/current)"
detached_path="${codex_root}/detached/tachiko-work"
mkdir -p "$(dirname "${detached_path}")"
"${real_git}" -C "${primary}" worktree add --quiet --detach "${detached_path}" HEAD
outside_path="${test_dir}/developer-worktree"
"${real_git}" -C "${primary}" worktree add --quiet -b codex/outside \
  "${outside_path}" HEAD
outside_stale_path="${test_dir}/developer-stale-worktree"
"${real_git}" -C "${primary}" worktree add --quiet -b codex/outside-stale \
  "${outside_stale_path}" HEAD
rm -r -- "${outside_stale_path}"

printf 'tracked change\n' >>"${tracked_dirty_path}/tracked.txt"
printf 'untracked change\n' >"${untracked_dirty_path}/untracked.txt"
fixture_head="$("${real_git}" -C "${primary}" rev-parse HEAD)"
export GC_TEST_HEAD="${fixture_head}"
export GC_TEST_REPO="nurockplayer/tachiko-work"

fake_gh="${test_dir}/fake-gh"
cat >"${fake_gh}" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "repo" && "${2:-}" == "view" ]]; then
  printf 'main\n'
  exit 0
fi

if [[ "${1:-}" != "pr" || "${2:-}" != "list" ]]; then
  exit 64
fi

branch=""
args=("$@")
for ((index = 0; index < ${#args[@]}; index += 1)); do
  if [[ "${args[index]}" == "--head" && $((index + 1)) -lt ${#args[@]} ]]; then
    branch="${args[index + 1]}"
  fi
done

row() {
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$1" "$2" "$3" "$4" "$5" "$6" "$7"
}

case "${branch}" in
  codex/open)
    row 101 OPEN - codex/open "${GC_TEST_HEAD}" main "${GC_TEST_REPO}"
    ;;
  codex/merged)
    row 102 MERGED 2026-09-04T00:00:00Z codex/merged "${GC_TEST_HEAD}" main "${GC_TEST_REPO}"
    ;;
  codex/ambiguous)
    row 103 OPEN - codex/ambiguous "${GC_TEST_HEAD}" main "${GC_TEST_REPO}"
    row 104 CLOSED - codex/ambiguous "${GC_TEST_HEAD}" main "${GC_TEST_REPO}"
    ;;
  codex/head-mismatch)
    row 105 MERGED 2026-09-04T00:00:00Z codex/head-mismatch \
      0000000000000000000000000000000000000000 main "${GC_TEST_REPO}"
    ;;
  codex/branch-mismatch)
    row 106 MERGED 2026-09-04T00:00:00Z wrong-branch "${GC_TEST_HEAD}" main "${GC_TEST_REPO}"
    ;;
  codex/repo-mismatch)
    row 107 MERGED 2026-09-04T00:00:00Z codex/repo-mismatch "${GC_TEST_HEAD}" main other/repo
    ;;
  codex/gh-unavailable)
    exit 23
    ;;
  codex/closed)
    row 108 CLOSED - codex/closed "${GC_TEST_HEAD}" main "${GC_TEST_REPO}"
    ;;
  codex/current)
    row 109 MERGED 2026-09-04T00:00:00Z codex/current "${GC_TEST_HEAD}" main "${GC_TEST_REPO}"
    ;;
  *)
    exit 0
    ;;
esac
EOF
chmod +x "${fake_gh}"

prune_log="${test_dir}/prune.log"
fake_git="${test_dir}/fake-git"
cat >"${fake_git}" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

seen_worktree=0
for argument in "$@"; do
  if [[ "${argument}" == "worktree" ]]; then
    seen_worktree=1
  elif [[ "${seen_worktree}" == "1" && "${argument}" == "prune" ]]; then
    printf 'prune\n' >>"${GC_TEST_PRUNE_LOG}"
  fi
done
exec "${GC_TEST_REAL_GIT}" "$@"
EOF
chmod +x "${fake_git}"
export GC_TEST_REAL_GIT="${real_git}"
export GC_TEST_PRUNE_LOG="${prune_log}"

before_list="$("${real_git}" -C "${primary}" worktree list --porcelain)"
dry_output="${test_dir}/dry-run.txt"
dry_error="${test_dir}/dry-run.err"
dry_status=0
run_gc "${primary}" >"${dry_output}" 2>"${dry_error}" || dry_status="$?"
[[ "${dry_status}" -eq 2 ]]
assert_contains "${dry_output}" "KEEP"
assert_contains "${dry_output}" "open/tachiko-work"
assert_contains "${dry_output}" "DELETE"
assert_contains "${dry_output}" "merged/tachiko-work"
assert_contains "${dry_output}" "DIRTY"
assert_contains "${dry_output}" "dirty-tracked/tachiko-work"
assert_contains "${dry_output}" "dirty-untracked/tachiko-work"
assert_contains "${dry_output}" "UNKNOWN"
assert_contains "${dry_output}" "ambiguous/tachiko-work"
assert_contains "${dry_output}" "head-mismatch/tachiko-work"
assert_contains "${dry_output}" "branch-mismatch/tachiko-work"
assert_contains "${dry_output}" "repo-mismatch/tachiko-work"
assert_contains "${dry_output}" "gh-unavailable/tachiko-work"
assert_contains "${dry_output}" "closed unmerged PR #108"
assert_contains "${dry_output}" "detached/tachiko-work"
assert_contains "${dry_output}" "worktree is detached"
assert_contains "${dry_output}" "SUMMARY active=13"
assert_contains "${dry_output}" "PRUNE skipped (dry-run; no mutation)"
assert_not_contains "${dry_output}" "developer-worktree"
[[ ! -e "${prune_log}" ]]
[[ -d "${merged_path}" ]]
after_dry_list="$("${real_git}" -C "${primary}" worktree list --porcelain)"
[[ "${before_list}" == "${after_dry_list}" ]]

current_output="${test_dir}/current.txt"
current_status=0
run_gc_from "${current_path}" "${primary}" >"${current_output}" \
  2>"${test_dir}/current.err" ||
  current_status="$?"
[[ "${current_status}" -eq 2 ]]
assert_contains "${current_output}" "PROTECTED"
assert_contains "${current_output}" "primary"
assert_contains "${current_output}" "current/tachiko-work"

unavailable_gh="${test_dir}/unavailable-gh"
cat >"${unavailable_gh}" <<'EOF'
#!/usr/bin/env bash
exit 23
EOF
chmod +x "${unavailable_gh}"
unavailable_output="${test_dir}/unavailable.txt"
unavailable_status=0
CODEX_WORKTREE_GC_GIT="${fake_git}" CODEX_WORKTREE_GC_GH="${unavailable_gh}" \
  bash "${gc_script}" --repository "${primary}" --codex-root "${codex_root}" \
  >"${unavailable_output}" 2>"${test_dir}/unavailable.err" ||
  unavailable_status="$?"
[[ "${unavailable_status}" -eq 2 ]]
assert_contains "${unavailable_output}" "UNKNOWN"
assert_contains "${unavailable_output}" "GitHub repository state is unavailable"
[[ ! -e "${prune_log}" ]]

foreign_origin_output="${test_dir}/foreign-origin.txt"
"${real_git}" -C "${primary}" remote set-url origin \
  git@gitlab.example.invalid:nurockplayer/tachiko-work.git
foreign_origin_status=0
run_gc "${primary}" >"${foreign_origin_output}" 2>"${test_dir}/foreign-origin.err" ||
  foreign_origin_status="$?"
[[ "${foreign_origin_status}" -eq 2 ]]
assert_contains "${foreign_origin_output}" "repository origin identity is unresolved"
"${real_git}" -C "${primary}" remote set-url origin \
  git@github.com:nurockplayer/tachiko-work.git

blocked_apply_output="${test_dir}/blocked-apply.txt"
blocked_apply_status=0
run_gc_from "${current_path}" "${primary}" --apply >"${blocked_apply_output}" \
  2>"${test_dir}/blocked-apply.err" || blocked_apply_status="$?"
[[ "${blocked_apply_status}" -eq 2 ]]
assert_contains "${blocked_apply_output}" "out-of-scope prunable worktree"
[[ -d "${merged_path}" ]]
[[ ! -e "${prune_log}" ]]
"${real_git}" -C "${primary}" worktree prune --expire=now >/dev/null

in_scope_stale_path="${codex_root}/stale-in-scope/tachiko-work"
mkdir -p "$(dirname "${in_scope_stale_path}")"
"${real_git}" -C "${primary}" worktree add --quiet -b codex/in-scope-stale \
  "${in_scope_stale_path}" HEAD
rm -r -- "${in_scope_stale_path}"

in_scope_blocked_output="${test_dir}/in-scope-blocked-apply.txt"
in_scope_blocked_status=0
run_gc_from "${current_path}" "${primary}" --apply >"${in_scope_blocked_output}" \
  2>"${test_dir}/in-scope-blocked-apply.err" || in_scope_blocked_status="$?"
[[ "${in_scope_blocked_status}" -eq 2 ]]
assert_contains "${in_scope_blocked_output}" "in-scope prunable worktree"
[[ -d "${merged_path}" ]]
[[ ! -e "${prune_log}" ]]
"${real_git}" -C "${primary}" worktree prune --expire=now >/dev/null
[[ ! -e "${in_scope_stale_path}" ]]

apply_output="${test_dir}/apply.txt"
apply_status=0
run_gc_from "${current_path}" "${primary}" --apply >"${apply_output}" \
  2>"${test_dir}/apply.err" ||
  apply_status="$?"
[[ "${apply_status}" -eq 2 ]]
assert_contains "${apply_output}" "DELETE"
assert_contains "${apply_output}" "merged/tachiko-work"
assert_contains "${apply_output}" "PRUNE applied"
[[ ! -d "${merged_path}" ]]
[[ -s "${prune_log}" ]]
[[ "$(wc -l <"${prune_log}" | tr -d '[:space:]')" -eq 1 ]]
post_apply_list="$("${real_git}" -C "${primary}" worktree list --porcelain)"
if grep -F -- "${merged_path}" <<<"${post_apply_list}" >/dev/null; then
  echo "codex-worktree-gc-check: removed worktree remains registered" >&2
  exit 1
fi
[[ -d "${open_path}" ]]
[[ -d "${tracked_dirty_path}" ]]
[[ -d "${untracked_dirty_path}" ]]
[[ -d "${ambiguous_path}" ]]
[[ -d "${head_mismatch_path}" ]]
[[ -d "${branch_mismatch_path}" ]]
[[ -d "${repo_mismatch_path}" ]]
[[ -d "${github_unavailable_path}" ]]
[[ -d "${closed_path}" ]]
[[ -d "${current_path}" ]]
[[ -d "${detached_path}" ]]
[[ -d "${outside_path}" ]]

repeat_output="${test_dir}/repeat.txt"
repeat_status=0
run_gc_from "${current_path}" "${primary}" --apply >"${repeat_output}" \
  2>"${test_dir}/repeat.err" ||
  repeat_status="$?"
[[ "${repeat_status}" -eq 2 ]]
assert_not_contains "${repeat_output}" "DELETE"
assert_contains "${repeat_output}" "PRUNE applied"
[[ "$(wc -l <"${prune_log}" | tr -d '[:space:]')" -eq 2 ]]

echo "codex worktree GC check passed: KEEP/DELETE/DIRTY/UNKNOWN/protected, dry-run, apply+prune, identity guards, and idempotence"
