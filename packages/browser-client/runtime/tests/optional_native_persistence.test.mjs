import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import test from "node:test";

const script = fileURLToPath(new URL("./optional_native_persistence.mjs", import.meta.url));
const names = [
  "sole-false-portable-false", "sole-false-portable-true",
  "sole-true-portable-false", "sole-true-portable-true",
  ...["csv", "xlsx"].flatMap(source =>
    ["rename", "add_blank", "add_fill", "remove", "insert", "insert_blank", "delete"].map(operation => `${source}-${operation}`)),
  "refusals", "formulas", "cell-compatibility-csv", "cell-compatibility-xlsx",
  "foreign-current", "header-history-collision", "untouched-compatibility", "scalar-compatibility",
  "formula-output-policy", "csv-output-scope", "presentation-output-policy", "style-inventory-policy", "references-headers-policy", "closed-grammar-policy", "boolean-output-policy", "precision-budget-policy",
];

async function fixture(t, records) {
  const root = await mkdtemp(join(tmpdir(), "554-parity-negative-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const wasm = join(root, "empty.wasm");
  // A valid empty WASM module reproduced the original false success.
  await writeFile(wasm, Uint8Array.of(0, 97, 115, 109, 1, 0, 0, 0));
  const captures = names.map(name => join(root, `${name}.json`));
  await Promise.all(captures.map(path => writeFile(path, JSON.stringify(records))));
  return { wasm, captures };
}

function refuses(wasm, captures, message) {
  const result = spawnSync(process.execPath, [script, wasm, ...captures], { encoding: "utf8" });
  assert.equal(result.error, undefined);
  assert.equal(result.signal, null);
  assert.notEqual(result.status, 0, result.stdout);
  assert.match(result.stderr, message);
  assert.equal(result.stdout, "", "invalid inputs cannot emit parity success receipts");
}

test("empty native captures fail even with an empty WASM module", async t => {
  const { wasm, captures } = await fixture(t, []);
  refuses(wasm, captures, /must contain a nonempty records array/);
});

test("missing and duplicate native cases fail the expected inventory", async t => {
  const { wasm, captures } = await fixture(t, [{}]);
  refuses(wasm, captures.slice(0, 3), /inventory must include all expected cases/);
  refuses(wasm, [captures[0], captures[1], captures[2], captures[0]], /duplicate native capture case/);
});

test("non-array native captures fail before replay", async t => {
  const { wasm, captures } = await fixture(t, {});
  refuses(wasm, captures, /must contain a nonempty records array/);
});

test("WASM missing required exports fails before replay", async t => {
  const { wasm, captures } = await fixture(t, [{}]);
  refuses(wasm, captures, /WASM lacks required memory export/);
  // Valid module exporting memory, but none of the required ABI functions.
  await writeFile(wasm, Uint8Array.of(0, 97, 115, 109, 1, 0, 0, 0,
    5, 3, 1, 0, 1, 7, 10, 1, 6, 109, 101, 109, 111, 114, 121, 2, 0));
  refuses(wasm, captures, /WASM lacks required function export tachiko_designer_request_reserve/);
});
