#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: bash scripts/pre-push-check.sh [--fast] [--help]

Run the advisory, read-only local checks that catch common late CI failures.
The script does not install a Git hook or change Rust toolchain defaults.

Options:
  --fast  Skip the Rust 1.85.0 workspace and wasm32 runtime checks.
  --help  Show this help.

Clippy uses TACHIKO_CLIPPY_TOOLCHAIN when set, otherwise stable. The script
compares that toolchain with the newest stable reported by `rustup check` and
warns if it is older. Install the suggested toolchain and set
TACHIKO_CLIPPY_TOOLCHAIN to use it. A rustup/network check failure is a warning.

An existing CARGO_TARGET_DIR is honored.

Checks run in this order:
  1. cargo fmt --all -- --check
  2. workspace Clippy with all targets and warnings denied
  3. browser-client runtime Clippy with all targets and warnings denied
  4. Rust 1.85.0 workspace check (omitted with --fast)
  5. Rust 1.85.0 wasm32 browser-client runtime check (omitted with --fast)
  6. documentation consistency check
EOF
}

fast=0
show_help=0
for argument in "$@"; do
  case "${argument}" in
    --fast) fast=1 ;;
    --help) show_help=1 ;;
    *)
      echo "pre-push-check: unknown argument: ${argument}" >&2
      echo "Run 'bash scripts/pre-push-check.sh --help' for usage." >&2
      exit 2
      ;;
  esac
done

if [[ "${show_help}" -eq 1 ]]; then
  usage
  exit 0
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

version_is_older() {
  local local_version="$1"
  local newest_version="$2"
  local local_major local_minor local_patch
  local newest_major newest_minor newest_patch

  IFS=. read -r local_major local_minor local_patch <<<"${local_version}"
  IFS=. read -r newest_major newest_minor newest_patch <<<"${newest_version}"

  if ((local_major != newest_major)); then
    ((local_major < newest_major))
  elif ((local_minor != newest_minor)); then
    ((local_minor < newest_minor))
  else
    ((local_patch < newest_patch))
  fi
}

warn_if_clippy_toolchain_is_old() {
  local toolchain="$1"
  local rustup_output newest_version local_output local_version

  if ! rustup_output="$(rustup check 2>&1)"; then
    echo "WARNING: unable to compare ${toolchain} with the newest stable toolchain (rustup check failed)." >&2
    echo "         Check network access and run 'rustup check' manually." >&2
    return 0
  fi

  newest_version="$(printf '%s\n' "${rustup_output}" \
    | sed -nE '/^stable-[^[:space:]]+[[:space:]]+-/ { s/.*[Uu]pdate available[[:space:]]*:.*->[[:space:]]*([0-9]+\.[0-9]+\.[0-9]+).*/\1/p; s/.*[Uu]p to date[[:space:]]*:[[:space:]]*([0-9]+\.[0-9]+\.[0-9]+).*/\1/p; }' \
    | head -n 1)"
  if [[ -z "${newest_version}" ]]; then
    echo "WARNING: rustup check succeeded, but its newest stable version could not be parsed." >&2
    return 0
  fi

  if ! local_output="$(rustup run "${toolchain}" rustc --version 2>&1)"; then
    echo "WARNING: unable to read the installed version for ${toolchain}; Clippy will report if it is unavailable." >&2
    return 0
  fi
  local_version="$(sed -nE 's/^rustc ([0-9]+\.[0-9]+\.[0-9]+).*/\1/p' <<<"${local_output}")"
  if [[ -z "${local_version}" ]]; then
    echo "WARNING: unable to parse the installed Rust version for ${toolchain}." >&2
    return 0
  fi

  if version_is_older "${local_version}" "${newest_version}"; then
    cat >&2 <<EOF
WARNING: Clippy toolchain ${toolchain} is Rust ${local_version}; newest stable is ${newest_version}.
         Fix: rustup toolchain install ${newest_version} --profile minimal -c clippy
         Then run with: TACHIKO_CLIPPY_TOOLCHAIN=${newest_version} bash scripts/pre-push-check.sh
EOF
  fi
}

clippy_toolchain="${TACHIKO_CLIPPY_TOOLCHAIN:-stable}"
warn_if_clippy_toolchain_is_old "${clippy_toolchain}"

run_check() {
  local label="$1"
  shift
  echo "==> ${label}"
  "$@"
}

run_check "cargo fmt --all -- --check" cargo fmt --all -- --check
run_check "workspace Clippy (${clippy_toolchain})" \
  rustup run "${clippy_toolchain}" cargo clippy --workspace --all-targets --locked -- -D warnings
run_check "browser-client runtime Clippy (${clippy_toolchain})" \
  rustup run "${clippy_toolchain}" cargo clippy \
  --manifest-path packages/browser-client/runtime/Cargo.toml \
  --all-targets --locked -- -D warnings

if [[ "${fast}" -eq 0 ]]; then
  run_check "Rust 1.85.0 workspace check" \
    rustup run 1.85.0 cargo check --workspace --all-targets --locked
  run_check "Rust 1.85.0 wasm32 browser-client runtime check" \
    rustup run 1.85.0 cargo check \
    --manifest-path packages/browser-client/runtime/Cargo.toml \
    --target wasm32-unknown-unknown --all-targets --locked
else
  echo "==> --fast: skipping Rust 1.85.0 and wasm32 checks"
fi

run_check "documentation consistency" bash scripts/docs-consistency-check.sh
echo "pre-push-check passed"
