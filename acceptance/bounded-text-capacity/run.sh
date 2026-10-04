#!/usr/bin/env bash
set -euo pipefail

# Existing exported kit must be built from the exact clean candidate. The caller
# owns the serial heavy slot, disk-backed cache/TMPDIR, and process memory guard.
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
kit="$(realpath "${1:?exact candidate kit required}")"
out="${2:?new absolute evidence directory required}"
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
uv run --no-project "${repo_root}/acceptance/bounded-text-capacity/oracle.py" generate "${out}/fixtures"
uv run --no-project "${repo_root}/acceptance/bounded-text-capacity/resource_fixtures.py" "${out}/fixtures"
manifest="${repo_root}/packages/browser-client/runtime/Cargo.toml"

pnpm --dir "${repo_root}/packages/browser-client" exec node "${repo_root}/acceptance/bounded-text-capacity/boundaries.mjs" "${kit}/designer_runtime.wasm" "${out}/fixtures" "${out}/boundaries.json"
cargo run --release --locked --manifest-path "${manifest}" --example capacity_profile_boundaries -- all > "${out}/native-profile-boundaries.log" 2>&1
# These are sequential OS processes. `run` must exit before either reopen.
cargo run --release --locked --manifest-path "${manifest}" --example bounded_text_capacity -- run "${out}/fixtures" "${out}/native"
cargo run --release --locked --manifest-path "${manifest}" --example bounded_text_capacity -- reopen "${out}/fixtures" "${out}/native" csv
cargo run --release --locked --manifest-path "${manifest}" --example bounded_text_capacity -- reopen "${out}/fixtures" "${out}/native" xlsx
pnpm --dir "${repo_root}/packages/browser-client" exec node e2e/bounded-text-capacity.ts "${kit}" "${out}/fixtures" "${out}/worker"
uv run --no-project "${repo_root}/acceptance/bounded-text-capacity/oracle.py" verify "${out}"
[[ "$(git -C "${repo_root}" rev-parse HEAD)" == "${source_commit}" ]]
[[ -z "$(git -C "${repo_root}" status --porcelain --untracked-files=all)" ]]
WORK_CLIENT_KIT="${kit}" WORK_CORE_COMMIT="${source_commit}" pnpm --dir "${repo_root}/packages/browser-client" exec node "${repo_root}/tests/consumer-kit/seed/tests/kit.mjs"
sha256sum --check "${out}/kit-manifest.sha256"
