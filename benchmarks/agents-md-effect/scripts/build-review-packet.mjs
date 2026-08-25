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
  validateRequiredReviewRoles,
} from "./scan-review-packet.mjs";
import {constructionEvidenceContext, loadControllerContext} from "./controller-context.mjs";

function usage() {
  console.error(
    "usage: node build-review-packet.mjs --case-id TW-01 --candidate-id <32-hex> " +
      "--input-root /abs/input --input-manifest /abs/input.json " +
      "--variant /abs/variant [--variant /abs/variant] " +
      "--contract /abs/contract.json --output-dir /abs/new-output " +
      "--terminal-receipt /abs/terminal.json " +
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

async function bindInputManifest(manifestPath, inputFiles) {
  const manifestBytes = await readFile(manifestPath);
  const manifest = JSON.parse(strictText(manifestBytes, "review input manifest"));
  if (manifest.schema !== "tachiko-review-packet-input-v1" || !Array.isArray(manifest.artifacts)) {
    throw new Error("invalid review input manifest schema");
  }
  const entries = [...manifest.artifacts];
  entries.sort(byteSort);
  if (JSON.stringify(entries) !== JSON.stringify(manifest.artifacts)) {
    throw new Error("review input manifest artifacts must use unsigned UTF-8 path order");
  }
  if (new Set(entries.map((entry) => entry.path)).size !== entries.length) {
    throw new Error("review input manifest contains duplicate paths");
  }
  const actualPaths = inputFiles.map((entry) => entry.path);
  if (JSON.stringify(entries.map((entry) => entry.path)) !== JSON.stringify(actualPaths)) {
    throw new Error("review input manifest must bind every and only reviewer-visible artifact");
  }
  const reviewRoles = [];
  for (const [index, entry] of entries.entries()) {
    if (
      typeof entry.path !== "string" ||
      !Array.isArray(entry.roles) ||
      entry.roles.length === 0 ||
      !Number.isSafeInteger(entry.bytes) ||
      entry.bytes < 0 ||
      !/^[0-9a-f]{64}$/.test(entry.sha256)
    ) {
      throw new Error("invalid review input manifest artifact");
    }
    if (entry.roles.length !== 1 || new Set(entry.roles).size !== 1) {
      throw new Error("exactly one review artifact role is required for each artifact");
    }
    for (const role of entry.roles) {
      reviewRoles.push(role);
    }
    const bytes = await readFile(inputFiles[index].absolutePath);
    if (bytes.length !== entry.bytes || sha256(bytes) !== entry.sha256) {
      throw new Error("review input artifact differs from its trusted manifest");
    }
    inputFiles[index].bytes = bytes;
    inputFiles[index].roles = entry.roles;
  }
  validateRequiredReviewRoles(reviewRoles, "review artifact");
  return {bytes: manifestBytes, sha256: sha256(manifestBytes)};
}

let terminalReceiptPath;
let terminalContext = {};
let evidenceContext = constructionEvidenceContext();

function evidenceBindings() {
  return evidenceContext.context ? {
    phase: evidenceContext.context.phase,
    wave_id: evidenceContext.context.wave_id,
    run_id: evidenceContext.context.run_id,
    attempt_id: evidenceContext.context.attempt_id,
    controller_context_sha256: evidenceContext.context_sha256,
  } : {controller_context_sha256: null};
}

async function run() {
const {values, variants: variantArguments} = parseArgs(process.argv.slice(2));
evidenceContext = await loadControllerContext({
  path: values.get("controller-context"),
  expectedSha256: values.get("expected-controller-context-sha256"),
  required: values.get("require-formal-context") === "true",
});
if (values.has("require-formal-context") && values.get("require-formal-context") !== "true") {
  throw new Error("require-formal-context only accepts true");
}
for (const key of [
  "case-id",
  "candidate-id",
  "input-root",
  "input-manifest",
  "contract",
  "output-dir",
  "terminal-receipt",
  "custodian-id",
  "custodian-eligible",
  "frozen-at",
]) {
  if (!values.has(key)) usage();
}
for (const key of ["input-root", "input-manifest", "contract", "output-dir", "terminal-receipt"]) {
  if (!isAbsolute(values.get(key))) throw new Error(`--${key} must be an absolute path`);
}
if (variantArguments.length === 0 || variantArguments.some((path) => !isAbsolute(path))) {
  throw new Error("at least one absolute --variant path is required");
}
if (!/^TW-0[1-9]$/.test(values.get("case-id"))) throw new Error("invalid --case-id");
if (!/^[0-9a-f]{32}$/.test(values.get("candidate-id"))) {
  throw new Error("--candidate-id must be opaque lowercase 128-bit hex");
}
if (evidenceContext.context &&
    (evidenceContext.context.case_id !== values.get("case-id") ||
      evidenceContext.context.candidate_id !== values.get("candidate-id"))) {
  throw new Error("controller context does not bind this review case and candidate");
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

const proposedTerminalReceiptPath = await canonicalNewPath(
  resolve(values.get("terminal-receipt")),
  "terminal receipt",
);
terminalContext = {
  classification: evidenceContext.classification,
  formal_result_eligible: evidenceContext.formal_result_eligible,
  ...evidenceBindings(),
  case_id: values.get("case-id"),
  candidate_id: values.get("candidate-id"),
  frozen_at: values.get("frozen-at"),
};

await requireDirectory(resolve(values.get("input-root")), "input root");
await requireRegular(resolve(values.get("input-manifest")), "review input manifest");
await requireRegular(resolve(values.get("contract")), "contract");
for (const path of variantArguments) await requireRegular(resolve(path), "registered variant");
const inputRoot = await realpath(values.get("input-root"));
const inputManifestPath = await realpath(values.get("input-manifest"));
const contractPath = await realpath(values.get("contract"));
const variantPaths = await Promise.all(variantArguments.map((path) => realpath(path)));
const outputDir = await canonicalNewPath(resolve(values.get("output-dir")), "output directory");

for (const [label, path] of [
  ["output directory", outputDir],
  ["terminal receipt", proposedTerminalReceiptPath],
  ["review input manifest", inputManifestPath],
  ["contract", contractPath],
  ...variantPaths.map((path) => ["registered variant", path]),
]) {
  if (isWithin(inputRoot, path) || isWithin(path, inputRoot)) {
    throw new Error(`${label} and candidate input must be disjoint; paths overlap`);
  }
}
if (
  isWithin(outputDir, proposedTerminalReceiptPath) ||
  isWithin(proposedTerminalReceiptPath, outputDir)
) {
  throw new Error("terminal receipt and packet output must be disjoint; paths overlap");
}
terminalReceiptPath = proposedTerminalReceiptPath;

const blinding = await loadBlindingInputs(contractPath, variantPaths);
terminalContext.contract_sha256 = blinding.contractIdentity.sha256;
terminalContext.variant_set_commitment_sha256 = blinding.variantSet.commitment_sha256;
const matcher = compileMatcher(blinding.variants);
const inputFiles = await walkInput(inputRoot);
const inputManifest = await bindInputManifest(inputManifestPath, inputFiles);
terminalContext.input_manifest_sha256 = inputManifest.sha256;
const rendered = [];
const privateEvents = [];
const totalRuleCounts = emptyRuleCounts();
const displayedPaths = new Set();

for (const input of inputFiles) {
  const originalBytes = input.bytes;
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
    roles: input.roles,
    displayPath,
    bytes: renderedBytes,
    matchCounts: {path: pathCounts, content: contentCounts},
    manifest: {
      display_path: displayPath,
      path_redacted: pathRules.length > 0,
      review_role: input.roles[0],
      original_path_sha256: originalPathSha256,
      pre_render_bytes: originalBytes.length,
      pre_render_sha256: originalArtifactSha256,
      rendered_bytes: renderedBytes.length,
      rendered_sha256: sha256(renderedBytes),
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
const publicArtifacts = rendered
  .map((entry) => entry.manifest)
  .sort((left, right) =>
    Buffer.from(left.display_path, "utf8").compare(Buffer.from(right.display_path, "utf8")),
  );
const publicManifest = {
  schema: "tachiko-review-packet-public-manifest-v1",
  protocol_id: blinding.contract.protocol_id,
  case_id: values.get("case-id"),
  candidate_id: values.get("candidate-id"),
  frozen_at: values.get("frozen-at"),
  contract_sha256: blinding.contractIdentity.sha256,
  rule_set_commitment_sha256: sha256(canonicalBytes(blinding.contract.machine_match_rules)),
  variant_set_commitment_sha256: blinding.variantSet.commitment_sha256,
  input_manifest_sha256: inputManifest.sha256,
  artifacts: publicArtifacts,
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
const scanReceipt = makeScanReceipt(scan, blinding, evidenceContext);
const scanReceiptBytes = canonicalBytes(scanReceipt);
await writeFile(resolve(outputDir, "scan-receipt.json"), scanReceiptBytes, {
  mode: 0o600,
  flag: "wx",
});

const finalMessage = rendered.find((entry) => entry.roles.includes("final_message"));

const receipt = {
  schema: "tachiko-review-packet-receipt-v1",
  classification: evidenceContext.classification,
  formal_result_eligible: evidenceContext.formal_result_eligible,
  ...evidenceBindings(),
  protocol_id: blinding.contract.protocol_id,
  case_id: values.get("case-id"),
  candidate_id: values.get("candidate-id"),
  frozen_at: values.get("frozen-at"),
  custodian: {id: values.get("custodian-id"), eligible: true},
  contract: blinding.contractIdentity,
  variant_set: blinding.variantSet,
  input_manifest_sha256: inputManifest.sha256,
  artifact_manifest_sha256: sha256(publicManifestBytes),
  private_match_map_sha256: sha256(privateMapBytes),
  scan_receipt_sha256: sha256(scanReceiptBytes),
  rendered_packet_sha256: scan.tree_sha256,
  match_counts_by_rule: totalRuleCounts,
  match_counts_by_artifact: rendered.map((entry) => ({
    opaque_path_alias: `artifact-${entry.manifest.original_path_sha256}`,
    path: entry.matchCounts.path,
    content: entry.matchCounts.content,
  })),
  final_message: {
    raw_bytes: finalMessage.manifest.pre_render_bytes,
    raw_sha256: finalMessage.manifest.pre_render_sha256,
    redacted_bytes: finalMessage.manifest.rendered_bytes,
    redacted_sha256: finalMessage.manifest.rendered_sha256,
  },
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
const receiptBytes = canonicalBytes(receipt);
await writeFile(resolve(outputDir, "receipt.json"), receiptBytes, {
  mode: 0o600,
  flag: "wx",
});
return {receipt, receiptBytes};
}

try {
  const result = await run();
  const terminal = {
    schema: "tachiko-review-packet-terminal-v1",
    ...terminalContext,
    output_receipt_sha256: sha256(result.receiptBytes),
    rendered_packet_sha256: result.receipt.rendered_packet_sha256,
    safe_to_release: result.receipt.safe_to_release,
    terminal_classification: result.receipt.safe_to_release ? "qualified" : "invalid_discarded",
    failure: result.receipt.safe_to_release ? null : "residual_match",
  };
  await writeFile(terminalReceiptPath, canonicalBytes(terminal), {mode: 0o600, flag: "wx"});
  console.log(JSON.stringify(result.receipt));
  if (!result.receipt.safe_to_release) process.exitCode = 1;
} catch (error) {
  if (terminalReceiptPath && !existsSync(terminalReceiptPath)) {
    const terminal = {
      schema: "tachiko-review-packet-terminal-v1",
      ...terminalContext,
      safe_to_release: false,
      terminal_classification: "invalid_discarded",
      failure: "packet_construction_failed",
    };
    await writeFile(terminalReceiptPath, canonicalBytes(terminal), {mode: 0o600, flag: "wx"});
  }
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
