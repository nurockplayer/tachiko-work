#!/usr/bin/env node

import {createHash} from "node:crypto";
import {existsSync} from "node:fs";
import {mkdir, writeFile} from "node:fs/promises";
import {spawnSync} from "node:child_process";
import {dirname, isAbsolute, resolve} from "node:path";
import {fileURLToPath} from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const benchmarkDir = resolve(scriptDir, "..");
const lock = JSON.parse(
  await import("node:fs/promises").then(({readFile}) =>
    readFile(resolve(benchmarkDir, "environment-lock.json"), "utf8"),
  ),
);

function fail(message) {
  throw new Error(message);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function canonicalJson(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  if (value && typeof value === "object") {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
}

if (process.argv.length !== 4 || process.argv[2] !== "--output") {
  console.error("usage: node materialize-model-catalog.mjs --output /abs/new/catalog.json");
  process.exit(2);
}
const output = resolve(process.argv[3]);
if (!isAbsolute(process.argv[3]) || existsSync(output)) {
  fail("output must be an absolute path that does not already exist");
}

const result = spawnSync(
  lock.controlled_agent.codex_binary_path,
  ["debug", "models", "--bundled"],
  {encoding: null, maxBuffer: 2 * 1024 * 1024},
);
if (result.status !== 0) {
  fail(Buffer.from(result.stderr ?? []).toString("utf8"));
}
const bytes = Buffer.from(result.stdout);
if (
  bytes.length !== lock.controlled_agent.bundled_model_catalog.bytes ||
  sha256(bytes) !== lock.controlled_agent.bundled_model_catalog.raw_sha256
) {
  fail("bundled model catalog bytes differ from the environment lock");
}
const catalog = JSON.parse(bytes.toString("utf8"));
const model = catalog.models.find(
  (candidate) => candidate.slug === lock.controlled_agent.model_id,
);
if (!model) fail("locked model is absent from the bundled catalog");
if (
  sha256(`${canonicalJson(catalog)}\n`) !==
    lock.controlled_agent.bundled_model_catalog.canonical_catalog_sha256 ||
  sha256(`${canonicalJson(model)}\n`) !==
    lock.controlled_agent.bundled_model_catalog.model_record_sha256 ||
  sha256(`${model.base_instructions}\n`) !==
    lock.controlled_agent.bundled_model_catalog.base_instructions_sha256
) {
  fail("locked model record or base instructions differ from the environment lock");
}

await mkdir(dirname(output), {recursive: true});
await writeFile(output, bytes, {mode: 0o600, flag: "wx"});
console.log(
  JSON.stringify({
    output,
    bytes: bytes.length,
    raw_sha256: sha256(bytes),
    canonical_catalog_sha256: sha256(`${canonicalJson(catalog)}\n`),
    model_record_sha256: sha256(`${canonicalJson(model)}\n`),
    base_instructions_sha256: sha256(`${model.base_instructions}\n`),
  }),
);
