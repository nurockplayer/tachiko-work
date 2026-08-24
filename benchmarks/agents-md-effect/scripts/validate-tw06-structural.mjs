#!/usr/bin/env node

import {lstat, readFile, realpath} from "node:fs/promises";
import {spawnSync} from "node:child_process";
import {dirname, isAbsolute, relative, resolve} from "node:path";

function usage() {
  console.error(
    "usage: node validate-tw06-structural.mjs --candidate-root /abs/repo " +
      "--base <40-hex> --candidate <commit-ish>",
  );
  process.exit(2);
}

function parseArgs(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || value === undefined) usage();
    values.set(key.slice(2), value);
  }
  return values;
}

function fail(message) {
  throw new Error(message);
}

function runGit(root, args) {
  const result = spawnSync(
    "rtk",
    [
      "proxy",
      "git",
      "-c",
      "core.hooksPath=/dev/null",
      "-c",
      "core.attributesfile=/dev/null",
      "-c",
      "diff.external=",
      ...args,
    ],
    {cwd: root, encoding: null, maxBuffer: 16 * 1024 * 1024},
  );
  if (result.status !== 0) {
    fail(Buffer.from(result.stderr ?? result.stdout ?? []).toString("utf8"));
  }
  return Buffer.from(result.stdout);
}

function inside(path, root) {
  const fromRoot = relative(root, path);
  return fromRoot === "" || (!fromRoot.startsWith("..") && !isAbsolute(fromRoot));
}

function forbiddenReason(path) {
  if (path.startsWith("crates/")) return "runtime/source scope";
  const protectedBaseScripts = new Set([
    "scripts/collaboration-smoke.sh",
    "scripts/entity-lifecycle-smoke.sh",
    "scripts/first-user-smoke.sh",
    "scripts/formula-authoring-smoke.sh",
    "scripts/generate-third-party-licenses.sh",
    "scripts/package-binary.sh",
    "scripts/release-check.sh",
    "scripts/release-lib.sh",
    "scripts/verify-release-archive.sh",
  ]);
  if (protectedBaseScripts.has(path)) return "existing behavior/release automation scope";
  if (path.startsWith(".github/workflows/")) return "release/CI automation scope";
  if (path === ".codex/config.toml") return "agent tooling configuration scope";
  if (
    /(^|\/)Cargo\.toml$/.test(path) ||
    path === "Cargo.lock" ||
    path.startsWith(".cargo/") ||
    path === "rust-toolchain" ||
    path === "rust-toolchain.toml"
  ) {
    return "package metadata scope";
  }
  if (
    /(^|\/)(package\.json|pnpm-lock\.yaml|package-lock\.json|yarn\.lock|bun\.lockb?)$/.test(
      path,
    )
  ) {
    return "package metadata scope";
  }
  if (
    /(^|\/)(?:LICENSE|LICENCE|NOTICE|COPYING)(?:[-._].*)?$/.test(path) ||
    path === "THIRD_PARTY_LICENSES.md"
  ) {
    return "existing license text scope";
  }
  return null;
}

function markdownTargets(text) {
  const targets = [];
  const pattern = /!?\[[^\]]*\]\((<[^>]+>|[^)\s]+)(?:\s+["'][^"']*["'])?\)/g;
  for (const match of text.matchAll(pattern)) {
    let target = match[1];
    if (target.startsWith("<") && target.endsWith(">")) {
      target = target.slice(1, -1);
    }
    targets.push(target);
  }
  return targets;
}

const args = parseArgs(process.argv.slice(2));
for (const key of ["candidate-root", "base", "candidate"]) {
  if (!args.has(key)) usage();
}

const root = await realpath(resolve(args.get("candidate-root")));
const base = args.get("base");
if (!/^[0-9a-f]{40}$/.test(base)) fail("base must be a full commit SHA");
const candidate = runGit(root, ["rev-parse", `${args.get("candidate")}^{commit}`])
  .toString("utf8")
  .trim();
if (!/^[0-9a-f]{40}$/.test(candidate)) fail("candidate did not resolve exactly");
const candidateTree = runGit(root, ["rev-parse", `${candidate}^{tree}`])
  .toString("utf8")
  .trim();
const workspaceTree = runGit(root, ["rev-parse", "HEAD^{tree}"])
  .toString("utf8")
  .trim();
const workspaceStatus = runGit(root, ["status", "--porcelain"])
  .toString("utf8")
  .trim();
if (candidateTree !== workspaceTree || workspaceStatus !== "") {
  fail("workspace bytes do not equal the clean frozen candidate tree");
}

const raw = runGit(root, [
  "diff",
  "--no-ext-diff",
  "--no-textconv",
  "--no-renames",
  "--name-status",
  "-z",
  base,
  candidate,
]);
const fields = raw.toString("utf8").split("\0").filter(Boolean);
if (fields.length % 2 !== 0) fail("unexpected name-status record structure");

const changes = [];
for (let index = 0; index < fields.length; index += 2) {
  const status = fields[index];
  const path = fields[index + 1];
  if (!/^[AMDTCUXB]$/.test(status)) fail(`unexpected diff status ${status}`);
  changes.push({status, path});
}

const forbidden = changes
  .map((change) => ({...change, reason: forbiddenReason(change.path)}))
  .filter((change) => change.reason !== null);
const brokenLinks = [];
for (const change of changes) {
  if (change.status === "D" || !change.path.endsWith(".md")) continue;
  const source = resolve(root, change.path);
  if (!inside(source, root)) fail(`changed Markdown path escapes root: ${change.path}`);
  const info = await lstat(source);
  if (!info.isFile()) {
    brokenLinks.push({source: change.path, target: null, reason: "not a regular file"});
    continue;
  }
  const text = await readFile(source, "utf8");
  for (const rawTarget of markdownTargets(text)) {
    if (
      rawTarget === "" ||
      rawTarget.startsWith("#") ||
      /^[a-z][a-z0-9+.-]*:/i.test(rawTarget) ||
      rawTarget.startsWith("//")
    ) {
      continue;
    }
    const withoutFragment = rawTarget.split("#", 1)[0].split("?", 1)[0];
    let decoded;
    try {
      decoded = decodeURIComponent(withoutFragment);
    } catch {
      brokenLinks.push({source: change.path, target: rawTarget, reason: "invalid URL encoding"});
      continue;
    }
    const destination = decoded.startsWith("/")
      ? resolve(root, decoded.slice(1))
      : resolve(dirname(source), decoded);
    if (!inside(destination, root)) {
      brokenLinks.push({source: change.path, target: rawTarget, reason: "escapes repository"});
      continue;
    }
    try {
      const resolvedDestination = await realpath(destination);
      if (!inside(resolvedDestination, root)) {
        brokenLinks.push({
          source: change.path,
          target: rawTarget,
          reason: "resolves outside repository",
        });
      }
    } catch (error) {
      if (error?.code !== "ENOENT") throw error;
      brokenLinks.push({source: change.path, target: rawTarget, reason: "target absent"});
    }
  }
}

const result = {
  contract_id: "TW-06-structural-scope-v1",
  base_commit: base,
  candidate_commit: candidate,
  pass: forbidden.length === 0 && brokenLinks.length === 0,
  changed_paths: changes,
  forbidden_changes: forbidden,
  broken_repository_links: brokenLinks,
};
console.log(JSON.stringify(result));
if (!result.pass) process.exitCode = 1;
