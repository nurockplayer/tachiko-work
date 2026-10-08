#!/usr/bin/env bash
set -euo pipefail

# Existing exported kit must be built from the exact clean candidate. The caller
# owns the serial heavy slot, disk-backed cache/TMPDIR, and process memory guard.
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
kit="$(realpath "${1:?exact candidate kit required}")"
out="${2:?new absolute evidence directory required}"
v3_capture="$(realpath "${3:?same-candidate completed v3 capture required}")"
v3_run_id="${4:?exact v3 capture run UUID required}"
[[ "${out}" == /* && ! -e "${out}" ]]
: "${TMPDIR:?set a disk-backed TMPDIR before qualification}"
export CARGO_BUILD_JOBS=2
export RUST_TEST_THREADS=1
export PYTHONDONTWRITEBYTECODE=1
[[ -z "$(git -C "${repo_root}" status --porcelain --untracked-files=all)" ]] || {
  echo "Qualification requires a clean committed source checkout." >&2; exit 1;
}
source_commit="$(git -C "${repo_root}" rev-parse HEAD)"
WORK_CLIENT_KIT="${kit}" WORK_CORE_COMMIT="${source_commit}" pnpm --dir "${repo_root}/packages/browser-client" exec node "${repo_root}/tests/consumer-kit/seed/tests/kit.mjs"
mkdir "${out}"
git -C "${repo_root}" rev-parse HEAD > "${out}/source-commit.txt"
sha256sum "${kit}/artifact-manifest.json" > "${out}/kit-manifest.sha256"
# Existing589 seal contract: complete16 native +2 Worker cases, same exact
# candidate/run and immutable kit; no historic receipt or separate kit credit.
pnpm --dir "${repo_root}/packages/browser-client" exec node "${repo_root}/acceptance/date-definition-save-v3/verify-capture-seal.mjs" "${v3_capture}" "${source_commit}" "${v3_run_id}" "${kit}" > "${out}/v3-seal-before.json"
uv run --no-project "${repo_root}/acceptance/bounded-text-capacity/oracle.py" generate "${out}/fixtures"
uv run --no-project "${repo_root}/acceptance/bounded-text-capacity/resource_fixtures.py" "${out}/fixtures"
manifest="${repo_root}/packages/browser-client/runtime/Cargo.toml"
# Small sensor/receipt controls, no browser/product credit. Heavy stages remain
# serial and require the caller's original reference/resource/slot admission.
pnpm --dir "${repo_root}/packages/browser-client" exec node e2e/capacity-rss-controls.ts "${out}/rss-controls.json"
uv run --no-project "${repo_root}/acceptance/bounded-text-capacity/rss_oracle_controls.py" "${out}/rss-controls.json" > "${out}/rss-controls-result.json"

pnpm --dir "${repo_root}/packages/browser-client" exec node "${repo_root}/acceptance/bounded-text-capacity/boundaries.mjs" "${kit}/designer_runtime.wasm" "${out}/fixtures" "${out}/boundaries.json"
cargo run --release --locked --manifest-path "${manifest}" --example capacity_profile_boundaries -- all > "${out}/native-profile-boundaries.log" 2>&1
# Saved-import supplement: all seven groups include exact projections and the
# distinct 8,407-row metadata-aware atomic refusal; ordinary profile is separate.
cargo test --locked --manifest-path "${manifest}" --test saved_import_convergence > "${out}/saved-import-convergence.log" 2>&1
cargo run --release --locked --manifest-path "${manifest}" --example capacity_saved_open_boundaries -- all > "${out}/saved-open-boundaries.log" 2>&1
uv run --no-project "${repo_root}/acceptance/bounded-text-capacity/saved_import_oracle.py" generate "${out}/compact66-fixtures"
for shape in compact66 full8406; do
  fixtures="${out}/compact66-fixtures"; fixture_manifest="saved-import-manifest.json"
  if [[ "${shape}" == full8406 ]]; then fixtures="${out}/fixtures"; fixture_manifest="manifest.json"; fi
  pnpm --dir "${repo_root}/packages/browser-client" exec node e2e/saved-import-worker.mjs "${kit}" "${fixtures}" "${out}/saved-import-${shape}" "${fixture_manifest}"
  uv run --no-project "${repo_root}/acceptance/bounded-text-capacity/saved_import_oracle.py" verify "${out}/saved-import-${shape}" "${fixtures}/${fixture_manifest}"
done
# Serial: two actual save APIs, each complete import/history and fresh-process gate.
for carrier in opaque canonical; do
  cargo run --release --locked --manifest-path "${manifest}" --example bounded_text_capacity -- run "${out}/fixtures" "${out}/native-${carrier}" "${carrier}"
  cargo run --release --locked --manifest-path "${manifest}" --example bounded_text_capacity -- reopen "${out}/fixtures" "${out}/native-${carrier}" csv "${carrier}"
  cargo run --release --locked --manifest-path "${manifest}" --example bounded_text_capacity -- reopen "${out}/fixtures" "${out}/native-${carrier}" xlsx "${carrier}"
done
pnpm --dir "${repo_root}/packages/browser-client" exec node e2e/bounded-text-capacity.ts "${kit}" "${out}/fixtures" "${out}/worker"
uv run --no-project "${repo_root}/acceptance/bounded-text-capacity/oracle.py" verify "${out}"
[[ "$(git -C "${repo_root}" rev-parse HEAD)" == "${source_commit}" ]]
[[ -z "$(git -C "${repo_root}" status --porcelain --untracked-files=all)" ]]
WORK_CLIENT_KIT="${kit}" WORK_CORE_COMMIT="${source_commit}" pnpm --dir "${repo_root}/packages/browser-client" exec node "${repo_root}/tests/consumer-kit/seed/tests/kit.mjs"
sha256sum --check "${out}/kit-manifest.sha256"
pnpm --dir "${repo_root}/packages/browser-client" exec node "${repo_root}/acceptance/date-definition-save-v3/verify-capture-seal.mjs" "${v3_capture}" "${source_commit}" "${v3_run_id}" "${kit}" > "${out}/v3-seal-after.json"
cmp "${out}/v3-seal-before.json" "${out}/v3-seal-after.json"
