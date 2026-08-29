import type {
  DesignerRequest,
  DesignerWireReply,
} from "./protocol.ts";

type DesignerWasmExports = {
  memory: WebAssembly.Memory;
  tachiko_designer_request_reserve(length: number): number;
  tachiko_designer_request_run(): void;
  tachiko_designer_response_ptr(): number;
  tachiko_designer_response_len(): number;
};

const MAX_WIRE_REQUEST_BYTES = 65_536;

export type DesignerWasmBridge = {
  request(request: DesignerRequest): DesignerWireReply;
};

export async function createDesignerWasmBridge(
  wasmUrl: string,
): Promise<DesignerWasmBridge> {
  const response = await fetch(wasmUrl);
  if (!response.ok) {
    throw new Error(`Designer runtime could not be loaded (${String(response.status)}).`);
  }
  const result = await WebAssembly.instantiateStreaming(response, {});
  const exports = result.instance.exports as DesignerWasmExports;
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();

  return {
    request: (request) => {
      const input = encoder.encode(JSON.stringify(request));
      if (input.length > MAX_WIRE_REQUEST_BYTES) {
        return {
          status: "error",
          error: {
            code: "request_too_large",
            message: "The Designer request exceeds the private bridge byte limit.",
            current_revision: "unavailable",
            diagnostics: [],
          },
        };
      }
      const requestPointer = exports.tachiko_designer_request_reserve(input.length);
      new Uint8Array(exports.memory.buffer, requestPointer, input.length).set(input);
      exports.tachiko_designer_request_run();
      const responsePointer = exports.tachiko_designer_response_ptr();
      const responseLength = exports.tachiko_designer_response_len();
      const bytes = new Uint8Array(
        exports.memory.buffer,
        responsePointer,
        responseLength,
      ).slice();
      return JSON.parse(decoder.decode(bytes)) as DesignerWireReply;
    },
  };
}
