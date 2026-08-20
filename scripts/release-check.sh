#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
script_dir="${repo_root}/scripts"
# shellcheck source=scripts/release-lib.sh
source "${script_dir}/release-lib.sh"
cd "${repo_root}"

echo "==> formatting"
cargo fmt --all --check

echo "==> Clippy (warnings are errors)"
cargo clippy --workspace --all-targets --locked -- -D warnings

echo "==> workspace tests"
cargo test --workspace --all-targets --locked

echo "==> warning-free documentation"
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked

echo "==> minimum supported Rust 1.85.0"
if ! command -v rustup >/dev/null 2>&1; then
  echo "release-check: rustup is required to verify Rust 1.85.0; install rustup and run 'rustup toolchain install 1.85.0'" >&2
  exit 1
fi
if ! msrv_description="$(rustup run 1.85.0 rustc --version 2>/dev/null)"; then
  echo "release-check: Rust 1.85.0 is not installed; run 'rustup toolchain install 1.85.0'" >&2
  exit 1
fi
if [[ "${msrv_description}" != rustc\ 1.85.0\ * ]]; then
  echo "release-check: expected Rust 1.85.0, found '${msrv_description}'" >&2
  exit 1
fi
rustup run 1.85.0 cargo check --workspace --all-targets --locked

echo "==> Cargo source packages"
cargo package --workspace --locked --no-verify

echo "==> first-user workflow"
bash scripts/first-user-smoke.sh

echo "==> semantic collaboration workflow"
bash scripts/collaboration-smoke.sh

native_target="$(rustc -vV | awk '/^host: / { print $2 }')"
[[ -n "${native_target}" ]] || {
  echo "release-check: could not determine the native Rust target" >&2
  exit 1
}

if ! tachiko_supported_target "${native_target}"; then
  echo "release-check: native target '${native_target}' is not a supported release target" >&2
  exit 1
fi

echo "==> native release archive (${native_target})"
cargo build --package tachiko-cli --release --locked --target "${native_target}"
release_output_dir="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-release-check.XXXXXX")"
cleanup() {
  rm -rf -- "${release_output_dir}"
}
trap cleanup EXIT

version="$(tachiko_workspace_version "${repo_root}")" || exit 1
artifact_root="tachiko-${version}-${native_target}"

umask_022_dir="${release_output_dir}/umask-022"
umask_077_dir="${release_output_dir}/umask-077"
mkdir "${umask_022_dir}" "${umask_077_dir}"
(umask 022 && bash scripts/package-binary.sh "${native_target}" "${umask_022_dir}") >/dev/null
(umask 077 && bash scripts/package-binary.sh "${native_target}" "${umask_077_dir}") >/dev/null

archive_path="${umask_022_dir}/${artifact_root}.tar.gz"
strict_umask_archive="${umask_077_dir}/${artifact_root}.tar.gz"
cmp "${archive_path}" "${strict_umask_archive}"
cmp "${archive_path}.sha256" "${strict_umask_archive}.sha256"
bash scripts/verify-release-archive.sh "${native_target}" "${archive_path}"

echo "==> tamper rejection"
tamper_dir="${release_output_dir}/tamper"
mkdir "${tamper_dir}"
tampered_archive="${tamper_dir}/${artifact_root}.tar.gz"
cp "${archive_path}" "${tampered_archive}"
cp "${archive_path}.sha256" "${tampered_archive}.sha256"
printf 'tamper' >>"${tampered_archive}"
if bash scripts/verify-release-archive.sh "${native_target}" "${tampered_archive}" >/dev/null 2>&1; then
  echo "release-check: verifier accepted a tampered archive" >&2
  exit 1
fi

echo "==> interrupted publication cleanup"
interrupted_dir="${release_output_dir}/interrupted"
mkdir "${interrupted_dir}"
interrupt_status=0
TACHIKO_RELEASE_TEST_INTERRUPT_AFTER_ARCHIVE_MOVE=1 \
  bash scripts/package-binary.sh "${native_target}" "${interrupted_dir}" >/dev/null 2>&1 ||
  interrupt_status="$?"
if [[ "${interrupt_status}" -eq 0 ]]; then
  echo "release-check: interrupted packager unexpectedly succeeded" >&2
  exit 1
fi
if [[ -n "$(find "${interrupted_dir}" ! -path "${interrupted_dir}" -print)" ]]; then
  echo "release-check: interrupted packager left an archive, checksum, lock, or partial output" >&2
  exit 1
fi

echo "==> concurrent no-clobber publication"
concurrent_dir="${release_output_dir}/concurrent"
mkdir "${concurrent_dir}"
bash scripts/package-binary.sh "${native_target}" "${concurrent_dir}" >/dev/null 2>&1 &
package_pid_one="$!"
bash scripts/package-binary.sh "${native_target}" "${concurrent_dir}" >/dev/null 2>&1 &
package_pid_two="$!"
package_status_one=0
package_status_two=0
wait "${package_pid_one}" || package_status_one="$?"
wait "${package_pid_two}" || package_status_two="$?"
if ! { [[ "${package_status_one}" -eq 0 && "${package_status_two}" -ne 0 ]] ||
  [[ "${package_status_one}" -ne 0 && "${package_status_two}" -eq 0 ]]; }; then
  echo "release-check: concurrent packaging must produce exactly one success; got ${package_status_one} and ${package_status_two}" >&2
  exit 1
fi
concurrent_archive="${concurrent_dir}/${artifact_root}.tar.gz"
[[ ! -e "${concurrent_dir}/.${artifact_root}.lock" ]] || {
  echo "release-check: package lock remained after concurrent publication" >&2
  exit 1
}
bash scripts/verify-release-archive.sh "${native_target}" "${concurrent_archive}"

echo "release check passed"
