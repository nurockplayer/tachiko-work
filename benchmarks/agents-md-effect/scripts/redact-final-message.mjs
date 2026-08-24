#!/usr/bin/env node

import {createHash} from "node:crypto";
import {existsSync} from "node:fs";
import {mkdir, readFile, writeFile} from "node:fs/promises";
import {dirname, isAbsolute, resolve} from "node:path";

function usage() {
  console.error(
    "usage: node redact-final-message.mjs --input /abs/final.txt " +
      "--output /abs/redacted.txt --receipt /abs/redaction-receipt.json",
  );
  process.exit(2);
}

function parseArgs(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    if (!argv[index]?.startsWith("--") || argv[index + 1] === undefined) usage();
    values.set(argv[index].slice(2), argv[index + 1]);
  }
  return values;
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

const args = parseArgs(process.argv.slice(2));
for (const key of ["input", "output", "receipt"]) if (!args.has(key)) usage();
const input = resolve(args.get("input"));
const output = resolve(args.get("output"));
const receiptPath = resolve(args.get("receipt"));
for (const [label, path] of [["output", output], ["receipt", receiptPath]]) {
  if (!isAbsolute(args.get(label)) || existsSync(path)) {
    throw new Error(`${label} must be an absolute path that does not exist`);
  }
}

const rawBytes = await readFile(input);
const raw = rawBytes.toString("utf8");
const patterns = [
  /AGENTS\.md/i,
  /\bAGENTS(?: instruction| variant| overlay| file)\b/i,
  /\bBaseline A\b/i,
  /\bVariant B\b/i,
  /\b(?:experiment|controlled|benchmark) arm\b/i,
  /\barm[- _]?[AB]\b/i,
  /\bA\/B(?: experiment| run| result| label)?\b/i,
  /\bUltra(?: run| result| configuration)?\b/i,
  /\b(?:system|developer) (?:prompt|instruction|message)s?\b/i,
];
let redactedLineCount = 0;
const redacted = raw
  .split(/(?<=\n)/)
  .map((line) => {
    if (!patterns.some((pattern) => pattern.test(line))) return line;
    redactedLineCount += 1;
    return line.endsWith("\n")
      ? "[instruction-reference redacted]\n"
      : "[instruction-reference redacted]";
  })
  .join("");
const redactedBytes = Buffer.from(redacted, "utf8");
await mkdir(dirname(output), {recursive: true});
await writeFile(output, redactedBytes, {mode: 0o600, flag: "wx"});
const receipt = {
  rule_version: "instruction-reference-redaction-v1",
  raw_sha256: sha256(rawBytes),
  redacted_sha256: sha256(redactedBytes),
  redacted_line_count: redactedLineCount,
  raw_bytes: rawBytes.length,
  redacted_bytes: redactedBytes.length,
};
await mkdir(dirname(receiptPath), {recursive: true});
await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, {
  mode: 0o600,
  flag: "wx",
});
console.log(JSON.stringify(receipt));
