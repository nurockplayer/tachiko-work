#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ -n "${TACHIKO_BIN:-}" ]]; then
  tachiko_bin="${TACHIKO_BIN}"
else
  cargo build --manifest-path "${repo_root}/Cargo.toml" -p tachiko-cli
  tachiko_bin="${repo_root}/target/debug/tachiko"
fi

smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-entity-lifecycle.XXXXXX")"
trap 'rm -rf "${smoke_dir}"' EXIT

base="${smoke_dir}/base.ro"
duplicated="${smoke_dir}/duplicated.ro"
duplicate_repeat="${smoke_dir}/duplicate-repeat.ro"
named="${smoke_dir}/named.ro"
tuned="${smoke_dir}/tuned.ro"
renamed="${smoke_dir}/renamed.ro"
blocked="${smoke_dir}/blocked.ro"
pruned="${smoke_dir}/pruned.ro"
exported="${smoke_dir}/renamed.json"

"${tachiko_bin}" init "${base}" \
  --id game-balance \
  --title "Moonfall: growing roster" >"${smoke_dir}/init.txt"

"${tachiko_bin}" entity duplicate "${base}" iron_sword steel_sword \
  --output "${duplicated}" >"${smoke_dir}/duplicate.txt"
"${tachiko_bin}" entity duplicate "${base}" iron_sword steel_sword \
  --output "${duplicate_repeat}" >"${smoke_dir}/duplicate-repeat.txt"
grep -F "duplicated iron_sword as steel_sword" \
  "${smoke_dir}/duplicate.txt" >/dev/null
cmp "${duplicated}" "${duplicate_repeat}"

"${tachiko_bin}" set "${duplicated}" steel_sword.name "Steel Sword" \
  --output "${named}" >"${smoke_dir}/set-name.txt"
"${tachiko_bin}" set "${named}" steel_sword.damage 45 \
  --output "${tuned}" >"${smoke_dir}/set-damage.txt"

"${tachiko_bin}" entity rename "${tuned}" steel_sword moonblade \
  --output "${renamed}" >"${smoke_dir}/rename.txt"
grep -F "renamed steel_sword -> moonblade" "${smoke_dir}/rename.txt" >/dev/null

"${tachiko_bin}" validate "${renamed}" >"${smoke_dir}/validate-renamed.txt"
"${tachiko_bin}" explain "${renamed}" moonblade.dps >"${smoke_dir}/explain.txt"
grep -F "moonblade.dps = 50" "${smoke_dir}/explain.txt" >/dev/null
grep -F "formula: ([moonblade.damage] / [moonblade.attack_interval])" \
  "${smoke_dir}/explain.txt" >/dev/null
"${tachiko_bin}" calculate "${renamed}" >"${smoke_dir}/calculate.json"
grep -F '"moonblade.damage": 45.0' "${smoke_dir}/calculate.json" >/dev/null
grep -F '"moonblade.dps": 50.0' "${smoke_dir}/calculate.json" >/dev/null
"${tachiko_bin}" export "${renamed}" "${exported}" >"${smoke_dir}/export.txt"
grep -F '"moonblade"' "${exported}" >/dev/null

if "${tachiko_bin}" entity remove "${renamed}" iron_sword \
  --output "${blocked}" >"${smoke_dir}/remove-blocked.txt" \
  2>"${smoke_dir}/remove-blocked.err"; then
  echo "entity lifecycle smoke: referenced removal unexpectedly succeeded" >&2
  exit 1
fi
[[ ! -e "${blocked}" ]]
grep -F "alric.weapon" "${smoke_dir}/remove-blocked.err" >/dev/null
grep -F "shop.matches_for_sword" "${smoke_dir}/remove-blocked.err" >/dev/null
grep -F "tempered_blade.grants_weapon" "${smoke_dir}/remove-blocked.err" >/dev/null

"${tachiko_bin}" entity remove "${renamed}" moonblade \
  --output "${pruned}" >"${smoke_dir}/remove.txt"
grep -F "removed moonblade" "${smoke_dir}/remove.txt" >/dev/null
"${tachiko_bin}" validate "${pruned}" >"${smoke_dir}/validate-pruned.txt"
cmp "${base}" "${pruned}"

echo "entity lifecycle smoke passed: duplicate → tune → rename → explain → protect → remove → canonical round trip"
