#!/usr/bin/env bash
set -u

if (($# < 1)); then
  echo "codex-rustc-wrapper: Cargo did not provide a compiler command" >&2
  exit 1
fi

compiler="$1"
shift
sccache_bin="${TACHIKO_CODEX_SCCACHE_BIN:-}"
if [[ -n "${sccache_bin}" && -x "${sccache_bin}" ]]; then
  if "${sccache_bin}" "${compiler}" "$@"; then
    exit 0
  fi
  echo "codex-rustc-wrapper: sccache invocation failed; falling back to direct rustc" >&2
fi

exec "${compiler}" "$@"
