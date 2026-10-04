// Independent observable resource/atomicity probes against the actual Rust WASM.
// No bridge mock, product-source rewrite, or alternate semantic implementation.
// node boundaries.mjs WASM FIXTURES RESULT [parser-only]
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";

const [wasmPath, fixtures, resultPath, mode] = process.argv.slice(2);
assert(wasmPath && fixtures && resultPath);
const wasm = await readFile(wasmPath);
const { instance } = await WebAssembly.instantiate(wasm, {});
const e = instance.exports;
const encoder = new TextEncoder();
const decoder = new TextDecoder();
const records = [];
const options = { delimiter: ",", header: true };
const selection = { column_types: [["text", "text", "text"]], extra_columns: [[]] };
let occurrence = 10;
let memoryPeak = e.memory.buffer.byteLength;
let completed = false;
const sha = bytes => createHash("sha256").update(bytes).digest("hex");
const json = value => encoder.encode(JSON.stringify(value));
const uuid = () => `00000000-0000-4000-8000-${String(occurrence++).padStart(12, "0")}`;
function readReply() {
  memoryPeak = Math.max(memoryPeak, e.memory.buffer.byteLength);
  const bytes = new Uint8Array(e.memory.buffer, e.tachiko_designer_response_ptr(), e.tachiko_designer_response_len()).slice();
  return JSON.parse(decoder.decode(bytes));
}
function request(bytes) {
  const pointer = e.tachiko_designer_request_reserve(bytes.length);
  // An over-arena probe deliberately does not write through a rejected pointer.
  if (pointer !== 0 && bytes.length <= 4 * 1024 * 1024) new Uint8Array(e.memory.buffer, pointer, bytes.length).set(bytes);
}
function project(bytes) {
  const pointer = e.tachiko_designer_project_reserve(bytes.length);
  new Uint8Array(e.memory.buffer, pointer, bytes.length).set(bytes);
}
function ordinary(value, size) {
  const bytes = typeof value === "string" ? encoder.encode(value) : json(value);
  request(size ? padded(bytes, size) : bytes);
  e.tachiko_designer_request_run();
  return readReply();
}
function spreadsheet(operation, bytes = new Uint8Array(), size) {
  return spreadsheetRaw(json(operation), bytes, size);
}
function spreadsheetRaw(wire, bytes = new Uint8Array(), size) {
  project(bytes);
  request(size ? padded(wire, size) : wire);
  e.tachiko_designer_spreadsheet_run();
  return readReply();
}
function padded(bytes, size) {
  assert(bytes.length <= size, `wire ${bytes.length} exceeds padding target ${size}`);
  const result = new Uint8Array(size).fill(32);
  result.set(bytes);
  return result;
}
function okay(reply, type) {
  assert.equal(reply.status, "ok", JSON.stringify(reply).slice(0, 900));
  assert.equal(reply.response.type, type);
  return reply.response.payload;
}
function error(reply, pattern) {
  assert.equal(reply.status, "error", "operation unexpectedly succeeded");
  if (pattern) assert.match(reply.error.message, pattern);
}
function output() {
  const result = new Uint8Array(e.memory.buffer, e.tachiko_designer_project_ptr(), e.tachiko_designer_project_len()).slice();
  e.tachiko_designer_project_release();
  return result;
}
function observe() {
  e.tachiko_designer_occurrence_observe();
  return readReply();
}
function save(revision) {
  request(encoder.encode(revision));
  e.tachiko_designer_project_export();
  okay(readReply(), "project_exported");
  return output();
}
function importing(source, format = "csv", extra = {}) {
  return spreadsheet({ type: "import", format, csv_options: options, selection,
    occurrence_id: uuid(), install: true, ...extra }, source);
}
function csv(rows, headers = ["account_id", "profile_url", "unix_timestamp"]) {
  return encoder.encode([headers, ...rows].map(row => row.map(value => `"${value.replaceAll('"', '""')}"`).join(",")).join("\r\n") + "\r\n");
}
async function probe(name, callback) {
  const before = performance.now();
  try {
    await callback();
    records.push({ name, outcome: "PASS", milliseconds: performance.now() - before,
      wasm_bytes: e.memory.buffer.byteLength });
  } catch (failure) {
    records.push({ name, outcome: "FAIL", message: String(failure),
      milliseconds: performance.now() - before, wasm_bytes: e.memory.buffer.byteLength });
    throw failure;
  }
}

try {
  const smallCsv = okay(importing(await readFile(join(fixtures, "source-8.csv"))), "imported");
  const small = okay(importing(await readFile(join(fixtures, "source-8.xlsx")), "xlsx"), "imported");
  assert.deepEqual(small.opened.table.rows.map(row => row.fields.map(field => field.stored)),
    smallCsv.opened.table.rows.map(row => row.fields.map(field => field.stored)));
  records.push({ name: "new eight-row CSV/XLSX all-cell Text controls", outcome: "PASS" });
  const initial = save(small.opened.bootstrap.revision);
  const originalOccurrence = observe();
  for (const item of JSON.parse(await readFile(join(fixtures, "resource-manifest.json"), "utf8"))) {
    await probe(item.file, async () => {
      const reply = spreadsheet({ type: "inspect", format: "xlsx", csv_options: options }, await readFile(join(fixtures, item.file)));
      if (item.parser_accept) okay(reply, "import_preview");
      else error(reply, item.boundary === "source" ? /bytes|size|MiB/ : undefined);
      assert.deepEqual(observe(), originalOccurrence);
      assert.equal(sha(save(small.opened.bootstrap.revision)), sha(initial));
    });
  }
  await probe("ordinary request 65536 and 65537", () => {
    okay(ordinary({ type: "query_table", collection: small.opened.bootstrap.default_collection }, 65_536), "table");
    const refusal = ordinary({ type: "query_table", collection: small.opened.bootstrap.default_collection }, 65_537);
    error(refusal);
    assert.equal(refusal.error.code, "request_too_large");
    const invalid = ordinary("!", 65_537);
    error(invalid);
    assert.equal(invalid.error.code, "request_too_large");
    assert.deepEqual(observe(), originalOccurrence);
    assert.equal(sha(save(small.opened.bootstrap.revision)), sha(initial));
  });
  if (mode !== "parser-only") {
    await probe("small plain capacity preview uses complete profile", () => {
      const rows = Array.from({ length: 8 }, () => ["x".repeat(4096), "y".repeat(4096), "z".repeat(4096)]);
      const input = csv(rows);
      const preview = okay(spreadsheet({ type: "inspect", format: "csv", csv_options: options }, input), "import_preview");
      assert(json(preview).length > 65_536);
      assert.deepEqual(preview.sheets[0].rows.map(row => row.map(cell => cell.value)),
        rows.map(row => row.map(value => ({ kind: "text", value }))));
      assert.deepEqual(observe(), originalOccurrence);
      assert.equal(sha(save(small.opened.bootstrap.revision)), sha(initial));
      const candidate = okay(importing(input), "imported");
      assert.deepEqual(candidate.opened.table.rows.map(row => row.fields.map(field => field.stored)),
        rows.map(row => row.map(value => ({ kind: "text", value }))));
    });
    const source = await readFile(join(fixtures, "source-8406.csv"));
    let imported = okay(importing(source), "imported");
    let metadata = imported.metadata;
    let collection = imported.opened.bootstrap.default_collection;
    let revision = imported.opened.bootstrap.revision;
    const target = imported.opened.table.rows[0].fields[0].target;
    const baselineValue = imported.opened.table.rows[0].fields[0].stored.value;
    const exportOperation = format => ({ type: "export", format, expected_revision: revision, metadata,
      collection: metadata.sheets[0].schema_id });
    function snapshot() {
      const table = okay(ordinary({ type: "query_table", collection }), "table");
      const exports = {};
      for (const format of ["csv", "xlsx"]) {
        okay(spreadsheet(exportOperation(format)), "spreadsheet_exported");
        exports[format] = sha(output());
      }
      return { occurrence: observe(), table: sha(json(table)), project: sha(save(revision)), exports };
    }
    function publish(type, rest = {}) {
      const publication = okay(ordinary({ type, expected_revision: revision, ...rest }), "published");
      assert.equal(publication.base_revision, revision);
      assert.notEqual(publication.resulting_revision, revision);
      revision = publication.resulting_revision;
      return publication;
    }
    function unchanged(before) { assert.deepEqual(snapshot(), before); }
    await probe("Unicode XML-sensitive metadata header39 and name31 roundtrips", async () => {
      const before = snapshot();
      const changed = structuredClone(metadata);
      changed.sheets[0].name = '雪&"<\'>\n\t\r'.concat("n".repeat(22));
      const names = ["雪&", "<i>", 'url"\''.concat("x".repeat(20))];
      assert.equal(Array.from(changed.sheets[0].name).length, 31);
      for (let c = 0; c < 3; c++) changed.sheets[0].columns[c].name = names[c];
      for (const format of ["csv", "xlsx"]) {
        okay(spreadsheet({ ...exportOperation(format), metadata: changed }), "spreadsheet_exported");
        const bytes = output();
        await writeFile(join(dirname(resultPath), `capacity-metadata.${format}`), bytes, { flag: "wx" });
        const candidate = okay(importing(bytes, format, { install: false }), "imported");
        assert.deepEqual(candidate.metadata.sheets[0].columns.map(c => c.name), names);
        if (format === "xlsx") assert.equal(candidate.metadata.sheets[0].name, changed.sheets[0].name);
        assert.deepEqual(candidate.opened.table.rows.map(row => row.fields.map(field => field.stored)),
          imported.opened.table.rows.map(row => row.fields.map(field => field.stored)));
        unchanged(before);
      }
    });
    // Establish both history stacks, then prove each refusal preserves them.
    publish("edit_scalar", { target, input: { kind: "text", value: "history-earlier" } });
    publish("edit_scalar", { target, input: { kind: "text", value: "history-marker" } });
    publish("undo");
    const stable = snapshot();
    const refuse = async (name, action, capacity) => probe(name, () => { error(action(), capacity); unchanged(stable); });
    for (const format of ["csv", "xlsx"]) {
      for (const file of [`source-8407.${format}`, `malformed.${format}`]) {
        const invalid = await readFile(join(fixtures, file));
        await refuse(`atomic import ${file}`, () => importing(invalid, format), file.startsWith("source-") ? /8,?406/ : undefined);
      }
    }
    await refuse("capacity selected Number", () => importing(source, "csv", {
      selection: { column_types: [["text", "text", "number"]], extra_columns: [[]] },
    }));
    await refuse("capacity fourth selected field", () => importing(source, "csv", {
      selection: { column_types: [["text", "text", "text"]], extra_columns: [[{ name: "extra", field_type: "text" }]] },
    }));
    const closureSource = await readFile(join(fixtures, "candidate-output-closure.xlsx"));
    await refuse("candidate whose CSV escaping exceeds output budget", () => importing(closureSource, "xlsx"));
    const thirteen = csv(Array.from({ length: 8406 }, () => Array(13).fill("v")), Array.from({ length: 13 }, (_, c) => `field_${c}`));
    await refuse("thirteen-column profile remains unadmitted", () => importing(thirteen, "csv", {
      selection: { column_types: [Array(13).fill("text")], extra_columns: [[]] },
    }));
    await refuse("capacity without header", () => importing(source.subarray(source.indexOf(10) + 1), "csv", {
      csv_options: { delimiter: ",", header: false },
    }));
    for (const value of ["x".repeat(4097), "=1+1", "  @SUM(A1)", "bad\u0000xml"]) {
      await refuse(`atomic Text refusal ${JSON.stringify(value.slice(0, 20))}`, () => ordinary({
        type: "edit_scalar", expected_revision: revision, target, input: { kind: "text", value },
      }), value.length === 4097 ? /4,?096|4 KiB/ : undefined);
    }
    await refuse("stale scalar edit", () => ordinary({ type: "edit_scalar", expected_revision: "resident/0", target,
      input: { kind: "text", value: "stale" } }));
    for (const mutation of [
      m => { m.sheets[0].rows[0].styles[0].bold = true; },
      m => { m.sheets[0].columns[0].width = 12; },
      m => { m.sheets[0].has_header = false; },
      m => { m.sheets[0].rows[1].entity_id = m.sheets[0].rows[0].entity_id; },
      m => { m.sheets[0].columns[0].name = "=SUM(A1)"; },
      m => { [m.sheets[0].rows[0], m.sheets[0].rows[1]] = [m.sheets[0].rows[1], m.sheets[0].rows[0]]; },
      m => { [m.sheets[0].columns[0], m.sheets[0].columns[1]] = [m.sheets[0].columns[1], m.sheets[0].columns[0]]; },
    ]) {
      const changed = structuredClone(metadata); mutation(changed);
      await refuse(`metadata refusal ${mutation.toString()}`, () => spreadsheet({ ...exportOperation("xlsx"), metadata: changed }));
    }
    await probe("metadata discriminator permits type-last objects", () => {
      const { type, ...rest } = exportOperation("csv");
      const wire = json({ ...rest, type });
      assert(wire.length > 65_536);
      okay(spreadsheetRaw(wire), "spreadsheet_exported");
      assert.equal(sha(output()), stable.exports.csv);
      const inspected = okay(spreadsheetRaw(json({ metadata, type: "inspect_project" }), save(revision)), "opened");
      const resident = okay(ordinary({ type: "query_table", collection }), "table");
      assert.deepEqual(inspected.table.rows, resident.rows);
      unchanged(stable);
    });
    for (const [name, wire] of [
      ["invalid nonmetadata payload", '{"type":"import","selection":null}'],
      ["duplicate export then import", '{"type":"export","type":"import"}'],
      ["duplicate import then export", '{"type":"import","type":"export"}'],
      ["duplicate export", '{"type":"export","type":"export"}'],
      ["malformed allowed tag", '{"type":"export",'],
      ["positional array control", '["export"]'],
      ["deep ignored nonmetadata payload", '{"ignored":' + "[".repeat(512) + "0" + "]".repeat(512) + ',"type":"import"}'],
    ]) {
      await probe(`metadata discriminator refuses ${name}`, () => {
        const refused = spreadsheetRaw(encoder.encode(wire), new Uint8Array(), 65_537);
        error(refused);
        assert.equal(refused.error.code, "request_too_large");
        unchanged(stable);
      });
    }
    await probe("allowed discriminator retains typed payload validation", () => {
      const refused = spreadsheetRaw(json({ type: "export", metadata: null }), new Uint8Array(), 65_537);
      error(refused);
      assert.notEqual(refused.error.code, "request_too_large");
      unchanged(stable);
    });
    await probe("history remains executable after all refusals", () => {
      publish("redo");
      let fields = okay(ordinary({ type: "query_fields", expected_revision: revision, fields: [target] }), "fields");
      assert.equal(fields.fields[0].stored.value, "history-marker");
      publish("undo");
      fields = okay(ordinary({ type: "query_fields", expected_revision: revision, fields: [target] }), "fields");
      assert.equal(fields.fields[0].stored.value, "history-earlier");
      publish("undo");
      fields = okay(ordinary({ type: "query_fields", expected_revision: revision, fields: [target] }), "fields");
      assert.equal(fields.fields[0].stored.value, baselineValue);
      publish("redo");
      fields = okay(ordinary({ type: "query_fields", expected_revision: revision, fields: [target] }), "fields");
      assert.equal(fields.fields[0].stored.value, "history-earlier");
    });
    await probe("metadata request exact 4MiB and limit plus one", () => {
      const before = snapshot();
      okay(spreadsheet(exportOperation("csv"), new Uint8Array(), 4 * 1024 * 1024), "spreadsheet_exported");
      assert.equal(sha(output()), before.exports.csv);
      error(spreadsheet(exportOperation("csv"), new Uint8Array(), 4 * 1024 * 1024 + 1));
      unchanged(before);
    });
    await probe("encoded CSV header row exact 39 bytes and plus one", () => {
      const before = snapshot();
      const changed = structuredClone(metadata);
      // Ordinary ASCII avoids quoting ambiguity: the baseline header is 39 bytes.
      changed.sheets[0].columns[0].name += "x";
      error(spreadsheet({ ...exportOperation("csv"), metadata: changed }), /39/);
      unchanged(before);
    });
    await probe("oversized non-metadata spreadsheet operations", () => {
      const before = snapshot();
      for (const operation of [
        { type: "inspect", format: "csv", csv_options: options },
        { type: "import", format: "csv", csv_options: options, selection, occurrence_id: uuid(), install: true },
      ]) error(spreadsheet(operation, source, 65_537));
      unchanged(before);
    });
    await probe("4096 UTF8 Text byte ceiling", () => {
      publish("edit_scalar", { target, input: { kind: "text", value: "雪".repeat(1365) + "x" } });
      const fields = okay(ordinary({ type: "query_fields", expected_revision: revision, fields: [target] }), "fields");
      assert.equal(encoder.encode(fields.fields[0].stored.value).length, 4096);
      publish("undo");
    });
    await probe("aggregate Text exact 1MiB and plus one", () => {
      const rows = Array.from({ length: 8406 }, () => ["v", "v", "v"]);
      let remaining = 1024 * 1024 - 8406 * 3;
      for (const row of rows) for (let c = 0; c < 3 && remaining; c++) {
        const added = Math.min(remaining, 4095); row[c] += "a".repeat(added); remaining -= added;
      }
      assert.equal(remaining, 0);
      imported = okay(importing(csv(rows)), "imported");
      metadata = imported.metadata; collection = imported.opened.bootstrap.default_collection; revision = imported.opened.bootstrap.revision;
      const before = snapshot();
      rows.at(-1)[2] += "a";
      error(importing(csv(rows)), /1,?048,?576|1 MiB/);
      unchanged(before);
    });
    await probe("CSV export exact 2MiB and candidate limit plus one", async () => {
      imported = okay(importing(await readFile(join(fixtures, "export-byte-0.xlsx")), "xlsx"), "imported");
      metadata = imported.metadata; collection = imported.opened.bootstrap.default_collection; revision = imported.opened.bootstrap.revision;
      okay(spreadsheet(exportOperation("csv")), "spreadsheet_exported");
      assert.equal(output().length, 2 * 1024 * 1024);
      const before = snapshot();
      error(importing(await readFile(join(fixtures, "export-byte-1.xlsx")), "xlsx"), /2,?097,?152|2 MiB/);
      unchanged(before);
      // Its actual CSV would fit, but the common 39-byte reserved envelope does not.
      error(importing(await readFile(join(fixtures, "export-short-header-reserve.xlsx")), "xlsx"), /2,?097,?152|2 MiB/);
      unchanged(before);
      const field = imported.opened.table.rows.flatMap(row => row.fields)
        .find(field => field.stored?.value?.includes("a"));
      assert(field, "independent exact-boundary fixture has a replaceable letter");
      error(ordinary({ type: "edit_scalar", expected_revision: revision, target: field.target,
        input: { kind: "text", value: field.stored.value.replace("a", '"') } }), /2,?097,?152|2 MiB/);
      unchanged(before);
    });
    await probe("empty Text retains existing missing-value representation", () => {
      const rows = Array.from({ length: 8406 }, () => ["v", "v", "v"]);
      rows[8405][2] = "";
      imported = okay(importing(csv(rows)), "imported");
      metadata = imported.metadata; collection = imported.opened.bootstrap.default_collection; revision = imported.opened.bootstrap.revision;
      assert.equal(imported.opened.table.columns[2].field_type, "text");
      const final = imported.opened.table.rows[8405];
      assert.equal(final.id, metadata.sheets[0].rows[8405].entity_id);
      const cell = final.fields.find(field => field.target.field === imported.opened.table.columns[2].id);
      assert(cell === undefined || cell.stored === null || cell.stored?.value === "");
      okay(spreadsheet(exportOperation("csv")), "spreadsheet_exported");
      assert(decoder.decode(output()).endsWith('v,v,""\r\n'));
      const bytes = save(revision);
      project(bytes); request(encoder.encode(uuid())); e.tachiko_designer_project_open();
      const reopened = okay(readReply(), "opened");
      assert.equal(reopened.table.rows.length, 8406);
      assert.deepEqual(reopened.table.rows[8405], final);
    });
    assert(memoryPeak <= 256 * 1024 * 1024, `raw ABI WASM highwater ${memoryPeak}; separate actual Worker measurement required`);
  }
  completed = true;
} catch (failure) {
  if (records.at(-1)?.outcome !== "FAIL") records.push({ name: "remaining acceptance", outcome: "FAIL", message: String(failure) });
  throw failure;
} finally {
  await writeFile(resultPath, JSON.stringify({ mode: mode ?? "all", completed, wasm_sha256: sha(wasm), records,
    wasm_linear_memory_high_water_bytes: memoryPeak,
    note: "Raw ABI under Node; not browser Worker memory or consumer qualification." }, null, 2) + "\n", { flag: "wx" });
}
