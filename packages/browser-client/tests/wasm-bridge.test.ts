import { afterEach, describe, expect, it, vi } from "vitest";
import { createDesignerWasmBridge } from "../src/runtime/wasm-bridge.ts";
import type { SpreadsheetOperation } from "../src/runtime/interop-protocol.ts";

const operation: SpreadsheetOperation = {
  type: "export", expected_revision: "resident/1", format: "xlsx", collection: "sheet",
  metadata: { version: 1, sheets: [] },
};

async function fixture(receiptLength: number, arenaLength: number, responseType = "spreadsheet_exported") {
  const memory = new WebAssembly.Memory({ initial: 1 });
  const response = new TextEncoder().encode(JSON.stringify({
    status: "ok", response: { type: responseType, payload: {
      revision: "resident/1", byte_length: receiptLength, ledger: [],
    } },
  }));
  new Uint8Array(memory.buffer, 8192, response.length).set(response);
  new Uint8Array(memory.buffer, 16384, 4).set([1, 2, 3, 99]);
  const pointer = vi.fn(() => 16384);
  const release = vi.fn(() => { new Uint8Array(memory.buffer, 16384, 4).fill(0); });
  const selectedExport = vi.fn();
  const exports = {
    memory,
    tachiko_designer_request_reserve: () => 0,
    tachiko_designer_project_reserve: () => 4096,
    tachiko_designer_spreadsheet_run: () => undefined,
    tachiko_designer_project_export_v3: selectedExport,
    tachiko_designer_response_ptr: () => 8192,
    tachiko_designer_response_len: () => response.length,
    tachiko_designer_project_ptr: pointer,
    tachiko_designer_project_len: () => arenaLength,
    tachiko_designer_project_release: release,
  };
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response()));
  vi.spyOn(WebAssembly, "instantiateStreaming").mockResolvedValue({
    instance: { exports }, module: {},
  });
  return { bridge: await createDesignerWasmBridge("/runtime.wasm"), pointer, release, selectedExport };
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


describe("selected v3 WASM export ownership", () => {
  it("routes selected export and copies opaque bytes before arena release", async () => {
    const { bridge, release, selectedExport } = await fixture(3, 3, "project_exported");
    const result = bridge.exportProjectV3("resident/1");
    expect(selectedExport).toHaveBeenCalledOnce();
    expect(result.status).toBe("ok");
    if (result.status !== "ok") throw new Error("Expected selected export");
    expect(result.export.revision).toBe("resident/1");
    expect([...new Uint8Array(result.export.bytes)]).toEqual([1, 2, 3]);
    expect(release).toHaveBeenCalledOnce();
  });

  it("rejects a mismatched receipt and releases the arena", async () => {
    const { bridge, release } = await fixture(4, 3, "project_exported");
    expect(() => bridge.exportProjectV3("resident/1")).toThrow("did not match its receipt");
    expect(release).toHaveBeenCalledOnce();
  });

  it("releases the arena when the response has the wrong operation type", async () => {
    const { bridge, release } = await fixture(3, 3);
    expect(() => bridge.exportProjectV3("resident/1")).toThrow("Expected 'project_exported'");
    expect(release).toHaveBeenCalledOnce();
  });
});
