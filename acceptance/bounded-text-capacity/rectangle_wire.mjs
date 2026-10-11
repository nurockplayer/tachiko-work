// Work #507: actual Rust WASM; frozen independent matrices, fresh Node processes.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const [wasmPath, fixtures, destination, reopenFile] = process.argv.slice(2);
assert(wasmPath && fixtures && destination);
const sha = bytes => createHash("sha256").update(bytes).digest("hex");
const wasm = await readFile(wasmPath);
const { instance } = await WebAssembly.instantiate(wasm, {});
const e = instance.exports;
const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });
const json = value => encoder.encode(JSON.stringify(value));
let sequence = reopenFile ? 100_000 : 600;
const uuid = () => `00000000-0000-4000-8000-${String(++sequence).padStart(12, "0")}`;
const reply = () => JSON.parse(decoder.decode(new Uint8Array(e.memory.buffer,
  e.tachiko_designer_response_ptr(), e.tachiko_designer_response_len()).slice()));
function request(bytes) {
  const pointer = e.tachiko_designer_request_reserve(bytes.length);
  assert(pointer !== 0);
  new Uint8Array(e.memory.buffer, pointer, bytes.length).set(bytes);
}
function ordinary(operation) {
  request(json(operation)); e.tachiko_designer_request_run(); return reply();
}
function spreadsheet(operation, bytes = new Uint8Array()) {
  const pointer = e.tachiko_designer_project_reserve(bytes.length);
  if (bytes.length) {
    assert(pointer !== 0);
    new Uint8Array(e.memory.buffer, pointer, bytes.length).set(bytes);
  }
  request(json(operation)); e.tachiko_designer_spreadsheet_run(); return reply();
}
function okay(value, type) {
  assert.equal(value.status, "ok", JSON.stringify(value).slice(0, 1000));
  assert.equal(value.response.type, type); return value.response.payload;
}
function output() {
  const bytes = new Uint8Array(e.memory.buffer, e.tachiko_designer_project_ptr(),
    e.tachiko_designer_project_len()).slice();
  e.tachiko_designer_project_release(); return bytes;
}
const options = { delimiter: ",", header: true };
const selection = caseValue => ({ column_types: [caseValue.kinds], extra_columns: [[]] });
function importing(bytes, format, caseValue, extra = {}) {
  return spreadsheet({ type: "import", format, csv_options: options,
    selection: selection(caseValue), occurrence_id: uuid(), install: true, ...extra }, bytes);
}
let revision, collection, metadata;
function save(carrier) {
  request(encoder.encode(revision));
  if (carrier === "canonical") e.tachiko_designer_canonical_tree_export();
  else e.tachiko_designer_project_export();
  okay(reply(), carrier === "canonical" ? "canonical_tree_exported" : "project_exported");
  return output();
}
function exported(format) {
  okay(spreadsheet({ type: "export", format, expected_revision: revision, metadata,
    collection: metadata.sheets[0].schema_id }), "spreadsheet_exported");
  return output();
}
const table = () => okay(ordinary({ type: "query_table", collection }), "table");
function verify(actual, caseValue, values) {
  assert.equal(actual.rows.length, values.length);
  assert.equal(actual.collection.entity_count, values.length);
  assert.deepEqual(actual.columns.map(c => c.field_type), caseValue.kinds);
  assert.deepEqual(metadata.sheets[0].columns.map(c => c.name), caseValue.headers);
  assert.deepEqual(actual.columns.map(c => c.id), metadata.sheets[0].columns.map(c => c.field_id));
  assert.deepEqual(actual.rows.map(r => r.id), metadata.sheets[0].rows.map(r => r.entity_id));
  actual.rows.forEach((row, r) => {
    assert.equal(row.fields.length, caseValue.kinds.length);
    row.fields.forEach((field, c) => {
      assert.deepEqual(field.target, { entity: row.id, field: actual.columns[c].id });
      assert.deepEqual(field.stored, values[r][c], `cell ${r},${c}`);
      assert.equal(field.formula, null); assert.deepEqual(field.diagnostics, []);
    });
  });
}
function publish(type, rest = {}) {
  const result = okay(ordinary({ type, expected_revision: revision, ...rest }), "published");
  assert.equal(result.base_revision, revision); assert.notEqual(result.resulting_revision, revision);
  revision = result.resulting_revision;
}
function snapshot() {
  e.tachiko_designer_occurrence_observe();
  return { occurrence: reply(), table: sha(json(table())), project: sha(save("opaque")),
    csv: sha(exported("csv")), xlsx: sha(exported("xlsx")) };
}
function refused(replyValue, before) {
  assert.equal(replyValue.status, "error", "expected refusal");
  assert.deepEqual(snapshot(), before, "refusal mutated resident, revision or export bytes");
}
const manifest = JSON.parse(await readFile(join(fixtures, "cases.json"), "utf8"));
const hashes = JSON.parse(await readFile(join(fixtures, "SHA256.json"), "utf8"));
async function source(caseValue, format) {
  const name = `${caseValue.name}.${format}`;
  const bytes = await readFile(join(fixtures, name));
  assert.equal(sha(bytes), hashes[name]); return bytes;
}
const captures = [];
async function capture(caseValue, stem, state, phase, sourceFormat, carrier) {
  for (const format of ["csv", "xlsx"]) {
    const file = `${stem}.${format}`;
    await writeFile(join(destination, file), exported(format), { flag: "wx" });
    captures.push({ case: caseValue.name, file, state, phase, sourceFormat, carrier });
  }
}
if (reopenFile) {
  const saved = JSON.parse(await readFile(reopenFile, "utf8"));
  assert.notEqual(process.pid, saved.pid);
  metadata = saved.metadata;
  const opened = okay(spreadsheet({ type: "open_project", occurrence_id: uuid(), metadata },
    await readFile(join(destination, saved.file))), "opened");
  revision = opened.bootstrap.revision; collection = opened.bootstrap.default_collection;
  verify(opened.table, saved.caseValue, saved.values); verify(table(), saved.caseValue, saved.values);
  assert.equal(sha(save(saved.carrier)), saved.savedSha256);
  await capture(saved.caseValue, saved.stem + "-reopen", "edited", "reopened", saved.sourceFormat, saved.carrier);
  await writeFile(reopenFile + ".receipt.json", JSON.stringify({ pid: process.pid, captures }));
} else {
  await mkdir(destination);
  const result = { wasm_sha256: sha(wasm), completed: false, records: [] };
  try {
    for (const caseValue of manifest.accepted) {
      for (const format of ["csv", "xlsx"]) {
        const start = performance.now();
        const bytes = await source(caseValue, format);
        // The baseline must exercise a valid explicit typed confirmation,
        // independently of the separate inspection feasibility assertion.
        if (caseValue.name === "320x2-mixed") {
          const candidate = okay(importing(bytes, format, caseValue, { install: false }), "imported");
          assert.equal(candidate.opened.table.rows.length, 320);
        }
        const preview = okay(spreadsheet({ type: "inspect", format, csv_options: options }, bytes), "import_preview");
        assert.equal(preview.sheets[0].rows.length, caseValue.values.length);
        // CSV inspection preserves original lexical Text, including negatives.
        if (format === "csv") preview.sheets[0].rows.forEach((row, r) => row.forEach((cell, c) => {
          const expected = caseValue.values[r][c];
          if (expected === null) assert.equal(cell.value.kind, "empty");
          else {
            assert.equal(cell.value.kind, "text");
            if (expected.kind === "text") assert.equal(cell.value.value, expected.value);
            else assert.equal(Number(cell.value.value), expected.value);
          }
        }));
        const imported = okay(importing(bytes, format, caseValue), "imported");
        metadata = imported.metadata; collection = imported.opened.bootstrap.default_collection;
        revision = imported.opened.bootstrap.revision;
        const values = structuredClone(caseValue.values);
        verify(imported.opened.table, caseValue, values);
        if (["320x2-mixed", "1000x16-mixed", "8406x3-text"].includes(caseValue.name)) {
          for (const [r, c] of [[0, 0], [Math.floor(values.length / 2), caseValue.kinds.length - 1], [values.length - 1, 0]]) {
            const target = table().rows[r].fields[c].target;
            const value = caseValue.kinds[c] === "number" ? { kind: "number", value: 700.25 }
              : { kind: "text", value: '000-edited-雪,"first"\r\nkept' };
            publish("edit_scalar", { target, input: value.kind === "number" ? { kind: "number", input: "700.25" } : value });
            const original = values[r][c]; values[r][c] = value; verify(table(), caseValue, values);
            publish("undo"); values[r][c] = original; verify(table(), caseValue, values);
            publish("redo"); values[r][c] = value; verify(table(), caseValue, values);
          }
          publish("undo"); // Both histories populated while refusals execute.
          const before = snapshot();
          for (const rejected of manifest.refused) for (const rejectedFormat of ["csv", "xlsx"]) {
            const rejectedBytes = await source(rejected, rejectedFormat);
            if (["320x2-mixed-expression", "320x2-mixed-nonfinite", "320x2-mixed-column"].includes(rejected.name)
                && (rejectedFormat === "csv" || rejected.name === "320x2-mixed-expression"))
              refused(spreadsheet({ type: "inspect", format: rejectedFormat, csv_options: options }, rejectedBytes), before);
            refused(importing(rejectedBytes, rejectedFormat, rejected), before);
          }
          if (caseValue.kinds.includes("number"))
            refused(importing(bytes, format, caseValue, { selection: { column_types: [caseValue.kinds.map(() => "text")], extra_columns: [[]] } }), before);
          refused(spreadsheet({ type: "open_project", occurrence_id: uuid(), metadata: { ...metadata, version: 2 } }, save("opaque")), before);
          refused(ordinary({ type: "remove_rows", expected_revision: revision, entities: [table().rows[0].id] }), before);
          publish("redo"); verify(table(), caseValue, values);
          for (const carrier of ["opaque", "canonical"]) {
            const stem = `${caseValue.name}-${format}-${carrier}`;
            const file = `${stem}.project`;
            const saved = save(carrier);
            await writeFile(join(destination, file), saved, { flag: "wx" });
            await capture(caseValue, stem + "-save", "edited", "saved", format, carrier);
            const reopen = join(destination, `${stem}.json`);
            await writeFile(reopen, JSON.stringify({ pid: process.pid, metadata, file, stem, carrier,
              sourceFormat: format, savedSha256: sha(saved), caseValue, values }));
            execFileSync(process.execPath, [fileURLToPath(import.meta.url), wasmPath, fixtures, destination, reopen], { stdio: "inherit", timeout: 180_000 });
            const receipt = JSON.parse(await readFile(reopen + ".receipt.json", "utf8"));
            assert.notEqual(receipt.pid, process.pid); captures.push(...receipt.captures);
          }
        }
        result.records.push({ case: caseValue.name, format, outcome: "PASS", milliseconds: performance.now() - start,
          wasm_memory_bytes: e.memory.buffer.byteLength });
      }
    }
    result.completed = true;
  } catch (error) {
    result.failure = String(error); process.exitCode = 1;
  } finally {
    await writeFile(join(destination, "captures.json"), JSON.stringify(captures));
    await writeFile(join(destination, "result.json"), JSON.stringify(result, null, 2));
    console.log(JSON.stringify(result));
  }
}
