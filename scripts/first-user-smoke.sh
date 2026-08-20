#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ -n "${TACHIKO_BIN:-}" ]]; then
  tachiko_bin="${TACHIKO_BIN}"
else
  cargo build --manifest-path "${repo_root}/Cargo.toml" -p tachiko-cli
  tachiko_bin="${repo_root}/target/debug/tachiko"
fi

smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-first-user.XXXXXX")"
trap 'rm -rf "${smoke_dir}"' EXIT

base="${smoke_dir}/game-balance.ro"
buffed="${smoke_dir}/buffed-sword.ro"
export_one="${smoke_dir}/game-balance.json"
export_two="${smoke_dir}/game-balance-repeat.json"

"${tachiko_bin}" init "${base}" \
  --id game-balance \
  --title "Moonfall: starter balance" >"${smoke_dir}/init.txt"
cmp "${base}" "${repo_root}/examples/game-balance/game-balance.ro"

"${tachiko_bin}" show "${base}" >"${smoke_dir}/show.txt"
grep -F "weapons · Iron Sword [iron_sword]" "${smoke_dir}/show.txt" >/dev/null
grep -F "dps: 40 (formula)" "${smoke_dir}/show.txt" >/dev/null

"${tachiko_bin}" explain "${base}" iron_sword.dps >"${smoke_dir}/explain.txt"
grep -F "formula: (iron_sword.damage / iron_sword.attack_interval)" \
  "${smoke_dir}/explain.txt" >/dev/null

"${tachiko_bin}" set "${base}" iron_sword.damage 45 \
  --output "${buffed}" >"${smoke_dir}/set.txt"
grep -F "affected dps: 40 -> 50" "${smoke_dir}/set.txt" >/dev/null

"${tachiko_bin}" diff "${base}" "${buffed}" >"${smoke_dir}/diff.txt"
grep -F "damage: 36 -> 45" "${smoke_dir}/diff.txt" >/dev/null
grep -F "affected dps: 40 -> 50" "${smoke_dir}/diff.txt" >/dev/null

"${tachiko_bin}" validate "${buffed}" >"${smoke_dir}/validate.txt"
"${tachiko_bin}" calculate "${buffed}" >"${smoke_dir}/calculate-one.json"
"${tachiko_bin}" calculate "${buffed}" >"${smoke_dir}/calculate-two.json"
cmp "${smoke_dir}/calculate-one.json" "${smoke_dir}/calculate-two.json"
grep -F '"iron_sword.dps": 50.0' "${smoke_dir}/calculate-one.json" >/dev/null

"${tachiko_bin}" export "${buffed}" "${export_one}" >"${smoke_dir}/export-one.txt"
"${tachiko_bin}" export "${buffed}" "${export_two}" >"${smoke_dir}/export-two.txt"
cmp "${export_one}" "${export_two}"
grep -F '"dps": 50.0' "${export_one}" >/dev/null

echo "first-user smoke passed: create → explore → explain → edit → review → validate → export"
