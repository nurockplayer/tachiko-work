#!/usr/bin/env bash
set -euo pipefail

fail() {
  echo "package-experimental-designer-client: $*" >&2
  exit 1
}

if [[ "$#" -ne 3 ]]; then
  fail "internal usage: OUTPUT_DIRECTORY SOURCE_COMMIT SCRATCH_DIRECTORY"
fi

output_arg="$1"
source_commit="$2"
scratch="$3"
[[ "${source_commit}" =~ ^[0-9a-f]{40}$ ]] || fail "SOURCE_COMMIT must be a full SHA-1 commit ID"
[[ -d "${scratch}" && ! -L "${scratch}" ]] || fail "SCRATCH_DIRECTORY must be a real directory"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source_root="$(cd "${script_dir}/.." && pwd)"
designer_dir="${source_root}/apps/designer"

if [[ "${output_arg}" == /* ]]; then
  output_dir="${output_arg}"
else
  output_dir="${PWD}/${output_arg}"
fi
output_parent_arg="$(dirname "${output_dir}")"
output_name="$(basename "${output_dir}")"
[[ "${output_name}" != "." && "${output_name}" != ".." ]] ||
  fail "output must name a dedicated kit directory"
mkdir -p "${output_parent_arg}"
output_parent="$(cd "${output_parent_arg}" && pwd -P)" || fail "could not resolve output parent"
output_dir="${output_parent}/${output_name}"
[[ ! -e "${output_dir}" && ! -L "${output_dir}" ]] ||
  fail "output must be absent: ${output_dir}"

for required in \
  "${designer_dir}/package.json" \
  "${designer_dir}/pnpm-lock.yaml" \
  "${designer_dir}/tsconfig.experimental-client.json" \
  "${designer_dir}/experimental-client-kit/README.md" \
  "${designer_dir}/experimental-client-kit/package.json" \
  "${source_root}/LICENSE-APACHE" \
  "${source_root}/LICENSE-MIT" \
  "${source_root}/THIRD_PARTY_LICENSES.md" \
  "${source_root}/scripts/experimental-designer-client-publish.c"; do
  [[ -f "${required}" && ! -L "${required}" ]] || fail "captured source is missing a required regular file: ${required}"
done

# Keep package-manager and Cargo configuration outside the captured source
# explicit. The project pins pnpm; Cargo still builds from the captured tree.
clean_env=(env -i "PATH=${PATH}")
if [[ "${HOME+x}" == x ]]; then clean_env+=("HOME=${HOME}"); fi
if [[ "${TMPDIR+x}" == x ]]; then clean_env+=("TMPDIR=${TMPDIR}"); fi
if [[ "${RUSTUP_HOME+x}" == x ]]; then clean_env+=("RUSTUP_HOME=${RUSTUP_HOME}"); fi
clean_env+=(
  "RUSTUP_TOOLCHAIN=stable"
  "CARGO_HOME=${scratch}/cargo-home"
  "NPM_CONFIG_USERCONFIG=/dev/null"
  "NPM_CONFIG_GLOBALCONFIG=/dev/null"
  "XDG_CONFIG_HOME=${scratch}/config-home"
)
mkdir "${scratch}/cargo-home" "${scratch}/config-home"

pnpm_version="$(cd "${designer_dir}" && "${clean_env[@]}" pnpm --version)"
[[ "${pnpm_version}" == "11.25.0" ]] || fail "pnpm 11.25.0 is required (found ${pnpm_version})"

stage_parent="$(mktemp -d "${output_parent}/.tachiko-experimental-client-stage.XXXXXX")"
cleanup_stage() { rm -rf -- "${stage_parent}"; }
trap cleanup_stage EXIT
kit_dir="${stage_parent}/kit"
mkdir "${kit_dir}"

(cd "${designer_dir}" && "${clean_env[@]}" pnpm install --frozen-lockfile --engine-strict)
env "${clean_env[@]:1}" bash "${source_root}/scripts/designer-runtime-build.sh"
(cd "${designer_dir}" && "${clean_env[@]}" pnpm exec tsc \
  --project tsconfig.experimental-client.json \
  --outDir "${kit_dir}" \
  --pretty false)

cp "${designer_dir}/public/designer_runtime.wasm" "${kit_dir}/designer_runtime.wasm"
cp "${designer_dir}/experimental-client-kit/README.md" "${kit_dir}/README.md"
cp "${designer_dir}/experimental-client-kit/package.json" "${kit_dir}/package.json"
mkdir "${kit_dir}/notices"
for notice in LICENSE-APACHE LICENSE-MIT THIRD_PARTY_LICENSES.md; do
  cp "${source_root}/${notice}" "${kit_dir}/notices/${notice}"
done

node - "${kit_dir}" "${source_commit}" <<'NODE'
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const [kitDir, sourceCommit] = process.argv.slice(2);
const files = [];
function walk(relative) {
  const absolute = path.join(kitDir, relative);
  for (const entry of fs.readdirSync(absolute, { withFileTypes: true })) {
    const child = relative ? path.join(relative, entry.name) : entry.name;
    const childAbsolute = path.join(kitDir, child);
    const stat = fs.lstatSync(childAbsolute);
    if (stat.isSymbolicLink()) throw new Error(`kit contains symlink: ${child}`);
    if (stat.isDirectory()) walk(child);
    else if (stat.isFile()) files.push({
      path: child.split(path.sep).join("/"),
      sha256: crypto.createHash("sha256").update(fs.readFileSync(childAbsolute)).digest("hex"),
    });
    else throw new Error(`kit contains unsupported entry: ${child}`);
  }
}
walk("");
files.sort((left, right) => left.path < right.path ? -1 : left.path > right.path ? 1 : 0);
const required = ["experimental-client.js", "experimental-client.d.ts", "experimental-client.worker.js", "designer_runtime.wasm"];
for (const file of required) {
  if (!files.some(candidate => candidate.path === file)) throw new Error(`kit is missing required asset: ${file}`);
}
const licenseNotices = [
  "notices/LICENSE-APACHE",
  "notices/LICENSE-MIT",
  "notices/THIRD_PARTY_LICENSES.md",
];
for (const notice of licenseNotices) {
  if (!files.some(candidate => candidate.path === notice)) throw new Error(`kit is missing notice: ${notice}`);
}
fs.writeFileSync(path.join(kitDir, "artifact-manifest.json"), `${JSON.stringify({
  schema: "tachiko-experimental-designer-client-kit-manifest-v1",
  sourceRepository: "nurockplayer/tachiko-work",
  sourceCommit,
  stability: "experimental",
  entry: "experimental-client.js",
  capabilities: [
    "createExperimentalDesignerClient",
    "projectTransferFromFiles",
    "bootstrap",
    "inspectProject",
    "openProject",
    "closeProject",
    "queryTable",
    "queryFields",
    "editNumber",
    "editText",
    "editBoolean",
    "editDate",
    "updateFormula",
    "exportProject",
    "close",
    "inspectSpreadsheet",
    "importSpreadsheet",
    "inspectImportedProject",
    "exportSpreadsheet",
    "exportNativeTrackerSpreadsheet",
    "exportNativeBudgetSpreadsheet",
    "previewCleanup",
    "commitCleanup",
    "copyFormula",
    "newTracker",
    "newBudget",
    "trackerCommand",
    "openLocalDocument",
  ],
  files,
  licenseNotices,
}, null, 2)}\n`);
NODE

publish_bin="${scratch}/experimental-designer-client-publish"
cc -std=c11 -Wall -Wextra -Werror "${source_root}/scripts/experimental-designer-client-publish.c" -o "${publish_bin}"
"${publish_bin}" "${kit_dir}" "${output_dir}" ||
  fail "output appeared during publication or could not be atomically published: ${output_dir}"

trap - EXIT
rm -rf -- "${stage_parent}"
