import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import type { DesignerResponse, DesignerWireReply } from "../src/runtime/protocol.ts";
import type { SpreadsheetOperation } from "../src/runtime/interop-protocol.ts";

type DesignerAbi = WebAssembly.Exports & {
  memory: WebAssembly.Memory;
  tachiko_designer_request_reserve(length: number): number;
  tachiko_designer_project_reserve(length: number): number;
  tachiko_designer_request_run(): void;
  tachiko_designer_spreadsheet_run(): void;
  tachiko_designer_project_export(): void;
  tachiko_designer_response_ptr(): number;
  tachiko_designer_response_len(): number;
  tachiko_designer_project_ptr(): number;
  tachiko_designer_project_len(): number;
};
const encoder = new TextEncoder();
const decoder = new TextDecoder();
const source = encoder.encode("10,2\n20,4\n");
const importOptions = {
  type: "import", format: "csv", csv_options: { delimiter: ",", header: false },
  selection: { column_types: [["number", "number"]], extra_columns: [[]] },
  occurrence_id: "12345678-1234-4234-8234-123456789abc", install: false,
} satisfies SpreadsheetOperation;

type PayloadByType = { [Response in DesignerResponse as Response["type"]]: Response["payload"] };
function payload<T extends keyof PayloadByType>(reply: DesignerWireReply, type: T): PayloadByType[T] {
  expect(reply.status, JSON.stringify(reply)).toBe("ok");
  if (reply.status !== "ok") throw new Error(JSON.stringify(reply.error));
  expect(reply.response.type).toBe(type);
  return reply.response.payload as PayloadByType[T];
}

async function runtime() {
  // The release build copies this exact public artifact into the Designer distribution.
  const wasm = new Uint8Array(await readFile(new URL("../public/designer_runtime.wasm", import.meta.url)));
  const { instance } = await WebAssembly.instantiate(wasm, {});
  const abi = instance.exports as DesignerAbi;
  const requestBytes = (bytes: Uint8Array): void => {
    const pointer = abi.tachiko_designer_request_reserve(bytes.length);
    new Uint8Array(abi.memory.buffer, pointer, bytes.length).set(bytes);
  };
  const request = (operation: unknown): void => { requestBytes(encoder.encode(JSON.stringify(operation))); };
  const reply = (): DesignerWireReply => JSON.parse(decoder.decode(new Uint8Array(abi.memory.buffer, abi.tachiko_designer_response_ptr(), abi.tachiko_designer_response_len()))) as DesignerWireReply;
  const bytes = (): Uint8Array => new Uint8Array(abi.memory.buffer, abi.tachiko_designer_project_ptr(), abi.tachiko_designer_project_len()).slice();
  const run = (operation: unknown, input: Uint8Array = new Uint8Array()): DesignerWireReply => {
    request(operation);
    const pointer = abi.tachiko_designer_project_reserve(input.length);
    new Uint8Array(abi.memory.buffer, pointer, input.length).set(input);
    abi.tachiko_designer_spreadsheet_run();
    return reply();
  };
  return { abi, request, requestBytes, reply, bytes, run };
}

async function editedRuntime() {
  const bridge = await runtime();
  const previewReply = bridge.run(importOptions, source);
  const importedReply = bridge.run({ ...importOptions, install: true }, source);
  // Exact response equality includes metadata, projected identities and fidelity evidence.
  expect(importedReply).toEqual(previewReply);
  const imported = payload(importedReply, "imported");
  const fields = imported.opened.table.rows[0]?.fields;
  const input = fields?.[0]; const target = fields?.[1];
  if (!input || !target) throw new Error("Imported numeric fixture has no first row targets");
  bridge.request({ type: "formula_update", expected_revision: imported.opened.table.revision, target: target.target, source: `[${input.address}] * 2` });
  bridge.abi.tachiko_designer_request_run();
  const publication = payload(bridge.reply(), "published");
  const collection = imported.metadata.sheets[0]?.schema_id;
  if (!collection) throw new Error("Imported sheet metadata is missing");
  const exportOptions = { type: "export", expected_revision: publication.resulting_revision, metadata: imported.metadata, format: "csv", collection } satisfies SpreadsheetOperation;
  const assertEditedSnapshot = (): string => {
    const receipt = payload(bridge.run(exportOptions), "spreadsheet_exported");
    expect(receipt.revision).toBe(publication.resulting_revision);
    const csv = decoder.decode(bridge.bytes());
    expect(csv).toBe("10,20\r\n20,4\r\n");
    return csv;
  };
  return { ...bridge, imported, previewReply, publication, exportOptions, assertEditedSnapshot };
}

test("spreadsheet ABI keeps exact two-phase import and protects an edited snapshot across rejected admission", async () => {
  const bridge = await editedRuntime();
  bridge.assertEditedSnapshot();
  expect(bridge.run(importOptions, source)).toEqual(bridge.previewReply);
  bridge.assertEditedSnapshot();
  expect(bridge.run({ ...importOptions, install: true }, encoder.encode("bad\n")).status).toBe("error");
  bridge.assertEditedSnapshot();
  const implicitInstall: Record<string, unknown> = { ...importOptions };
  delete implicitInstall.install;
  expect(bridge.run(implicitInstall, source).status).toBe("error");
  bridge.assertEditedSnapshot();

  bridge.requestBytes(encoder.encode(bridge.publication.resulting_revision));
  bridge.abi.tachiko_designer_project_export();
  payload(bridge.reply(), "project_exported");
  const bundle = bridge.bytes();
  payload(bridge.run({ type: "inspect_project", metadata: bridge.imported.metadata }, bundle), "opened");
  bridge.assertEditedSnapshot();
  const invalidMetadata = structuredClone(bridge.imported.metadata);
  const sheet = invalidMetadata.sheets[0];
  if (!sheet) throw new Error("Missing metadata fixture");
  sheet.schema_id = "unknown";
  expect(bridge.run({ type: "inspect_project", metadata: invalidMetadata }, bundle).status).toBe("error");
  bridge.assertEditedSnapshot();
  expect(bridge.run({ ...bridge.exportOptions, collection: "unknown" }).status).toBe("error");
  expect(bridge.abi.tachiko_designer_project_len()).toBe(0);
  bridge.assertEditedSnapshot();
});

test("spreadsheet ABI consumes both arena rejection flags even when the request error wins", async () => {
  const bridge = await editedRuntime();
  bridge.abi.tachiko_designer_request_reserve(65_537);
  bridge.abi.tachiko_designer_project_reserve(67_108_865);
  bridge.abi.tachiko_designer_spreadsheet_run();
  expect(bridge.reply().status).toBe("error");
  // Do not reserve/reset project storage: the failed invocation itself must consume its flag.
  bridge.request(bridge.exportOptions);
  bridge.abi.tachiko_designer_spreadsheet_run();
  const recovered = payload(bridge.reply(), "spreadsheet_exported");
  expect(recovered.revision).toBe(bridge.publication.resulting_revision);
  expect(decoder.decode(bridge.bytes())).toBe("10,20\r\n20,4\r\n");
});

test("spreadsheet ABI rejects unrepresentable source and final selected columns without changing resident bytes or history", async () => {
  const bridge = await runtime();
  bridge.request({ type: "new_tracker", occurrence_id: importOptions.occurrence_id });
  bridge.abi.tachiko_designer_request_run();
  const opened = payload(bridge.reply(), "opened");
  const firstColumn = opened.table.columns[0];
  if (!firstColumn) throw new Error("Missing Tracker input column");
  bridge.request({ type: "paste_cells", expected_revision: opened.bootstrap.revision,
    collection: opened.bootstrap.default_collection, start_entity: null,
    start_field: firstColumn.id, rows: [["Preserve this history"]] });
  bridge.abi.tachiko_designer_request_run();
  const edited = payload(bridge.reply(), "published");
  const queryTable = () => {
    bridge.request({ type: "query_table", collection: opened.bootstrap.default_collection });
    bridge.abi.tachiko_designer_request_run();
    return payload(bridge.reply(), "table");
  };
  const editedTable = queryTable();
  const assertResident = (): void => { expect(queryTable()).toEqual(editedTable); };
  const snapshot = (): Uint8Array => {
    bridge.requestBytes(encoder.encode(edited.resulting_revision));
    bridge.abi.tachiko_designer_project_export();
    payload(bridge.reply(), "project_exported");
    return bridge.bytes();
  };
  const before = snapshot();
  for (const invalid of ["bad\u0000text", "bad\u000btext", "bad\ufffetext"]) {
    for (const csv of [`Label,Amount\n${invalid},2\n`, `${invalid},Amount\ntext,2\n`]) {
      const inspected = payload(bridge.run({
        type: "inspect", format: "csv", csv_options: { delimiter: ",", header: true },
      }, encoder.encode(csv)), "import_preview");
      expect(inspected.ledger.some(finding => finding.blocking)).toBe(true);
      expect(snapshot()).toEqual(before);
      assertResident();
    }
    for (const install of [false, true]) {
      const selected = {
        ...importOptions, install,
        selection: {
          ...importOptions.selection,
          extra_columns: [[{ name: invalid, field_type: "text" }]],
        },
      } satisfies SpreadsheetOperation;
      expect(bridge.run(selected, source).status).toBe("error");
      expect(snapshot()).toEqual(before);
      assertResident();
      const textOptions = {
        ...importOptions, install,
        selection: { column_types: [["text", "number"]], extra_columns: [[]] },
      } satisfies SpreadsheetOperation;
      expect(bridge.run(textOptions, encoder.encode(`${invalid},2\n`)).status).toBe("error");
      expect(snapshot()).toEqual(before);
      assertResident();
    }
  }
  // Failed preview/install attempts must also preserve the existing undo edge.
  bridge.request({ type: "undo", expected_revision: edited.resulting_revision });
  bridge.abi.tachiko_designer_request_run();
  const undone = payload(bridge.reply(), "published");
  const restored = queryTable();
  expect(restored.revision).toBe(undone.resulting_revision);
  expect(restored.rows).toEqual(opened.table.rows);
});

test("spreadsheet ABI inserts explicit XLSX headers with shifted formulas while CSV and source metadata remain unchanged", async () => {
  const bridge = await editedRuntime();
  const originalMetadata = structuredClone(bridge.imported.metadata);
  const receipt = payload(bridge.run(bridge.exportOptions), "spreadsheet_exported");
  expect(receipt.ledger.some(finding => finding.code === "csv_values_only")).toBe(true);
  const originalCsv = decoder.decode(bridge.bytes());
  const xlsxReceipt = payload(bridge.run({ ...bridge.exportOptions, format: "xlsx" }), "spreadsheet_exported");
  expect(xlsxReceipt.ledger.some(finding => finding.code === "xlsx_header_inserted" && finding.category === "converted")).toBe(true);
  const xlsx = bridge.bytes();
  const inspected = payload(bridge.run({ type: "inspect", format: "xlsx", csv_options: { delimiter: ",", header: true } }, xlsx), "import_preview");
  const sheet = inspected.sheets[0];
  expect(sheet?.has_header).toBe(true);
  expect(sheet?.rows).toHaveLength(2);
  expect(sheet?.rows[0]?.[1]?.formula).toContain("$A$2");
  expect(sheet?.rows[0]?.[1]?.value).toEqual({ kind: "number", value: 20 });
  expect(bridge.imported.metadata).toEqual(originalMetadata);
  expect(bridge.assertEditedSnapshot()).toBe(originalCsv);
});
