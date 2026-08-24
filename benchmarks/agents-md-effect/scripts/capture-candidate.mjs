#!/usr/bin/env node

import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import {
  chmod,
  lstat,
  mkdir,
  readFile,
  readdir,
  readlink,
  realpath,
  symlink,
  writeFile,
} from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { dirname, isAbsolute, posix, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const benchmarkDir = resolve(scriptDir, "..");
const GIT_CONFIGURATION = [
  "-c",
  "core.hooksPath=/dev/null",
  "-c",
  "core.attributesFile=/dev/null",
  "-c",
  "core.autocrlf=false",
  "-c",
  "core.safecrlf=false",
  "-c",
  "protocol.file.allow=always",
];

function usage() {
  console.error(
    "usage: node capture-candidate.mjs --case TW-01 --workspace /abs/workspace " +
      "--source-repo /abs/controller-repo --exclusions-file /abs/exclusions.json " +
      "--trusted-dir /abs/output --expected-agents-sha256 <sha256>",
  );
  process.exit(2);
}

function fail(message) {
  throw new Error(message);
}

function parseArgs(argv) {
  if (argv.length % 2 !== 0) usage();
  const allowed = new Set([
    "case",
    "workspace",
    "source-repo",
    "exclusions-file",
    "trusted-dir",
    "expected-agents-sha256",
  ]);
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || value === undefined) usage();
    const name = key.slice(2);
    if (!allowed.has(name) || values.has(name)) usage();
    values.set(name, value);
  }
  return values;
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function isInside(candidate, parent) {
  const pathFromParent = relative(parent, candidate);
  return (
    pathFromParent === "" ||
    (!pathFromParent.startsWith("..") && !isAbsolute(pathFromParent))
  );
}

function isolatedGitEnvironment(overrides = {}) {
  const environment = { ...process.env };
  for (const key of Object.keys(environment)) {
    if (
      [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_COMMON_DIR",
        "GIT_CONFIG_PARAMETERS",
        "GIT_CONFIG_COUNT",
      ].includes(key) ||
      /^GIT_CONFIG_(?:KEY|VALUE)_\d+$/.test(key)
    ) {
      delete environment[key];
    }
  }
  return {
    ...environment,
    GIT_CONFIG_NOSYSTEM: "1",
    GIT_CONFIG_GLOBAL: "/dev/null",
    GIT_ATTR_NOSYSTEM: "1",
    GIT_ALTERNATE_OBJECT_DIRECTORIES: "",
    LANG: "C.UTF-8",
    LC_ALL: "C.UTF-8",
    ...overrides,
  };
}

function git(args, cwd, options = {}) {
  const result = spawnSync("rtk", ["proxy", "git", ...GIT_CONFIGURATION, ...args], {
    cwd,
    encoding: null,
    input: options.input,
    maxBuffer: 256 * 1024 * 1024,
    env: isolatedGitEnvironment(options.env),
  });
  if (!options.allowFailure && result.status !== 0) {
    fail(
      `rtk proxy git ${args.join(" ")} failed: ` +
        Buffer.from(result.stderr ?? result.stdout ?? []).toString("utf8"),
    );
  }
  return result;
}

function outputText(result) {
  return Buffer.from(result.stdout ?? []).toString("utf8").trim();
}

function rejectObjectAlternates(gitDirectory) {
  for (const name of ["alternates", "http-alternates"]) {
    if (existsSync(resolve(gitDirectory, "objects", "info", name))) {
      fail(`trusted object database must not use ${name}`);
    }
  }
}

function identity(stat) {
  return {
    device: stat.dev.toString(),
    inode: stat.ino.toString(),
    uid: stat.uid.toString(),
    gid: stat.gid.toString(),
    mode: Number(stat.mode & 0o7777n),
  };
}

function validateExclusionPaths(value) {
  if (!Array.isArray(value)) fail("exclusions-file must contain a JSON array");
  const seen = new Set();
  for (const path of value) {
    if (
      typeof path !== "string" ||
      path === "" ||
      path.includes("\\") ||
      path.includes("\0") ||
      posix.isAbsolute(path) ||
      posix.normalize(path) !== path ||
      path === "." ||
      path === ".." ||
      path.startsWith("../")
    ) {
      fail(`invalid non-normalized exclusion path: ${JSON.stringify(path)}`);
    }
    if (path === ".git" || path.startsWith(".git/") || path === "AGENTS.md") {
      fail(`reserved capture exclusion must not be repeated: ${path}`);
    }
    if (seen.has(path)) fail(`duplicate capture exclusion path: ${path}`);
    seen.add(path);
  }
  return value;
}

function pathIsExcluded(path, exclusions) {
  if (path === ".git" || path.startsWith(".git/")) return true;
  if (path === "AGENTS.md") return true;
  return exclusions.some((root) => path === root || path.startsWith(`${root}/`));
}

function decodePathName(name) {
  const bytes = Buffer.isBuffer(name) ? name : Buffer.from(name);
  const decoded = bytes.toString("utf8");
  if (!Buffer.from(decoded, "utf8").equals(bytes) || decoded.includes("/") || decoded === "") {
    fail("workspace contains an invalid non-UTF-8 path component");
  }
  return decoded;
}

async function collectRawTree(root, exclusions) {
  const entries = [];

  async function walk(directory, relativeDirectory) {
    const children = await readdir(directory, { encoding: "buffer", withFileTypes: true });
    children.sort((left, right) =>
      Buffer.compare(
        Buffer.isBuffer(left.name) ? left.name : Buffer.from(left.name),
        Buffer.isBuffer(right.name) ? right.name : Buffer.from(right.name),
      ),
    );
    for (const child of children) {
      const name = decodePathName(child.name);
      if (name === "." || name === "..") fail("workspace contains an invalid path component");
      const path = relativeDirectory === "" ? name : `${relativeDirectory}/${name}`;
      if (pathIsExcluded(path, exclusions)) continue;
      const absolutePath = resolve(directory, name);
      if (!isInside(absolutePath, root)) fail(`workspace path escapes capture root: ${path}`);
      const metadata = await lstat(absolutePath, { bigint: true });
      if (metadata.isDirectory()) {
        await walk(absolutePath, path);
      } else if (metadata.isFile()) {
        const bytes = await readFile(absolutePath);
        entries.push({
          path,
          type: "regular",
          mode: (metadata.mode & 0o111n) === 0n ? "100644" : "100755",
          bytes: bytes.length,
          sha256: sha256(bytes),
          content: bytes,
        });
      } else if (metadata.isSymbolicLink()) {
        const bytes = await readlink(absolutePath, { encoding: "buffer" });
        entries.push({
          path,
          type: "symlink",
          mode: "120000",
          bytes: bytes.length,
          sha256: sha256(bytes),
          content: bytes,
        });
      } else {
        fail(`workspace contains unsupported filesystem node: ${path}`);
      }
    }
  }

  await walk(root, "");
  const publicEntries = entries.map(({ content, ...entry }) => entry);
  const manifestBytes = Buffer.from(
    `${JSON.stringify({ version: 1, entries: publicEntries }, null, 2)}\n`,
    "utf8",
  );
  return { entries, manifestBytes, digest: sha256(manifestBytes) };
}

function parseNullTerminated(bytes) {
  const values = [];
  let start = 0;
  for (let index = 0; index < bytes.length; index += 1) {
    if (bytes[index] === 0) {
      if (index > start) values.push(bytes.subarray(start, index).toString("utf8"));
      start = index + 1;
    }
  }
  if (start !== bytes.length) fail("trusted Git returned malformed NUL-delimited output");
  return values;
}

async function materializeTrustedTree(tree, root, exclusions, gitDirectory, cwd) {
  const treeBytes = Buffer.from(
    git(["ls-tree", "-r", "-z", "--full-tree", tree], cwd, {
      env: { GIT_DIR: gitDirectory },
    }).stdout,
  );
  for (const record of parseNullTerminated(treeBytes)) {
    const match = /^(\d+) (\w+) ([0-9a-f]{40})\t([\s\S]+)$/.exec(record);
    if (!match) fail("trusted candidate tree contains an invalid entry");
    const [, mode, type, oid, path] = match;
    if (pathIsExcluded(path, exclusions)) continue;
    if (type !== "blob" || !["100644", "100755", "120000"].includes(mode)) {
      fail(`trusted candidate tree contains unsupported entry: ${path}`);
    }
    const destination = resolve(root, ...path.split("/"));
    if (!isInside(destination, root)) fail(`trusted tree path escapes round-trip root: ${path}`);
    await mkdir(dirname(destination), { recursive: true, mode: 0o700 });
    const bytes = Buffer.from(
      git(["cat-file", "blob", oid], cwd, { env: { GIT_DIR: gitDirectory } }).stdout,
    );
    if (mode === "120000") {
      await symlink(bytes, destination);
    } else {
      const fileMode = mode === "100755" ? 0o755 : 0o644;
      await writeFile(destination, bytes, { mode: fileMode });
      await chmod(destination, fileMode);
    }
  }
}

const args = parseArgs(process.argv.slice(2));
for (const key of [
  "case",
  "workspace",
  "source-repo",
  "exclusions-file",
  "trusted-dir",
  "expected-agents-sha256",
]) {
  if (!args.has(key)) usage();
}
for (const key of ["workspace", "source-repo", "exclusions-file", "trusted-dir"]) {
  if (!isAbsolute(args.get(key))) fail(`${key} must be an absolute path`);
}

const caseId = args.get("case");
const expectedAgentsSha256 = args.get("expected-agents-sha256");
if (!/^[0-9a-f]{64}$/.test(expectedAgentsSha256)) {
  fail("expected-agents-sha256 must be a lowercase SHA-256 digest");
}
const workspace = await realpath(args.get("workspace"));
const sourceRepo = await realpath(args.get("source-repo"));
const exclusionsFile = await realpath(args.get("exclusions-file"));
const trustedDir = resolve(args.get("trusted-dir"));
if (existsSync(trustedDir)) fail("trusted-dir must not exist");
for (const [leftName, left, rightName, right] of [
  ["trusted-dir", trustedDir, "workspace", workspace],
  ["trusted-dir", trustedDir, "source-repo", sourceRepo],
  ["workspace", workspace, "source-repo", sourceRepo],
]) {
  if (isInside(left, right) || isInside(right, left)) {
    fail(`${leftName} and ${rightName} must be disjoint`);
  }
}
if (isInside(exclusionsFile, workspace)) {
  fail("exclusions-file must be outside the candidate workspace");
}

const manifest = JSON.parse(
  await readFile(resolve(benchmarkDir, "evaluator/cases.json"), "utf8"),
);
const caseEntry = manifest.cases.find((candidate) => candidate.id === caseId);
if (!caseEntry) fail(`unknown case ${caseId}`);

const sourceStat = await lstat(sourceRepo, { bigint: true });
if (!sourceStat.isDirectory()) fail("source-repo must be a controller-trusted directory");
const exclusionsStat = await lstat(exclusionsFile, { bigint: true });
if (!exclusionsStat.isFile() || exclusionsStat.isSymbolicLink()) {
  fail("exclusions-file must be a regular non-symlink file");
}
const exclusionsBytes = await readFile(exclusionsFile);
let parsedExclusions;
try {
  parsedExclusions = JSON.parse(exclusionsBytes.toString("utf8"));
} catch (error) {
  fail(`invalid exclusions-file JSON: ${error.message}`);
}
const exclusions = validateExclusionPaths(parsedExclusions);

const overlayPath = resolve(workspace, "AGENTS.md");
const overlayStat = await lstat(overlayPath, { bigint: true });
if (!overlayStat.isFile() || overlayStat.isSymbolicLink()) {
  fail("root AGENTS.md overlay must be a regular non-symlink file");
}
const overlayBytes = await readFile(overlayPath);
const overlaySha256 = sha256(overlayBytes);
if (overlaySha256 !== expectedAgentsSha256) {
  fail("AGENTS.md overlay changed during the candidate run");
}

const rawTree = await collectRawTree(workspace, exclusions);
await mkdir(trustedDir, { recursive: false, mode: 0o700 });
const bareRepo = resolve(trustedDir, "objects.git");
const indexPath = resolve(trustedDir, "candidate.index");
git(["clone", "--bare", "--no-local", "--no-hardlinks", sourceRepo, bareRepo], trustedDir);
rejectObjectAlternates(bareRepo);
const trustedGitEnvironment = { GIT_DIR: bareRepo, GIT_INDEX_FILE: indexPath };
git(["cat-file", "-e", `${caseEntry.historical_base_commit}^{commit}`], trustedDir, {
  env: trustedGitEnvironment,
});
git(["read-tree", "--empty"], trustedDir, { env: trustedGitEnvironment });

const indexEntries = [];
for (const entry of rawTree.entries) {
  const oid = outputText(
    git(["hash-object", "-w", "--no-filters", "--stdin"], trustedDir, {
      env: trustedGitEnvironment,
      input: entry.content,
    }),
  );
  if (!/^[0-9a-f]{40}$/.test(oid)) fail(`invalid blob object ID for ${entry.path}`);
  indexEntries.push({ path: entry.path, mode: entry.mode, oid });
}

const baseTreeBytes = Buffer.from(
  git(
    ["ls-tree", "-r", "-z", "--full-tree", caseEntry.historical_base_commit],
    trustedDir,
    { env: trustedGitEnvironment },
  ).stdout,
);
for (const record of parseNullTerminated(baseTreeBytes)) {
  const match = /^(\d+) (\w+) ([0-9a-f]{40})\t([\s\S]+)$/.exec(record);
  if (!match) fail("trusted base tree contains an invalid entry");
  const [, mode, , oid, path] = match;
  if (pathIsExcluded(path, exclusions)) indexEntries.push({ path, mode, oid });
}
indexEntries.sort((left, right) => Buffer.compare(Buffer.from(left.path), Buffer.from(right.path)));
const indexInput = Buffer.concat(
  indexEntries.map(({ mode, oid, path }) => Buffer.from(`${mode} ${oid}\t${path}\0`, "utf8")),
);
git(["update-index", "-z", "--index-info"], trustedDir, {
  env: trustedGitEnvironment,
  input: indexInput,
});
const candidateTree = outputText(
  git(["write-tree"], trustedDir, { env: trustedGitEnvironment }),
);
if (!/^[0-9a-f]{40}$/.test(candidateTree)) fail("trusted candidate tree is invalid");

const commitEnvironment = {
  ...trustedGitEnvironment,
  GIT_AUTHOR_NAME: "Tachiko Benchmark Capture",
  GIT_AUTHOR_EMAIL: "capture.invalid@example.invalid",
  GIT_COMMITTER_NAME: "Tachiko Benchmark Capture",
  GIT_COMMITTER_EMAIL: "capture.invalid@example.invalid",
  GIT_AUTHOR_DATE: "2000-01-01T00:00:00Z",
  GIT_COMMITTER_DATE: "2000-01-01T00:00:00Z",
};
const candidateCommit = outputText(
  git(
    ["commit-tree", candidateTree, "-p", caseEntry.historical_base_commit],
    trustedDir,
    { env: commitEnvironment, input: Buffer.from("trusted raw candidate\n") },
  ),
);
if (!/^[0-9a-f]{40}$/.test(candidateCommit)) fail("trusted candidate commit is invalid");

const patchBytes = Buffer.from(
  git(
    [
      "diff",
      "--binary",
      "--full-index",
      "--no-ext-diff",
      "--no-textconv",
      caseEntry.historical_base_commit,
      candidateTree,
      "--",
    ],
    trustedDir,
    { env: trustedGitEnvironment },
  ).stdout,
);
if (
  patchBytes.length > 0 &&
  !patchBytes.subarray(0, 11).equals(Buffer.from("diff --git ", "utf8"))
) {
  fail("captured non-empty candidate patch is not a raw applyable Git patch");
}
const changedFiles = parseNullTerminated(
  Buffer.from(
    git(
      [
        "diff",
        "--name-only",
        "-z",
        "--no-ext-diff",
        "--no-textconv",
        caseEntry.historical_base_commit,
        candidateTree,
        "--",
      ],
      trustedDir,
      { env: trustedGitEnvironment },
    ).stdout,
  ),
);
if (changedFiles.some((path) => pathIsExcluded(path, exclusions))) {
  fail("captured candidate patch changes an excluded path");
}
const numstat = Buffer.from(
  git(
    [
      "diff",
      "--numstat",
      "--no-ext-diff",
      "--no-textconv",
      caseEntry.historical_base_commit,
      candidateTree,
      "--",
    ],
    trustedDir,
    { env: trustedGitEnvironment },
  ).stdout,
).toString("utf8");
const totals = numstat
  .split("\n")
  .filter(Boolean)
  .reduce(
    (sum, line) => {
      const [insertions, deletions] = line.split("\t");
      if (/^\d+$/.test(insertions)) sum.insertions += Number(insertions);
      if (/^\d+$/.test(deletions)) sum.deletions += Number(deletions);
      return sum;
    },
    { insertions: 0, deletions: 0 },
  );

const patchPath = resolve(trustedDir, "candidate.patch");
const manifestPath = resolve(trustedDir, "raw-manifest.json");
await Promise.all([
  writeFile(patchPath, patchBytes, { mode: 0o600 }),
  writeFile(manifestPath, rawTree.manifestBytes, { mode: 0o600 }),
]);

const roundTripPath = resolve(trustedDir, "round-trip");
await mkdir(roundTripPath, { mode: 0o700 });
await materializeTrustedTree(candidateTree, roundTripPath, exclusions, bareRepo, trustedDir);
const roundTripTree = await collectRawTree(roundTripPath, exclusions);
if (
  roundTripTree.digest !== rawTree.digest ||
  !roundTripTree.manifestBytes.equals(rawTree.manifestBytes)
) {
  fail("trusted candidate tree failed raw filesystem round-trip equality");
}
const roundTripManifestPath = resolve(trustedDir, "round-trip-manifest.json");
await writeFile(roundTripManifestPath, roundTripTree.manifestBytes, { mode: 0o600 });

const indexBytes = await readFile(indexPath);
const receipt = {
  protocol_id: manifest.protocol_id,
  case_id: caseId,
  historical_base_commit: caseEntry.historical_base_commit,
  workspace,
  source_repo: { path: sourceRepo, type: "directory", ...identity(sourceStat) },
  overlay: {
    path: "AGENTS.md",
    type: "regular",
    sha256: overlaySha256,
    bytes: overlayBytes.length,
    ...identity(overlayStat),
  },
  exclusions: {
    file_path: exclusionsFile,
    file_sha256: sha256(exclusionsBytes),
    file_bytes: exclusionsBytes.length,
    paths: exclusions,
  },
  raw_manifest: {
    path: manifestPath,
    sha256: rawTree.digest,
    bytes: rawTree.manifestBytes.length,
    entries: rawTree.entries.length,
  },
  raw_tree_digest_sha256: rawTree.digest,
  capture_exclusion_list_sha256: sha256(exclusionsBytes),
  trusted_raw_capture: true,
  trusted_object_database: { path: bareRepo },
  trusted_index: { path: indexPath, sha256: sha256(indexBytes), bytes: indexBytes.length },
  candidate_commit: candidateCommit,
  candidate_tree: candidateTree,
  head_after: candidateCommit,
  commits_ahead: 1,
  agents_sha256_after: overlaySha256,
  agents_unchanged: true,
  diff_sha256: sha256(patchBytes),
  diff_bytes: patchBytes.length,
  changed_files: changedFiles,
  insertions: totals.insertions,
  deletions: totals.deletions,
  empty_patch: patchBytes.length === 0,
  round_trip: {
    path: roundTripPath,
    manifest_path: roundTripManifestPath,
    digest_sha256: roundTripTree.digest,
    equal: true,
  },
  round_trip_digest_sha256: roundTripTree.digest,
  created_at: new Date().toISOString(),
};
await writeFile(
  resolve(trustedDir, "capture-receipt.json"),
  `${JSON.stringify(receipt, null, 2)}\n`,
  { mode: 0o600 },
);
console.log(JSON.stringify(receipt));
