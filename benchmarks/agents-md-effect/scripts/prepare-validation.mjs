#!/usr/bin/env node

import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { lstat, mkdir, readFile, readdir, readlink, realpath, writeFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { basename, dirname, isAbsolute, posix, relative, resolve } from "node:path";
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
    "usage: node prepare-validation.mjs --case TW-03 --source-repo /abs/repo " +
      "--patch-file /abs/candidate.patch --capture-receipt /abs/capture-receipt.json " +
      "--workspace /abs/validation-copy --trusted-dir /abs/trusted-output",
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
    "source-repo",
    "patch-file",
    "capture-receipt",
    "workspace",
    "trusted-dir",
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

async function canonicalizeProspectivePath(path, label) {
  let existingAncestor = path;
  const suffix = [];
  for (;;) {
    try {
      await lstat(existingAncestor);
      break;
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
      const parent = dirname(existingAncestor);
      if (parent === existingAncestor) fail(`${label} has no existing ancestor`);
      suffix.unshift(basename(existingAncestor));
      existingAncestor = parent;
    }
  }
  let canonicalAncestor;
  try {
    canonicalAncestor = await realpath(existingAncestor);
  } catch {
    fail(`${label} has an invalid or broken symlink ancestor`);
  }
  const ancestorStat = await lstat(canonicalAncestor);
  if (!ancestorStat.isDirectory()) fail(`${label} ancestor must be a directory`);
  return resolve(canonicalAncestor, ...suffix);
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

function sameIdentity(actual, expected) {
  return (
    expected &&
    expected.device === actual.dev.toString() &&
    expected.inode === actual.ino.toString() &&
    expected.uid === actual.uid.toString() &&
    expected.gid === actual.gid.toString() &&
    expected.mode === Number(actual.mode & 0o7777n)
  );
}

function validateExpectedOverlayIdentity(value, bytes) {
  const keys = [
    "schema",
    "path",
    "type",
    "device",
    "inode",
    "uid",
    "gid",
    "mode",
    "bytes",
    "sha256",
  ];
  if (
    !value ||
    typeof value !== "object" ||
    Array.isArray(value) ||
    JSON.stringify(Object.keys(value)) !== JSON.stringify(keys) ||
    value.schema !== "tachiko-agents-overlay-identity-v1" ||
    value.path !== "AGENTS.md" ||
    value.type !== "regular" ||
    ![value.device, value.inode, value.uid, value.gid].every(
      (entry) => typeof entry === "string" && /^\d+$/.test(entry),
    ) ||
    !Number.isInteger(value.mode) ||
    value.mode < 0 ||
    value.mode > 0o7777 ||
    !Number.isSafeInteger(value.bytes) ||
    value.bytes < 0 ||
    !/^[0-9a-f]{64}$/.test(value.sha256)
  ) {
    fail("capture receipt has an invalid pre-run overlay identity contract");
  }
  const canonicalBytes = Buffer.from(`${JSON.stringify(value, null, 2)}\n`, "utf8");
  if (!canonicalBytes.equals(bytes)) {
    fail("pre-run overlay identity file is not canonical JSON");
  }
  return value;
}

function validateExclusions(value) {
  if (!Array.isArray(value)) fail("capture receipt exclusions must be an array");
  const normalizedPaths = [];
  const seen = new Set();
  for (const path of value) {
    if (
      typeof path !== "string" ||
      path === "" ||
      path.includes("\\") ||
      path.includes("\0") ||
      posix.isAbsolute(path)
    ) {
      fail(`capture receipt contains invalid exclusion path: ${JSON.stringify(path)}`);
    }
    const segments = [];
    for (const segment of path.split("/")) {
      if (segment === "" || segment === ".") continue;
      if (segment === "..") {
        fail(`capture receipt contains invalid exclusion path: ${JSON.stringify(path)}`);
      }
      segments.push(segment);
    }
    const normalized = segments.join("/");
    if (
      normalized === "" ||
      normalized === ".git" ||
      normalized.startsWith(".git/") ||
      normalized === "AGENTS.md" ||
      seen.has(normalized)
    ) {
      fail(`capture receipt contains invalid exclusion path: ${JSON.stringify(path)}`);
    }
    seen.add(normalized);
    normalizedPaths.push(normalized);
  }
  return normalizedPaths;
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
    fail("validation workspace contains an invalid non-UTF-8 path component");
  }
  return decoded;
}

async function rawManifest(root, exclusions) {
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
      const path = relativeDirectory === "" ? name : `${relativeDirectory}/${name}`;
      if (pathIsExcluded(path, exclusions)) continue;
      const absolutePath = resolve(directory, name);
      if (!isInside(absolutePath, root)) fail(`validation path escapes workspace: ${path}`);
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
        });
      } else if (metadata.isSymbolicLink()) {
        const bytes = await readlink(absolutePath, { encoding: "buffer" });
        entries.push({
          path,
          type: "symlink",
          mode: "120000",
          bytes: bytes.length,
          sha256: sha256(bytes),
        });
      } else {
        fail(`validation workspace contains unsupported filesystem node: ${path}`);
      }
    }
  }
  await walk(root, "");
  const bytes = Buffer.from(`${JSON.stringify({ version: 1, entries }, null, 2)}\n`, "utf8");
  return { bytes, digest: sha256(bytes), entries: entries.length };
}

async function verifiedPath(path, captureDir, label) {
  if (typeof path !== "string" || !isAbsolute(path)) fail(`${label} path is invalid`);
  const actual = await realpath(path);
  if (!isInside(actual, captureDir)) fail(`${label} must remain inside capture artifacts`);
  return actual;
}

const args = parseArgs(process.argv.slice(2));
for (const key of [
  "case",
  "source-repo",
  "patch-file",
  "capture-receipt",
  "workspace",
  "trusted-dir",
]) {
  if (!args.has(key)) usage();
  if (key !== "case" && !isAbsolute(args.get(key))) fail(`${key} must be an absolute path`);
}

const caseId = args.get("case");
const sourceRepo = await realpath(args.get("source-repo"));
const patchFile = await realpath(args.get("patch-file"));
const captureReceiptPath = await realpath(args.get("capture-receipt"));
const captureDir = dirname(captureReceiptPath);
const workspace = await canonicalizeProspectivePath(resolve(args.get("workspace")), "workspace");
const trustedDir = await canonicalizeProspectivePath(
  resolve(args.get("trusted-dir")),
  "trusted-dir",
);
for (const candidate of [workspace, trustedDir]) {
  if (existsSync(candidate)) fail("workspace and trusted-dir must not exist");
}
for (const [leftName, left, rightName, right] of [
  ["workspace", workspace, "trusted-dir", trustedDir],
  ["workspace", workspace, "source-repo", sourceRepo],
  ["trusted-dir", trustedDir, "source-repo", sourceRepo],
  ["workspace", workspace, "capture artifacts", captureDir],
  ["trusted-dir", trustedDir, "capture artifacts", captureDir],
]) {
  if (isInside(left, right) || isInside(right, left)) {
    fail(`${leftName} and ${rightName} must be disjoint`);
  }
}

const manifest = JSON.parse(
  await readFile(resolve(benchmarkDir, "evaluator/cases.json"), "utf8"),
);
const caseEntry = manifest.cases.find((candidate) => candidate.id === caseId);
if (!caseEntry) fail(`unknown case ${caseId}`);
const patchBytes = await readFile(patchFile);
if (patchBytes.length === 0) fail("empty candidate patch is a hard failure, not a validation input");
const captureReceiptBytes = await readFile(captureReceiptPath);
const captureReceipt = JSON.parse(captureReceiptBytes.toString("utf8"));

if (
  captureReceipt.protocol_id !== manifest.protocol_id ||
  captureReceipt.case_id !== caseId ||
  captureReceipt.historical_base_commit !== caseEntry.historical_base_commit ||
  captureReceipt.trusted_raw_capture !== true ||
  captureReceipt.diff_sha256 !== sha256(patchBytes) ||
  captureReceipt.diff_bytes !== patchBytes.length ||
  !/^[0-9a-f]{40}$/.test(captureReceipt.candidate_commit) ||
  !/^[0-9a-f]{40}$/.test(captureReceipt.candidate_tree)
) {
  fail("capture receipt does not bind this protocol, case, base, patch, and candidate");
}
if (
  !captureReceipt.source_repo ||
  captureReceipt.source_repo.path !== sourceRepo ||
  captureReceipt.source_repo.type !== "directory"
) {
  fail("capture receipt does not bind the trusted source repository");
}
const sourceStat = await lstat(sourceRepo, { bigint: true });
if (!sourceStat.isDirectory() || !sameIdentity(sourceStat, captureReceipt.source_repo)) {
  fail("trusted source repository identity changed after capture");
}

if (
  !captureReceipt.overlay_pre_run ||
  !captureReceipt.overlay ||
  captureReceipt.overlay_identity_equal !== true ||
  captureReceipt.overlay.schema !== "tachiko-agents-overlay-identity-v1" ||
  captureReceipt.overlay.path !== "AGENTS.md" ||
  captureReceipt.overlay.type !== "regular" ||
  captureReceipt.agents_sha256_after !== captureReceipt.overlay.sha256 ||
  captureReceipt.agents_unchanged !== true
) {
  fail("capture receipt does not bind the root AGENTS.md overlay");
}
if (
  typeof captureReceipt.overlay_pre_run.file_path !== "string" ||
  !isAbsolute(captureReceipt.overlay_pre_run.file_path)
) {
  fail("capture receipt pre-run overlay identity path is invalid");
}
const overlayIdentityInputStat = await lstat(captureReceipt.overlay_pre_run.file_path, {
  bigint: true,
});
if (!overlayIdentityInputStat.isFile() || overlayIdentityInputStat.isSymbolicLink()) {
  fail("pre-run overlay identity must remain a regular non-symlink file");
}
const overlayIdentityPath = await realpath(captureReceipt.overlay_pre_run.file_path);
const overlayIdentityStat = await lstat(overlayIdentityPath, { bigint: true });
const overlayIdentityBytes = await readFile(overlayIdentityPath);
const expectedOverlayIdentity = validateExpectedOverlayIdentity(
  JSON.parse(overlayIdentityBytes.toString("utf8")),
  overlayIdentityBytes,
);
if (
  !overlayIdentityStat.isFile() ||
  overlayIdentityStat.isSymbolicLink() ||
  captureReceipt.overlay_pre_run.file_sha256 !== sha256(overlayIdentityBytes) ||
  captureReceipt.overlay_pre_run.file_bytes !== overlayIdentityBytes.length ||
  JSON.stringify(captureReceipt.overlay_pre_run.expected) !==
    JSON.stringify(expectedOverlayIdentity) ||
  JSON.stringify(captureReceipt.overlay) !== JSON.stringify(expectedOverlayIdentity)
) {
  fail("capture receipt pre-run and post-run overlay identities do not match");
}
const capturedWorkspace = await realpath(captureReceipt.workspace);
const overlayPath = resolve(capturedWorkspace, "AGENTS.md");
const overlayStat = await lstat(overlayPath, { bigint: true });
const overlayBytes = await readFile(overlayPath);
if (
  !overlayStat.isFile() ||
  overlayStat.isSymbolicLink() ||
  !sameIdentity(overlayStat, captureReceipt.overlay) ||
  overlayBytes.length !== captureReceipt.overlay.bytes ||
  sha256(overlayBytes) !== captureReceipt.overlay.sha256
) {
  fail("captured root AGENTS.md overlay type, identity, or hash changed");
}

if (!captureReceipt.exclusions) fail("capture receipt is missing exclusions");
if (
  typeof captureReceipt.exclusions.file_path !== "string" ||
  !isAbsolute(captureReceipt.exclusions.file_path)
) {
  fail("capture receipt exclusion-list path is invalid");
}
const exclusionsInputStat = await lstat(captureReceipt.exclusions.file_path, { bigint: true });
if (!exclusionsInputStat.isFile() || exclusionsInputStat.isSymbolicLink()) {
  fail("capture exclusion list must remain a regular non-symlink file");
}
const exclusionsPath = await realpath(captureReceipt.exclusions.file_path);
const exclusionsStat = await lstat(exclusionsPath, { bigint: true });
const exclusionsBytes = await readFile(exclusionsPath);
const exclusions = validateExclusions(JSON.parse(exclusionsBytes.toString("utf8")));
if (
  !exclusionsStat.isFile() ||
  exclusionsStat.isSymbolicLink() ||
  exclusionsBytes.length !== captureReceipt.exclusions.file_bytes ||
  sha256(exclusionsBytes) !== captureReceipt.exclusions.file_sha256 ||
  captureReceipt.capture_exclusion_list_sha256 !== captureReceipt.exclusions.file_sha256 ||
  JSON.stringify(exclusions) !== JSON.stringify(captureReceipt.exclusions.paths)
) {
  fail("capture exclusion list bytes or paths do not match the receipt");
}

const manifestPath = await verifiedPath(captureReceipt.raw_manifest?.path, captureDir, "raw manifest");
const manifestBytes = await readFile(manifestPath);
const parsedManifest = JSON.parse(manifestBytes.toString("utf8"));
if (
  captureReceipt.raw_manifest.sha256 !== sha256(manifestBytes) ||
  captureReceipt.raw_manifest.bytes !== manifestBytes.length ||
  captureReceipt.raw_manifest.entries !== parsedManifest.entries?.length ||
  captureReceipt.raw_tree_digest_sha256 !== captureReceipt.raw_manifest.sha256
) {
  fail("raw manifest bytes and digest do not match the capture receipt");
}
const roundTripManifestPath = await verifiedPath(
  captureReceipt.round_trip?.manifest_path,
  captureDir,
  "round-trip manifest",
);
const roundTripManifestBytes = await readFile(roundTripManifestPath);
if (
  captureReceipt.round_trip.equal !== true ||
  captureReceipt.round_trip.digest_sha256 !== sha256(roundTripManifestBytes) ||
  captureReceipt.round_trip.digest_sha256 !== captureReceipt.raw_tree_digest_sha256 ||
  captureReceipt.round_trip_digest_sha256 !== captureReceipt.raw_tree_digest_sha256 ||
  !roundTripManifestBytes.equals(manifestBytes)
) {
  fail("capture round-trip manifest equality is not proven");
}

const capturedObjectDatabase = await verifiedPath(
  captureReceipt.trusted_object_database?.path,
  captureDir,
  "trusted object database",
);
const capturedIndex = await verifiedPath(
  captureReceipt.trusted_index?.path,
  captureDir,
  "trusted index",
);
const capturedIndexBytes = await readFile(capturedIndex);
if (
  captureReceipt.trusted_index.sha256 !== sha256(capturedIndexBytes) ||
  captureReceipt.trusted_index.bytes !== capturedIndexBytes.length
) {
  fail("trusted capture index bytes do not match the receipt");
}
const capturedGitEnvironment = { GIT_DIR: capturedObjectDatabase };
rejectObjectAlternates(capturedObjectDatabase);
git(["cat-file", "-e", `${captureReceipt.candidate_commit}^{commit}`], captureDir, {
  env: capturedGitEnvironment,
});
const capturedCommitTree = outputText(
  git(["show", "-s", "--format=%T", captureReceipt.candidate_commit], captureDir, {
    env: capturedGitEnvironment,
  }),
);
const capturedCommitParents = outputText(
  git(["show", "-s", "--format=%P", captureReceipt.candidate_commit], captureDir, {
    env: capturedGitEnvironment,
  }),
);
if (
  capturedCommitTree !== captureReceipt.candidate_tree ||
  capturedCommitParents !== caseEntry.historical_base_commit
) {
  fail("trusted candidate commit does not bind the captured tree and frozen base");
}
const receiptChangedFiles = parseNullTerminated(
  Buffer.from(
    git(
      [
        "diff",
        "--name-only",
        "-z",
        "--no-ext-diff",
        "--no-textconv",
        caseEntry.historical_base_commit,
        captureReceipt.candidate_tree,
        "--",
      ],
      captureDir,
      { env: capturedGitEnvironment },
    ).stdout,
  ),
);
if (
  JSON.stringify(receiptChangedFiles) !== JSON.stringify(captureReceipt.changed_files) ||
  receiptChangedFiles.some((path) => pathIsExcluded(path, exclusions))
) {
  fail("capture receipt changed-file list is invalid");
}

await mkdir(trustedDir, { recursive: false, mode: 0o700 });
const bareRepo = resolve(trustedDir, "source.git");
const bundlePath = resolve(trustedDir, "base.bundle");
git(["clone", "--bare", "--no-local", "--no-hardlinks", sourceRepo, bareRepo], trustedDir);
rejectObjectAlternates(bareRepo);
const bareEnvironment = { GIT_DIR: bareRepo };
git(["cat-file", "-e", `${caseEntry.historical_base_commit}^{commit}`], trustedDir, {
  env: bareEnvironment,
});
git(["update-ref", "refs/heads/benchmark-base", caseEntry.historical_base_commit], trustedDir, {
  env: bareEnvironment,
});
git(["bundle", "create", bundlePath, "refs/heads/benchmark-base"], trustedDir, {
  env: bareEnvironment,
});
git(["bundle", "verify", bundlePath], trustedDir, { env: bareEnvironment });
await mkdir(dirname(workspace), { recursive: true });
git(["clone", "--branch", "benchmark-base", bundlePath, workspace], dirname(workspace));
git(["remote", "remove", "origin"], workspace);

const targetLookup = git(
  ["cat-file", "-e", `${caseEntry.ground_truth_commit}^{commit}`],
  workspace,
  { allowFailure: true },
);
if (targetLookup.status === 0) fail("ground-truth target leaked into validation workspace");
git(["apply", "--index", "--binary", patchFile], workspace);
const appliedTree = outputText(git(["write-tree"], workspace));
if (appliedTree !== captureReceipt.candidate_tree) {
  fail(`captured candidate tree mismatch after apply: expected ${captureReceipt.candidate_tree}, got ${appliedTree}`);
}
const stagedFiles = parseNullTerminated(
  Buffer.from(git(["diff", "--cached", "--name-only", "-z", "HEAD", "--"], workspace).stdout),
);
if (
  JSON.stringify(stagedFiles) !== JSON.stringify(captureReceipt.changed_files) ||
  stagedFiles.some((path) => pathIsExcluded(path, exclusions))
) {
  fail("applied patch changed-file list does not match the verified capture receipt");
}
const appliedManifest = await rawManifest(workspace, exclusions);
if (
  appliedManifest.digest !== captureReceipt.raw_tree_digest_sha256 ||
  !appliedManifest.bytes.equals(manifestBytes)
) {
  fail("applied validation workspace does not equal the captured raw manifest");
}

const commitEnvironment = {
  GIT_AUTHOR_NAME: "Tachiko Benchmark Capture",
  GIT_AUTHOR_EMAIL: "capture.invalid@example.invalid",
  GIT_COMMITTER_NAME: "Tachiko Benchmark Capture",
  GIT_COMMITTER_EMAIL: "capture.invalid@example.invalid",
  GIT_AUTHOR_DATE: "2000-01-01T00:00:00Z",
  GIT_COMMITTER_DATE: "2000-01-01T00:00:00Z",
};
git(["commit", "--no-verify", "--no-gpg-sign", "-m", "trusted raw candidate"], workspace, {
  env: commitEnvironment,
});
const candidateCommit = outputText(git(["rev-parse", "HEAD"], workspace));
const candidateTree = outputText(git(["show", "-s", "--format=%T", "HEAD"], workspace));
if (
  candidateCommit !== captureReceipt.candidate_commit ||
  candidateTree !== captureReceipt.candidate_tree
) {
  fail("deterministic validation commit does not match trusted capture commit/tree");
}
const status = outputText(git(["status", "--porcelain"], workspace));
if (status !== "" && status !== "ok") fail(`validation workspace is dirty: ${status}`);

const receipt = {
  protocol_id: manifest.protocol_id,
  case_id: caseId,
  historical_base_commit: caseEntry.historical_base_commit,
  ground_truth_commit_absent_before_overlay: true,
  candidate_patch_sha256: sha256(patchBytes),
  capture_receipt_sha256: sha256(captureReceiptBytes),
  capture_receipt_verified: true,
  trusted_raw_capture: true,
  raw_tree_digest_sha256: captureReceipt.raw_tree_digest_sha256,
  capture_exclusion_list_sha256: captureReceipt.capture_exclusion_list_sha256,
  round_trip_digest_sha256: captureReceipt.round_trip_digest_sha256,
  captured_candidate_commit: captureReceipt.candidate_commit,
  captured_candidate_tree: captureReceipt.candidate_tree,
  capture_to_apply_tree_equal: true,
  candidate_commit: candidateCommit,
  candidate_tree: candidateTree,
  evaluator_oracle_present: false,
  clean_for_candidate_validation: true,
  created_at: new Date().toISOString(),
};
await writeFile(
  resolve(trustedDir, "validation-preparation-receipt.json"),
  `${JSON.stringify(receipt, null, 2)}\n`,
  { mode: 0o600 },
);
console.log(JSON.stringify(receipt));
