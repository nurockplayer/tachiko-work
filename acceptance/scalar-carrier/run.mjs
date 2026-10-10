// Actual Rust WASM only. Usage: node run.mjs EXACT_WASM NEW_RESULT_DIRECTORY
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const [wasmPath, destination] = process.argv.slice(2);
assert(wasmPath && destination, "supply an exact WASM file and a new result directory");
const resultDir = resolve(destination);
await mkdir(resultDir); // Refuse to overwrite an earlier run.
const wasm = await readFile(wasmPath);
const sha = bytes => createHash("sha256").update(bytes).digest("hex");
const result = { wasm_sha256: sha(wasm), completed: false, records: [] };
try {
  const { instance } = await WebAssembly.instantiate(wasm, {});
  const e = instance.exports;
  const encoder = new TextEncoder();
  const decoder = new TextDecoder("utf-8", { fatal: true });
  let sequence = 0;
  const uuid = () => `00000000-0000-4000-8000-${String(++sequence).padStart(12, "0")}`;
  const json = value => encoder.encode(JSON.stringify(value));
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
    assert.equal(value.status, "ok", JSON.stringify(value));
    assert.equal(value.response.type, type); return value.response.payload;
  }
  function output() {
    const bytes = new Uint8Array(e.memory.buffer, e.tachiko_designer_project_ptr(),
      e.tachiko_designer_project_len()).slice();
    e.tachiko_designer_project_release(); return bytes;
  }
  const options = { delimiter: ",", header: true };
  function importing(bytes, format, fieldType, install = true) {
    return spreadsheet({ type: "import", format, csv_options: options,
      selection: { column_types: [[fieldType]], extra_columns: [[]] },
      occurrence_id: uuid(), install }, bytes);
  }
  const imported = okay(importing(encoder.encode("Value\nresident\n"), "csv", "text"), "imported");
  const metadata = imported.metadata;
  const collection = imported.opened.bootstrap.default_collection;
  const target = imported.opened.table.rows[0].fields[0].target;
  let revision = imported.opened.bootstrap.revision;
  function save() {
    request(encoder.encode(revision)); e.tachiko_designer_project_export();
    okay(reply(), "project_exported"); return output();
  }
  function exported(format) {
    okay(spreadsheet({ type: "export", format, expected_revision: revision, metadata,
      collection: metadata.sheets[0].schema_id }), "spreadsheet_exported");
    return output();
  }
  function snapshot() {
    e.tachiko_designer_occurrence_observe(); const occurrence = reply();
    return { occurrence, table: okay(ordinary({ type: "query_table", collection }), "table"),
      project: sha(save()), csv: sha(exported("csv")), xlsx: sha(exported("xlsx")) };
  }
  function publish(type, rest = {}) {
    const publication = okay(ordinary({ type, expected_revision: revision, ...rest }), "published");
    assert.equal(publication.base_revision, revision);
    assert.notEqual(publication.resulting_revision, revision);
    revision = publication.resulting_revision;
  }
  const seed = exported("xlsx");
  const seedPath = join(resultDir, "seed.xlsx"); await writeFile(seedPath, seed, { flag: "wx" });
  execFileSync("python3", [join(dirname(fileURLToPath(import.meta.url)), "fixtures.py"),
    seedPath, join(resultDir, "fixtures")], { stdio: "pipe" });
  const manifest = JSON.parse(await readFile(join(resultDir, "fixtures/manifest.json"), "utf8"));
  const initial = save();
  publish("edit_scalar", { target, input: { kind: "text", value: "earlier" } });
  const earlier = save();
  publish("edit_scalar", { target, input: { kind: "text", value: "later" } });
  const later = save(); publish("undo");
  assert.deepEqual(save(), earlier);
  const stable = snapshot();
  for (const item of manifest) {
    try {
      const bytes = await readFile(join(resultDir, "fixtures", item.file));
      assert.equal(sha(bytes), item.sha256);
      const preview = okay(spreadsheet({ type: "inspect", format: "xlsx", csv_options: options }, bytes), "import_preview");
      if (item.refuse) {
        assert(preview.ledger.some(finding => finding.blocking && finding.code === "scalar_mapping_rejected"));
        const refusal = importing(bytes, "xlsx", item.field_type);
        assert.equal(refusal.status, "error", `${item.file} unexpectedly replaced the resident`);
        assert.equal(refusal.error.current_revision, revision);
      } else {
        assert(!preview.ledger.some(finding => finding.blocking));
        assert.deepEqual(preview.sheets[0].rows[0][0].value, item.expected);
        okay(importing(bytes, "xlsx", item.field_type, false), "imported");
      }
      assert.deepEqual(snapshot(), stable);
      result.records.push({ name: item.file, outcome: "PASS" });
    } catch (error) {
      result.records.push({ name: item.file, outcome: "FAIL", message: String(error) }); throw error;
    }
  }
  publish("redo"); assert.deepEqual(save(), later);
  publish("undo"); assert.deepEqual(save(), earlier);
  publish("undo"); assert.deepEqual(save(), initial);
  const beforeReopen = { csv: exported("csv"), xlsx: exported("xlsx") };
  const opened = okay(spreadsheet({ type: "open_project", occurrence_id: uuid(), metadata }, save()), "opened");
  revision = opened.bootstrap.revision;
  assert.deepEqual(save(), initial);
  for (const format of ["csv", "xlsx"]) assert.deepEqual(exported(format), beforeReopen[format]);
  result.records.push({ name: "resident-history-export-reopen", outcome: "PASS" });
  result.completed = true;
} catch (error) {
  result.failure = String(error); process.exitCode = 1;
} finally {
  await writeFile(join(resultDir, "result.json"), JSON.stringify(result, null, 2) + "\n", { flag: "wx" });
}
