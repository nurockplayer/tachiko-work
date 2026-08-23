#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

portable_packages=(
  tachiko-semantic-core
  tachiko-formula-engine
  tachiko-diff-engine
  tachiko-merge-engine
  tachiko-workspace-engine
  tachiko-ai-api
)

for package in "${portable_packages[@]}"; do
  cargo check --package "${package}" --target wasm32-unknown-unknown --locked
done

portable_sources=(
  crates/semantic-core/src
  crates/formula-engine/src
  crates/diff-engine/src
  crates/merge-engine/src
  crates/workspace-engine/src
  crates/ai-api/src
)
forbidden_pattern='std::fs|std::path|PathBuf|SystemTime|Instant::now|std::time|std::thread|thread::|spawn\(|rand::|getrandom|uuid::|std::env|env::|TcpStream|UdpSocket|reqwest|tokio|locale|Locale|unic_langid|icu_'

if rg -n "${forbidden_pattern}" "${portable_sources[@]}"; then
  echo "issue-26 portability audit: host capability reference found in portable production sources" >&2
  exit 1
fi

dependency_tree="$(cargo tree --target wasm32-unknown-unknown --package tachiko-ai-api --edges normal --locked)"
if grep -Eq '(^|[[:space:]])(getrandom|rand|uuid|libc|mio|socket2|tokio|reqwest) v' <<<"${dependency_tree}"; then
  echo "issue-26 portability audit: native/ambient dependency found in portable runtime tree" >&2
  echo "${dependency_tree}" >&2
  exit 1
fi

audit_dir="$(mktemp -d)"
trap 'rm -rf "${audit_dir}"' EXIT
native_target="$(rustc -vV | sed -n 's/^host: //p')"
cargo tree --target "${native_target}" --package tachiko-ai-api --edges normal --locked --prefix none \
  | sed -E 's/ v[0-9][^ ]*.*$//' | sort -u >"${audit_dir}/native.txt"
cargo tree --target wasm32-unknown-unknown --package tachiko-ai-api --edges normal --locked --prefix none \
  | sed -E 's/ v[0-9][^ ]*.*$//' | sort -u >"${audit_dir}/wasm.txt"
if ! cmp --silent "${audit_dir}/native.txt" "${audit_dir}/wasm.txt"; then
  echo "issue-26 portability audit: native and WASM normal dependency package sets differ" >&2
  diff -u "${audit_dir}/wasm.txt" "${audit_dir}/native.txt" >&2 || true
  exit 1
fi

echo "issue-26 portability audit passed: six ADR-0016 portable crates compile for wasm32-unknown-unknown"
echo "issue-26 portability audit passed: no filesystem/path/clock/random/thread/network/environment/locale references in portable production sources"
echo "issue-26 portability audit passed: portable runtime dependency tree contains no audited native/ambient runtime package"
echo "issue-26 portability audit passed: native and WASM normal dependency package sets match"
