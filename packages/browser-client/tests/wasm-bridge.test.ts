import { afterEach, describe, expect, it, vi } from "vitest";
import { createDesignerWasmBridge } from "../src/runtime/wasm-bridge.ts";
import type { SpreadsheetOperation } from "../src/runtime/interop-protocol.ts";

const operation: Extract<SpreadsheetOperation, { type: "export" }> = {
  type: "export", expected_revision: "resident/1", format: "xlsx", collection: "sheet",
  metadata: { version: 1, sheets: [] },
};

async function fixture(receiptLength: number, arenaLength: number) {
  const memory = new WebAssembly.Memory({ initial: 66 });
  const responseOffset = 4 * 1024 * 1024 + 8192;
  const outputOffset = responseOffset + 8192;
  const response = new TextEncoder().encode(JSON.stringify({
    status: "ok", response: { type: "spreadsheet_exported", payload: {
      revision: "resident/1", byte_length: receiptLength, ledger: [],
    } },
  }));
  new Uint8Array(memory.buffer, responseOffset, response.length).set(response);
  new Uint8Array(memory.buffer, outputOffset, 4).set([1, 2, 3, 99]);
  const pointer = vi.fn(() => outputOffset);
  const release = vi.fn(() => { new Uint8Array(memory.buffer, outputOffset, 4).fill(0); });
  const reserve = vi.fn(() => 0);
  const exports = {
    memory,
    tachiko_designer_request_reserve: reserve,
    tachiko_designer_project_reserve: () => 4096,
    tachiko_designer_spreadsheet_run: () => undefined,
    tachiko_designer_response_ptr: () => responseOffset,
    tachiko_designer_response_len: () => response.length,
    tachiko_designer_project_ptr: pointer,
    tachiko_designer_project_len: () => arenaLength,
    tachiko_designer_project_release: release,
  };
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response()));
  vi.spyOn(WebAssembly, "instantiateStreaming").mockResolvedValue({
    instance: { exports }, module: {},
  });
  return { bridge: await createDesignerWasmBridge("/runtime.wasm"), pointer, release, reserve };
}

afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); });

describe("spreadsheet WASM export receipt", () => {
  it.each([[2, 3], [4, 3]])("rejects receipt %i for arena %i before reading bytes and releases it", async (receipt, arena) => {
    const { bridge, pointer, release } = await fixture(receipt, arena);
    expect(() => bridge.spreadsheet(operation, new Uint8Array())).toThrow("did not match its receipt");
    expect(pointer).not.toHaveBeenCalled();
    expect(release).toHaveBeenCalledOnce();
  });

  it("copies exactly the matching arena before releasing its storage", async () => {
    const { bridge, release } = await fixture(3, 3);
    const result = bridge.spreadsheet(operation, new Uint8Array());
    expect(result.status).toBe("spreadsheet_exported");
    if (result.status !== "spreadsheet_exported") throw new Error("Expected export");
    expect([...new Uint8Array(result.export.bytes)]).toEqual([1, 2, 3]);
    expect(release).toHaveBeenCalledOnce();
  });
});

describe("private metadata request transport", () => {
  function metadataOperation(type: "export" | "inspect_project", size: number): SpreadsheetOperation {
    const value: Extract<SpreadsheetOperation, { type: "export" | "inspect_project" }> = type === "export" ? structuredClone(operation)
      : { type, metadata: { version: 1, sheets: [] } };
    const sheet = { schema_id: "s", name: "", has_header: true, columns: [], rows: [] };
    value.metadata.sheets.push(sheet);
    sheet.name = "x".repeat(size - new TextEncoder().encode(JSON.stringify(value)).length);
    return value;
  }

  it.each(["export", "inspect_project"] as const)("allows only %s through the metadata arena", async type => {
    const { bridge, reserve } = await fixture(3, 3);
    bridge.spreadsheet(metadataOperation(type, 4 * 1024 * 1024), new Uint8Array());
    expect(reserve).toHaveBeenLastCalledWith(4 * 1024 * 1024);
    reserve.mockClear();
    const refusal = bridge.spreadsheet(metadataOperation(type, 4 * 1024 * 1024 + 1), new Uint8Array());
    expect(refusal.status).toBe("error");
    expect(reserve).not.toHaveBeenCalled();
  });

  it("keeps ordinary requests and non-metadata spreadsheet operations at 64 KiB", async () => {
    const { bridge, reserve } = await fixture(3, 3);
    expect(bridge.request({ type: "query_table", collection: "x".repeat(65_536) }).status).toBe("error");
    expect(bridge.spreadsheet({ type: "inspect", format: "csv", csv_options: {
      delimiter: "x".repeat(65_536), header: true,
    } }, new Uint8Array()).status).toBe("error");
    expect(reserve).not.toHaveBeenCalled();
  });
});
