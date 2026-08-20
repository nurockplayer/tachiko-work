#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ -n "${TACHIKO_BIN:-}" ]]; then
  tachiko_bin="${TACHIKO_BIN}"
else
  cargo build --manifest-path "${repo_root}/Cargo.toml" -p tachiko-cli
  tachiko_bin="${repo_root}/target/debug/tachiko"
fi

smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-collaboration.XXXXXX")"
trap 'rm -rf "${smoke_dir}"' EXIT

base="${smoke_dir}/base.ro"
ours="${smoke_dir}/ours-damage.ro"
theirs="${smoke_dir}/theirs-attack-interval.ro"
merged="${smoke_dir}/merged.ro"

# Start every branch from the same canonical game-balance document.
"${tachiko_bin}" init "${base}" \
  --id game-balance \
  --title "Moonfall: starter balance" >"${smoke_dir}/init.txt"
"${tachiko_bin}" set "${base}" iron_sword.damage 45 \
  --output "${ours}" >"${smoke_dir}/ours-set.txt"
"${tachiko_bin}" set "${base}" iron_sword.attack_interval 0.8 \
  --output "${theirs}" >"${smoke_dir}/theirs-set.txt"

"${tachiko_bin}" merge "${base}" "${ours}" "${theirs}" \
  --output "${merged}" >"${smoke_dir}/merge.txt"
grep -F "damage: 36 -> 45" "${smoke_dir}/merge.txt" >/dev/null
grep -F "attack_interval: 0.9 -> 0.8" "${smoke_dir}/merge.txt" >/dev/null
grep -F "affected dps: 40 -> 56.25" "${smoke_dir}/merge.txt" >/dev/null

"${tachiko_bin}" validate "${merged}" >"${smoke_dir}/validate.txt"
"${tachiko_bin}" calculate "${merged}" >"${smoke_dir}/calculate.json"
grep -F '"iron_sword.damage": 45.0' "${smoke_dir}/calculate.json" >/dev/null
grep -F '"iron_sword.attack_interval": 0.8' "${smoke_dir}/calculate.json" >/dev/null
grep -F '"iron_sword.dps": 56.25' "${smoke_dir}/calculate.json" >/dev/null

"${tachiko_bin}" diff "${base}" "${merged}" >"${smoke_dir}/diff.txt"
grep -F "damage: 36 -> 45" "${smoke_dir}/diff.txt" >/dev/null
grep -F "attack_interval: 0.9 -> 0.8" "${smoke_dir}/diff.txt" >/dev/null
grep -F "affected dps: 40 -> 56.25" "${smoke_dir}/diff.txt" >/dev/null

echo "collaboration smoke passed: branch → semantic merge → validate → calculate → review"
