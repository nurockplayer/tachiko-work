#!/usr/bin/env bash
set -euo pipefail

fail() {
  echo "export-experimental-designer-client: $*" >&2
  exit 1
}

if [[ "$#" -ne 1 ]]; then
  fail "usage: bash scripts/export-experimental-designer-client.sh OUTPUT_DIRECTORY"
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"

output_arg="$1"
if [[ "${output_arg}" == /* ]]; then
  output_dir="${output_arg}"
else
  output_dir="${PWD}/${output_arg}"
fi
output_name="$(basename "${output_dir}")"
[[ "${output_name}" != "." && "${output_name}" != ".." ]] ||
  fail "output must name a dedicated kit directory"

for tool in cc git node pnpm tar; do
  command -v "${tool}" >/dev/null 2>&1 || fail "required tool not found: ${tool}"
done

# The exporter is a source-derived build. Refuse every form of worktree input
# before resolving HEAD, including staged, unstaged, and untracked files.
git_env=(env -i "PATH=${PATH}")
if [[ "${HOME+x}" == x ]]; then git_env+=("HOME=${HOME}"); fi
if [[ "${TMPDIR+x}" == x ]]; then git_env+=("TMPDIR=${TMPDIR}"); fi
git_env+=(
  "GIT_CONFIG_NOSYSTEM=1"
  "GIT_CONFIG_GLOBAL=/dev/null"
  "GIT_CONFIG_SYSTEM=/dev/null"
)
status="$("${git_env[@]}" git -C "${repo_root}" --no-replace-objects \
  -c core.attributesFile=/dev/null -c core.fsmonitor=false -c core.untrackedCache=false \
  status --porcelain=v1 --untracked-files=all)" ||
  fail "could not inspect source checkout state"
[[ -z "${status}" ]] || fail "source checkout must be clean (staged, modified, or untracked files found)"

source_commit="$("${git_env[@]}" git -C "${repo_root}" --no-replace-objects \
  -c core.attributesFile=/dev/null rev-parse --verify 'HEAD^{commit}' 2>/dev/null)" ||
  fail "HEAD does not resolve to a source commit"
[[ "${source_commit}" =~ ^[0-9a-f]{40}$ ]] ||
  fail "source Git object format must provide a 40-character commit ID"

scratch="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-experimental-client.XXXXXX")"
cleanup() { rm -rf -- "${scratch}"; }
trap cleanup EXIT

# Read the source materializer from the captured commit, rather than from the
# live checkout. A same-path replacement after preflight therefore cannot
# influence archival validation or any later build/copy operation.
source_helper="${scratch}/designer-rc-source.sh"
"${git_env[@]}" git -C "${repo_root}" --no-replace-objects \
  -c core.attributesFile=/dev/null show "${source_commit}:scripts/designer-rc-source.sh" >"${source_helper}" ||
  fail "could not load captured source materializer"
[[ -s "${source_helper}" && ! -L "${source_helper}" ]] ||
  fail "captured source materializer is not a regular file"
# shellcheck source=/dev/null
source "${source_helper}"

source_root="${scratch}/source"
tachiko_rc_materialize_source "${repo_root}" "${source_commit}" "${source_root}" ||
  fail "could not materialize exact Git source"
tachiko_rc_check_ancestor_cargo_config "${source_root}" ||
  fail "source scratch parent is not safe for Cargo"

# The captured script performs every source-derived build/copy operation, so
# neither a later live-worktree change nor a replacement symlink can influence
# the kit that records source_commit.
bash "${source_root}/scripts/package-experimental-designer-client.sh" \
  "${output_dir}" "${source_commit}" "${scratch}"

echo "experimental Designer client kit exported to ${output_dir} from ${source_commit}"
