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
// queryTable refuses above 1,000 rows; page the complete table through windows.
function table() {
  const rows = [];
  let columns, collectionSummary;
  for (;;) {
    const window = okay(ordinary({ type: "query_table_window", expected_revision: revision,
      collection, view: {}, start: rows.length, count: 64 }), "table_window");
    columns ??= window.columns;
    collectionSummary ??= window.collection;
    rows.push(...window.rows);
    if (window.rows.length === 0 || rows.length >= window.total_rows) {
      return { revision, collection: collectionSummary, columns, rows };
    }
  }
}
function verify(actual, caseValue, values) {
  assert.equal(actual.rows.length, values.length);
  assert.equal(actual.collection.entity_count, values.length);
  assert.deepEqual(actual.columns.map(c => c.field_type), caseValue.kinds);
  assert.deepEqual(metadata.sheets[0].columns.map(c => c.name), caseValue.headers);
  assert.deepEqual(actual.columns.map(c => c.id), metadata.sheets[0].columns.map(c => c.field_id));
  assert.deepEqual(actual.rows.map(r => r.id), metadata.sheets[0].rows.map(r => r.entity_id));
  actual.rows.forEach((row, r) => {
    const present = actual.columns.flatMap((column, c) => values[r][c] === null ? []
      : [{ target: { entity: row.id, field: column.id }, stored: values[r][c], c }]);
    // Exact ordered targets reject unknown, duplicate, missing or extra cells.
    assert.deepEqual(row.fields.map(field => field.target), present.map(cell => cell.target));
    row.fields.forEach((field, index) => {
      assert.deepEqual(field.stored, present[index].stored, `cell ${r},${present[index].c}`);
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
function inspectionBudgetCases(result, before) {
  const cases = [
    ["64x3-negative-oversize", 64, 3, 1200, "-13.25", "refused"],
    ["64x2-negative-oversize", 64, 2, 1200, "-13.25", "refused"],
    ["64x3-safe-legacy", 64, 3, 1200, "13.25", "capacity"],
    ["65x3-negative-mixed", 65, 3, 1200, "-13.25", "capacity"],
    ["8x3-negative-ordinary", 8, 3, 8, "-13.25", "ordinary"],
  ];
  for (const [name, rows, columns, textLength, numericText, expected] of cases) {
    const headers = ["a", "b", "c"].slice(0, columns);
    const values = ["x".repeat(textLength), numericText, "label"].slice(0, columns);
    const bytes = encoder.encode([headers.join(","),
      ...Array.from({ length: rows }, () => values.join(","))].join("\r\n"));
    if (name === "64x3-negative-oversize") {
      assert.equal(bytes.length, 77_765);
      assert.equal(sha(bytes), "8c386f3b27fc8e9d055aabfde159418d0fbd811d6f037765e44ff99ba4f26283");
    }
    const inspected = spreadsheet({ type: "inspect", format: "csv", csv_options: options }, bytes);
    const responseBytes = e.tachiko_designer_response_len();
    if (expected === "refused") {
      assert.equal(inspected.status, "error", `${name} escaped the ordinary reply budget`);
      assert.equal(inspected.error.code, "query_too_large");
      assert.match(inspected.error.message, /bounded maximum is 65536/);
      assert(responseBytes <= 65_536);
    } else {
      const preview = okay(inspected, "import_preview");
      assert.equal(preview.sheets.length, 1);
      assert.equal(preview.sheets[0].rows.length, rows);
      assert.deepEqual(preview.sheets[0].columns.map(column => column.name), headers);
      for (const row of preview.sheets[0].rows) {
        assert.deepEqual(row.map(cell => cell.value), values.map(value => ({ kind: "text", value })),
          `${name} changed original lexical Text`);
      }
      assert(expected === "capacity" ? responseBytes > 65_536 : responseBytes <= 65_536);
    }
    assert.deepEqual(snapshot(), before, `${name} inspection changed the resident or exports`);
    result.inspection_budget.push({ case: name, outcome: "PASS", expected,
      source_sha256: sha(bytes), source_bytes: bytes.length, response_bytes: responseBytes });
  }
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
  const result = { wasm_sha256: sha(wasm), completed: false, records: [], inspection_budget: [] };
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
            const actual = table();
            const target = actual.rows[r].fields.find(field => field.target.field === actual.columns[c].id)?.target;
            assert(target, "selected populated logical cell");
            const value = caseValue.kinds[c] === "number" ? { kind: "number", value: 700.25 }
              : { kind: "text", value: '000-edited-雪,"first"\r\nkept' };
            publish("edit_scalar", { target, input: value.kind === "number" ? { kind: "number", input: "700.25" } : value });
            const original = values[r][c]; values[r][c] = value; verify(table(), caseValue, values);
            publish("undo"); values[r][c] = original; verify(table(), caseValue, values);
            publish("redo"); values[r][c] = value; verify(table(), caseValue, values);
          }
          publish("undo"); // Both histories populated while refusals execute.
          const before = snapshot();
          if (caseValue.name === "320x2-mixed" && format === "csv") inspectionBudgetCases(result, before);
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
    assert.equal(result.inspection_budget.length, 5, "all inspection-budget controls must execute");
    result.completed = true;
  } catch (error) {
    result.failure = String(error); process.exitCode = 1;
  } finally {
    await writeFile(join(destination, "captures.json"), JSON.stringify(captures));
    await writeFile(join(destination, "result.json"), JSON.stringify(result, null, 2));
    console.log(JSON.stringify(result));
  }
}
