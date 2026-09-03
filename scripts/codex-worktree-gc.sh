#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: bash scripts/codex-worktree-gc.sh [--apply] [--repository PATH]
       [--codex-root PATH]

Audit registered Codex worktrees for this repository. The default is a
read-only dry run. --apply removes only a clean worktree whose live GitHub PR
is a uniquely identified merged PR, then runs git worktree prune.

Options:
  --apply              Remove proven-eligible worktrees.
  --repository PATH    Repository checkout to inspect/protect. Defaults to the
                       current checkout.
  --codex-root PATH    Only registered worktrees below this directory are
                       candidates. Defaults to CODEX_WORKTREE_ROOT or
                       $HOME/.codex/worktrees.
  --help               Show this help.

Exit status 0 means the audit completed without UNKNOWN classifications. Exit
status 2 means the audit completed but at least one candidate was unresolved;
no UNKNOWN candidate is removed. Other non-zero statuses indicate an invalid
invocation or an operational failure.
EOF
}

die() {
  echo "codex-worktree-gc: $*" >&2
  exit 1
}

command_available() {
  local command_name="$1"
  if [[ "${command_name}" == */* ]]; then
    [[ -x "${command_name}" ]]
  else
    command -v "${command_name}" >/dev/null 2>&1
  fi
}

canonical_directory() {
  local path="$1"
  if [[ -d "${path}" ]]; then
    (cd "${path}" && pwd -P)
    return
  fi

  local parent
  parent="$(dirname "${path}")"
  if [[ -d "${parent}" ]]; then
    printf '%s/%s\n' "$(cd "${parent}" && pwd -P)" "$(basename "${path}")"
  else
    printf '%s\n' "${path}"
  fi
}

format_size() {
  local kib="$1"
  awk -v kib="${kib}" 'BEGIN {
    if (kib >= 1048576) {
      printf "%.1f GiB", kib / 1048576
    } else if (kib >= 1024) {
      printf "%.1f MiB", kib / 1024
    } else {
      printf "%d KiB", kib
    }
  }'
}

worktree_size() {
  local path="$1"
  local size
  size="$(du -sk "${path}" 2>/dev/null | awk 'NR == 1 { print $1 }')" || return 1
  [[ "${size}" =~ ^[0-9]+$ ]] || return 1
  printf '%s\n' "${size}"
}

path_is_below() {
  local path="$1"
  local root="$2"
  [[ "${path}" == "${root}"/* ]]
}

run_git() {
  env -u GIT_DIR -u GIT_WORK_TREE -u GIT_COMMON_DIR -u GIT_CEILING_DIRECTORIES \
    "${git_command}" "$@"
}

run_gh() {
  "${gh_command}" "$@"
}

remote_slug() {
  local remote="$1"
  local host
  remote="${remote%/}"
  remote="${remote%.git}"
  case "${remote}" in
    git@*:*)
      host="${remote#git@}"
      host="${host%%:*}"
      [[ "${host}" == "github.com" ]] || return 1
      remote="${remote#*:}"
      ;;
    ssh://*/*|https://*/*|http://*/*)
      remote="${remote#*://}"
      host="${remote%%/*}"
      host="${host##*@}"
      [[ "${host}" == "github.com" ]] || return 1
      remote="${remote#*/}"
      ;;
    *)
      return 1
      ;;
  esac
  [[ "${remote}" =~ ^[^/[:space:]]+/[^/[:space:]]+$ ]] || return 1
  printf '%s\n' "${remote}"
}

mode="dry-run"
repository_arg=""
codex_root_arg=""
while (($# > 0)); do
  case "$1" in
    --apply)
      mode="apply"
      shift
      ;;
    --repository)
      (($# >= 2)) || die "--repository requires a path"
      repository_arg="$2"
      shift 2
      ;;
    --codex-root)
      (($# >= 2)) || die "--codex-root requires a path"
      codex_root_arg="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      die "unknown option '$1' (use --help for usage)"
      ;;
  esac
done

git_command="${CODEX_WORKTREE_GC_GIT:-git}"
gh_command="${CODEX_WORKTREE_GC_GH:-gh}"
command_available "${git_command}" || die "git is required"

invoking_root=""
if invoking_root="$(run_git rev-parse --show-toplevel 2>/dev/null)"; then
  invoking_root="$(canonical_directory "${invoking_root}")"
fi

if [[ -n "${repository_arg}" ]]; then
  [[ -d "${repository_arg}" ]] || die "repository path does not exist: ${repository_arg}"
  repository_arg="$(canonical_directory "${repository_arg}")"
  repo_root="$(run_git -C "${repository_arg}" rev-parse --show-toplevel 2>/dev/null)" ||
    die "not a Git worktree: ${repository_arg}"
else
  repo_root="$(run_git rev-parse --show-toplevel 2>/dev/null)" ||
    die "run this command inside a Git worktree or pass --repository"
fi
repo_root="$(canonical_directory "${repo_root}")"
repo_common_dir="$(run_git -C "${repo_root}" rev-parse --git-common-dir 2>/dev/null)" ||
  die "could not resolve the repository Git directory"
if [[ "${repo_common_dir}" != /* ]]; then
  repo_common_dir="${repo_root}/${repo_common_dir}"
fi
repo_common_dir="$(canonical_directory "${repo_common_dir}")"

if [[ -n "${codex_root_arg}" ]]; then
  codex_root="${codex_root_arg}"
elif [[ -n "${CODEX_WORKTREE_ROOT:-}" ]]; then
  codex_root="${CODEX_WORKTREE_ROOT}"
elif [[ -n "${HOME:-}" ]]; then
  codex_root="${HOME}/.codex/worktrees"
else
  die "Codex worktree root is unresolved; pass --codex-root"
fi
[[ -d "${codex_root}" ]] || die "Codex worktree root does not exist: ${codex_root}"
codex_root="$(canonical_directory "${codex_root}")"

repo_remote=""
repo_slug=""
repo_locator=""
if repo_remote="$(run_git -C "${repo_root}" remote get-url origin 2>/dev/null)"; then
  repo_slug="$(remote_slug "${repo_remote}" 2>/dev/null || true)"
fi
if [[ -n "${repo_slug}" ]]; then
  repo_locator="github.com/${repo_slug}"
fi

worktree_listing="$(run_git -C "${repo_root}" worktree list --porcelain 2>/dev/null)" ||
  die "could not list registered worktrees"

worktree_paths=()
worktree_heads=()
worktree_branches=()
worktree_locked=()
worktree_prunable=()
worktree_index=-1
while IFS= read -r line || [[ -n "${line}" ]]; do
  case "${line}" in
    worktree\ *)
      worktree_paths+=("${line#worktree }")
      worktree_heads+=("")
      worktree_branches+=("")
      worktree_locked+=("0")
      worktree_prunable+=("0")
      worktree_index=$((worktree_index + 1))
      ;;
    HEAD\ *)
      if ((worktree_index >= 0)); then
        worktree_heads[worktree_index]="${line#HEAD }"
      fi
      ;;
    branch\ refs/heads/*)
      if ((worktree_index >= 0)); then
        worktree_branches[worktree_index]="${line#branch }"
      fi
      ;;
    locked*)
      if ((worktree_index >= 0)); then
        worktree_locked[worktree_index]="1"
      fi
      ;;
    prunable*)
      if ((worktree_index >= 0)); then
        worktree_prunable[worktree_index]="1"
      fi
      ;;
  esac
done <<< "${worktree_listing}"

primary_root=""
if ((${#worktree_paths[@]} > 0)); then
  primary_root="$(canonical_directory "${worktree_paths[0]}")"
fi
current_root="$(canonical_directory "${repo_root}")"

path_is_in_codex_root() {
  local registered_path="$1"
  local candidate_path
  candidate_path="$(canonical_directory "${registered_path}")"
  path_is_below "${candidate_path}" "${codex_root}" ||
    path_is_below "${registered_path}" "${codex_root}"
}

github_checked="0"
github_ready="0"
github_default_branch=""
github_reason=""
load_github_default_branch() {
  if [[ "${github_checked}" == "1" ]]; then
    [[ "${github_ready}" == "1" ]]
    return
  fi
  github_checked="1"
  if [[ -z "${repo_slug}" ]]; then
    github_reason="repository origin identity is unresolved"
    return 1
  fi
  if ! command_available "${gh_command}"; then
    github_reason="GitHub CLI is unavailable"
    return 1
  fi
  if ! github_default_branch="$(run_gh repo view "${repo_locator}" \
    --json defaultBranchRef --jq '.defaultBranchRef.name // empty' 2>/dev/null)"; then
    github_reason="GitHub repository state is unavailable"
    return 1
  fi
  if [[ -z "${github_default_branch}" || "${github_default_branch}" == *$'\t'* ||
    "${github_default_branch}" == *$'\n'* ]]; then
    github_reason="GitHub default branch identity is ambiguous"
    return 1
  fi
  github_ready="1"
}

pr_number=""
pr_state=""
pr_merged_at=""
pr_head_ref=""
pr_head_oid=""
pr_base_ref=""
pr_head_repository=""
pr_reason=""
pr_classification=""
lookup_pr() {
  local branch="$1"
  local head="$2"
  local pr_rows=""
  local pr_record=""
  local pr_count=0
  local line

  if ! load_github_default_branch; then
    pr_reason="${github_reason}"
    return 1
  fi
  if ! pr_rows="$(run_gh pr list --repo "${repo_locator}" --head "${branch}" \
    --state all --limit 100 \
    --json number,state,mergedAt,headRefName,headRefOid,baseRefName,headRepository \
    --jq '[.[] | [(.number | tostring), .state, (.mergedAt // "-"), .headRefName, .headRefOid, .baseRefName, (.headRepository.nameWithOwner // "")] | @tsv] | .[]' \
    2>/dev/null)"; then
    pr_reason="GitHub PR state is unavailable"
    return 1
  fi

  while IFS= read -r line || [[ -n "${line}" ]]; do
    [[ -z "${line}" ]] && continue
    pr_count=$((pr_count + 1))
    pr_record="${line}"
  done <<< "${pr_rows}"

  if ((pr_count != 1)); then
    if ((pr_count == 0)); then
      pr_reason="no unique PR matched branch"
    else
      pr_reason="multiple PRs matched branch"
    fi
    return 1
  fi

  IFS=$'\t' read -r pr_number pr_state pr_merged_at pr_head_ref \
    pr_head_oid pr_base_ref pr_head_repository <<< "${pr_record}"
  if [[ "${pr_merged_at}" == "-" ]]; then
    pr_merged_at=""
  fi
  if [[ -z "${pr_number}" || -z "${pr_state}" || -z "${pr_head_ref}" ||
    -z "${pr_head_oid}" || -z "${pr_base_ref}" || -z "${pr_head_repository}" ]]; then
    pr_reason="PR identity fields are incomplete"
    return 1
  fi
  if [[ ! "${pr_number}" =~ ^[1-9][0-9]*$ ]]; then
    pr_reason="PR number identity is invalid"
    return 1
  fi
  if [[ "${pr_head_ref}" != "${branch}" ]]; then
    pr_reason="PR branch identity mismatch"
    return 1
  fi
  if [[ "${pr_head_oid}" != "${head}" ]]; then
    pr_reason="PR HEAD identity mismatch"
    return 1
  fi
  if [[ "${pr_base_ref}" != "${github_default_branch}" ]]; then
    pr_reason="PR base branch identity mismatch"
    return 1
  fi
  if [[ "${pr_head_repository}" != "${repo_slug}" ]]; then
    pr_reason="PR repository identity mismatch"
    return 1
  fi

  case "${pr_state}" in
    OPEN)
      if [[ -n "${pr_merged_at}" ]]; then
        pr_reason="PR state is inconsistent"
        return 1
      fi
      pr_classification="KEEP"
      pr_reason="open PR #${pr_number}"
      ;;
    CLOSED)
      if [[ -n "${pr_merged_at}" ]]; then
        pr_reason="PR state is inconsistent"
        return 1
      fi
      pr_classification="KEEP"
      pr_reason="closed unmerged PR #${pr_number}; explicit policy required"
      ;;
    MERGED)
      if [[ -z "${pr_merged_at}" ]]; then
        pr_reason="merged PR has no live merge timestamp"
        return 1
      fi
      pr_classification="DELETE"
      pr_reason="merged PR #${pr_number}"
      ;;
    *)
      pr_reason="unsupported PR state '${pr_state}'"
      return 1
      ;;
  esac
}

classification=""
classification_reason=""
classification_size_kib=""
classification_size_label="?"
classify_worktree() {
  local index="$1"
  local registered_path="${worktree_paths[index]}"
  local registered_head="${worktree_heads[index]}"
  local registered_branch="${worktree_branches[index]}"
  local path
  local actual_head=""
  local actual_branch=""
  local candidate_common_dir=""
  local status_output=""
  local size_available="0"

  classification="UNKNOWN"
  classification_reason="unresolved local state"
  classification_size_kib=""
  classification_size_label="?"
  path="$(canonical_directory "${registered_path}")"

  if classification_size_kib="$(worktree_size "${registered_path}" 2>/dev/null)"; then
    classification_size_label="$(format_size "${classification_size_kib}")"
    size_available="1"
  else
    classification_reason="disk usage is unavailable"
  fi

  if [[ "${path}" == "${primary_root}" || "${path}" == "${current_root}" ||
    ( -n "${invoking_root}" && "${path}" == "${invoking_root}" ) ]]; then
    classification="PROTECTED"
    classification_reason="primary/current protected checkout"
    return
  fi
  if [[ "${size_available}" != "1" ]]; then
    return
  fi
  if [[ "${worktree_locked[index]}" == "1" ]]; then
    classification="KEEP"
    classification_reason="worktree is locked"
    return
  fi
  if [[ "${worktree_prunable[index]}" == "1" ]]; then
    classification_reason="worktree registration is prunable"
    return
  fi
  if [[ ! -d "${registered_path}" ]]; then
    classification_reason="worktree path is missing"
    return
  fi
  if ! path_is_below "${path}" "${codex_root}"; then
    classification_reason="worktree path escapes Codex root"
    return
  fi
  if [[ "${path}" != "${registered_path}" ]]; then
    classification_reason="worktree path resolves through a different filesystem path"
    return
  fi
  if ! candidate_common_dir="$(run_git -C "${registered_path}" rev-parse --git-common-dir 2>/dev/null)"; then
    classification_reason="worktree Git directory is unavailable"
    return
  fi
  if [[ "${candidate_common_dir}" != /* ]]; then
    candidate_common_dir="${registered_path}/${candidate_common_dir}"
  fi
  candidate_common_dir="$(canonical_directory "${candidate_common_dir}")"
  if [[ "${candidate_common_dir}" != "${repo_common_dir}" ]]; then
    classification_reason="worktree belongs to a different Git repository"
    return
  fi
  if ! actual_head="$(run_git -C "${registered_path}" rev-parse --verify HEAD 2>/dev/null)"; then
    classification_reason="worktree HEAD is unavailable"
    return
  fi
  if [[ -z "${registered_head}" || "${actual_head}" != "${registered_head}" ]]; then
    classification_reason="local HEAD differs from worktree registration"
    return
  fi
  if ! actual_branch="$(run_git -C "${registered_path}" symbolic-ref --quiet --short HEAD 2>/dev/null)"; then
    classification_reason="worktree is detached"
    return
  fi
  if [[ -z "${registered_branch}" || "${registered_branch}" != "refs/heads/${actual_branch}" ]]; then
    classification_reason="local branch differs from worktree registration"
    return
  fi
  if ! status_output="$(run_git -C "${registered_path}" status --porcelain=v1 \
    --untracked-files=all 2>/dev/null)"; then
    classification_reason="worktree status is unavailable"
    return
  fi
  if [[ -n "${status_output}" ]]; then
    classification="DIRTY"
    classification_reason="tracked or untracked changes present"
    return
  fi

  if ! lookup_pr "${actual_branch}" "${actual_head}"; then
    classification_reason="${pr_reason}"
    return
  fi
  classification="${pr_classification}"
  classification_reason="${pr_reason}"
}

echo "Codex worktree audit (${mode}): repository=${repo_root} codex-root=${codex_root}"
printf '%-9s %-34s %-68s %s\n' "STATUS" "WORKTREE" "REASON" "DISK"

active_count=0
keep_count=0
delete_count=0
dirty_count=0
unknown_count=0
protected_count=0
reclaimable_kib=0
delete_indices=()

for index in "${!worktree_paths[@]}"; do
  registered_path="${worktree_paths[index]}"
  candidate_path="$(canonical_directory "${registered_path}")"
  if ! path_is_below "${candidate_path}" "${codex_root}" &&
    ! path_is_below "${registered_path}" "${codex_root}"; then
    continue
  fi

  active_count=$((active_count + 1))
  relative_path="${candidate_path#"${codex_root}"/}"
  if [[ -z "${relative_path}" || "${relative_path}" == "${candidate_path}" ]]; then
    relative_path="$(basename "${candidate_path}")"
  fi
  classify_worktree "${index}"
  printf '%-9s %-34s %-68s %s\n' "${classification}" "${relative_path}" \
    "${classification_reason}" "${classification_size_label}"

  case "${classification}" in
    KEEP)
      keep_count=$((keep_count + 1))
      ;;
    DELETE)
      delete_count=$((delete_count + 1))
      reclaimable_kib=$((reclaimable_kib + classification_size_kib))
      delete_indices+=("${index}")
      ;;
    DIRTY)
      dirty_count=$((dirty_count + 1))
      ;;
    PROTECTED)
      protected_count=$((protected_count + 1))
      ;;
    UNKNOWN)
      unknown_count=$((unknown_count + 1))
      ;;
  esac
done

prune_scope_reason=""
check_prune_scope() {
  local fresh_listing=""
  local line
  local registered_path=""
  local registered_prunable="0"

  if ! fresh_listing="$(run_git -C "${repo_root}" worktree list --porcelain 2>/dev/null)"; then
    prune_scope_reason="Git worktree listing is unavailable"
    return 2
  fi

  while IFS= read -r line || [[ -n "${line}" ]]; do
    case "${line}" in
      worktree\ *)
        if [[ -n "${registered_path}" && "${registered_prunable}" == "1" ]] &&
          path_is_in_codex_root "${registered_path}"; then
          prune_scope_reason="in-scope prunable worktree: ${registered_path}"
          return 1
        fi
        if [[ -n "${registered_path}" && "${registered_prunable}" == "1" ]]; then
          prune_scope_reason="out-of-scope prunable worktree: ${registered_path}"
          return 1
        fi
        registered_path="${line#worktree }"
        registered_prunable="0"
        ;;
      prunable*)
        registered_prunable="1"
        ;;
    esac
  done <<< "${fresh_listing}"

  if [[ -n "${registered_path}" && "${registered_prunable}" == "1" ]] &&
    path_is_in_codex_root "${registered_path}"; then
    prune_scope_reason="in-scope prunable worktree: ${registered_path}"
    return 1
  fi
  if [[ -n "${registered_path}" && "${registered_prunable}" == "1" ]]; then
    prune_scope_reason="out-of-scope prunable worktree: ${registered_path}"
    return 1
  fi
  return 0
}

printf 'SUMMARY active=%d keep=%d delete=%d dirty=%d protected=%d unknown=%d\n' \
  "${active_count}" "${keep_count}" "${delete_count}" "${dirty_count}" \
  "${protected_count}" "${unknown_count}"
if ((reclaimable_kib > 0)); then
  if [[ "${mode}" == "dry-run" ]]; then
    printf 'RECLAIMABLE estimated=%s\n' "$(format_size "${reclaimable_kib}")"
  else
    printf 'RECLAIMABLE proven=%s\n' "$(format_size "${reclaimable_kib}")"
  fi
else
  echo "RECLAIMABLE estimated=0 KiB"
fi

if [[ "${mode}" == "apply" ]]; then
  prune_scope_status=0
  check_prune_scope || prune_scope_status="$?"
  if ((prune_scope_status == 2)); then
    die "cannot prove prune scope: ${prune_scope_reason}"
  fi
  if ((prune_scope_status == 1)); then
    echo "PRUNE skipped (${prune_scope_reason})"
    echo "RESULT blocked: ${prune_scope_reason}"
    exit 2
  fi

  if ((${#delete_indices[@]} > 0)); then
    for index in "${delete_indices[@]}"; do
      github_checked="0"
      github_ready="0"
      classify_worktree "${index}"
      if [[ "${classification}" != "DELETE" ]]; then
        echo "codex-worktree-gc: apply refused for ${worktree_paths[index]}: ${classification_reason}" >&2
        exit 1
      fi
      run_git -C "${repo_root}" worktree remove "${worktree_paths[index]}"
    done
  fi

  prune_scope_status=0
  check_prune_scope || prune_scope_status="$?"
  if ((prune_scope_status == 2)); then
    die "cannot prove prune scope: ${prune_scope_reason}"
  fi
  if ((prune_scope_status == 1)); then
    echo "PRUNE skipped (${prune_scope_reason})"
    echo "RESULT blocked: ${prune_scope_reason}"
    exit 2
  fi

  prune_output=""
  if ! prune_output="$(run_git -C "${repo_root}" worktree prune --verbose 2>&1)"; then
    echo "codex-worktree-gc: git worktree prune failed" >&2
    printf '%s\n' "${prune_output}" >&2
    exit 1
  fi
  if [[ -n "${prune_output}" ]]; then
    printf '%s\n' "${prune_output}"
  fi
  echo "PRUNE applied"
else
  echo "PRUNE skipped (dry-run; no mutation)"
fi

if ((unknown_count > 0)); then
  echo "RESULT blocked: ${unknown_count} UNKNOWN classification(s) were not removed"
  exit 2
fi
echo "RESULT complete: no UNKNOWN classifications"
