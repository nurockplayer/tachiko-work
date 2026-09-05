import type {
  DesignerRequest,
  ProjectExport,
  DesignerWireReply,
} from "./protocol.ts";

type DesignerWasmExports = {
  memory: WebAssembly.Memory;
  tachiko_designer_request_reserve(length: number): number;
  tachiko_designer_request_run(): void;
  tachiko_designer_response_ptr(): number;
  tachiko_designer_response_len(): number;
  tachiko_designer_project_reserve(length: number): number;
  tachiko_designer_project_open(): void;
  tachiko_designer_project_inspect(): void;
  tachiko_designer_project_export(): void;
  tachiko_designer_project_release(): void;
  tachiko_designer_project_close(): void;
  tachiko_designer_project_ptr(): number;
  tachiko_designer_project_len(): number;
};

const MAX_WIRE_REQUEST_BYTES = 65_536;
const MAX_PROJECT_TRANSFER_BYTES = 64 * 1024 * 1024;

export type DesignerWasmBridge = {
  request(request: DesignerRequest): DesignerWireReply;
  inspectProject(bytes: Uint8Array): DesignerWireReply;
  openProject(bytes: Uint8Array, occurrenceId: string): DesignerWireReply;
  exportProject(expectedRevision: string):
    | { status: "ok"; export: ProjectExport }
    | Extract<DesignerWireReply, { status: "error" }>;
  closeProject(): void;
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

  const writeRequest = (input: Uint8Array): boolean => {
    if (input.length > MAX_WIRE_REQUEST_BYTES) return false;
    const requestPointer = exports.tachiko_designer_request_reserve(input.length);
    new Uint8Array(exports.memory.buffer, requestPointer, input.length).set(input);
    return true;
  };
  const readReply = (): DesignerWireReply => {
    const responsePointer = exports.tachiko_designer_response_ptr();
    const responseLength = exports.tachiko_designer_response_len();
    const bytes = new Uint8Array(
      exports.memory.buffer,
      responsePointer,
      responseLength,
    ).slice();
    return JSON.parse(decoder.decode(bytes)) as DesignerWireReply;
  };
  const tooLargeReply = (
    message: string,
  ): Extract<DesignerWireReply, { status: "error" }> => ({
    status: "error",
    error: {
      code: "request_too_large",
      message,
      current_revision: "unavailable",
      diagnostics: [],
    },
  });

  return {
    request: (request) => {
      const input = encoder.encode(JSON.stringify(request));
      if (input.length > MAX_WIRE_REQUEST_BYTES) {
        return tooLargeReply("The Designer request exceeds the private bridge byte limit.");
      }
      writeRequest(input);
      exports.tachiko_designer_request_run();
      return readReply();
    },
    inspectProject: (bytes) => {
      if (bytes.byteLength > MAX_PROJECT_TRANSFER_BYTES) {
        return tooLargeReply("The project exceeds the private 64 MiB host transfer boundary.");
      }
      const pointer = exports.tachiko_designer_project_reserve(bytes.byteLength);
      new Uint8Array(exports.memory.buffer, pointer, bytes.byteLength).set(bytes);
      exports.tachiko_designer_project_inspect();
      return readReply();
    },
    openProject: (bytes, occurrenceId) => {
      if (bytes.byteLength > MAX_PROJECT_TRANSFER_BYTES) {
        return tooLargeReply(
          "The project exceeds the private 64 MiB host transfer boundary.",
        );
      }
      const occurrence = encoder.encode(occurrenceId);
      if (!writeRequest(occurrence)) {
        return tooLargeReply("The host occurrence identity exceeds the bridge limit.");
      }
      const pointer = exports.tachiko_designer_project_reserve(bytes.byteLength);
      new Uint8Array(exports.memory.buffer, pointer, bytes.byteLength).set(bytes);
      exports.tachiko_designer_project_open();
      return readReply();
    },
    exportProject: (expectedRevision) => {
      const revision = encoder.encode(expectedRevision);
      if (!writeRequest(revision)) {
        return tooLargeReply("The expected revision exceeds the bridge limit.");
      }
      exports.tachiko_designer_project_export();
      try {
        const reply = readReply();
        if (reply.status === "error") return reply;
        if (reply.response.type !== "project_exported") {
          throw new Error(
            `Expected 'project_exported' response, received '${reply.response.type}'.`,
          );
        }
        const pointer = exports.tachiko_designer_project_ptr();
        const length = exports.tachiko_designer_project_len();
        if (length !== reply.response.payload.byte_length) {
          throw new Error("Designer project export length did not match its receipt.");
        }
        const projectBytes = new Uint8Array(
          exports.memory.buffer,
          pointer,
          length,
        ).slice();
        return {
          status: "ok",
          export: {
            revision: reply.response.payload.revision,
            bytes: projectBytes.buffer,
          },
        };
      } finally {
        exports.tachiko_designer_project_release();
      }
    },
    closeProject: () => {
      exports.tachiko_designer_project_close();
    },
  };
}
