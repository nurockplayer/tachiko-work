#!/usr/bin/env bash
set -euo pipefail

usage() { echo "Usage: $0 OUTPUT_DIR [EXPECTED_COMMIT]" >&2; }
fail() { echo "verify-designer-rc: $*" >&2; exit 1; }

if [[ "$#" -lt 1 || "$#" -gt 2 ]]; then usage; exit 2; fi
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
source "${script_dir}/designer-rc-source.sh"
tachiko_rc_source_env
output_argument="${1%/}"
[[ -n "${output_argument}" ]] || fail "OUTPUT_DIR must not be empty"
expected_argument="${2:-HEAD}"

for tool in git node pnpm tar; do
  command -v "${tool}" >/dev/null 2>&1 || fail "required tool not found: ${tool}"
done

if [[ "${output_argument}" = /* ]]; then
  output_parent_argument="$(dirname "${output_argument}")"
else
  output_parent_argument="$(dirname "${PWD}/${output_argument}")"
fi
output_leaf="$(basename "${output_argument}")"
output_parent="$(cd "${output_parent_argument}" 2>/dev/null && pwd)" || fail "OUTPUT_DIR parent does not exist"
output_dir="${output_parent}/${output_leaf}"
[[ -d "${output_dir}" ]] || fail "OUTPUT_DIR does not exist: ${output_dir}"

resolved_expected="$(tachiko_rc_resolve_commit "${repo_root}" "${expected_argument}" 2>/dev/null)" ||
  fail "EXPECTED_COMMIT does not resolve to a commit: ${expected_argument}"
site_dir="${output_dir}/site"
manifest_path="${output_dir}/site-manifest.json"
archive_path="${output_dir}/designer-rc.tar.gz"
checksum_path="${archive_path}.sha256"
commit_path="${output_dir}/source/commit.txt"
versions_path="${output_dir}/source/versions.txt"
verification_dir="${output_dir}/artifact-verification"
[[ -d "${site_dir}" ]] || fail "missing packaged site/"
[[ ! -L "${site_dir}" ]] || fail "packaged site/ must not be a symlink"
[[ -f "${manifest_path}" ]] || fail "missing site-manifest.json"
[[ -f "${archive_path}" ]] || fail "missing designer-rc.tar.gz"
[[ -f "${checksum_path}" ]] || fail "missing designer-rc.tar.gz.sha256"
[[ -f "${commit_path}" ]] || fail "missing source/commit.txt"
[[ -f "${versions_path}" ]] || fail "missing source/versions.txt"
for license_file in LICENSE-APACHE LICENSE-MIT THIRD_PARTY_LICENSES.md; do
  [[ -f "${output_dir}/licenses/${license_file}" ]] || fail "missing copied license or notice: ${license_file}"
done
[[ ! -e "${verification_dir}" ]] || fail "refusing to reuse existing artifact-verification/"
printf '%s\n' "${resolved_expected}" | cmp -s - "${commit_path}" || fail "packaged source revision does not match expected commit"

# Check the exact payload before and after Playwright. Read archive members
# without extracting them into the filesystem, and compare them with the site
# and metadata being verified.
verify_integrity() {
  local manifest_status=0
  tachiko_rc_node - "${site_dir}" "${manifest_path}" "${resolved_expected}" <<'NODE' || manifest_status=$?
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const [siteRoot, manifestPath, expectedCommit] = process.argv.slice(2);
const fail = message => { throw new Error(message); };
if (!fs.lstatSync(siteRoot).isDirectory() || fs.lstatSync(siteRoot).isSymbolicLink()) fail("site root must be a real directory");
let manifest;
try { manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8")); }
catch (error) { fail(`invalid site-manifest.json: ${error.message}`); }
if (!manifest || typeof manifest !== "object" || Array.isArray(manifest)) fail("manifest must be an object");
if (Object.keys(manifest).sort().join(",") !== "commit,files,schema") fail("manifest has unexpected fields");
if (manifest.schema !== "tachiko-designer-rc-manifest-v1") fail("unsupported manifest schema");
if (manifest.commit !== expectedCommit) fail("manifest revision does not match expected commit");
if (!Array.isArray(manifest.files) || manifest.files.length === 0) fail("manifest must contain site files");

const actual = new Map();
function walk(relative) {
  const absolute = path.join(siteRoot, relative);
  for (const entry of fs.readdirSync(absolute, { withFileTypes: true })) {
    const child = relative ? path.join(relative, entry.name) : entry.name;
    const childAbsolute = path.join(siteRoot, child);
    const stat = fs.lstatSync(childAbsolute);
    if (stat.isSymbolicLink()) fail(`site contains symlink: ${child}`);
    if (stat.isDirectory()) walk(child);
    else if (stat.isFile()) actual.set(child.split(path.sep).join("/"), {
      bytes: stat.size,
      sha256: crypto.createHash("sha256").update(fs.readFileSync(childAbsolute)).digest("hex"),
    });
    else fail(`site contains unsupported entry: ${child}`);
  }
}
walk("");
if (!actual.has("index.html")) fail("site manifest requires index.html");
if (!actual.has("designer_runtime.wasm")) fail("site manifest requires designer_runtime.wasm");

const listed = new Set();
for (const file of manifest.files) {
  if (!file || typeof file !== "object" || Array.isArray(file) ||
      Object.keys(file).sort().join(",") !== "bytes,path,sha256") fail("manifest contains malformed file entry");
  if (typeof file.path !== "string" || !file.path || file.path.startsWith("/") ||
      file.path.includes("\\") || file.path.split("/").some(part => !part || part === "." || part === ".."))
    fail(`manifest contains unsafe path: ${file.path}`);
  if (!Number.isSafeInteger(file.bytes) || file.bytes < 0 || !/^[0-9a-f]{64}$/.test(file.sha256))
    fail(`manifest contains invalid digest metadata: ${file.path}`);
  if (listed.has(file.path)) fail(`manifest contains duplicate path: ${file.path}`);
  listed.add(file.path);
  const actualFile = actual.get(file.path);
  if (!actualFile) fail(`manifest is missing packaged file: ${file.path}`);
  if (actualFile.bytes !== file.bytes || actualFile.sha256 !== file.sha256) fail(`packaged file hash mismatch: ${file.path}`);
}
if (listed.size !== actual.size) {
  for (const filePath of actual.keys()) if (!listed.has(filePath)) fail(`site contains unlisted file: ${filePath}`);
}
const sortedPaths = manifest.files.map(file => file.path).slice().sort((a, b) => a < b ? -1 : a > b ? 1 : 0);
if (sortedPaths.some((filePath, index) => filePath !== manifest.files[index].path)) fail("manifest files are not sorted");
NODE

  [[ "${manifest_status}" -eq 0 ]] || return "${manifest_status}"

  local archive_status=0
  tachiko_rc_node - "${archive_path}" "${checksum_path}" "designer-rc.tar.gz" "${output_dir}" <<'NODE' || archive_status=$?
const fs = require("node:fs");
const crypto = require("node:crypto");
const [archivePath, checksumPath, archiveName, outputRoot] = process.argv.slice(2);
const lines = fs.readFileSync(checksumPath, "utf8").split("\n");
if (lines.length !== 2 || !lines[0]) throw new Error("checksum must contain exactly one line");
const match = /^([0-9a-f]{64})  ([^\s]+)$/.exec(lines[0]);
if (!match || match[2] !== archiveName) throw new Error("checksum has invalid format or archive name");
const actual = crypto.createHash("sha256").update(fs.readFileSync(archivePath)).digest("hex");
if (actual !== match[1]) throw new Error("archive checksum mismatch");
const path = require("node:path");
const { execFileSync } = require("node:child_process");
const expected = new Map();
function payload(relative) {
  const absolute = path.join(outputRoot, relative);
  const stat = fs.lstatSync(absolute);
  if (stat.isSymbolicLink()) throw new Error(`payload contains symlink: ${relative}`);
  if (stat.isDirectory()) {
    expected.set(`${relative}/`, null);
    for (const child of fs.readdirSync(absolute)) payload(`${relative}/${child}`);
  } else if (stat.isFile()) expected.set(relative, fs.readFileSync(absolute));
  else throw new Error(`unsupported payload entry: ${relative}`);
}
for (const root of ["site", "site-manifest.json", "source", "licenses"]) payload(root);
const listing = execFileSync("tar", ["-tf", archivePath], { encoding: "utf8", maxBuffer: 16 * 1024 * 1024 });
if (!listing.endsWith("\n")) throw new Error("invalid archive listing");
const members = listing.slice(0, -1).split("\n");
if (members.length !== expected.size || new Set(members).size !== members.length || members.some(member => !expected.has(member)))
  throw new Error("archive payload inventory differs from the tested artifact");
const types = execFileSync("tar", ["-tvf", archivePath], { encoding: "utf8", maxBuffer: 16 * 1024 * 1024 }).trimEnd().split("\n");
if (types.length !== members.length || types.some(line => line[0] !== "-" && line[0] !== "d"))
  throw new Error("archive contains unsupported member types");
for (let index = 0; index < members.length; index++) {
  const member = members[index];
  const bytes = expected.get(member);
  if ((bytes === null ? "d" : "-") !== types[index][0]) throw new Error(`archive member type mismatch: ${member}`);
  if (bytes === null) continue;
  const archivedBytes = execFileSync("tar", ["-xOf", archivePath, member], { maxBuffer: Math.max(bytes.length + 1, 65536) });
  if (!archivedBytes.equals(bytes)) throw new Error(`archive payload differs from tested file: ${member}`);
}

NODE
  [[ "${archive_status}" -eq 0 ]] || return "${archive_status}"
}

verify_integrity || fail "packaged site integrity check failed before browser verification"
digest_file() {
  tachiko_rc_node - "$1" <<'NODE'
const fs = require("node:fs");
const crypto = require("node:crypto");
process.stdout.write(crypto.createHash("sha256").update(fs.readFileSync(process.argv[2])).digest("hex"));
NODE
}
baseline_manifest_digest="$(digest_file "${manifest_path}")"
baseline_checksum_digest="$(digest_file "${checksum_path}")"

scratch="$(mktemp -d "${TMPDIR:-/tmp}/tachiko-designer-rc-verify.XXXXXX")"
cleanup() { rm -rf -- "${scratch}"; }
trap cleanup EXIT
test_root="${scratch}/source"
tachiko_rc_materialize_source "${repo_root}" "${resolved_expected}" "${test_root}" || fail "could not materialize exact Git source"
tachiko_rc_check_ancestor_cargo_config "${test_root}" || fail "source scratch parent is not safe for Cargo"
test_designer="${test_root}/apps/designer"
[[ -f "${test_designer}/package.json" && -f "${test_designer}/pnpm-lock.yaml" ]] || fail "expected commit has no complete designer source"

# Corepack selects the package-manager pin from process cwd before pnpm parses
# its own arguments. Resolve every pnpm invocation inside the archived app.
pnpm_version="$(cd "${test_designer}" && "${TACHIKO_RC_SOURCE_ENV[@]}" pnpm --version)"
[[ "${pnpm_version}" == "11.25.0" ]] || fail "pnpm 11.25.0 is required (found ${pnpm_version})"

# ABI journeys read this generated module directly. Exercise the packaged bytes,
# rather than rebuilding a second module or borrowing the current checkout.
mkdir -p "${test_designer}/public"
cp "${site_dir}/designer_runtime.wasm" "${test_designer}/public/designer_runtime.wasm"
cmp -s "${site_dir}/designer_runtime.wasm" "${test_designer}/public/designer_runtime.wasm" || fail "test WASM differs from packaged module"

mkdir "${verification_dir}" "${verification_dir}/test-results"
config_home="${scratch}/config-home"
cargo_home="${scratch}/cargo-home"
mkdir "${config_home}" "${cargo_home}"
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
{
  echo "source: git archive ${resolved_expected}"
  echo "cwd: ${test_designer}"
  echo "command: pnpm install --frozen-lockfile --engine-strict"
  (cd "${test_designer}" && "${clean_env[@]}" pnpm install --frozen-lockfile --engine-strict)
} >"${verification_dir}/install.log" 2>&1 || fail "frozen dependency install failed; inspect ${verification_dir}/install.log"

playwright_module="${test_designer}/node_modules/@playwright/test/index.mjs"
[[ -f "${playwright_module}" ]] || fail "@playwright/test missing from archived source dependencies"
config_path="${scratch}/playwright.config.mjs"
tachiko_rc_node - "${config_path}" "${playwright_module}" "${test_designer}/e2e" "${site_dir}" "${test_designer}" "${verification_dir}" <<'NODE'
const fs = require("node:fs");
const [configPath, playwrightModule, testDir, siteDir, cwd, verificationDir] = process.argv.slice(2);
const js = value => `'${value.replaceAll("\\", "\\\\").replaceAll("'", "\\'").replaceAll("\r", "\\r").replaceAll("\n", "\\n")}'`;
const shell = value => `'${value.replaceAll("'", "'\\''")}'`;
const command = `pnpm exec vite preview --outDir ${shell(siteDir)} --host 127.0.0.1 --port 4173 --strictPort`;
const source = `import { defineConfig } from ${js(playwrightModule)};

export default defineConfig({
  testDir: ${js(testDir)},
  testIgnore: /experimental-client\\.spec\\.ts$/,
  fullyParallel: false,
  retries: 0,
  reporter: [["line"], ["json", { outputFile: ${js(`${verificationDir}/report.json`)} }]],
  outputDir: ${js(`${verificationDir}/test-results`)},
  preserveOutput: "always",
  use: {
    baseURL: "http://127.0.0.1:4173",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  webServer: {
    command: ${JSON.stringify(command)},
    cwd: ${js(cwd)},
    url: "http://127.0.0.1:4173",
    reuseExistingServer: false,
  },
});
`;
fs.writeFileSync(configPath, source);
NODE

inventory_status=0
set +e
(cd "${test_designer}" && "${clean_env[@]}" pnpm exec playwright test --config="${config_path}" --list --reporter=json) >"${verification_dir}/expected-tests.json" 2>"${verification_dir}/expected-tests.log"
inventory_status=$?
set -e
[[ "${inventory_status}" -eq 0 ]] || fail "archived Playwright test inventory failed; inspect ${verification_dir}/expected-tests.log"
tachiko_rc_node - "${verification_dir}/expected-tests.json" "${verification_dir}/expected-tests-summary.json" <<'NODE'
const fs = require("node:fs");
const [reportPath, summaryPath] = process.argv.slice(2);
const report = JSON.parse(fs.readFileSync(reportPath, "utf8"));
const identities = [];
function visit(suite) {
  for (const spec of suite.specs ?? []) for (const test of spec.tests ?? [])
    identities.push(`${spec.file ?? suite.file ?? ""}::${spec.title}::${test.projectName ?? ""}`);
  for (const child of suite.suites ?? []) visit(child);
}
for (const suite of report.suites ?? []) visit(suite);
if (identities.length === 0 || new Set(identities).size !== identities.length) throw new Error("archived Playwright inventory is empty or duplicated");
fs.writeFileSync(summaryPath, `${JSON.stringify({ schema: "tachiko-designer-rc-inventory-v1", identities }, null, 2)}\n`);
NODE

browser_status=0
set +e
(cd "${test_designer}" && "${clean_env[@]}" pnpm exec playwright test --config="${config_path}") >"${verification_dir}/report.line.log" 2>&1
browser_status=$?
set -e

report_status=0
if [[ ! -f "${verification_dir}/report.json" ]]; then
  printf '%s\n' '{"schema":"tachiko-designer-rc-e2e-report-v1","error":"Playwright did not produce report.json"}' >"${verification_dir}/report-summary.json"
  report_status=1
else
  tachiko_rc_node - "${verification_dir}/report.json" "${verification_dir}/expected-tests-summary.json" "${verification_dir}/report-summary.json" <<'NODE' || report_status=$?
const fs = require("node:fs");
const [reportPath, inventoryPath, summaryPath] = process.argv.slice(2);
const report = JSON.parse(fs.readFileSync(reportPath, "utf8"));
const inventory = JSON.parse(fs.readFileSync(inventoryPath, "utf8"));
if (inventory.schema !== "tachiko-designer-rc-inventory-v1" || !Array.isArray(inventory.identities)) throw new Error("invalid archived Playwright inventory");
const expected = inventory.identities;
const actual = [];
const stats = { expectedTests: expected.length, actualTests: 0, passedTests: 0, skippedTests: 0, unexpectedTests: 0, flakyTests: 0 };
function visit(suite) {
  for (const spec of suite.specs ?? []) {
    for (const test of spec.tests ?? []) {
      stats.actualTests += 1;
      const identity = `${spec.file ?? suite.file ?? ""}::${spec.title}::${test.projectName ?? ""}`;
      actual.push(identity);
      const statuses = (test.results ?? []).map(result => result.status);
      if (test.status === "skipped" || statuses.includes("skipped")) stats.skippedTests += 1;
      else if (test.status === "flaky") stats.flakyTests += 1;
      else if (test.expectedStatus === "passed" && statuses.length > 0 && statuses.every(status => status === "passed")) stats.passedTests += 1;
      else stats.unexpectedTests += 1;
    }
  }
  for (const child of suite.suites ?? []) visit(child);
}
for (const suite of report.suites ?? []) visit(suite);
if (expected.length === 0) throw new Error("Playwright inventory contains zero tests");
if (new Set(expected).size !== expected.length || new Set(actual).size !== actual.length) throw new Error("test identity inventory contains duplicates");
if (expected.length !== actual.length || expected.some(identity => !actual.includes(identity)) || actual.some(identity => !expected.includes(identity))) throw new Error("executed test identities differ from the archived inventory");
fs.writeFileSync(summaryPath, `${JSON.stringify({ schema: "tachiko-designer-rc-e2e-report-v1", ...stats }, null, 2)}\n`);
if (stats.actualTests === 0 || stats.skippedTests !== 0 || stats.unexpectedTests !== 0 || stats.flakyTests !== 0 || stats.passedTests !== stats.actualTests)
  throw new Error(`non-passing test report: ${JSON.stringify(stats)}`);
NODE
fi

post_status=0
post_log="${verification_dir}/post-verify.log"
: >"${post_log}"
verify_integrity >>"${post_log}" 2>&1 || post_status=$?
current_manifest_digest="$(digest_file "${manifest_path}")"
current_checksum_digest="$(digest_file "${checksum_path}")"
if [[ "${current_manifest_digest}" != "${baseline_manifest_digest}" ]]; then
  echo "site-manifest.json changed during browser verification" >>"${post_log}"
  post_status=1
fi
if [[ "${current_checksum_digest}" != "${baseline_checksum_digest}" ]]; then
  echo "designer-rc.tar.gz.sha256 changed during browser verification" >>"${post_log}"
  post_status=1
fi
if [[ "${post_status}" -ne 0 ]]; then fail "post-browser artifact integrity verification failed; inspect ${verification_dir}"; fi
if [[ "${report_status}" -ne 0 ]]; then fail "Playwright report did not meet the archived test inventory; inspect ${verification_dir}"; fi
if [[ "${browser_status}" -ne 0 ]]; then fail "Playwright verification failed; inspect ${verification_dir}/report.line.log"; fi
echo "verified designer RC ${resolved_expected} with the archived test inventory passing"
echo "verification: ${verification_dir}"
