#!/usr/bin/env node

import {existsSync} from "node:fs";
import {lstat, mkdir, readFile, readdir, realpath, writeFile} from "node:fs/promises";
import {basename, dirname, isAbsolute, relative, resolve} from "node:path";
import {
  REDACTION,
  RULE_IDS,
  canonicalBytes,
  compileMatcher,
  loadBlindingInputs,
  makeScanReceipt,
  scanPacketTree,
  sha256,
  splitLines,
  strictText,
} from "./scan-review-packet.mjs";

function usage() {
  console.error(
    "usage: node build-review-packet.mjs --case-id TW-01 --candidate-id <32-hex> " +
      "--input-root /abs/input --variant /abs/variant [--variant /abs/variant] " +
      "--contract /abs/contract.json --output-dir /abs/new-output " +
      "--custodian-id <opaque-id> --custodian-eligible true --frozen-at <RFC3339>",
  );
  process.exit(2);
}

function parseArgs(argv) {
  const values = new Map();
  const variants = [];
  for (let index = 0; index < argv.length; index += 2) {
    const flag = argv[index];
    const value = argv[index + 1];
    if (!flag?.startsWith("--") || value === undefined) usage();
    const key = flag.slice(2);
    if (key === "variant") variants.push(value);
    else if (values.has(key)) throw new Error(`duplicate --${key}`);
    else values.set(key, value);
  }
  return {values, variants};
}

function isWithin(parent, child) {
  const path = relative(parent, child);
  return path === "" || (!path.startsWith("..") && !isAbsolute(path));
}

function byteSort(left, right) {
  return Buffer.from(left.path, "utf8").compare(Buffer.from(right.path, "utf8"));
}

async function requireRegular(path, label) {
  const metadata = await lstat(path);
  if (!metadata.isFile() || metadata.isSymbolicLink()) {
    throw new Error(`${label} must be a non-symlink regular file`);
  }
}

async function requireDirectory(path, label) {
  const metadata = await lstat(path);
  if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
    throw new Error(`${label} must be a non-symlink directory`);
  }
}

async function canonicalNewPath(path, label) {
  if (existsSync(path)) throw new Error(`${label} must not already exist`);
  const realParent = await realpath(dirname(path));
  return resolve(realParent, basename(path));
}

async function walkInput(root) {
  const files = [];
  async function walk(directory, prefix = "") {
    const entries = await readdir(directory, {encoding: "buffer", withFileTypes: true});
    entries.sort((left, right) => Buffer.compare(left.name, right.name));
    for (const entry of entries) {
      const name = strictText(entry.name, "reviewer-visible path component");
      const path = prefix ? `${prefix}/${name}` : name;
      const absolutePath = resolve(directory, name);
      const metadata = await lstat(absolutePath);
      if (metadata.isSymbolicLink()) {
        throw new Error(`${path} is a symlink; reviewer-visible artifacts must be regular`);
      }
      if (metadata.isDirectory()) await walk(absolutePath, path);
      else if (metadata.isFile()) files.push({absolutePath, path});
      else throw new Error(`${path} is not a regular file or directory`);
    }
  }
  await walk(root);
  files.sort(byteSort);
  return files;
}

function emptyRuleCounts() {
  return Object.fromEntries(RULE_IDS.map((ruleId) => [ruleId, 0]));
}

function addRule(counts, ruleId) {
  counts[ruleId] += 1;
}

const {values, variants: variantArguments} = parseArgs(process.argv.slice(2));
for (const key of [
  "case-id",
  "candidate-id",
  "input-root",
  "contract",
  "output-dir",
  "custodian-id",
  "custodian-eligible",
  "frozen-at",
]) {
  if (!values.has(key)) usage();
}
for (const key of ["input-root", "contract", "output-dir"]) {
  if (!isAbsolute(values.get(key))) throw new Error(`--${key} must be an absolute path`);
}
if (variantArguments.length === 0 || variantArguments.some((path) => !isAbsolute(path))) {
  throw new Error("at least one absolute --variant path is required");
}
if (!/^TW-0[1-9]$/.test(values.get("case-id"))) throw new Error("invalid --case-id");
if (!/^[0-9a-f]{32}$/.test(values.get("candidate-id"))) {
  throw new Error("--candidate-id must be opaque lowercase 128-bit hex");
}
if (!/^[A-Za-z0-9_.-]{1,64}$/.test(values.get("custodian-id"))) {
  throw new Error("invalid --custodian-id");
}
if (values.get("custodian-eligible") !== "true") {
  throw new Error("custodian must be explicitly eligible");
}
if (new Date(values.get("frozen-at")).toISOString() !== values.get("frozen-at")) {
  throw new Error("--frozen-at must be canonical RFC3339 UTC");
}

await requireDirectory(resolve(values.get("input-root")), "input root");
await requireRegular(resolve(values.get("contract")), "contract");
for (const path of variantArguments) await requireRegular(resolve(path), "registered variant");
const inputRoot = await realpath(values.get("input-root"));
const contractPath = await realpath(values.get("contract"));
const variantPaths = await Promise.all(variantArguments.map((path) => realpath(path)));
const outputDir = await canonicalNewPath(resolve(values.get("output-dir")), "output directory");

for (const [label, path] of [
  ["output directory", outputDir],
  ["contract", contractPath],
  ...variantPaths.map((path) => ["registered variant", path]),
]) {
  if (isWithin(inputRoot, path) || isWithin(path, inputRoot)) {
    throw new Error(`${label} and candidate input must be disjoint; paths overlap`);
  }
}

const blinding = await loadBlindingInputs(contractPath, variantPaths);
const matcher = compileMatcher(blinding.variants);
const inputFiles = await walkInput(inputRoot);
const rendered = [];
const privateEvents = [];
const totalRuleCounts = emptyRuleCounts();
const displayedPaths = new Set();

for (const input of inputFiles) {
  const originalBytes = await readFile(input.absolutePath);
  const originalText = strictText(originalBytes, `${input.path} content`);
  const originalArtifactSha256 = sha256(originalBytes);
  const originalPathSha256 = sha256(Buffer.from(input.path, "utf8"));
  const opaquePathAlias = `artifact-${originalPathSha256}`;
  const pathRules = matcher(input.path);
  const displayPath = pathRules.length > 0 ? `redacted-path-${originalPathSha256}` : input.path;
  if (displayedPaths.has(displayPath)) throw new Error("rendered path collision");
  displayedPaths.add(displayPath);
  const pathCounts = emptyRuleCounts();
  const contentCounts = emptyRuleCounts();
  for (const ruleId of pathRules) {
    addRule(pathCounts, ruleId);
    addRule(totalRuleCounts, ruleId);
    privateEvents.push({
      _sort_path: input.path,
      original_artifact_sha256: originalArtifactSha256,
      opaque_path_alias: opaquePathAlias,
      line_number: 1,
      rule_id: ruleId,
      pre_sha256: originalPathSha256,
      post_sha256: sha256(Buffer.from(displayPath, "utf8")),
    });
  }
  const renderedLines = [];
  for (const [index, line] of splitLines(originalText).entries()) {
    const rules = matcher(line.content);
    const renderedContent = rules.length > 0 ? REDACTION : line.content;
    for (const ruleId of rules) {
      addRule(contentCounts, ruleId);
      addRule(totalRuleCounts, ruleId);
      privateEvents.push({
        _sort_path: input.path,
        original_artifact_sha256: originalArtifactSha256,
        opaque_path_alias: opaquePathAlias,
        line_number: index + 1,
        rule_id: ruleId,
        pre_sha256: sha256(Buffer.from(line.content, "utf8")),
        post_sha256: sha256(Buffer.from(renderedContent, "utf8")),
      });
    }
    renderedLines.push(`${renderedContent}${line.terminator}`);
  }
  const renderedBytes = Buffer.from(renderedLines.join(""), "utf8");
  rendered.push({
    path: input.path,
    displayPath,
    bytes: renderedBytes,
    manifest: {
      display_path: displayPath,
      original_path_sha256: originalPathSha256,
      pre_render_bytes: originalBytes.length,
      pre_render_sha256: originalArtifactSha256,
      rendered_bytes: renderedBytes.length,
      rendered_sha256: sha256(renderedBytes),
      match_counts: {path: pathCounts, content: contentCounts},
    },
  });
}

privateEvents.sort((left, right) => {
  const pathOrder = Buffer.from(left._sort_path).compare(Buffer.from(right._sort_path));
  if (pathOrder !== 0) return pathOrder;
  if (left.line_number !== right.line_number) return left.line_number - right.line_number;
  return left.rule_id.localeCompare(right.rule_id);
});
const privateMap = {
  schema: "tachiko-review-packet-private-match-map-v1",
  contract_sha256: blinding.contractIdentity.sha256,
  variant_set_commitment_sha256: blinding.variantSet.commitment_sha256,
  events: privateEvents.map(({_sort_path, ...event}) => event),
};
const privateMapBytes = canonicalBytes(privateMap);
const publicManifest = {
  schema: "tachiko-review-packet-public-manifest-v1",
  protocol_id: blinding.contract.protocol_id,
  case_id: values.get("case-id"),
  candidate_id: values.get("candidate-id"),
  frozen_at: values.get("frozen-at"),
  contract_sha256: blinding.contractIdentity.sha256,
  rule_set_commitment_sha256: sha256(canonicalBytes(blinding.contract.machine_match_rules)),
  variant_set_commitment_sha256: blinding.variantSet.commitment_sha256,
  artifacts: rendered.map((entry) => entry.manifest),
  match_counts_by_rule: totalRuleCounts,
  safe_to_scan: true,
  semantic_scoring_performed: false,
};
const publicManifestBytes = canonicalBytes(publicManifest);

const packetDir = resolve(outputDir, "packet");
await mkdir(packetDir, {recursive: true, mode: 0o700});
for (const artifact of rendered) {
  const destination = resolve(packetDir, artifact.displayPath);
  if (!isWithin(packetDir, destination)) throw new Error("rendered artifact escaped packet root");
  await mkdir(dirname(destination), {recursive: true, mode: 0o700});
  await writeFile(destination, artifact.bytes, {mode: 0o600, flag: "wx"});
}
await writeFile(resolve(packetDir, "packet-manifest.json"), publicManifestBytes, {
  mode: 0o600,
  flag: "wx",
});
await writeFile(resolve(outputDir, "private-match-map.json"), privateMapBytes, {
  mode: 0o600,
  flag: "wx",
});

const scan = await scanPacketTree(packetDir, blinding);
const scanReceipt = makeScanReceipt(scan, blinding);
const scanReceiptBytes = canonicalBytes(scanReceipt);
await writeFile(resolve(outputDir, "scan-receipt.json"), scanReceiptBytes, {
  mode: 0o600,
  flag: "wx",
});

const receipt = {
  schema: "tachiko-review-packet-receipt-v1",
  classification: "construction_pilot_only",
  formal_result_eligible: false,
  protocol_id: blinding.contract.protocol_id,
  case_id: values.get("case-id"),
  candidate_id: values.get("candidate-id"),
  frozen_at: values.get("frozen-at"),
  custodian: {id: values.get("custodian-id"), eligible: true},
  contract: blinding.contractIdentity,
  variant_set: blinding.variantSet,
  artifact_manifest_sha256: sha256(publicManifestBytes),
  private_match_map_sha256: sha256(privateMapBytes),
  scan_receipt_sha256: sha256(scanReceiptBytes),
  rendered_packet_sha256: scan.tree_sha256,
  match_counts_by_rule: totalRuleCounts,
  post_render_scan: {
    match_count: scan.match_count,
    match_counts_by_rule: scan.match_counts_by_rule,
    safe_to_release: scan.safe_to_release,
  },
  safe_to_release: scan.safe_to_release,
  terminal_classification: scan.safe_to_release ? "qualified" : "invalid_discarded",
  semantic_scoring_performed: false,
  multi_reviewer_panel_claimed: false,
};
await writeFile(resolve(outputDir, "receipt.json"), canonicalBytes(receipt), {
  mode: 0o600,
  flag: "wx",
});
console.log(JSON.stringify(receipt));
if (!scan.safe_to_release) process.exitCode = 1;
