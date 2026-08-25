#!/usr/bin/env node

import {createHash} from "node:crypto";
import {existsSync} from "node:fs";
import {lstat, mkdir, readFile, readdir, realpath, writeFile} from "node:fs/promises";
import {dirname, isAbsolute, relative, resolve} from "node:path";
import {pathToFileURL} from "node:url";

const RULE_IDS = ["R1", "R2", "R3", "R4"];
const REDACTION = "[instruction-reference redacted]";
const IDENTIFIERS = [
  "agents.md",
  "baseline a",
  "variant b",
  "experiment arm",
  "system instruction",
  "developer instruction",
  "instruction variant",
];
const UTF8_DECODER = new TextDecoder("utf-8", {fatal: true});

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function canonicalBytes(value) {
  return Buffer.from(`${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function byteSort(left, right) {
  return Buffer.from(left.path, "utf8").compare(Buffer.from(right.path, "utf8"));
}

function strictText(bytes, label) {
  let text;
  try {
    text = UTF8_DECODER.decode(bytes);
  } catch {
    throw new Error(`${label} is not strict UTF-8`);
  }
  if (/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/u.test(text)) {
    throw new Error(`${label} contains unsupported binary control bytes`);
  }
  return text;
}

function splitLines(text) {
  const lines = [];
  let start = 0;
  let index = 0;
  while (index < text.length) {
    if (text[index] !== "\r" && text[index] !== "\n") {
      index += 1;
      continue;
    }
    const content = text.slice(start, index);
    let terminator = text[index];
    if (terminator === "\r" && text[index + 1] === "\n") {
      terminator = "\r\n";
      index += 1;
    }
    lines.push({content, terminator});
    index += 1;
    start = index;
  }
  if (start < text.length) lines.push({content: text.slice(start), terminator: ""});
  return lines;
}

function tokens(text) {
  return text.normalize("NFC").toLowerCase().match(/[\p{L}\p{N}]+/gu) ?? [];
}

function tokenWindows(lineTokens) {
  const windows = [];
  for (let index = 0; index + 8 <= lineTokens.length; index += 1) {
    windows.push(JSON.stringify(lineTokens.slice(index, index + 8)));
  }
  return windows;
}

function hasShared32Bytes(lineBytes, variantWindows) {
  for (let index = 0; index + 32 <= lineBytes.length; index += 1) {
    if (variantWindows.has(lineBytes.subarray(index, index + 32).toString("hex"))) return true;
  }
  return false;
}

function levenshteinWithin(leftText, rightText) {
  const left = [...leftText];
  const right = [...rightText];
  const maximumLength = Math.max(left.length, right.length);
  const maximumDistance = Math.floor(maximumLength * 0.15);
  if (Math.abs(left.length - right.length) > maximumDistance) return false;
  let previous = Array.from({length: right.length + 1}, (_, index) => index);
  for (let leftIndex = 1; leftIndex <= left.length; leftIndex += 1) {
    const current = [leftIndex];
    for (let rightIndex = 1; rightIndex <= right.length; rightIndex += 1) {
      current[rightIndex] = Math.min(
        current[rightIndex - 1] + 1,
        previous[rightIndex] + 1,
        previous[rightIndex - 1] + (left[leftIndex - 1] === right[rightIndex - 1] ? 0 : 1),
      );
    }
    previous = current;
  }
  return previous[right.length] <= maximumDistance;
}

function validateContract(contract) {
  if (
    contract.protocol_id !== "tachiko-agents-effect-v1" ||
    contract.contract_id !== "review-packet-blinding-v1" ||
    contract.contract_version !== 1 ||
    !Array.isArray(contract.machine_match_rules) ||
    contract.machine_match_rules.length !== 4
  ) {
    throw new Error("unsupported review-packet blinding contract");
  }
  for (const [index, rule] of contract.machine_match_rules.entries()) {
    if (!rule.startsWith(`${RULE_IDS[index]}:`)) {
      throw new Error("review-packet contract rule order does not match R1-R4");
    }
  }
}

async function readRegular(path, label) {
  const metadata = await lstat(path);
  if (!metadata.isFile() || metadata.isSymbolicLink()) {
    throw new Error(`${label} must be a non-symlink regular file`);
  }
  return readFile(path);
}

export async function loadBlindingInputs(contractPath, variantPaths) {
  if (variantPaths.length === 0) throw new Error("at least one registered variant is required");
  const contractBytes = await readRegular(contractPath, "contract");
  const contract = JSON.parse(strictText(contractBytes, "contract"));
  validateContract(contract);
  const variants = [];
  for (const path of variantPaths) {
    const bytes = await readRegular(path, "registered variant");
    const text = strictText(bytes, "registered variant");
    variants.push({bytes: bytes.length, sha256: sha256(bytes), text});
  }
  const identities = variants
    .map(({bytes, sha256: digest}) => ({bytes, sha256: digest}))
    .sort((left, right) => left.sha256.localeCompare(right.sha256));
  if (new Set(identities.map((entry) => entry.sha256)).size !== identities.length) {
    throw new Error("registered variant byte set contains duplicates");
  }
  return {
    contract,
    contractBytes,
    contractIdentity: {bytes: contractBytes.length, sha256: sha256(contractBytes)},
    variants,
    variantSet: {
      count: identities.length,
      commitment_sha256: sha256(canonicalBytes(identities)),
    },
  };
}

export function compileMatcher(variants) {
  const variantLines = variants.flatMap((variant) =>
    splitLines(variant.text).map((line) => line.content.normalize("NFC")),
  );
  const byteWindows = new Set();
  const eightTokenWindows = new Set();
  const nearLines = [];
  for (const line of variantLines) {
    const bytes = Buffer.from(line, "utf8");
    for (let index = 0; index + 32 <= bytes.length; index += 1) {
      byteWindows.add(bytes.subarray(index, index + 32).toString("hex"));
    }
    const lineTokens = tokens(line);
    for (const window of tokenWindows(lineTokens)) eightTokenWindows.add(window);
    const lowered = line.toLowerCase();
    if ([...lowered].length >= 32 && lineTokens.length >= 8) {
      nearLines.push({lowered, tokenCount: lineTokens.length});
    }
  }
  return function matchLine(originalLine) {
    const normalized = originalLine.normalize("NFC");
    const lowered = normalized.toLowerCase();
    const lineTokens = tokens(normalized);
    const matches = [];
    if (IDENTIFIERS.some((identifier) => lowered.includes(identifier))) matches.push("R1");
    if (hasShared32Bytes(Buffer.from(normalized, "utf8"), byteWindows)) matches.push("R2");
    if (tokenWindows(lineTokens).some((window) => eightTokenWindows.has(window))) {
      matches.push("R3");
    }
    if (
      [...lowered].length >= 32 &&
      lineTokens.length >= 8 &&
      nearLines.some((variantLine) =>
        variantLine.tokenCount >= 8 && levenshteinWithin(lowered, variantLine.lowered),
      )
    ) {
      matches.push("R4");
    }
    return matches;
  };
}

async function walkRegularFiles(root) {
  const files = [];
  async function walk(absoluteDirectory, relativePrefix) {
    const directoryMetadata = await lstat(absoluteDirectory);
    if (!directoryMetadata.isDirectory() || directoryMetadata.isSymbolicLink()) {
      throw new Error("review tree directories must be non-symlink directories");
    }
    const entries = await readdir(absoluteDirectory, {encoding: "buffer", withFileTypes: true});
    entries.sort((left, right) => Buffer.compare(left.name, right.name));
    for (const entry of entries) {
      const name = strictText(entry.name, "reviewer-visible path component");
      const relativePath = relativePrefix ? `${relativePrefix}/${name}` : name;
      const absolutePath = resolve(absoluteDirectory, name);
      const metadata = await lstat(absolutePath);
      if (metadata.isSymbolicLink()) {
        throw new Error(`${relativePath} is a symlink; reviewer-visible artifacts must be regular`);
      }
      if (metadata.isDirectory()) {
        await walk(absolutePath, relativePath);
      } else if (metadata.isFile()) {
        files.push({absolutePath, path: relativePath});
      } else {
        throw new Error(`${relativePath} is not a regular file or directory`);
      }
    }
  }
  await walk(root, "");
  files.sort(byteSort);
  return files;
}

export async function scanPacketTree(packetDir, blinding) {
  const matcher = compileMatcher(blinding.variants);
  const files = await walkRegularFiles(packetDir);
  const artifacts = [];
  const counts = Object.fromEntries(RULE_IDS.map((id) => [id, 0]));
  let matchCount = 0;
  for (const file of files) {
    const pathMatches = matcher(file.path);
    const bytes = await readFile(file.absolutePath);
    const text = strictText(bytes, `${file.path} content`);
    const contentMatches = [];
    for (const [index, line] of splitLines(text).entries()) {
      for (const ruleId of matcher(line.content)) {
        contentMatches.push({line_number: index + 1, rule_id: ruleId});
      }
    }
    for (const ruleId of pathMatches) counts[ruleId] += 1;
    for (const match of contentMatches) counts[match.rule_id] += 1;
    matchCount += pathMatches.length + contentMatches.length;
    artifacts.push({
      path_sha256: sha256(Buffer.from(file.path, "utf8")),
      bytes: bytes.length,
      sha256: sha256(bytes),
      path_match_rule_ids: pathMatches,
      content_matches: contentMatches,
    });
  }
  const treeEntries = artifacts.map(({path_sha256, bytes, sha256: digest}) => ({
    path_sha256,
    bytes,
    sha256: digest,
  }));
  return {
    files: artifacts,
    file_count: artifacts.length,
    tree_sha256: sha256(canonicalBytes(treeEntries)),
    match_count: matchCount,
    match_counts_by_rule: counts,
    safe_to_release: matchCount === 0,
  };
}

function parseArgs(argv) {
  const values = new Map();
  const variants = [];
  for (let index = 0; index < argv.length; index += 2) {
    const flag = argv[index];
    const value = argv[index + 1];
    if (!flag?.startsWith("--") || value === undefined) throw new Error("invalid arguments");
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

async function runCli() {
  const {values, variants} = parseArgs(process.argv.slice(2));
  for (const key of ["packet-dir", "contract", "receipt"]) {
    if (!values.has(key) || !isAbsolute(values.get(key))) {
      throw new Error(`--${key} must be an absolute path`);
    }
  }
  if (variants.some((path) => !isAbsolute(path))) {
    throw new Error("--variant paths must be absolute");
  }
  const packetMetadata = await lstat(resolve(values.get("packet-dir")));
  if (!packetMetadata.isDirectory() || packetMetadata.isSymbolicLink()) {
    throw new Error("packet directory must be a non-symlink directory");
  }
  await readRegular(resolve(values.get("contract")), "contract");
  for (const path of variants) await readRegular(resolve(path), "registered variant");
  const packetDir = await realpath(values.get("packet-dir"));
  const receiptPath = resolve(values.get("receipt"));
  if (existsSync(receiptPath)) throw new Error("scan receipt must not already exist");
  const receiptParent = await realpath(dirname(receiptPath));
  if (isWithin(packetDir, resolve(receiptParent, receiptPath.slice(dirname(receiptPath).length + 1)))) {
    throw new Error("scan receipt must be disjoint from the packet directory");
  }
  const contractPath = await realpath(values.get("contract"));
  const variantPaths = await Promise.all(variants.map((path) => realpath(path)));
  for (const [label, path] of [
    ["contract", contractPath],
    ...variantPaths.map((path) => ["registered variant", path]),
  ]) {
    if (isWithin(packetDir, path) || isWithin(path, packetDir)) {
      throw new Error(`${label} and packet directory must be disjoint; paths overlap`);
    }
  }
  const blinding = await loadBlindingInputs(
    contractPath,
    variantPaths,
  );
  const scan = await scanPacketTree(packetDir, blinding);
  const receipt = {
    schema: "tachiko-review-packet-scan-v1",
    classification: "construction_pilot_only",
    formal_result_eligible: false,
    contract: blinding.contractIdentity,
    variant_set: blinding.variantSet,
    packet_tree_sha256: scan.tree_sha256,
    scanned_file_count: scan.file_count,
    match_count: scan.match_count,
    match_counts_by_rule: scan.match_counts_by_rule,
    safe_to_release: scan.safe_to_release,
    terminal_classification: scan.safe_to_release ? "qualified" : "invalid_discarded",
    semantic_scoring_performed: false,
    qualification: "subjective_packet_transport_only",
  };
  await mkdir(dirname(receiptPath), {recursive: true});
  await writeFile(receiptPath, canonicalBytes(receipt), {mode: 0o600, flag: "wx"});
  console.log(JSON.stringify(receipt));
  if (!scan.safe_to_release) process.exitCode = 1;
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  runCli().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}

export {REDACTION, RULE_IDS, canonicalBytes, sha256, splitLines, strictText};
