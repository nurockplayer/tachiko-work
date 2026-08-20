#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: $0 <target> <output-directory>" >&2
}

fail() {
  echo "package-binary: $*" >&2
  exit 1
}

if [[ "$#" -ne 2 ]]; then
  usage
  exit 2
fi

target="$1"
output_dir="$2"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
# shellcheck source=scripts/release-lib.sh
source "${script_dir}/release-lib.sh"

tachiko_supported_target "${target}" ||
  fail "unsupported target '${target}'; expected a supported Tachiko release target"
version="$(tachiko_workspace_version "${repo_root}")" || exit 1
executable_name="$(tachiko_executable_name "${target}")"

binary="${repo_root}/target/${target}/release/${executable_name}"
[[ -f "${binary}" ]] ||
  fail "missing release binary '${binary}'; build it with cargo build --release --locked --target ${target} -p tachiko-cli"
[[ -x "${binary}" ]] || fail "release binary is not executable: ${binary}"

while IFS= read -r payload; do
  [[ -f "${repo_root}/${payload}" ]] || fail "missing required payload: ${payload}"
done <<<"$(tachiko_release_payloads)"

mkdir -p "${output_dir}"
output_dir="$(cd "${output_dir}" && pwd)"

artifact_root="tachiko-${version}-${target}"
archive_name="${artifact_root}.tar.gz"
archive_path="${output_dir}/${archive_name}"
checksum_path="${archive_path}.sha256"

[[ ! -e "${archive_path}" ]] || fail "refusing to overwrite existing archive: ${archive_path}"
[[ ! -e "${checksum_path}" ]] || fail "refusing to overwrite existing checksum: ${checksum_path}"

lock_dir="${output_dir}/.${artifact_root}.lock"
lock_owned=0
lock_created=0
pending_signal_status=0
work_dir=""
published_archive=0
published_checksum=0
completed=0

cleanup() {
  local status="$?"
  trap - EXIT HUP INT TERM
  set +e
  if [[ "${completed}" -ne 1 ]]; then
    if [[ "${published_checksum}" -eq 1 ]]; then
      rm -f -- "${checksum_path}"
    fi
    if [[ "${published_archive}" -eq 1 ]]; then
      rm -f -- "${archive_path}"
    fi
  fi
  if [[ -n "${work_dir}" ]]; then
    rm -rf -- "${work_dir}"
  fi
  if [[ "${lock_owned}" -eq 1 ]]; then
    rmdir -- "${lock_dir}"
  fi
  exit "${status}"
}

handle_signal() {
  local status="$1"
  trap - HUP INT TERM
  exit "${status}"
}

defer_signal() {
  local status="$1"
  if [[ "${pending_signal_status}" -eq 0 ]]; then
    pending_signal_status="${status}"
  fi
}

# Signal delivery is deferred across the only ownership critical section:
# mkdir may have created the lock before Bash can record that this process owns
# it. Once ownership is recorded, pending signals exit through normal cleanup.
trap cleanup EXIT
trap 'defer_signal 129' HUP
trap 'defer_signal 130' INT
trap 'defer_signal 143' TERM
if mkdir "${lock_dir}" 2>/dev/null; then
  lock_created=1
  if [[ "${TACHIKO_RELEASE_TEST_INTERRUPT_AFTER_LOCK_MKDIR:-0}" == "1" ]]; then
    kill -TERM "$$"
  fi
  lock_owned=1
fi

# Install immediate handling before observing deferred state. A signal arriving
# during this transition is therefore either recorded or handled immediately.
trap 'handle_signal 129' HUP
trap 'handle_signal 130' INT
trap 'handle_signal 143' TERM
if [[ "${pending_signal_status}" -ne 0 ]]; then
  exit "${pending_signal_status}"
fi
if [[ "${lock_created}" -ne 1 ]]; then
  fail "artifact is already being packaged or has a stale lock: ${lock_dir}"
fi

# Recheck within the lock so a completed concurrent publisher cannot be
# mistaken for an artifact that is still safe to replace.
[[ ! -e "${archive_path}" ]] || fail "refusing to overwrite existing archive: ${archive_path}"
[[ ! -e "${checksum_path}" ]] || fail "refusing to overwrite existing checksum: ${checksum_path}"

work_dir="$(mktemp -d "${output_dir}/.tachiko-package.XXXXXX")"

stage_root="${work_dir}/${artifact_root}"
mkdir "${stage_root}"
cp "${binary}" "${stage_root}/${executable_name}"
while IFS= read -r payload; do
  cp "${repo_root}/${payload}" "${stage_root}/${payload}"
done <<<"$(tachiko_release_payloads)"
chmod 0755 "${stage_root}"
chmod 0755 "${stage_root}/${executable_name}"
while IFS= read -r payload; do
  chmod 0644 "${stage_root}/${payload}"
done <<<"$(tachiko_release_payloads)"

# Fixed timestamps, ownership, order, and gzip headers make equivalent inputs
# produce equivalent archives. COPYFILE_DISABLE prevents AppleDouble entries on
# macOS; the exact file list avoids filesystem-dependent traversal order.
TZ=UTC touch -t 198001010000 "${stage_root}/${executable_name}"
while IFS= read -r payload; do
  TZ=UTC touch -t 198001010000 "${stage_root}/${payload}"
done <<<"$(tachiko_release_payloads)"
TZ=UTC touch -t 198001010000 "${stage_root}"

archive_members=("${artifact_root}" "${artifact_root}/${executable_name}")
while IFS= read -r payload; do
  archive_members+=("${artifact_root}/${payload}")
done <<<"$(tachiko_release_payloads)"

partial_archive="${work_dir}/${archive_name}.partial"
tar_version="$(tar --version 2>/dev/null || true)"
(
  cd "${work_dir}"
  if [[ "${tar_version}" == *"GNU tar"* ]]; then
    tar \
      --format=ustar \
      --owner=root:0 \
      --group=root:0 \
      --no-recursion \
      -cf - \
      "${archive_members[@]}"
  elif [[ "${tar_version}" == *"bsdtar"* ]]; then
    COPYFILE_DISABLE=1 tar \
      --format=ustar \
      --uid 0 \
      --gid 0 \
      --uname root \
      --gname root \
      --no-recursion \
      -cf - \
      "${archive_members[@]}"
  else
    fail "unsupported tar implementation; install GNU tar or bsdtar"
  fi
) | gzip -n >"${partial_archive}"

digest="$(tachiko_sha256_digest "${partial_archive}")" || exit 1
[[ "${digest}" =~ ^[0-9a-fA-F]{64}$ ]] || fail "SHA-256 tool returned an invalid digest"

partial_checksum="${work_dir}/${archive_name}.sha256.partial"
printf '%s  %s\n' "${digest}" "${archive_name}" >"${partial_checksum}"

# Ownership is claimed before each move. If a signal lands immediately after
# mv changes the filesystem but before Bash advances, cleanup still removes the
# half-published pair while this invocation owns the artifact lock.
published_archive=1
mv "${partial_archive}" "${archive_path}"
if [[ "${TACHIKO_RELEASE_TEST_INTERRUPT_AFTER_ARCHIVE_MOVE:-0}" == "1" ]]; then
  kill -TERM "$$"
fi
published_checksum=1
mv "${partial_checksum}" "${checksum_path}"
completed=1

printf '%s\n' "${archive_path}"
printf '%s\n' "${checksum_path}"
