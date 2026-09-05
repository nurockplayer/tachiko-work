#!/usr/bin/env bash

# Materialise a commit without allowing checkout state, Git replacement
# objects, or attribute files to change the source consumed by the RC tools.

tachiko_rc_source_env() {
  TACHIKO_RC_SOURCE_ENV=(env -i "PATH=${PATH}")
  if [[ "${HOME+x}" == x ]]; then TACHIKO_RC_SOURCE_ENV+=("HOME=${HOME}"); fi
  if [[ "${TMPDIR+x}" == x ]]; then TACHIKO_RC_SOURCE_ENV+=("TMPDIR=${TMPDIR}"); fi
  TACHIKO_RC_SOURCE_ENV+=(
    "GIT_CONFIG_NOSYSTEM=1"
    "GIT_CONFIG_GLOBAL=/dev/null"
    "GIT_CONFIG_SYSTEM=/dev/null"
  )
}

tachiko_rc_git() {
  local repo_root="$1"
  shift
  tachiko_rc_source_env
  "${TACHIKO_RC_SOURCE_ENV[@]}" git -C "${repo_root}" --no-replace-objects -c core.attributesFile=/dev/null "$@"
}

tachiko_rc_node() {
  tachiko_rc_source_env
  "${TACHIKO_RC_SOURCE_ENV[@]}" node "$@"
}

tachiko_rc_resolve_commit() {
  local repo_root="$1"
  local requested="$2"
  tachiko_rc_git "${repo_root}" rev-parse --verify "${requested}^{commit}"
}

tachiko_rc_materialize_source() {
  local repo_root="$1"
  local commit="$2"
  local destination="$3"
  local archive_path="${destination}.tar"
  local tree_path="${destination}.tree"
  tachiko_rc_source_env

  [[ ! -e "${destination}" ]] || { echo "source destination already exists: ${destination}" >&2; return 1; }
  mkdir "${destination}" || return 1
  "${TACHIKO_RC_SOURCE_ENV[@]}" git -C "${repo_root}" --no-replace-objects -c core.attributesFile=/dev/null archive --format=tar --output="${archive_path}" "${commit}" || return 1
  "${TACHIKO_RC_SOURCE_ENV[@]}" git -C "${repo_root}" --no-replace-objects -c core.attributesFile=/dev/null ls-tree -rz --full-tree -r "${commit}" >"${tree_path}" || return 1
  tar -xf "${archive_path}" -C "${destination}" || return 1

  local validation_status=0
  tachiko_rc_node - "${repo_root}" "${destination}" "${tree_path}" <<'NODE' || validation_status=$?
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const [repoRoot, sourceRoot, treePath] = process.argv.slice(2);
const fail = message => { throw new Error(message); };
let objectFormat = "sha1";
try {
  objectFormat = require("child_process").execFileSync(
    "git", ["-C", repoRoot, "--no-replace-objects", "-c", "core.attributesFile=/dev/null", "config", "--get", "extensions.objectFormat"],
    { encoding: "utf8", env: { PATH: process.env.PATH, HOME: process.env.HOME, GIT_CONFIG_NOSYSTEM: "1", GIT_CONFIG_GLOBAL: "/dev/null", GIT_CONFIG_SYSTEM: "/dev/null" }, stdio: ["ignore", "pipe", "ignore"] },
  ).trim() || "sha1";
} catch (error) {
  if (error.status !== 1) throw error;
}
if (objectFormat !== "sha1" && objectFormat !== "sha256") fail(`unsupported Git object format: ${objectFormat}`);
const expected = new Map();
const raw = fs.readFileSync(treePath);
for (const record of raw.toString("utf8").split("\0")) {
  if (!record) continue;
  const tab = record.indexOf("\t");
  if (tab < 0) fail("git ls-tree produced malformed output");
  const [mode, type, oid] = record.slice(0, tab).split(" ");
  const relative = record.slice(tab + 1);
  if (type !== "blob" || (mode !== "100644" && mode !== "100755")) fail(`source contains unsupported tree entry: ${relative}`);
  if (!/^[0-9a-f]+$/.test(oid) || oid.length !== (objectFormat === "sha1" ? 40 : 64)) fail(`source contains malformed blob: ${relative}`);
  expected.set(relative, { mode, oid });
}
if (expected.size === 0) fail("source archive contains no regular files");
const actual = new Map();
function walk(relative) {
  const absolute = path.join(sourceRoot, relative);
  for (const entry of fs.readdirSync(absolute, { withFileTypes: true })) {
    const child = relative ? path.join(relative, entry.name) : entry.name;
    const childAbsolute = path.join(sourceRoot, child);
    const stat = fs.lstatSync(childAbsolute);
    if (stat.isSymbolicLink()) fail(`source archive contains symlink: ${child}`);
    if (stat.isDirectory()) {
      if (![...expected.keys()].some(file => file.startsWith(`${child}/`))) fail(`source archive contains extra directory: ${child}`);
      walk(child);
    } else if (stat.isFile()) {
      const normalized = child.split(path.sep).join("/");
      const mode = (stat.mode & 0o111) !== 0 ? "100755" : "100644";
      const contents = fs.readFileSync(childAbsolute);
      const header = Buffer.from(`blob ${contents.length}\0`);
      const oid = crypto.createHash(objectFormat).update(header).update(contents).digest("hex");
      actual.set(normalized, { mode, oid });
    } else fail(`source archive contains unsupported entry: ${child}`);
  }
}
walk("");
if (actual.size !== expected.size) fail(`source archive file count differs from Git tree (${actual.size} versus ${expected.size})`);
for (const [relative, sourceEntry] of expected) {
  const actualEntry = actual.get(relative);
  if (!actualEntry) fail(`source archive omitted Git file: ${relative}`);
  if (actualEntry.mode !== sourceEntry.mode || actualEntry.oid !== sourceEntry.oid) fail(`source archive content differs from Git blob: ${relative}`);
}
for (const relative of actual.keys()) if (!expected.has(relative)) fail(`source archive contains extra file: ${relative}`);
NODE
  [[ "${validation_status}" -eq 0 ]] || return "${validation_status}"
  rm -f -- "${archive_path}" "${tree_path}" || return 1
}

tachiko_rc_check_ancestor_cargo_config() {
  local source_root="$1"
  local ancestor
  ancestor="$(cd "${source_root}/.." && pwd -P)" || return 1
  while :; do
    for config in "${ancestor}/.cargo/config" "${ancestor}/.cargo/config.toml"; do
      [[ ! -e "${config}" && ! -L "${config}" ]] || { echo "refusing source under ancestor Cargo config ${config}; use a clean scratch parent" >&2; return 1; }
    done
    [[ "${ancestor}" == "/" ]] && break
    ancestor="$(cd "$(dirname "${ancestor}")" && pwd -P)" || return 1
  done
}
