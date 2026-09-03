#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
script_dir="${repo_root}/scripts"
# shellcheck source=scripts/release-lib.sh
source "${script_dir}/release-lib.sh"
cd "${repo_root}"

if ! command -v rustup >/dev/null 2>&1; then
  echo "release-check: rustup is required; install stable and Rust 1.85.0 before running the release gate" >&2
  exit 1
fi
if ! stable_description="$(rustup run stable rustc --version 2>/dev/null)"; then
  echo "release-check: stable Rust is not installed; run 'rustup toolchain install stable'" >&2
  exit 1
fi

# Select stable for the entire gate, including bare cargo/rustc invocations and
# nested smoke/packaging scripts. This process-local export overrides an
# inherited toolchain selection without modifying the caller's rustup override.
export RUSTUP_TOOLCHAIN=stable
selected_description="$(rustc --version)"
if [[ "${selected_description}" != "${stable_description}" ]]; then
  echo "release-check: stable selection failed; expected '${stable_description}', found '${selected_description}'" >&2
  exit 1
fi
echo "==> selected release toolchain: ${selected_description}"

echo "==> formatting"
cargo fmt --all --check

echo "==> Codex repository tooling"
bash scripts/codex-worktree-gc-check.sh
bash scripts/codex-cargo-check.sh

echo "==> ADR-0016 workspace dependency graph"
node scripts/workspace-dependency-check.mjs

echo "==> Clippy (warnings are errors)"
cargo clippy --workspace --all-targets --locked -- -D warnings

echo "==> workspace tests"
cargo test --workspace --all-targets --locked

echo "==> executed native/WASM portable semantic conformance"
bash scripts/portable-conformance-check.sh

echo "==> first-party Web Designer vertical slice"
bash scripts/designer-check.sh

echo "==> strict operational-evidence foundation"
bash scripts/operational-evidence-check.sh

echo "==> warning-free documentation"
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked

echo "==> minimum supported Rust 1.85.0"
if ! msrv_description="$(rustup run 1.85.0 rustc --version 2>/dev/null)"; then
  echo "release-check: Rust 1.85.0 is not installed; run 'rustup toolchain install 1.85.0'" >&2
  exit 1
fi
if [[ "${msrv_description}" != rustc\ 1.85.0\ * ]]; then
  echo "release-check: expected Rust 1.85.0, found '${msrv_description}'" >&2
  exit 1
fi
rustup run 1.85.0 cargo check --workspace --all-targets --locked
rustup run 1.85.0 cargo check --manifest-path apps/designer/runtime/Cargo.toml \
  --target wasm32-unknown-unknown --all-targets --locked

echo "==> audited third-party license notices"
notice_check_dir="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-notice-check.XXXXXX")"
cleanup_notice_check() {
  rm -rf -- "${notice_check_dir}"
}
trap cleanup_notice_check EXIT
bash scripts/generate-third-party-licenses.sh >"${notice_check_dir}/generated.md"
if ! cmp -s THIRD_PARTY_LICENSES.md "${notice_check_dir}/generated.md"; then
  echo "release-check: THIRD_PARTY_LICENSES.md is stale; regenerate it with 'bash scripts/generate-third-party-licenses.sh > THIRD_PARTY_LICENSES.md'" >&2
  exit 1
fi
cp "${notice_check_dir}/generated.md" "${notice_check_dir}/modified.md"
printf '\n<!-- release-check drift probe -->\n' >>"${notice_check_dir}/modified.md"
if cmp -s THIRD_PARTY_LICENSES.md "${notice_check_dir}/modified.md"; then
  echo "release-check: dependency-license drift control failed to reject a modified notice" >&2
  exit 1
fi
rm -rf -- "${notice_check_dir}"
trap - EXIT

echo "==> Cargo source packages"
cargo package --workspace --locked --no-verify

echo "==> standalone Game Dev Alpha workflow"
bash scripts/first-user-smoke.sh

echo "==> semantic collaboration workflow"
bash scripts/collaboration-smoke.sh

echo "==> semantic entity lifecycle workflow"
bash scripts/entity-lifecycle-smoke.sh

echo "==> computational formula authoring workflow"
bash scripts/formula-authoring-smoke.sh

echo "==> optional Git-native Game Dev Alpha workflow"
bash scripts/git-ci-smoke.sh

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

echo "==> interrupted lock acquisition cleanup"
lock_interrupted_dir="${release_output_dir}/lock-interrupted"
mkdir "${lock_interrupted_dir}"
lock_interrupt_status=0
TACHIKO_RELEASE_TEST_INTERRUPT_AFTER_LOCK_MKDIR=1 \
  bash scripts/package-binary.sh "${native_target}" "${lock_interrupted_dir}" >/dev/null 2>&1 ||
  lock_interrupt_status="$?"
if [[ "${lock_interrupt_status}" -lt 128 ]]; then
  echo "release-check: lock-window interruption must exit with a signal-like status; got ${lock_interrupt_status}" >&2
  exit 1
fi
if [[ -n "$(find "${lock_interrupted_dir}" ! -path "${lock_interrupted_dir}" -print)" ]]; then
  echo "release-check: lock-window interruption left a lock, output, or partial state" >&2
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
