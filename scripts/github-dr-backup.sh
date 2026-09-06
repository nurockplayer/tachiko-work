#!/usr/bin/env bash
set -euo pipefail

# GitHub-to-GitLab disaster-recovery helper.
#
# The source replica contains Git refs only.  Collaboration data is captured
# as a dated, checksummed JSON snapshot which can be committed to the target's
# independent metadata branch by the caller.  Credentials are read by the
# GitHub CLI/Git transport from their normal environment and never form part
# of a snapshot or an informational message from this script.

program="github-dr-backup"
git_bin="${GITHUB_DR_GIT:-git}"
gh_bin="${GITHUB_DR_GH:-gh}"
jq_bin="${GITHUB_DR_JQ:-jq}"
node_bin="${GITHUB_DR_NODE:-node}"

secret_values=()
for secret_name in \
  GH_TOKEN \
  GITHUB_TOKEN \
  GITHUB_DR_GITHUB_TOKEN \
  GITLAB_TOKEN \
  GITHUB_DR_GITLAB_TOKEN \
  GITHUB_DR_TOKEN; do
  secret_value="${!secret_name:-}"
  [[ -n "${secret_value}" ]] && secret_values+=("${secret_value}")
done

redact_text() {
  local line="$1"
  local secret
  for secret in "${secret_values[@]}"; do
    [[ -n "${secret}" ]] || continue
    line="${line//"${secret}"/[REDACTED]}"
  done
  printf '%s\n' "${line}"
}

redact_file() {
  local file="$1"
  local line
  while IFS= read -r line || [[ -n "${line}" ]]; do
    redact_text "${line}"
  done <"${file}"
}

die() {
  local message="$*"
  redact_text "${program}: ${message}" >&2
  exit 1
}

tool_path() {
  local requested="$1"
  local fallback="$2"
  if [[ -n "${requested}" && "${requested}" == */* ]]; then
    [[ -x "${requested}" ]] || die "required tool is not executable: ${fallback}"
    printf '%s\n' "${requested}"
    return
  fi
  command -v "${requested:-${fallback}}" 2>/dev/null ||
    die "required tool is unavailable: ${fallback}"
}

git_bin="$(tool_path "${git_bin}" git)"
jq_bin="$(tool_path "${jq_bin}" jq)"
node_bin="$(tool_path "${node_bin}" node)"

tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/github-dr-backup.XXXXXX")"
snapshot_cleanup_path=""
cleanup_tmp() {
  rm -rf -- "${tmp_root}"
}

cleanup_snapshot_stage() {
  [[ -n "${snapshot_cleanup_path:-}" ]] || return 0
  rm -rf -- "${snapshot_cleanup_path}"
}

cleanup_snapshot_and_tmp() {
  cleanup_snapshot_stage
  cleanup_tmp
}

trap cleanup_tmp EXIT

last_command_error=""

# Run a command without allowing its stdout/stderr to become an accidental
# credential channel.  On failure, retain redacted diagnostics in a temporary
# file so an optional API endpoint can distinguish permission unavailability
# from a real transport or partial-capture failure.
capture_command() {
  local destination="$1"
  shift
  local command_stdout command_stderr command_status
  command_stdout="$(mktemp "${tmp_root}/stdout.XXXXXX")"
  command_stderr="$(mktemp "${tmp_root}/stderr.XXXXXX")"
  last_command_error=""

  if "$@" >"${command_stdout}" 2>"${command_stderr}"; then
    redact_file "${command_stdout}" >"${destination}"
    rm -f -- "${command_stdout}" "${command_stderr}"
    return 0
  else
    command_status="$?"
    last_command_error="$(mktemp "${tmp_root}/error.XXXXXX")"
    if [[ -s "${command_stdout}" ]]; then
      redact_file "${command_stdout}" >"${last_command_error}"
    fi
    if [[ -s "${command_stderr}" ]]; then
      redact_file "${command_stderr}" >>"${last_command_error}"
    fi
    if [[ -s "${last_command_error}" ]]; then
      redact_file "${last_command_error}" >&2
    fi
    rm -f -- "${command_stdout}" "${command_stderr}"
    return "${command_status}"
  fi
}

validate_json() {
  local file="$1"
  local error_file
  error_file="$(mktemp "${tmp_root}/jq-error.XXXXXX")"
  if ! "${jq_bin}" -e . "${file}" > /dev/null 2>"${error_file}"; then
    if [[ -s "${error_file}" ]]; then
      redact_file "${error_file}" >&2
    fi
    rm -f -- "${error_file}"
    die "invalid JSON payload: ${file##*/}"
  fi
  rm -f -- "${error_file}"
}

normalize_collection() {
  local file="$1"
  local normalized error_file
  normalized="${file}.normalized"
  error_file="$(mktemp "${tmp_root}/jq-error.XXXXXX")"
  if ! "${jq_bin}" -c \
    'if type == "array" and length > 0 and all(.[]; type == "array") then add else . end' \
    "${file}" >"${normalized}" 2>"${error_file}"; then
    if [[ -s "${error_file}" ]]; then
      redact_file "${error_file}" >&2
    fi
    rm -f -- "${normalized}" "${error_file}"
    return 1
  fi
  rm -f -- "${error_file}"
  if ! "${jq_bin}" -e 'type == "array"' "${normalized}" > /dev/null 2>"${error_file}"; then
    if [[ -s "${error_file}" ]]; then
      redact_file "${error_file}" >&2
    fi
    rm -f -- "${normalized}" "${error_file}"
    return 1
  fi
  rm -f -- "${file}"
  mv -- "${normalized}" "${file}"
}

api_repository() {
  local destination="$1"
  local endpoint="$2"
  local command_status
  if capture_command "${destination}" \
    "${gh_bin}" api --method GET \
    --header 'Accept: application/vnd.github+json' \
    --header 'X-GitHub-Api-Version: 2022-11-28' \
    "${endpoint}"; then
    validate_json "${destination}"
    return 0
  else
    command_status="$?"
    return "${command_status}"
  fi
}

api_collection() {
  local destination="$1"
  local endpoint="$2"
  local command_status
  if capture_command "${destination}" \
    "${gh_bin}" api --paginate --slurp --method GET \
    --header 'Accept: application/vnd.github+json' \
    --header 'X-GitHub-Api-Version: 2022-11-28' \
    "${endpoint}"; then
    normalize_collection "${destination}" || die "could not normalize API collection: ${endpoint%%\?*}"
    return 0
  else
    command_status="$?"
    return "${command_status}"
  fi
}

api_is_access_unavailable() {
  [[ -n "${last_command_error}" ]] || return 1
  grep -Eiq \
    'HTTP[[:space:]]+(403|404)|[[:space:]](403|404)[[:space:]]|forbidden|not found|resource not accessible|requires? permission|must have admin' \
    "${last_command_error}"
}

json_string() {
  local file="$1"
  local query="$2"
  local value
  value="$("${jq_bin}" -r "${query}" "${file}")" || die "could not read JSON field from ${file##*/}"
  [[ -n "${value}" && "${value}" != "null" ]] || die "missing required JSON field in ${file##*/}"
  printf '%s\n' "${value}"
}

git_local_dir() {
  local repository="$1"
  [[ -d "${repository}" ]] || return 1
  "${git_bin}" --git-dir="${repository}" rev-parse --is-bare-repository > /dev/null 2>&1
}

replicate() {
  local source=""
  local target=""
  local metadata_branch="dr-metadata"

  while (($# > 0)); do
    case "$1" in
      --source)
        (($# >= 2)) || die "--source requires a value"
        source="$2"
        shift 2
        ;;
      --target)
        (($# >= 2)) || die "--target requires a value"
        target="$2"
        shift 2
        ;;
      --metadata-branch)
        (($# >= 2)) || die "--metadata-branch requires a value"
        metadata_branch="$2"
        shift 2
        ;;
      --help|-h)
        usage
        ;;
      *)
        die "unknown replicate option: $1"
        ;;
    esac
  done

  [[ -n "${source}" ]] || die "replicate requires --source"
  [[ -n "${target}" ]] || die "replicate requires --target"
  [[ "${metadata_branch}" =~ ^[A-Za-z0-9._/-]+$ ]] ||
    die "metadata branch contains unsupported characters"
  [[ "${metadata_branch}" != /* && "${metadata_branch}" != */ && "${metadata_branch}" != *..* ]] ||
    die "metadata branch must be a single valid Git ref path"

  local source_head_ref=""
  if [[ -d "${source}/.git" || -f "${source}/.git" ]]; then
    # actions/checkout may leave the worktree detached while its complete
    # GitHub branch set lives under refs/remotes/origin/*.
    source_head_ref="$(${git_bin} -C "${source}" symbolic-ref --quiet \
      refs/remotes/origin/HEAD 2>/dev/null || true)"
    if [[ -z "${source_head_ref}" ]]; then
      source_head_ref="$(${git_bin} -C "${source}" symbolic-ref --quiet \
        HEAD 2>/dev/null || true)"
    fi
  elif git_local_dir "${source}"; then
    source_head_ref="$(${git_bin} --git-dir="${source}" symbolic-ref --quiet \
      HEAD 2>/dev/null || true)"
  fi

  local mirror_parent mirror source_branch
  mirror_parent="$(mktemp -d "${tmp_root}/mirror.XXXXXX")"
  mirror="${mirror_parent}/source.git"
  if ! capture_command "${tmp_root}/clone-output" \
    "${git_bin}" clone --mirror --quiet "${source}" "${mirror}"; then
    die "could not clone the source repository"
  fi

  if [[ -z "${source_head_ref}" ]]; then
    source_head_ref="$(${git_bin} --git-dir="${mirror}" symbolic-ref --quiet \
      HEAD 2>/dev/null || true)"
  fi
  case "${source_head_ref}" in
    refs/remotes/origin/*)
      source_head_ref="refs/heads/${source_head_ref#refs/remotes/origin/}"
      ;;
    refs/heads/*)
      ;;
    *)
      die "source repository does not expose a default branch"
      ;;
  esac
  source_branch="${source_head_ref#refs/heads/}"
  [[ "${source_branch}" != "${metadata_branch}" ]] ||
    die "source default branch collides with reserved metadata branch"

  local source_head_map source_tag_map source_target_refs source_ref branch_name
  source_head_map="$(mktemp "${tmp_root}/source-heads.XXXXXX")"
  source_tag_map="$(mktemp "${tmp_root}/source-tags.XXXXXX")"
  source_target_refs="$(mktemp "${tmp_root}/source-target-refs.XXXXXX")"

  # A normal GitHub Actions checkout keeps the fetched branch refs under
  # refs/remotes/origin/*. Map those refs explicitly to target branch refs so
  # local-only heads are not mistaken for the source branch set. Bare source
  # fixtures and direct Git URLs retain the refs/heads fallback.
  while IFS= read -r source_ref; do
    branch_name="${source_ref#refs/remotes/origin/}"
    [[ "${branch_name}" != "${source_ref}" && "${branch_name}" != HEAD ]] || continue
    printf '%s\trefs/heads/%s\n' "${source_ref}" "${branch_name}" >>"${source_head_map}"
  done < <("${git_bin}" --git-dir="${mirror}" for-each-ref \
    --format='%(refname)' refs/remotes/origin)

  if [[ ! -s "${source_head_map}" ]]; then
    while IFS= read -r source_ref; do
      branch_name="${source_ref#refs/heads/}"
      [[ "${branch_name}" != "${source_ref}" && "${branch_name}" != HEAD ]] || continue
      printf '%s\trefs/heads/%s\n' "${source_ref}" "${branch_name}" >>"${source_head_map}"
    done < <("${git_bin}" --git-dir="${mirror}" for-each-ref \
      --format='%(refname)' refs/heads)
  fi
  [[ -s "${source_head_map}" ]] || die "source repository has no branches to replicate"

  while IFS= read -r source_ref; do
    [[ -n "${source_ref}" ]] || continue
    printf '%s\n' "${source_ref}" >>"${source_tag_map}"
  done < <("${git_bin}" --git-dir="${mirror}" for-each-ref \
    --format='%(refname)' refs/tags)

  while IFS=$'\t' read -r _ target_ref; do
    [[ -n "${target_ref}" ]] && printf '%s\n' "${target_ref}" >>"${source_target_refs}"
  done <"${source_head_map}"
  while IFS= read -r source_ref; do
    [[ -n "${source_ref}" ]] && printf '%s\n' "${source_ref}" >>"${source_target_refs}"
  done <"${source_tag_map}"

  grep -Fqx "${source_head_ref}" "${source_target_refs}" ||
    die "source default branch is not present in the fetched branch refs"
  if grep -Fqx "refs/heads/${metadata_branch}" "${source_target_refs}"; then
    die "source contains the reserved metadata branch"
  fi

  local target_refs target_ref target_kind target_deletes
  target_refs="$(mktemp "${tmp_root}/target-refs.XXXXXX")"
  if ! capture_command "${target_refs}" \
    "${git_bin}" ls-remote --heads --tags "${target}"; then
    die "could not inspect the backup target refs"
  fi
  target_deletes="$(mktemp "${tmp_root}/target-deletes.XXXXXX")"
  while IFS=$'\t' read -r _ target_ref; do
    [[ -n "${target_ref}" ]] || continue
    [[ "${target_ref}" == *'^{}' ]] && continue
    case "${target_ref}" in
      refs/heads/*)
        target_kind="head"
        ;;
      refs/tags/*)
        target_kind="tag"
        ;;
      *)
        continue
        ;;
    esac
    if [[ "${target_kind}" == head && "${target_ref}" == "refs/heads/${metadata_branch}" ]]; then
      continue
    fi
    if ! grep -Fqx "${target_ref}" "${source_target_refs}"; then
      printf ':%s\n' "${target_ref}" >>"${target_deletes}"
    fi
  done <"${target_refs}"

  local -a push_args
  push_args=(--quiet "${target}")
  while IFS=$'\t' read -r source_ref target_ref; do
    [[ -n "${source_ref}" && -n "${target_ref}" ]] || continue
    push_args+=("+${source_ref}:${target_ref}")
  done <"${source_head_map}"
  while IFS= read -r source_ref; do
    [[ -n "${source_ref}" ]] || continue
    push_args+=("+${source_ref}:${source_ref}")
  done <"${source_tag_map}"
  while IFS= read -r target_ref; do
    [[ -n "${target_ref}" ]] && push_args+=("${target_ref}")
  done <"${target_deletes}"
  if ! capture_command "${tmp_root}/push-output" \
    "${git_bin}" --git-dir="${mirror}" push "${push_args[@]}"; then
    die "could not publish source refs to the backup target"
  fi

  # A local bare fixture (and a local operator restore target) can retain the
  # source default-branch identity directly. Hosted GitLab targets advertise
  # the provider-managed default through the Git protocol; verify it after the
  # push so an ordinary clone cannot silently select another branch.
  if git_local_dir "${target}"; then
    if ! capture_command "${tmp_root}/head-output" \
      "${git_bin}" --git-dir="${target}" symbolic-ref HEAD "${source_head_ref}"; then
      die "could not set the backup target default branch"
    fi
  fi

  local target_head_info target_head_ref
  target_head_info="$(mktemp "${tmp_root}/target-head.XXXXXX")"
  if ! capture_command "${target_head_info}" \
    "${git_bin}" ls-remote --symref "${target}" HEAD; then
    die "could not verify the backup target default branch"
  fi
  target_head_ref="$(awk '$1 == "ref:" && $3 == "HEAD" { print $2; exit }' \
    "${target_head_info}")"
  [[ "${target_head_ref}" == "${source_head_ref}" ]] ||
    die "backup target default branch does not match source default branch"

  printf 'github-dr-backup: replicated source refs (default branch %s)\n' "${source_branch}"
}

snapshot() {
  local repository=""
  local output=""
  local source_head=""

  while (($# > 0)); do
    case "$1" in
      --repository)
        (($# >= 2)) || die "--repository requires a value"
        repository="$2"
        shift 2
        ;;
      --output)
        (($# >= 2)) || die "--output requires a value"
        output="$2"
        shift 2
        ;;
      --source-head)
        (($# >= 2)) || die "--source-head requires a value"
        source_head="$2"
        shift 2
        ;;
      --help|-h)
        usage
        ;;
      *)
        die "unknown snapshot option: $1"
        ;;
    esac
  done

  [[ "${repository}" =~ ^[^/[:space:]]+/[^/[:space:]]+$ ]] ||
    die "snapshot requires a repository in OWNER/REPOSITORY form"
  [[ -n "${output}" ]] || die "snapshot requires --output"
  [[ "${source_head}" =~ ^[0-9a-fA-F]{40,64}$ ]] ||
    die "snapshot requires a hexadecimal source HEAD SHA"
  local retention_count
  retention_count="${GITHUB_DR_RETENTION_COUNT:-7}"
  [[ "${retention_count}" =~ ^[1-9][0-9]*$ ]] ||
    die "GITHUB_DR_RETENTION_COUNT must be a positive integer"

  local output_parent output_name output_absolute snapshot_store stage repository_json default_branch
  local snapshot_at snapshot_id dated_snapshot latest_stage old_latest
  local omission_file omission_json api_status pr_number
  output_parent="$(dirname "${output}")"
  if output_parent="$(cd "${output_parent}" 2>/dev/null && pwd -P)"; then
    :
  else
    die "snapshot output parent does not exist"
  fi
  output_name="$(basename "${output}")"
  [[ -n "${output_name}" && "${output_name}" != '.' && "${output_name}" != '..' && "${output_name}" != '/' ]] ||
    die "snapshot output directory name is invalid"
  output_absolute="${output_parent}/${output_name}"
  [[ "${output_absolute}" != '/' && "${output_absolute}" != '//' ]] ||
    die "snapshot output directory is invalid"
  mkdir -p -- "${output_absolute}/snapshots"
  snapshot_store="${output_absolute}/snapshots"
  stage="$(mktemp -d "${output_absolute}/.snapshot.XXXXXX")"
  omission_file="${stage}/.omitted"
  : >"${omission_file}"

  # Keep any stage failure from touching the last known-good latest directory.
  # The stage itself is removed by the combined EXIT trap below. Keep the path
  # outside the function's local scope because EXIT traps run after it returns.
  snapshot_cleanup_path="${stage}"
  trap cleanup_snapshot_and_tmp EXIT

  repository_json="${stage}/repository.json"
  api_repository "${repository_json}" "repos/${repository}"
  default_branch="$(json_string "${repository_json}" '.default_branch // empty')"
  [[ "${default_branch}" != */* && "${default_branch}" != ..* ]] ||
    die "repository default branch is not a valid branch name"

  api_collection "${stage}/branches.json" "repos/${repository}/branches?per_page=100"
  api_collection "${stage}/tags.json" "repos/${repository}/tags?per_page=100"
  api_collection "${stage}/issues.json" "repos/${repository}/issues?state=all&per_page=100"
  api_collection "${stage}/issue_comments.json" "repos/${repository}/issues/comments?per_page=100"
  api_collection "${stage}/pull_requests.json" "repos/${repository}/pulls?state=all&per_page=100"
  api_collection "${stage}/labels.json" "repos/${repository}/labels?per_page=100"
  api_collection "${stage}/milestones.json" "repos/${repository}/milestones?state=all&per_page=100"
  api_collection "${stage}/releases.json" "repos/${repository}/releases?per_page=100"

  # Rulesets require an administration-readable token.  A denied endpoint is
  # an explicit unavailable class; a transport or other API error remains a
  # failed capture and therefore cannot replace the last good snapshot.
  api_status=0
  api_collection "${stage}/rulesets.json" "repos/${repository}/rulesets?per_page=100" || api_status="$?"
  if ((api_status != 0)); then
    if api_is_access_unavailable; then
      printf '[]\n' >"${stage}/rulesets.json"
      printf '%s\n' 'rulesets: unavailable to the supplied GitHub token' >>"${omission_file}"
    else
      die "metadata capture failed for rulesets"
    fi
  fi

  mkdir -p -- "${stage}/pull_requests"
  while IFS= read -r pr_number; do
    [[ -n "${pr_number}" ]] || continue
    [[ "${pr_number}" =~ ^[0-9]+$ ]] || die "pull request fixture has an invalid number"
    mkdir -p -- "${stage}/pull_requests/${pr_number}"
    api_collection "${stage}/pull_requests/${pr_number}/reviews.json" \
      "repos/${repository}/pulls/${pr_number}/reviews?per_page=100"
    api_collection "${stage}/pull_requests/${pr_number}/review_comments.json" \
      "repos/${repository}/pulls/${pr_number}/comments?per_page=100"
    api_collection "${stage}/pull_requests/${pr_number}/conversation_comments.json" \
      "repos/${repository}/issues/${pr_number}/comments?per_page=100"
  done < <("${jq_bin}" -r '.[] | .number // empty' "${stage}/pull_requests.json")

  # Git source replication carries workflow definitions.  Actions runtime
  # records and protected values are intentionally excluded from the JSON
  # snapshot and called out in the manifest below.
  printf '%s\n' \
    'workflow_runs,logs,artifacts and caches are not captured' \
    'GitHub/GitLab credentials, Actions secrets and protected values are not captured' \
    'GitHub-only settings without a permitted read API are not captured' \
    'workflow definitions are carried by the replicated Git refs' >>"${omission_file}"
  omission_json="$("${jq_bin}" -Rsc 'split("\n") | map(select(length > 0))' "${omission_file}")"
  rm -f -- "${omission_file}"

  snapshot_at="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
  snapshot_id="$(date -u '+%Y%m%dT%H%M%SZ')-$$"
  dated_snapshot="${snapshot_store}/${snapshot_id}"
  [[ ! -e "${dated_snapshot}" ]] || die "snapshot identifier already exists"

  "${node_bin}" - "${stage}" "${repository}" "${default_branch}" "${source_head}" \
    "${snapshot_at}" "${omission_json}" <<'EOF_MANIFEST'
const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

const [root, sourceRepository, defaultBranch, sourceHead, snapshotAt, omittedJson] = process.argv.slice(2);
const omitted = JSON.parse(omittedJson);

function walk(directory, relative = '') {
  const entries = fs.readdirSync(directory, {withFileTypes: true});
  const files = [];
  for (const entry of entries) {
    if (entry.name.startsWith('.')) continue;
    const childRelative = relative ? path.join(relative, entry.name) : entry.name;
    const child = path.join(directory, entry.name);
    if (entry.isDirectory()) files.push(...walk(child, childRelative));
    else files.push(childRelative.split(path.sep).join('/'));
  }
  return files;
}

function collectionCount(relative) {
  const value = JSON.parse(fs.readFileSync(path.join(root, relative), 'utf8'));
  if (!Array.isArray(value)) throw new Error(`${relative} is not an array`);
  return value.length;
}

const files = walk(root).filter((relative) => relative !== 'manifest.json');
const checksums = {};
for (const relative of files.sort()) {
  checksums[relative] = crypto.createHash('sha256')
    .update(fs.readFileSync(path.join(root, relative)))
    .digest('hex');
}

const pullRequestDirectory = path.join(root, 'pull_requests');
let reviewCount = 0;
let reviewCommentCount = 0;
let conversationCommentCount = 0;
if (fs.existsSync(pullRequestDirectory)) {
  for (const number of fs.readdirSync(pullRequestDirectory)) {
    const directory = path.join(pullRequestDirectory, number);
    if (!fs.statSync(directory).isDirectory()) continue;
    reviewCount += collectionCount(`pull_requests/${number}/reviews.json`);
    reviewCommentCount += collectionCount(`pull_requests/${number}/review_comments.json`);
    conversationCommentCount += collectionCount(`pull_requests/${number}/conversation_comments.json`);
  }
}

const manifest = {
  schema_version: 'github-dr-backup/v1',
  source_repository: sourceRepository,
  default_branch: defaultBranch,
  source_head: sourceHead,
  snapshot_at: snapshotAt,
  counts: {
    repository: 1,
    branches: collectionCount('branches.json'),
    tags: collectionCount('tags.json'),
    issues: collectionCount('issues.json'),
    issue_comments: collectionCount('issue_comments.json'),
    pull_requests: collectionCount('pull_requests.json'),
    pull_request_reviews: reviewCount,
    pull_request_review_comments: reviewCommentCount,
    pull_request_conversation_comments: conversationCommentCount,
    labels: collectionCount('labels.json'),
    milestones: collectionCount('milestones.json'),
    releases: collectionCount('releases.json'),
    rulesets: collectionCount('rulesets.json'),
    workflow_definitions: 'included in the replicated Git refs',
  },
  omitted_or_unavailable: omitted,
  capture: {
    api_pagination: 'GitHub REST collections requested with gh api --paginate --slurp',
    workflow_definitions: 'captured by Git source replication; not duplicated in this JSON snapshot',
    target_role: 'GitLab is an independent backup-only target; GitHub remains authoritative',
  },
  checksums,
};

fs.writeFileSync(path.join(root, 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`, {mode: 0o600});
EOF_MANIFEST

  # Publish the immutable dated capture first.  latest is a separate writable
  # view so operators can run the documented corruption/verification drill;
  # any failure before this point leaves an existing latest untouched.
  mv -- "${stage}" "${dated_snapshot}"
  stage=""
  snapshot_cleanup_path=""
  latest_stage="$(mktemp -d "${output_absolute}/.latest.XXXXXX")"
  rmdir -- "${latest_stage}"
  if ! cp -pR "${dated_snapshot}" "${latest_stage}"; then
    die "could not stage the latest snapshot view"
  fi
  old_latest="${output_absolute}/.latest.previous.$$"
  if [[ -e "${output_absolute}/latest" || -L "${output_absolute}/latest" ]]; then
    mv -- "${output_absolute}/latest" "${old_latest}" || die "could not rotate the prior latest snapshot"
  fi
  if ! mv -- "${latest_stage}" "${output_absolute}/latest"; then
    if [[ -e "${old_latest}" || -L "${old_latest}" ]]; then
      mv -- "${old_latest}" "${output_absolute}/latest" || true
    fi
    die "could not publish the latest snapshot"
  fi
  [[ ! -e "${old_latest}" && ! -L "${old_latest}" ]] || rm -rf -- "${old_latest}"

  local retention_candidate index
  index=0
  while IFS= read -r retention_candidate; do
    [[ -d "${retention_candidate}" ]] || continue
    index=$((index + 1))
    if ((index > retention_count)); then
      rm -rf -- "${retention_candidate}"
    fi
  done < <(
    for retention_candidate in "${snapshot_store}"/20*T*Z-*; do
      [[ -d "${retention_candidate}" ]] || continue
      printf '%s\n' "${retention_candidate}"
    done | sort -r
  )

  printf 'github-dr-backup: snapshot published at %s (source HEAD %s)\n' \
    "${output_absolute}/latest" "${source_head}"
}

verify() {
  local snapshot_path=""
  while (($# > 0)); do
    case "$1" in
      --snapshot)
        (($# >= 2)) || die "--snapshot requires a value"
        snapshot_path="$2"
        shift 2
        ;;
      --help|-h)
        usage
        ;;
      *)
        die "unknown verify option: $1"
        ;;
    esac
  done
  [[ -n "${snapshot_path}" ]] || die "verify requires --snapshot"
  [[ -d "${snapshot_path}" ]] || die "snapshot directory does not exist"
  [[ -f "${snapshot_path}/manifest.json" ]] || die "snapshot manifest is missing"

  local verify_error
  verify_error="$(mktemp "${tmp_root}/verify-error.XXXXXX")"
  if ! "${node_bin}" - "${snapshot_path}" <<'EOF_VERIFY' 2>"${verify_error}"
const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

const root = process.argv[2];
const manifestPath = path.join(root, 'manifest.json');
const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
if (typeof manifest.schema_version !== 'string' || !manifest.schema_version) throw new Error('missing schema_version');
if (typeof manifest.source_repository !== 'string' || !manifest.source_repository) throw new Error('missing source_repository');
if (typeof manifest.default_branch !== 'string' || !manifest.default_branch) throw new Error('missing default_branch');
if (!/^[0-9a-f]{40,64}$/i.test(String(manifest.source_head))) throw new Error('invalid source_head');
if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/.test(String(manifest.snapshot_at))) throw new Error('invalid snapshot_at');
if (!manifest.counts || typeof manifest.counts !== 'object') throw new Error('missing counts');
if (!Array.isArray(manifest.omitted_or_unavailable)) throw new Error('missing omissions disclosure');
if (!manifest.checksums || typeof manifest.checksums !== 'object' || Object.keys(manifest.checksums).length === 0) {
  throw new Error('missing checksums');
}

for (const [relative, expected] of Object.entries(manifest.checksums)) {
  if (!relative || path.isAbsolute(relative) || relative.split('/').includes('..')) throw new Error(`unsafe checksum path: ${relative}`);
  if (!/^[0-9a-f]{64}$/.test(String(expected))) throw new Error(`invalid checksum: ${relative}`);
  const absolute = path.join(root, relative);
  if (!fs.existsSync(absolute) || !fs.statSync(absolute).isFile()) throw new Error(`missing payload: ${relative}`);
  const actual = crypto.createHash('sha256').update(fs.readFileSync(absolute)).digest('hex');
  if (actual !== expected) throw new Error(`checksum mismatch: ${relative}`);
}

function walk(directory, relative = '') {
  const entries = fs.readdirSync(directory, {withFileTypes: true});
  const files = [];
  for (const entry of entries) {
    const childRelative = relative ? path.join(relative, entry.name) : entry.name;
    const child = path.join(directory, entry.name);
    if (entry.isDirectory()) files.push(...walk(child, childRelative));
    else files.push(childRelative.split(path.sep).join('/'));
  }
  return files;
}

const actualPayloads = new Set(walk(root).filter((relative) => relative !== 'manifest.json'));
for (const relative of actualPayloads) {
  if (!Object.prototype.hasOwnProperty.call(manifest.checksums, relative)) throw new Error(`unlisted payload: ${relative}`);
}
EOF_VERIFY
  then
    if [[ -s "${verify_error}" ]]; then
      redact_file "${verify_error}" >&2
    fi
    rm -f -- "${verify_error}"
    die "snapshot verification failed"
  fi
  rm -f -- "${verify_error}"

  # Check all currently supplied credential markers as a final disclosure
  # guard.  This is intentionally independent from checksum validation.
  local payload_file secret
  while IFS= read -r -d '' payload_file; do
    for secret in "${secret_values[@]}"; do
      [[ -n "${secret}" ]] || continue
      if grep -F -- "${secret}" "${payload_file}" >/dev/null 2>&1; then
        die "credential marker found in snapshot payload"
      fi
    done
  done < <(find "${snapshot_path}" -type f -print0)

  printf 'github-dr-backup: verified snapshot %s\n' "${snapshot_path}"
}

usage() {
  cat <<'EOF_USAGE'
Usage:
  github-dr-backup.sh replicate --source SOURCE --target TARGET [--metadata-branch NAME]
  github-dr-backup.sh snapshot --repository OWNER/REPOSITORY --output DIRECTORY --source-head SHA
  github-dr-backup.sh verify --snapshot DIRECTORY

Environment:
  GITHUB_DR_GIT / GITHUB_DR_GH / GITHUB_DR_JQ / GITHUB_DR_NODE override tools for fixtures.
  GITHUB_DR_RETENTION_COUNT bounds dated snapshots (default: 7).
  GH_TOKEN/GITHUB_TOKEN and GITHUB_DR_GITLAB_TOKEN are consumed by their
  respective transport/CLI and are never written into the snapshot.
EOF_USAGE
}

command="${1:-}"
if (($# > 0)); then
  shift
fi
case "${command}" in
  replicate)
    replicate "$@"
    ;;
  snapshot)
    snapshot "$@"
    ;;
  verify)
    verify "$@"
    ;;
  help|-h|--help)
    usage
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac
