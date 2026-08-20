#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: $0 <target> <archive.tar.gz>" >&2
}

fail() {
  echo "verify-release-archive: $*" >&2
  exit 1
}

if [[ "$#" -ne 2 ]]; then
  usage
  exit 2
fi

target="$1"
archive_argument="$2"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
# shellcheck source=scripts/release-lib.sh
source "${script_dir}/release-lib.sh"

tachiko_supported_target "${target}" ||
  fail "unsupported target '${target}'; expected a supported Tachiko release target"
version="$(tachiko_workspace_version "${repo_root}")" || exit 1

[[ -f "${archive_argument}" ]] || fail "archive not found: ${archive_argument}"
archive_dir="$(cd "$(dirname "${archive_argument}")" && pwd)"
archive_name="$(basename "${archive_argument}")"
archive_path="${archive_dir}/${archive_name}"

artifact_root="tachiko-${version}-${target}"
expected_archive_name="${artifact_root}.tar.gz"
[[ "${archive_name}" == "${expected_archive_name}" ]] ||
  fail "archive name '${archive_name}' does not match workspace version and target '${expected_archive_name}'"

checksum_path="${archive_path}.sha256"
[[ -f "${checksum_path}" ]] || fail "checksum not found: ${checksum_path}"
[[ "$(awk 'END { print NR }' "${checksum_path}")" -eq 1 ]] ||
  fail "checksum must contain exactly one line"
[[ "$(awk 'NR == 1 { print NF }' "${checksum_path}")" -eq 2 ]] ||
  fail "checksum must contain exactly a digest and archive basename"

expected_digest="$(awk 'NR == 1 { print $1 }' "${checksum_path}")"
listed_archive="$(awk 'NR == 1 { print $2 }' "${checksum_path}")"
[[ "${expected_digest}" =~ ^[0-9a-fA-F]{64}$ ]] || fail "checksum contains an invalid SHA-256 digest"
[[ "${listed_archive}" == "${archive_name}" ]] ||
  fail "checksum must reference the archive basename only: ${archive_name}"

actual_digest="$(tachiko_sha256_digest "${archive_path}")" || exit 1
actual_digest="$(printf '%s' "${actual_digest}" | tr '[:upper:]' '[:lower:]')"
expected_digest="$(printf '%s' "${expected_digest}" | tr '[:upper:]' '[:lower:]')"
[[ "${actual_digest}" == "${expected_digest}" ]] || fail "archive checksum mismatch"

executable_name="$(tachiko_executable_name "${target}")"

verify_dir="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-verify.XXXXXX")"
cleanup() {
  rm -rf -- "${verify_dir}"
}
trap cleanup EXIT

expected_members="${verify_dir}/expected-members.txt"
actual_members="${verify_dir}/actual-members.txt"
expected_types="${verify_dir}/expected-types.txt"
actual_types="${verify_dir}/actual-types.txt"

{
  printf '%s\n' "${artifact_root}/" "${artifact_root}/${executable_name}"
  while IFS= read -r payload; do
    printf '%s\n' "${artifact_root}/${payload}"
  done <<<"$(tachiko_release_payloads)"
} | LC_ALL=C sort >"${expected_members}"

tar -tzf "${archive_path}" | LC_ALL=C sort >"${actual_members}"
cmp -s "${expected_members}" "${actual_members}" ||
  fail "archive member set is unsafe or incomplete; expected only the executable, README, changelog, and two licenses"

{
  printf '%s\n' d -
  while IFS= read -r _payload; do
    printf '%s\n' -
  done <<<"$(tachiko_release_payloads)"
} | LC_ALL=C sort >"${expected_types}"
tar -tvzf "${archive_path}" | awk '{ print substr($1, 1, 1) }' |
  LC_ALL=C sort >"${actual_types}"
cmp -s "${expected_types}" "${actual_types}" ||
  fail "archive contains an unexpected member type; only one directory and regular files are allowed"

tar -xzf "${archive_path}" -C "${verify_dir}"
executable="${verify_dir}/${artifact_root}/${executable_name}"
[[ -x "${executable}" ]] || fail "archived CLI is not executable: ${artifact_root}/${executable_name}"

if ! version_output="$("${executable}" --version)"; then
  fail "archived CLI did not run natively for target ${target}"
fi
[[ "${version_output}" == "tachiko ${version}" ]] ||
  fail "archived CLI reported '${version_output}', expected 'tachiko ${version}'"

echo "verified ${archive_name}: checksum, safe payload, native tachiko ${version}"
