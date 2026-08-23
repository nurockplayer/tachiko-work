#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cargo build \
  --manifest-path "${repo_root}/spikes/issue-26-runtime/Cargo.toml" \
  --bin native-driver \
  --release \
  --locked

cargo build \
  --manifest-path "${repo_root}/spikes/issue-26-runtime/Cargo.toml" \
  --target wasm32-unknown-unknown \
  --release \
  --locked
