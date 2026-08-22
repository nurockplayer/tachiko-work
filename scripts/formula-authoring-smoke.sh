#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ -n "${TACHIKO_BIN:-}" ]]; then
  tachiko_bin="${TACHIKO_BIN}"
else
  cargo build --manifest-path "${repo_root}/Cargo.toml" -p tachiko-cli
  tachiko_bin="${repo_root}/target/debug/tachiko"
fi

smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-formula-authoring.XXXXXX")"
trap 'rm -rf "${smoke_dir}"' EXIT

base="${smoke_dir}/base.ro"
authored="${smoke_dir}/authored.ro"
authored_repeat="${smoke_dir}/authored-repeat.ro"
exported="${smoke_dir}/authored.json"
expression="min(60, [iron_sword.damage] / [iron_sword.attack_interval] + 5)"

"${tachiko_bin}" init "${base}" \
  --id game-balance \
  --title "Moonfall: authored computation" >"${smoke_dir}/init.txt"

"${tachiko_bin}" formula set "${base}" iron_sword.dps \
  --expression "${expression}" --output "${authored}" \
  >"${smoke_dir}/formula-set.txt"
"${tachiko_bin}" formula set "${base}" iron_sword.dps \
  --expression "${expression}" --output "${authored_repeat}" \
  >"${smoke_dir}/formula-set-repeat.txt"
grep -F "affected dps: 40 -> 45" "${smoke_dir}/formula-set.txt" >/dev/null
cmp "${authored}" "${authored_repeat}"

"${tachiko_bin}" explain "${authored}" iron_sword.dps >"${smoke_dir}/explain.txt"
grep -F "iron_sword.dps = 45" "${smoke_dir}/explain.txt" >/dev/null
grep -F "formula: min(60, (([iron_sword.damage] / [iron_sword.attack_interval]) + 5))" \
  "${smoke_dir}/explain.txt" >/dev/null

"${tachiko_bin}" diff "${base}" "${authored}" >"${smoke_dir}/diff.txt"
grep -F "dps: ([iron_sword.damage] / [iron_sword.attack_interval])" \
  "${smoke_dir}/diff.txt" >/dev/null
grep -F "affected dps: 40 -> 45" "${smoke_dir}/diff.txt" >/dev/null
"${tachiko_bin}" validate "${authored}" >"${smoke_dir}/validate.txt"
"${tachiko_bin}" calculate "${authored}" >"${smoke_dir}/calculate.json"
grep -F '"iron_sword.dps": 45.0' "${smoke_dir}/calculate.json" >/dev/null
"${tachiko_bin}" export "${authored}" "${exported}" >"${smoke_dir}/export.txt"
grep -F '"dps": 45.0' "${exported}" >/dev/null

for failure in parse reference cycle; do
  failure_output="${smoke_dir}/${failure}.ro"
  case "${failure}" in
    parse)
      failure_expression="min(1,"
      expected="formula parse error at byte"
      ;;
    reference)
      failure_expression="[missing.damage]"
      expected="formula address '[missing.damage]' cannot be resolved"
      ;;
    cycle)
      failure_expression="[iron_sword.dps] + 1"
      expected="formula dependency cycle"
      ;;
  esac

  if "${tachiko_bin}" formula set "${base}" iron_sword.dps \
    --expression "${failure_expression}" --output "${failure_output}" \
    >"${smoke_dir}/${failure}.txt" 2>"${smoke_dir}/${failure}.err"; then
    echo "formula authoring smoke: ${failure} failure unexpectedly succeeded" >&2
    exit 1
  fi
  [[ ! -e "${failure_output}" ]]
  grep -F "${expected}" "${smoke_dir}/${failure}.err" >/dev/null
done

echo "formula authoring smoke passed: parse → validate → calculate → explain → review → export → reject safely"
