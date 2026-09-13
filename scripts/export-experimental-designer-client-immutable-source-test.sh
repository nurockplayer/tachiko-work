#!/usr/bin/env bash
#
# This is a hermetic regression for Issue #359's immutable-source and
# absent-only-publication contract. It runs the real exporter script copied
# into a throwaway Git repository, while its fake pnpm makes emitted JS expose
# the source bytes from which the exporter actually built. It is intentionally
# not a substitute for the real build/runtime qualification.
set -euo pipefail

# TACHIKO_EXPORTER_UNDER_TEST selects an alternative exporter path; otherwise
# this test runs the repository exporter beside it.
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exporter_source="${TACHIKO_EXPORTER_UNDER_TEST:-${repo_root}/scripts/export-experimental-designer-client.sh}"
[[ -f "${exporter_source}" ]] || {
  echo "immutable-source test: exporter is missing: ${exporter_source}" >&2
  exit 1
}
producer_root="$(cd "$(dirname "${exporter_source}")/.." && pwd)"

test_root="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-359-immutable-source.XXXXXX")"
cleanup() { rm -rf -- "${test_root}"; }
trap cleanup EXIT
controlled_tmp="${test_root}/controlled-tmp"
controlled_tmp_link="${test_root}/controlled-tmp-link"
mkdir "${controlled_tmp}"
ln -s "${controlled_tmp}" "${controlled_tmp_link}"
controlled_tmp_physical="$(cd "${controlled_tmp_link}" && pwd -P)"

fail() { echo "immutable-source test: $*" >&2; exit 1; }
fixture="${test_root}/fixture"
fake_bin="${test_root}/bin"
mkdir -p "${fixture}/scripts" \
  "${fixture}/apps/designer/src" \
  "${fixture}/apps/designer/public" \
  "${fixture}/apps/designer/experimental-client-kit" \
  "${fake_bin}"
cp "${exporter_source}" "${fixture}/scripts/export-experimental-designer-client.sh"
chmod +x "${fixture}/scripts/export-experimental-designer-client.sh"
# Keep the actual source-capture/package/publication path under test. Only the
# expensive pnpm/Rust build path is replaced below. These helpers are part of
# the proposed #359 exporter implementation and must stay in sync with it.
for helper in \
  scripts/designer-rc-source.sh \
  scripts/package-experimental-designer-client.sh \
  scripts/experimental-designer-client-publish.c; do
  [[ -f "${producer_root}/${helper}" ]] || fail "exporter helper is missing: ${producer_root}/${helper}"
  cp "${producer_root}/${helper}" "${fixture}/${helper}"
done
chmod +x "${fixture}/scripts/designer-rc-source.sh" "${fixture}/scripts/package-experimental-designer-client.sh"

# The real exporter must invoke this only from its pinned private snapshot. If
# it instead compiles the live checkout, the test's same-path edits would leak
# into experimental-client.js and fail the committed-byte assertion below.
cat >"${fixture}/scripts/designer-runtime-build.sh" <<'EOF_BUILD'
#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "${repo_root}/apps/designer/public"
printf 'committed wasm\n' >"${repo_root}/apps/designer/public/designer_runtime.wasm"
if [[ -f "${TMPDIR}/race-output" ]]; then
  race_output="$(cat "${TMPDIR}/race-output")"
  mkdir "${race_output}"
  printf 'race sentinel\n' >"${race_output}/race-sentinel.txt"
  rm -- "${TMPDIR}/race-output"
fi
EOF_BUILD
chmod +x "${fixture}/scripts/designer-runtime-build.sh"

cat >"${fake_bin}/pnpm" <<'EOF_PNPM'
#!/usr/bin/env bash
set -euo pipefail
designer=""
if [[ "${1:-}" == "--dir" ]]; then
  designer="$2"
  shift 2
fi
[[ -n "${designer}" ]] || designer="$(pwd)"
case "${1:-}" in
  --version)
    printf '%s\n' '11.25.0'
    ;;
  install)
    # The fixture has no dependencies. The exporter still exercises its real
    # frozen-install command shape. An optional mutation happens only here:
    # after exporter preflight/archive but before compilation. It replaces a
    # same live path with a symlink; a safe exporter must still emit the bytes
    # already captured in its private snapshot.
    if [[ -f "${TMPDIR}/mutate-live-source" ]]; then
      live_source="$(cat "${TMPDIR}/mutate-live-source")"
      rm -- "${live_source}"
      ln -s "${TMPDIR}/live-replacement.ts" "${live_source}"
      rm -- "${TMPDIR}/mutate-live-source"
    fi
    ;;
  exec)
    shift
    [[ "${1:-}" == "tsc" ]] || { echo "unexpected pnpm exec: $*" >&2; exit 64; }
    shift
    out=''
    while [[ "$#" -gt 0 ]]; do
      if [[ "$1" == "--outDir" ]]; then out="$2"; shift 2; continue; fi
      shift
    done
    [[ -n "${out}" ]] || { echo 'fake tsc missing --outDir' >&2; exit 64; }
    mkdir -p "${out}/host" "${out}/runtime"
    printf '%s\n' "${designer}" >>"${TMPDIR}/designer-paths"
    # This is the source-byte oracle: the committed first line must be emitted.
    cp "${designer}/src/experimental-client.ts" "${out}/experimental-client.js"
    cp "${designer}/src/experimental-client.worker.ts" "${out}/experimental-client.worker.js"
    for file in experimental-client.d.ts experimental-client.worker.d.ts \
      host/project-transfer.d.ts host/project-transfer.js \
      runtime/client.d.ts runtime/client.js runtime/interop-protocol.d.ts runtime/interop-protocol.js \
      runtime/protocol.d.ts runtime/protocol.js runtime/wasm-bridge.d.ts runtime/wasm-bridge.js \
      runtime/worker-client.d.ts runtime/worker-client.js runtime/worker-runtime.d.ts runtime/worker-runtime.js; do
      printf '// fixture %s\n' "${file}" >"${out}/${file}"
    done
    ;;
  *)
    echo "unexpected fake pnpm invocation: $*" >&2
    exit 64
    ;;
esac
EOF_PNPM
chmod +x "${fake_bin}/pnpm"

cat >"${fixture}/apps/designer/src/experimental-client.ts" <<'EOF_CLIENT'
// COMMITTED_CLIENT_SOURCE
export const sourceIdentity = 'committed';
EOF_CLIENT
printf "// COMMITTED_WORKER_SOURCE\n" >"${fixture}/apps/designer/src/experimental-client.worker.ts"
printf '{"name":"fixture-designer","private":true}\n' >"${fixture}/apps/designer/package.json"
printf 'fixture tsconfig\n' >"${fixture}/apps/designer/tsconfig.experimental-client.json"
printf 'lockfileVersion: 9\n' >"${fixture}/apps/designer/pnpm-lock.yaml"
printf '# fixture README\n' >"${fixture}/apps/designer/experimental-client-kit/README.md"
printf '{"private":true}\n' >"${fixture}/apps/designer/experimental-client-kit/package.json"
printf 'Apache-2.0 fixture notice\n' >"${fixture}/LICENSE-APACHE"
printf 'MIT fixture notice\n' >"${fixture}/LICENSE-MIT"
printf 'third party fixture notice\n' >"${fixture}/THIRD_PARTY_LICENSES.md"

git -C "${fixture}" init --quiet
git -C "${fixture}" config user.email 'fixture@example.invalid'
git -C "${fixture}" config user.name 'Issue 359 fixture'
git -C "${fixture}" remote add origin 'https://github.com/nurockplayer/tachiko-work.git'
git -C "${fixture}" add .
git -C "${fixture}" commit --quiet -m 'committed exporter fixture'
source_commit="$(git -C "${fixture}" rev-parse HEAD)"
owned_slot="${controlled_tmp_physical}/tachiko-experimental-client.uid-$(id -u).source-${source_commit}"

run_export() {
  TMPDIR="${controlled_tmp_link}" PATH="${fake_bin}:${PATH}" /bin/bash "${fixture}/scripts/export-experimental-designer-client.sh" "$1"
}

assert_owned_slot_absent() {
  [[ ! -e "${owned_slot}" && ! -L "${owned_slot}" ]] ||
    fail "exporter cleanup retained owned scratch slot: ${owned_slot}"
}

assert_no_output() {
  [[ ! -e "$1" ]] || fail "failed source admission mutated output: $1"
}

assert_unchanged_file() {
  [[ "$(cat "$1")" == "$2" ]] || fail "existing destination was replaced or modified: $1"
}

assert_manifest_and_exact_inventory() {
  local output="$1"
  TEST_OUTPUT="${output}" TEST_SOURCE_COMMIT="${source_commit}" node --input-type=module <<'EOF_NODE'
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {lstat,readFile,readdir} from 'node:fs/promises';
import path from 'node:path';

const root=process.env.TEST_OUTPUT;
const expectedCommit=process.env.TEST_SOURCE_COMMIT;
const manifest=JSON.parse(await readFile(path.join(root,'artifact-manifest.json'),'utf8'));
assert.equal(manifest.sourceRepository,'nurockplayer/tachiko-work');
assert.equal(manifest.sourceCommit,expectedCommit);
assert.equal(manifest.stability,'experimental');
assert.equal(manifest.entry,'experimental-client.js');
assert.ok(Array.isArray(manifest.files)&&manifest.files.length>0,'complete file inventory is required');
assert.deepEqual(manifest.licenseNotices,[
  'notices/LICENSE-APACHE',
  'notices/LICENSE-MIT',
  'notices/THIRD_PARTY_LICENSES.md',
]);
const declared=new Map();
for(const file of manifest.files){
  assert.equal(typeof file.path,'string');
  assert.match(file.sha256,/^[a-f0-9]{64}$/);
  assert.ok(!declared.has(file.path),`duplicate manifest path: ${file.path}`);
  declared.set(file.path,file.sha256);
  const absolute=path.join(root,file.path);
  assert.equal((await lstat(absolute)).isSymbolicLink(),false,`artifact symlink: ${file.path}`);
  assert.equal(createHash('sha256').update(await readFile(absolute)).digest('hex'),file.sha256,`digest: ${file.path}`);
}
const actual=[];
async function walk(dir,prefix=''){
  for(const entry of await readdir(dir,{withFileTypes:true})){
    assert.equal(entry.isSymbolicLink(),false,`artifact symlink: ${prefix}${entry.name}`);
    const relative=prefix+entry.name;
    if(entry.isDirectory()) await walk(path.join(dir,entry.name),relative+'/');
    else actual.push(relative);
  }
}
await walk(root);
assert.deepEqual(actual.filter(file=>file!=='artifact-manifest.json').sort(),[...declared.keys()].sort());
for(const notice of manifest.licenseNotices) assert.ok(declared.has(notice),`notice not declared: ${notice}`);
const emitted=await readFile(path.join(root,'experimental-client.js'),'utf8');
assert.match(emitted,/COMMITTED_CLIENT_SOURCE/,'output leaked a same-path working-tree replacement');
assert.doesNotMatch(emitted,/DIRTY_CLIENT_SOURCE|STAGED_CLIENT_SOURCE|SYMLINK_CLIENT_SOURCE/);
EOF_NODE
}

# An occupied deterministic slot is never reusable or deletable. Check both a
# directory and a symlink before any successful export claims its own slot.
mkdir "${owned_slot}"
printf 'occupied directory sentinel\n' >"${owned_slot}/sentinel.txt"
occupied_slot_log="${test_root}/occupied-slot.log"
if run_export "${test_root}/occupied-slot-kit" >"${occupied_slot_log}" 2>&1; then
  fail 'occupied scratch directory was accepted'
fi
grep -F 'scratch slot is occupied for uid' "${occupied_slot_log}" >/dev/null ||
  fail 'occupied scratch directory did not report a non-reuse refusal'
assert_unchanged_file "${owned_slot}/sentinel.txt" 'occupied directory sentinel'
rm -rf -- "${owned_slot}"

slot_symlink_target="${test_root}/occupied-slot-target"
printf 'occupied symlink sentinel\n' >"${slot_symlink_target}"
ln -s "${slot_symlink_target}" "${owned_slot}"
occupied_symlink_log="${test_root}/occupied-slot-symlink.log"
if run_export "${test_root}/occupied-slot-symlink-kit" >"${occupied_symlink_log}" 2>&1; then
  fail 'occupied scratch symlink was accepted'
fi
grep -F 'scratch slot is occupied for uid' "${occupied_symlink_log}" >/dev/null ||
  fail 'occupied scratch symlink did not report a non-reuse refusal'
[[ -L "${owned_slot}" ]] || fail 'occupied scratch symlink was replaced or removed'
assert_unchanged_file "${slot_symlink_target}" 'occupied symlink sentinel'
rm -- "${owned_slot}"

# The package helper must reject any output whose resolved parent or destination
# lies inside the slot that the exporter owns and removes at exit.
inside_slot_output="${owned_slot}/nested-kit"
inside_slot_log="${test_root}/inside-slot.log"
if run_export "${inside_slot_output}" >"${inside_slot_log}" 2>&1; then
  fail 'output nested inside the owned scratch slot was accepted'
fi
grep -F 'output parent or destination resolves inside owned scratch slot' "${inside_slot_log}" >/dev/null ||
  fail 'nested output did not report the owned-slot rejection'
assert_no_output "${inside_slot_output}"
assert_owned_slot_absent

# Two same-SHA exports must materialize at the same physical path even though
# TMPDIR is supplied through a symlink. The final artifact diff remains the
# integration proof; this is only the focused path-identity unit assertion.
rm -f -- "${controlled_tmp}/designer-paths"
clean_output="${test_root}/clean-kit"
stable_output="${test_root}/stable-kit"
run_export "${clean_output}"
assert_manifest_and_exact_inventory "${clean_output}"
assert_owned_slot_absent
run_export "${stable_output}"
assert_manifest_and_exact_inventory "${stable_output}"
assert_owned_slot_absent
materialized_path_count="$(wc -l <"${controlled_tmp}/designer-paths" | tr -d ' ')"
[[ "${materialized_path_count}" == 2 ]] ||
  fail "expected two materialized designer paths, got ${materialized_path_count}"
first_materialized_designer_path="$(sed -n '1p' "${controlled_tmp}/designer-paths")"
second_materialized_designer_path="$(sed -n '2p' "${controlled_tmp}/designer-paths")"
expected_designer_path="${owned_slot}/source/apps/designer"
[[ "${first_materialized_designer_path}" == "${expected_designer_path}" && "${second_materialized_designer_path}" == "${expected_designer_path}" ]] ||
  fail 'same-SHA exports did not use one stable physical source path'

# This changes the live tracked path only after the exporter has completed its
# clean preflight and archived the commit. The successful kit must keep the
# already captured committed bytes and source SHA; inspecting only pre-existing
# dirty input would miss this time-of-check/time-of-use pressure.
printf "// SYMLINK_CLIENT_SOURCE\n" >"${controlled_tmp}/live-replacement.ts"
printf '%s\n' "${fixture}/apps/designer/src/experimental-client.ts" >"${controlled_tmp}/mutate-live-source"
inflight_output="${test_root}/inflight-mutation-kit"
run_export "${inflight_output}"
[[ -L "${fixture}/apps/designer/src/experimental-client.ts" ]] || fail 'fixture did not perform the in-flight same-path symlink replacement'
assert_manifest_and_exact_inventory "${inflight_output}"
rm -- "${fixture}/apps/designer/src/experimental-client.ts"
git -C "${fixture}" restore apps/designer/src/experimental-client.ts

# Each live-tree mutation must be rejected before it can create an output kit.
printf "// DIRTY_CLIENT_SOURCE\n" >"${fixture}/apps/designer/src/experimental-client.ts"
if run_export "${test_root}/dirty-kit"; then fail 'unstaged same-path source edit was accepted'; fi
assert_no_output "${test_root}/dirty-kit"
git -C "${fixture}" restore apps/designer/src/experimental-client.ts

printf "// STAGED_CLIENT_SOURCE\n" >"${fixture}/apps/designer/src/experimental-client.ts"
git -C "${fixture}" add apps/designer/src/experimental-client.ts
if run_export "${test_root}/staged-kit"; then fail 'staged same-path source edit was accepted'; fi
assert_no_output "${test_root}/staged-kit"
git -C "${fixture}" restore --staged apps/designer/src/experimental-client.ts
git -C "${fixture}" restore apps/designer/src/experimental-client.ts

printf "// UNTRACKED_CLIENT_SOURCE\n" >"${fixture}/apps/designer/src/untracked-local.ts"
if run_export "${test_root}/untracked-kit"; then fail 'untracked source data was accepted'; fi
assert_no_output "${test_root}/untracked-kit"
rm -- "${fixture}/apps/designer/src/untracked-local.ts"

printf "// SYMLINK_CLIENT_SOURCE\n" >"${test_root}/symlink-source.ts"
rm -- "${fixture}/apps/designer/src/experimental-client.ts"
ln -s "${test_root}/symlink-source.ts" "${fixture}/apps/designer/src/experimental-client.ts"
if run_export "${test_root}/symlink-kit"; then fail 'live symlink replacement was accepted'; fi
assert_no_output "${test_root}/symlink-kit"
rm -- "${fixture}/apps/designer/src/experimental-client.ts"
git -C "${fixture}" restore apps/designer/src/experimental-client.ts

# Publication is absent-only: an empty directory is still an existing target,
# and failure may not overwrite a sentinel in a nonempty target.
occupied_output="${test_root}/occupied-kit"
mkdir "${occupied_output}"
if run_export "${occupied_output}"; then fail 'empty existing target was reused'; fi
[[ -d "${occupied_output}" && -z "$(find "${occupied_output}" -mindepth 1 -print -quit)" ]] || fail 'empty target was mutated'

sentinel_output="${test_root}/sentinel-kit"
mkdir "${sentinel_output}"
printf 'do not replace\n' >"${sentinel_output}/sentinel.txt"
if run_export "${sentinel_output}"; then fail 'existing nonempty target was overwritten'; fi
assert_unchanged_file "${sentinel_output}/sentinel.txt" 'do not replace'

# The destination can appear after preflight and while the stage is built. The
# fake installer is deliberately not used for this: a source-derived build is
# allowed to mutate only its private snapshot, while the fake runtime build
# injects the external destination immediately before the no-replace publish.
race_output="${test_root}/race-kit"
printf '%s\n' "${race_output}" >"${controlled_tmp}/race-output"
if run_export "${race_output}"; then fail 'destination publication race was accepted'; fi
rm -f -- "${controlled_tmp}/race-output"
assert_unchanged_file "${race_output}/race-sentinel.txt" 'race sentinel'
[[ "$(find "${race_output}" -mindepth 1 -maxdepth 1 -print | wc -l | tr -d ' ')" == 1 ]] || fail 'race destination gained nested or partial kit files'
assert_owned_slot_absent

echo "Issue #359 immutable source and absent-only exporter regression passed"
