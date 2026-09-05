#!/usr/bin/env bash
set -euo pipefail

usage() { echo "Usage: $0 OUTPUT_DIR [COMMIT]" >&2; }
fail() { echo "package-designer-rc: $*" >&2; exit 1; }

if [[ "$#" -lt 1 || "$#" -gt 2 ]]; then usage; exit 2; fi
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
source "${script_dir}/designer-rc-source.sh"
tachiko_rc_source_env
output_argument="${1%/}"
[[ -n "${output_argument}" ]] || fail "OUTPUT_DIR must not be empty"
commit_argument="${2:-HEAD}"

for tool in git node pnpm tar rustup rustc; do
  command -v "${tool}" >/dev/null 2>&1 || fail "required tool not found: ${tool}"
done

if [[ "${output_argument}" = /* ]]; then
  output_parent_argument="$(dirname "${output_argument}")"
else
  output_parent_argument="$(dirname "${PWD}/${output_argument}")"
fi
output_leaf="$(basename "${output_argument}")"
[[ "${output_leaf}" != "." && "${output_leaf}" != ".." ]] || fail "invalid OUTPUT_DIR"
output_parent="$(cd "${output_parent_argument}" 2>/dev/null && pwd)" || fail "OUTPUT_DIR parent does not exist"
output_dir="${output_parent}/${output_leaf}"
mkdir "${output_dir}" 2>/dev/null || fail "refusing to overwrite existing OUTPUT_DIR: ${output_dir}"

resolved_commit="$(tachiko_rc_resolve_commit "${repo_root}" "${commit_argument}" 2>/dev/null)" ||
  fail "COMMIT does not resolve to a commit: ${commit_argument}"
scratch="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-designer-rc.XXXXXX")"
cleanup() { rm -rf -- "${scratch}"; }
trap cleanup EXIT

archived_root="${scratch}/source"
tachiko_rc_materialize_source "${repo_root}" "${resolved_commit}" "${archived_root}" || fail "could not materialize exact Git source"
tachiko_rc_check_ancestor_cargo_config "${archived_root}" || fail "source scratch parent is not safe for Cargo"
designer_root="${archived_root}/apps/designer"
[[ -f "${designer_root}/package.json" ]] || fail "archived commit has no apps/designer/package.json"
[[ -f "${designer_root}/pnpm-lock.yaml" ]] || fail "archived commit has no apps/designer/pnpm-lock.yaml"

# Corepack selects the package-manager pin from process cwd before pnpm parses
# its own arguments. Resolve every pnpm invocation inside the archived app.
pnpm_version="$(cd "${designer_root}" && "${TACHIKO_RC_SOURCE_ENV[@]}" pnpm --version)"
[[ "${pnpm_version}" == "11.25.0" ]] || fail "pnpm 11.25.0 is required (found ${pnpm_version})"

cargo_home="${scratch}/cargo-home"
config_home="${scratch}/config-home"
mkdir "${cargo_home}" "${config_home}"
clean_env=(env -i "PATH=${PATH}")
if [[ "${HOME+x}" == x ]]; then clean_env+=("HOME=${HOME}"); fi
if [[ "${TMPDIR+x}" == x ]]; then clean_env+=("TMPDIR=${TMPDIR}"); fi
if [[ "${RUSTUP_HOME+x}" == x ]]; then clean_env+=("RUSTUP_HOME=${RUSTUP_HOME}"); fi
clean_env+=(
  "RUSTUP_TOOLCHAIN=stable"
  "CARGO_HOME=${cargo_home}"
  "NPM_CONFIG_USERCONFIG=/dev/null"
  "NPM_CONFIG_GLOBALCONFIG=/dev/null"
  "XDG_CONFIG_HOME=${config_home}"
)

build_log="${output_dir}/build.log"
{
  echo "designer RC build"
  echo "commit: ${resolved_commit}"
  echo "source: git archive ${resolved_commit}"
  echo "cwd: ${designer_root}"
  echo "command: pnpm install --frozen-lockfile --engine-strict"
  (cd "${designer_root}" && "${clean_env[@]}" pnpm install --frozen-lockfile --engine-strict) || exit 1
  echo "command: pnpm build"
  (cd "${designer_root}" && "${clean_env[@]}" pnpm build)
} >"${build_log}" 2>&1 || fail "designer build failed; inspect ${build_log}"

site_source="${designer_root}/dist"
[[ -d "${site_source}" ]] || fail "designer build did not produce dist/"
mkdir "${output_dir}/site" "${output_dir}/source" "${output_dir}/licenses"
cp -R "${site_source}/." "${output_dir}/site/"
for license_file in LICENSE-APACHE LICENSE-MIT THIRD_PARTY_LICENSES.md; do
  [[ -f "${archived_root}/${license_file}" ]] || fail "missing archived license or notice: ${license_file}"
  cp "${archived_root}/${license_file}" "${output_dir}/licenses/${license_file}"
done
printf '%s\n' "${resolved_commit}" >"${output_dir}/source/commit.txt"
{
  echo "commit=${resolved_commit}"
  echo "node=$(tachiko_rc_node --version)"
  echo "pnpm=${pnpm_version}"
  echo "rustc=$("${clean_env[@]}" rustc --version)"
  echo "rustup=$("${clean_env[@]}" rustup --version | head -n 1)"
} >"${output_dir}/source/versions.txt"

tachiko_rc_node - "${output_dir}/site" "${output_dir}/site-manifest.json" "${resolved_commit}" <<'NODE'
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const [siteRoot, manifestPath, commit] = process.argv.slice(2);
const files = [];
function walk(relative) {
  const absolute = path.join(siteRoot, relative);
  for (const entry of fs.readdirSync(absolute, { withFileTypes: true })) {
    const child = relative ? path.join(relative, entry.name) : entry.name;
    const childAbsolute = path.join(siteRoot, child);
    const stat = fs.lstatSync(childAbsolute);
    if (stat.isSymbolicLink()) throw new Error(`site contains symlink: ${child}`);
    if (stat.isDirectory()) walk(child);
    else if (stat.isFile()) files.push({ path: child.split(path.sep).join("/"), bytes: stat.size, sha256: crypto.createHash("sha256").update(fs.readFileSync(childAbsolute)).digest("hex") });
    else throw new Error(`site contains unsupported entry: ${child}`);
  }
}
walk("");
for (const required of ["index.html", "designer_runtime.wasm"])
  if (!files.some(file => file.path === required)) throw new Error(`missing required site file: ${required}`);
files.sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
fs.writeFileSync(manifestPath, `${JSON.stringify({ schema: "tachiko-designer-rc-manifest-v1", commit, files }, null, 2)}\n`);
NODE

archive_name="designer-rc.tar.gz"
archive_path="${output_dir}/${archive_name}"
partial_archive="${scratch}/${archive_name}.partial"
(cd "${output_dir}" && tar -czf "${partial_archive}" site site-manifest.json licenses source)
mv "${partial_archive}" "${archive_path}"
archive_digest="$(tachiko_rc_node - "${archive_path}" <<'NODE'
const fs = require("node:fs");
const crypto = require("node:crypto");
const hash = crypto.createHash("sha256");
hash.update(fs.readFileSync(process.argv[2]));
process.stdout.write(`${hash.digest("hex")}\n`);
NODE
)"
printf '%s  %s\n' "${archive_digest}" "${archive_name}" >"${archive_path}.sha256"

echo "packaged designer RC for ${resolved_commit}"
echo "output: ${output_dir}"
