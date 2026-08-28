#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ -n "${TACHIKO_BIN:-}" ]]; then
  tachiko_bin="${TACHIKO_BIN}"
else
  cargo build --manifest-path "${repo_root}/Cargo.toml" -p tachiko-cli
  tachiko_bin="${repo_root}/target/debug/tachiko"
fi
if [[ "${tachiko_bin}" != */* ]]; then
  tachiko_bin="$(command -v "${tachiko_bin}")"
fi
tachiko_bin="$(cd "$(dirname "${tachiko_bin}")" && pwd)/$(basename "${tachiko_bin}")"

smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-first-user.XXXXXX")"
trap 'rm -rf "${smoke_dir}"' EXIT
if command -v git >/dev/null 2>&1 && \
  git -C "${smoke_dir}" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  rmdir "${smoke_dir}"
  smoke_dir="$(mktemp -d "/tmp/tachiko-first-user.XXXXXX")"
fi
cd "${smoke_dir}"
if command -v git >/dev/null 2>&1 && git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "first-user smoke: standalone lane must execute outside a Git worktree" >&2
  exit 1
fi

base_direct="${repo_root}/examples/game-balance/game-balance.ro"
base_project="${smoke_dir}/game-balance.roproj"
buffed_direct="${smoke_dir}/buffed-sword.ro"
buffed_project="${smoke_dir}/buffed-sword.roproj"
invalid_direct="${smoke_dir}/invalid-sword.ro"
export_one="${smoke_dir}/game-balance.json"
export_two="${smoke_dir}/game-balance-repeat.json"

"${tachiko_bin}" validate "${base_direct}" >"${smoke_dir}/validate-direct.txt"
"${tachiko_bin}" roproj materialize "${base_direct}" "${base_project}" \
  >"${smoke_dir}/materialize-base.txt"
"${tachiko_bin}" roproj validate "${base_project}" \
  >"${smoke_dir}/validate-base-project.txt"

"${tachiko_bin}" show "${base_project}" >"${smoke_dir}/show.txt"
grep -F "Moonfall: starter balance · 4 schemas · 4 entities · 3 formulas" \
  "${smoke_dir}/show.txt" >/dev/null
grep -F "weapons · Iron Sword (iron_sword) [" "${smoke_dir}/show.txt" >/dev/null
grep -F ": 40 (formula)" "${smoke_dir}/show.txt" >/dev/null

"${tachiko_bin}" explain "${base_project}" iron_sword.dps \
  >"${smoke_dir}/explain.txt"
grep -F "formula: ([iron_sword.damage] / [iron_sword.attack_interval])" \
  "${smoke_dir}/explain.txt" >/dev/null

"${tachiko_bin}" set "${base_project}" iron_sword.damage 45 \
  --output "${buffed_direct}" >"${smoke_dir}/set.txt"
grep -F "affected dps: 40 -> 50" "${smoke_dir}/set.txt" >/dev/null
"${tachiko_bin}" roproj materialize "${buffed_direct}" "${buffed_project}" \
  >"${smoke_dir}/materialize-buffed.txt"

"${tachiko_bin}" diff "${base_project}" "${buffed_project}" \
  >"${smoke_dir}/diff-one.txt"
"${tachiko_bin}" diff "${base_project}" "${buffed_project}" \
  >"${smoke_dir}/diff-two.txt"
cmp "${smoke_dir}/diff-one.txt" "${smoke_dir}/diff-two.txt"
grep -F "damage: 36 -> 45" "${smoke_dir}/diff-one.txt" >/dev/null
grep -F "affected dps: 40 -> 50" "${smoke_dir}/diff-one.txt" >/dev/null

"${tachiko_bin}" analyze changes "${base_project}" "${buffed_project}" \
  --before-state base --after-state buffed >"${smoke_dir}/analysis-one.json"
"${tachiko_bin}" analyze changes "${base_project}" "${buffed_project}" \
  --before-state base --after-state buffed >"${smoke_dir}/analysis-two.json"
cmp "${smoke_dir}/analysis-one.json" "${smoke_dir}/analysis-two.json"

"${tachiko_bin}" roproj validate "${buffed_project}" >"${smoke_dir}/validate.txt"
"${tachiko_bin}" calculate "${buffed_project}" >"${smoke_dir}/calculate-one.json"
"${tachiko_bin}" calculate "${buffed_project}" >"${smoke_dir}/calculate-two.json"
cmp "${smoke_dir}/calculate-one.json" "${smoke_dir}/calculate-two.json"
grep -F '"iron_sword.dps": 50.0' "${smoke_dir}/calculate-one.json" >/dev/null

"${tachiko_bin}" export "${buffed_project}" "${export_one}" \
  >"${smoke_dir}/export-one.txt"
"${tachiko_bin}" export "${buffed_project}" "${export_two}" \
  >"${smoke_dir}/export-two.txt"
cmp "${export_one}" "${export_two}"
grep -F '"dps": 50.0' "${export_one}" >/dev/null

if "${tachiko_bin}" set "${base_project}" iron_sword.attack_interval 0 \
  --output "${invalid_direct}" >"${smoke_dir}/invalid.txt" \
  2>"${smoke_dir}/invalid.err"; then
  echo "first-user smoke: invalid balance change unexpectedly persisted" >&2
  exit 1
fi
grep -F "divided by zero" "${smoke_dir}/invalid.err" >/dev/null
if [[ -e "${invalid_direct}" ]]; then
  echo "first-user smoke: rejected balance change created an output" >&2
  exit 1
fi

echo "first-user smoke passed: canonical project → edit → review → validate → export + local rejection"
