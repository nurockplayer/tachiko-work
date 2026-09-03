#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: bash scripts/codex-cargo.sh <cargo-subcommand> [options]

Run an ephemeral-agent Cargo command with a private target directory. The
default target is the current worktree's target/. CARGO_INCREMENTAL defaults to
0 for this process only. A shared sccache is opt-in with
TACHIKO_CODEX_SCCACHE=1; it is bounded by SCCACHE_CACHE_SIZE (5G by default)
and server-I/O failures use sccache's direct-rustc fallback.
EOF
}

die() {
  echo "codex-cargo: $*" >&2
  exit 1
}

path_is_below() {
  local path="$1"
  local root="$2"
  [[ "${path}" == "${root}"/* ]]
}

is_sccache_wrapper() {
  local wrapper="${RUSTC_WRAPPER:-}"
  [[ "$(basename "${wrapper}")" == "sccache" ]]
}

command -v git >/dev/null 2>&1 || die "git is required"
command -v cargo >/dev/null 2>&1 || die "cargo is required"
(($# > 0)) || {
  usage >&2
  exit 1
}

# Git environment overrides can make rev-parse resolve a different checkout
# from the directory in which this helper was invoked. They would also leak
# that foreign repository context into Cargo's child process.
unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_CEILING_DIRECTORIES

for cargo_argument in "$@"; do
  [[ "${cargo_argument}" == "--" ]] && break
  case "${cargo_argument}" in
    --target-dir|--target-dir=*)
      die "pass a worktree-local CARGO_TARGET_DIR; --target-dir is not allowed"
      ;;
  esac
done

repo_root="$(git rev-parse --show-toplevel 2>/dev/null)" ||
  die "run this command inside a Git worktree"
repo_root="$(cd "${repo_root}" && pwd -P)"

requested_target="${CARGO_TARGET_DIR:-}"
if [[ -z "${requested_target}" ]]; then
  target_dir="${repo_root}/target"
elif [[ "${requested_target}" == /* ]]; then
  target_dir="${requested_target}"
else
  target_dir="${PWD}/${requested_target}"
fi

target_parent="$(dirname "${target_dir}")"
[[ -d "${target_parent}" ]] || die "target directory parent does not exist: ${target_parent}"
canonical_parent="$(cd "${target_parent}" && pwd -P)"
canonical_target="${canonical_parent}/$(basename "${target_dir}")"
[[ ! -L "${target_dir}" ]] ||
  die "CARGO_TARGET_DIR must not be a symlink: ${target_dir}"
if [[ -e "${target_dir}" ]]; then
  [[ -d "${target_dir}" ]] || die "target path is not a directory: ${target_dir}"
  canonical_target="$(cd "${target_dir}" && pwd -P)"
fi
path_is_below "${canonical_target}" "${repo_root}" ||
  die "CARGO_TARGET_DIR must resolve below the current worktree: ${canonical_target}"

export CARGO_TARGET_DIR="${target_dir}"
export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-0}"

sccache_mode="${TACHIKO_CODEX_SCCACHE:-0}"
case "${sccache_mode}" in
  1|true|yes|on|auto)
    sccache_bin="${TACHIKO_CODEX_SCCACHE_BIN:-}"
    if [[ -z "${sccache_bin}" ]]; then
      sccache_bin="$(command -v sccache 2>/dev/null || true)"
    fi
    if [[ -n "${sccache_bin}" && -x "${sccache_bin}" ]]; then
      export SCCACHE_CACHE_SIZE="${SCCACHE_CACHE_SIZE:-5G}"
      export SCCACHE_IGNORE_SERVER_IO_ERROR=1
      export TACHIKO_CODEX_SCCACHE_BIN="${sccache_bin}"
      export RUSTC_WRAPPER="${repo_root}/scripts/codex-rustc-wrapper.sh"
      sccache_status="enabled (${sccache_bin}, limit ${SCCACHE_CACHE_SIZE})"
    else
      if is_sccache_wrapper; then
        unset RUSTC_WRAPPER
      fi
      sccache_status="unavailable; direct rustc"
    fi
    ;;
  0|false|no|off|"")
    if is_sccache_wrapper; then
      unset RUSTC_WRAPPER
    fi
    sccache_status="disabled"
    ;;
  *)
    die "TACHIKO_CODEX_SCCACHE must be 0/1 or off/on"
    ;;
esac

echo "codex-cargo: target=${CARGO_TARGET_DIR} incremental=${CARGO_INCREMENTAL} sccache=${sccache_status}" >&2
exec cargo "$@"
