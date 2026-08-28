#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ -n "${TACHIKO_BIN:-}" ]]; then
  tachiko_bin="${TACHIKO_BIN}"
else
  cargo build --manifest-path "${repo_root}/Cargo.toml" -p tachiko-cli
  tachiko_bin="${repo_root}/target/debug/tachiko"
fi

if ! command -v git >/dev/null 2>&1; then
  echo "git-ci-smoke: ordinary Git is required for the optional adapter journey" >&2
  exit 1
fi

smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-git-ci.XXXXXX")"
cleanup() {
  rm -rf -- "${smoke_dir}"
}
trap cleanup EXIT

outside_dir="${smoke_dir}/outside"
repository_dir="${smoke_dir}/repository"
mkdir "${outside_dir}" "${repository_dir}"

base_direct="${outside_dir}/base.ro"
buffed_direct="${outside_dir}/buffed.ro"
base_project="${outside_dir}/base.roproj"
buffed_project="${outside_dir}/buffed.roproj"
base_package="${outside_dir}/base-package.ro"

"${tachiko_bin}" init "${base_direct}" \
  --id game-balance --title "Moonfall: starter balance" >/dev/null
"${tachiko_bin}" set "${base_direct}" iron_sword.damage 45 \
  --output "${buffed_direct}" >/dev/null
"${tachiko_bin}" roproj materialize "${base_direct}" "${base_project}" >/dev/null
"${tachiko_bin}" roproj materialize "${buffed_direct}" "${buffed_project}" >/dev/null
"${tachiko_bin}" roproj pack "${base_project}" "${base_package}" >/dev/null

"${tachiko_bin}" validate "${base_project}" >/dev/null
"${tachiko_bin}" validate "${buffed_project}" >/dev/null
"${tachiko_bin}" diff "${base_project}" "${buffed_project}" \
  >"${smoke_dir}/outside-diff.txt"
"${tachiko_bin}" analyze changes "${base_project}" "${buffed_project}" \
  --before-state base --after-state working \
  >"${smoke_dir}/outside-analysis.json"
"${tachiko_bin}" analyze validation "${buffed_project}" \
  --source-state working >"${smoke_dir}/outside-validation.json"

git -C "${repository_dir}" init --quiet
cp "${repo_root}/.gitattributes" "${repository_dir}/.gitattributes"
cp -R "${base_project}" "${repository_dir}/game-balance.roproj"
cp "${base_package}" "${repository_dir}/game-balance.ro"
git -C "${repository_dir}" add .gitattributes game-balance.roproj game-balance.ro
git -C "${repository_dir}" \
  -c user.name="Tachiko CI" -c user.email="ci@tachiko.invalid" \
  commit --quiet -m "Track canonical balance project"
git -C "${repository_dir}" switch --quiet -c buffed-sword

rm -rf -- "${repository_dir}/game-balance.roproj"
cp -R "${buffed_project}" "${repository_dir}/game-balance.roproj"

git -C "${repository_dir}" diff --check
git -C "${repository_dir}" diff --numstat -- game-balance.roproj \
  >"${smoke_dir}/raw-numstat.txt"
if [[ "$(wc -l <"${smoke_dir}/raw-numstat.txt" | tr -d ' ')" != "1" ]]; then
  echo "git-ci-smoke: scalar edit must change exactly one project path" >&2
  cat "${smoke_dir}/raw-numstat.txt" >&2
  exit 1
fi
IFS=$'\t' read -r added_lines deleted_lines changed_path \
  <"${smoke_dir}/raw-numstat.txt"
if [[ "${added_lines}" != "1" || "${deleted_lines}" != "1" || \
  ! "${changed_path}" =~ ^game-balance\.roproj/entities/[0-9a-f]\.jsonl$ ]]; then
  echo "git-ci-smoke: expected one added/deleted record in one canonical shard" >&2
  cat "${smoke_dir}/raw-numstat.txt" >&2
  exit 1
fi

git -C "${repository_dir}" check-attr text eol diff -- "${changed_path}" \
  >"${smoke_dir}/attributes.txt"
grep -F "${changed_path}: text: set" "${smoke_dir}/attributes.txt" >/dev/null
grep -F "${changed_path}: eol: lf" "${smoke_dir}/attributes.txt" >/dev/null
grep -F "${changed_path}: diff: set" "${smoke_dir}/attributes.txt" >/dev/null

git -C "${repository_dir}" diff --no-ext-diff --no-textconv -- game-balance.roproj \
  >"${smoke_dir}/raw-diff.txt"
grep -F '"key":"iron_sword"' "${smoke_dir}/raw-diff.txt" >/dev/null
grep -F '"value":36' "${smoke_dir}/raw-diff.txt" >/dev/null
grep -F '"value":45' "${smoke_dir}/raw-diff.txt" >/dev/null

tracked_project="${repository_dir}/game-balance.roproj"
tracked_package="${repository_dir}/game-balance.ro"
"${tachiko_bin}" validate "${tracked_project}" >/dev/null
"${tachiko_bin}" diff "${base_project}" "${tracked_project}" \
  >"${smoke_dir}/inside-diff.txt"
"${tachiko_bin}" analyze changes "${base_project}" "${tracked_project}" \
  --before-state base --after-state working \
  >"${smoke_dir}/inside-analysis.json"
"${tachiko_bin}" analyze validation "${tracked_project}" \
  --source-state working >"${smoke_dir}/inside-validation.json"
cmp "${smoke_dir}/outside-diff.txt" "${smoke_dir}/inside-diff.txt"
cmp "${smoke_dir}/outside-analysis.json" "${smoke_dir}/inside-analysis.json"
cmp "${smoke_dir}/outside-validation.json" "${smoke_dir}/inside-validation.json"

git -C "${repository_dir}" diff --binary -- game-balance.roproj game-balance.ro \
  >"${smoke_dir}/before-mismatch.diff"
if "${tachiko_bin}" roproj compare-package "${tracked_package}" "${tracked_project}" \
  >"${smoke_dir}/stale-package.txt" 2>"${smoke_dir}/stale-package.err"; then
  echo "git-ci-smoke: a stale tracked package unexpectedly matched its source" >&2
  exit 1
fi
grep -F "portable_package.source_mismatch" "${smoke_dir}/stale-package.err" >/dev/null
cmp "${base_package}" "${tracked_package}"
git -C "${repository_dir}" diff --binary -- game-balance.roproj game-balance.ro \
  >"${smoke_dir}/after-mismatch.diff"
cmp "${smoke_dir}/before-mismatch.diff" "${smoke_dir}/after-mismatch.diff"

current_package="${smoke_dir}/current-package.ro"
"${tachiko_bin}" roproj pack "${tracked_project}" "${current_package}" >/dev/null
"${tachiko_bin}" roproj compare-package "${current_package}" "${tracked_project}" >/dev/null
cp "${current_package}" "${tracked_package}"
"${tachiko_bin}" roproj compare-package "${tracked_package}" "${tracked_project}" >/dev/null

noncanonical_project="${smoke_dir}/noncanonical.roproj"
cp -R "${tracked_project}" "${noncanonical_project}"
: >"${noncanonical_project}/entities/extra.jsonl"
if "${tachiko_bin}" validate "${noncanonical_project}" \
  >"${smoke_dir}/noncanonical.txt" 2>"${smoke_dir}/noncanonical.err"; then
  echo "git-ci-smoke: noncanonical project unexpectedly passed CI validation" >&2
  exit 1
fi
grep -F "representation" "${smoke_dir}/noncanonical.err" >/dev/null

invalid_project="${smoke_dir}/invalid.roproj"
cp -R "${tracked_project}" "${invalid_project}"
invalid_shard="$(grep -l '"key":"iron_sword"' "${invalid_project}"/entities/*.jsonl)"
sed 's/"value":0.9/"value":0/' "${invalid_shard}" >"${invalid_shard}.tmp"
mv "${invalid_shard}.tmp" "${invalid_shard}"
if "${tachiko_bin}" validate "${invalid_project}" \
  >"${smoke_dir}/invalid.txt" 2>"${smoke_dir}/invalid.err"; then
  echo "git-ci-smoke: semantically invalid project unexpectedly passed CI validation" >&2
  exit 1
fi
grep -F "divided by zero" "${smoke_dir}/invalid.err" >/dev/null

git -C "${repository_dir}" add game-balance.roproj game-balance.ro
git -C "${repository_dir}" \
  -c user.name="Tachiko CI" -c user.email="ci@tachiko.invalid" \
  commit --quiet -m "Buff Iron Sword"
if [[ -n "$(git -C "${repository_dir}" status --porcelain)" ]]; then
  echo "git-ci-smoke: committed adapter journey left an unexpected working-tree change" >&2
  exit 1
fi

echo "git/CI smoke passed: localized raw diff + standalone semantic review + canonical validation + package consistency"
