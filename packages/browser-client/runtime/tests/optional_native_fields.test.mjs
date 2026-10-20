import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import test from "node:test";

const script = fileURLToPath(new URL("./optional_native_fields.mjs", import.meta.url));
const suffixes = ["", ".capacity", ".formula", ".rowgate",
  ".editability-boolean", ".editability-date", ".editability-text"];

async function fixture(t, records) {
  const root = await mkdtemp(join(tmpdir(), "554-fields-parity-negative-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const wasm = join(root, "empty.wasm");
  await writeFile(wasm, Uint8Array.of(0, 97, 115, 109, 1, 0, 0, 0));
  const captures = suffixes.map(suffix => join(root, `optional.json${suffix}`));
  await Promise.all(captures.map((path, index) => writeFile(path, JSON.stringify(
    index === 0 || suffixes[index] === ".rowgate" ? records : {
      records, project: [0], occurrence_id: "00000000-0000-4000-8000-000000000551",
    },
  ))));
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

test("empty native and wrapped captures cannot pass with an empty WASM module", async t => {
  const { wasm, captures } = await fixture(t, [{}]);
  await writeFile(captures[0], "[]");
  refuses(wasm, captures, /must contain a nonempty records array/);
  await writeFile(captures[0], "[{}]");
  await writeFile(captures[1], JSON.stringify({ records: [] }));
  refuses(wasm, captures, /must contain a nonempty records array/);
});

test("missing and duplicate native cases fail the expected inventory", async t => {
  const { wasm, captures } = await fixture(t, [{}]);
  refuses(wasm, captures.slice(0, 6), /inventory must include all seven expected cases/);
  refuses(wasm, [...captures.slice(0, 6), captures[0]], /duplicate native capture case/);
});

test("non-array records and missing initial projects fail before replay", async t => {
  const { wasm, captures } = await fixture(t, {});
  refuses(wasm, captures, /must contain a nonempty records array/);
  const valid = await fixture(t, [{}]);
  await writeFile(valid.captures[1], JSON.stringify({ records: [{}] }));
  refuses(valid.wasm, valid.captures, /must contain its initial project/);
});

test("WASM missing required exports fails before replay", async t => {
  const { wasm, captures } = await fixture(t, [{}]);
  refuses(wasm, captures, /WASM lacks required memory export/);
  await writeFile(wasm, Uint8Array.of(0, 97, 115, 109, 1, 0, 0, 0,
    5, 3, 1, 0, 1, 7, 10, 1, 6, 109, 101, 109, 111, 114, 121, 2, 0));
  refuses(wasm, captures, /WASM lacks required function export tachiko_designer_request_reserve/);
});
