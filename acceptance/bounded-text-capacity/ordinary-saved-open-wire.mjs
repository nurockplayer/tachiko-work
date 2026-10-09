// Raw spreadsheet ABI checks for the original complete-wire Opened boundary.
// node ordinary-saved-open-wire.mjs WASM FIXTURE_DIR RESULT
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

const [wasmPath, fixtureDir, resultPath] = process.argv.slice(2);
assert(wasmPath && fixtureDir && resultPath, "expected WASM, fixture directory, and result path");

const wasm = await readFile(wasmPath);
const { instance } = await WebAssembly.instantiate(wasm, {});
const e = instance.exports;
const encoder = new TextEncoder();
const decoder = new TextDecoder();
const sha = bytes => createHash("sha256").update(bytes).digest("hex");
const json = value => encoder.encode(JSON.stringify(value));
const options = { delimiter: ",", header: true };
const selection = { column_types: [["text", "text", "text"]], extra_columns: [[]] };
let sequence = 80;
const uuid = () => `00000000-0000-4000-8000-${String(sequence++).padStart(12, "0")}`;
const cases = [];
let completed = false;

function readReply() {
  const pointer = e.tachiko_designer_response_ptr();
  const length = e.tachiko_designer_response_len();
  const bytes = new Uint8Array(e.memory.buffer, pointer, length).slice();
  return { bytes, reply: JSON.parse(decoder.decode(bytes)) };
}

function request(bytes) {
  const pointer = e.tachiko_designer_request_reserve(bytes.length);
  assert.notEqual(pointer, 0, `request reservation failed for ${bytes.length} bytes`);
  new Uint8Array(e.memory.buffer, pointer, bytes.length).set(bytes);
}

function project(bytes) {
  const pointer = e.tachiko_designer_project_reserve(bytes.length);
  if (bytes.length > 0) assert.notEqual(pointer, 0, `project reservation failed for ${bytes.length} bytes`);
  new Uint8Array(e.memory.buffer, pointer, bytes.length).set(bytes);
}

function spreadsheet(operation, savedBytes = new Uint8Array()) {
  project(savedBytes);
  request(json(operation));
  e.tachiko_designer_spreadsheet_run();
  return readReply();
}

function ordinary(operation) {
  request(json(operation));
  e.tachiko_designer_request_run();
  return readReply();
}

function okay(result, type) {
  assert.equal(result.reply.status, "ok", decoder.decode(result.bytes).slice(0, 500));
  assert.equal(result.reply.response.type, type);
  return result.reply.response.payload;
}

function refuse(result) {
  assert.equal(result.reply.status, "error", "saved-open boundary unexpectedly succeeded");
  assert.equal(result.reply.error.code, "query_too_large");
}

function output() {
  const bytes = new Uint8Array(
    e.memory.buffer,
    e.tachiko_designer_project_ptr(),
    e.tachiko_designer_project_len(),
  ).slice();
  e.tachiko_designer_project_release();
  return bytes;
}

function observe() {
  e.tachiko_designer_occurrence_observe();
  return okay(readReply(), "occurrence_observed");
}

function save(revision) {
  request(encoder.encode(revision));
  e.tachiko_designer_project_export();
  okay(readReply(), "project_exported");
  return output();
}

function queryTable(collection) {
  return okay(ordinary({ type: "query_table", collection }), "table");
}

function tableState(table) {
  const { revision, ...state } = table;
  return state;
}

function publish(type, revision, rest = {}) {
  const payload = okay(ordinary({ type, expected_revision: revision, ...rest }), "published");
  assert.equal(payload.base_revision, revision);
  assert.notEqual(payload.resulting_revision, revision);
  return payload.resulting_revision;
}

function csv(rows) {
  return encoder.encode(
    [["account_id", "profile_url", "unix_timestamp"], ...rows]
      .map(row => row.map(value => `"${value.replaceAll('"', '""')}"`).join(","))
      .join("\r\n") + "\r\n",
  );
}

function exportOperation(format, metadata, revision) {
  return {
    type: "export",
    format,
    expected_revision: revision,
    metadata,
    collection: metadata.sheets[0].schema_id,
  };
}

function snapshot(state) {
  const occurrence = observe();
  const table = queryTable(state.collection);
  const saved = save(occurrence.revision);
  const exports = {};
  for (const format of ["csv", "xlsx"]) {
    okay(spreadsheet(exportOperation(format, state.metadata, occurrence.revision)), "spreadsheet_exported");
    exports[format] = sha(output());
  }
  return {
    occurrence,
    table,
    saved_sha256: sha(saved),
    exports,
  };
}

function seedTwoRowResident() {
  const source = csv([
    ["id-1", "profile-1", "100"],
    ["id-2", "profile-2", "200"],
  ]);
  const importedResult = spreadsheet({
    type: "import",
    format: "csv",
    csv_options: options,
    selection,
    occurrence_id: uuid(),
    install: true,
  }, source);
  const imported = okay(importedResult, "imported");
  const state = {
    metadata: imported.metadata,
    collection: imported.metadata.sheets[0].schema_id,
    revision: imported.opened.bootstrap.revision,
  };
  const initialTable = queryTable(state.collection);
  assert.equal(initialTable.revision, state.revision, "initial table revision");
  const initial = tableState(initialTable);
  const target = initial.rows[0].fields[0].target;
  const earlierRevision = publish("edit_scalar", state.revision, {
    target,
    input: { kind: "text", value: "history-earlier" },
  });
  const earlierTable = queryTable(state.collection);
  assert.equal(earlierTable.revision, earlierRevision, "table revision after earlier edit");
  const earlier = tableState(earlierTable);
  const markerRevision = publish("edit_scalar", earlierRevision, {
    target,
    input: { kind: "text", value: "history-marker" },
  });
  const markerTable = queryTable(state.collection);
  assert.equal(markerTable.revision, markerRevision, "table revision after marker edit");
  const marker = tableState(markerTable);
  const restoredRevision = publish("undo", markerRevision);
  const restored = queryTable(state.collection);
  assert.equal(restored.revision, restoredRevision, "table revision after setup undo");
  assert.deepEqual(tableState(restored), earlier);
  state.revision = restoredRevision;
  state.history = { initial, earlier, marker };
  state.before = snapshot(state);
  return state;
}

function replayBothHistoryStacks(state) {
  let revision = publish("redo", state.revision);
  let table = queryTable(state.collection);
  assert.equal(table.revision, revision, "table revision after redo to marker");
  assert.deepEqual(tableState(table), state.history.marker);
  revision = publish("undo", revision);
  table = queryTable(state.collection);
  assert.equal(table.revision, revision, "table revision after undo to earlier");
  assert.deepEqual(tableState(table), state.history.earlier);
  revision = publish("undo", revision);
  table = queryTable(state.collection);
  assert.equal(table.revision, revision, "table revision after undo to initial");
  assert.deepEqual(tableState(table), state.history.initial);
  revision = publish("redo", revision);
  table = queryTable(state.collection);
  assert.equal(table.revision, revision, "table revision after redo to earlier");
  assert.deepEqual(tableState(table), state.history.earlier);
  revision = publish("redo", revision);
  table = queryTable(state.collection);
  assert.equal(table.revision, revision, "table revision after redo to marker");
  assert.deepEqual(tableState(table), state.history.marker);
  state.revision = revision;
}

function verifyFixture(item) {
  assert(["direct-v2", "canonical-v1", "selected-v3"].includes(item.carrier));
  assert([65_536, 65_537].includes(item.wire_bytes));
  assert.equal(item.file, `${item.carrier}-${item.wire_bytes}.twd`);
  assert(item.metadata && item.expected, "manifest must bind metadata and complete expected projection");
}

try {
  const manifest = JSON.parse(await readFile(join(fixtureDir, "manifest.json"), "utf8"));
  assert.equal(manifest.length, 6, "expected all three carriers at both ordinary wire boundaries");
  const keys = new Set();
  for (const item of manifest) {
    verifyFixture(item);
    const key = `${item.carrier}:${item.wire_bytes}`;
    assert(!keys.has(key), `duplicate fixture ${key}`);
    keys.add(key);
  }
  for (const carrier of ["direct-v2", "canonical-v1", "selected-v3"]) {
    for (const wireBytes of [65_536, 65_537]) assert(keys.has(`${carrier}:${wireBytes}`));
  }

  for (const item of manifest) {
    const name = `${item.carrier}-${item.wire_bytes}`;
    const replyBytes = {};
    try {
      const savedBytes = await readFile(join(fixtureDir, item.file));
      const state = seedTwoRowResident();
      if (item.wire_bytes === 65_536) {
        const beforeOccurrence = state.before.occurrence;
        const inspection = spreadsheet({ type: "inspect_project", metadata: item.metadata }, savedBytes);
        const inspected = okay(inspection, "opened");
        assert.equal(inspection.bytes.length, 65_536, `${name} inspection reply byte length`);
        replyBytes.inspect_project = inspection.bytes.length;
        assert.deepEqual(inspected, item.expected, `${name} full inspected projection`);
        assert.deepEqual(snapshot(state), state.before, `${name} inspection must not change resident state`);

        const openOccurrenceId = uuid();
        const opening = spreadsheet({
          type: "open_project",
          occurrence_id: openOccurrenceId,
          metadata: item.metadata,
        }, savedBytes);
        const opened = okay(opening, "opened");
        assert.equal(opening.bytes.length, 65_536, `${name} open reply byte length`);
        replyBytes.open_project = opening.bytes.length;
        assert.deepEqual(opened, item.expected, `${name} full opened projection`);
        const afterOccurrence = observe();
        assert.notEqual(afterOccurrence.scope, beforeOccurrence.scope, `${name} must install a fresh occurrence`);
        assert.equal(afterOccurrence.revision, "resident/0", `${name} fresh revision`);
        assert.equal(afterOccurrence.revision, opened.bootstrap.revision, `${name} projection revision`);
      } else {
        const inspection = spreadsheet({ type: "inspect_project", metadata: item.metadata }, savedBytes);
        refuse(inspection);
        replyBytes.inspect_project = inspection.bytes.length;
        assert.deepEqual(snapshot(state), state.before, `${name} refused inspection atomicity`);

        const opening = spreadsheet({
          type: "open_project",
          occurrence_id: uuid(),
          metadata: item.metadata,
        }, savedBytes);
        refuse(opening);
        replyBytes.open_project = opening.bytes.length;
        assert.deepEqual(snapshot(state), state.before, `${name} refused open atomicity`);
        replayBothHistoryStacks(state);
      }
      cases.push({ carrier: item.carrier, wire_bytes: item.wire_bytes, actual_reply_utf8_bytes: replyBytes, outcome: "PASS" });
    } catch (failure) {
      cases.push({
        carrier: item.carrier,
        wire_bytes: item.wire_bytes,
        actual_reply_utf8_bytes: replyBytes,
        outcome: "FAIL",
        message: String(failure),
      });
      throw failure;
    }
  }
  completed = cases.length === 6 && cases.every(item => item.outcome === "PASS");
  assert(completed, "all six ordinary saved-open cases must pass");
} catch (failure) {
  if (cases.at(-1)?.outcome !== "FAIL") cases.push({ outcome: "FAIL", message: String(failure) });
  throw failure;
} finally {
  await writeFile(resultPath, JSON.stringify({
    new_only: true,
    completed,
    wasm_sha256: sha(wasm),
    fixture_manifest_sha256: sha(await readFile(join(fixtureDir, "manifest.json"))),
    cases,
    note: "Raw spreadsheet ABI only; no browser Worker, kit, performance, or producer qualification credit.",
  }, null, 2) + "\n", { flag: "wx" });
}
