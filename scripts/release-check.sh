#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
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

case "${native_target}" in
  x86_64-unknown-linux-gnu | aarch64-apple-darwin | x86_64-apple-darwin | x86_64-pc-windows-msvc) ;;
  *)
    echo "release-check: native target '${native_target}' is not a supported release target" >&2
    exit 1
    ;;
esac

echo "==> native release archive (${native_target})"
cargo build --package tachiko-cli --release --locked --target "${native_target}"
release_output_dir="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-release-check.XXXXXX")"
cleanup() {
  rm -rf -- "${release_output_dir}"
}
trap cleanup EXIT

package_output="$(bash scripts/package-binary.sh "${native_target}" "${release_output_dir}")"
archive_path="$(printf '%s\n' "${package_output}" | awk 'NR == 1 { print }')"

[[ -n "${archive_path}" ]] || {
  echo "release-check: package script did not report an archive path" >&2
  exit 1
}
bash scripts/verify-release-archive.sh "${native_target}" "${archive_path}"

echo "release check passed"
