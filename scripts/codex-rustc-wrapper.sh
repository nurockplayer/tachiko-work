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
  # Let sccache distinguish server-I/O fallback from a real rustc failure.
  # Retrying every non-zero result would duplicate compiler diagnostics and
  # could double the latency of an ordinary failed compilation.
  exec "${sccache_bin}" "${compiler}" "$@"
fi

exec "${compiler}" "$@"
