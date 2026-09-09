import { describe, expect, it, vi } from "vitest";

import { readSingleLocalRoDocument } from "../src/host/local-document-ingress.ts";
import type { LocalDocumentHandle } from "../src/host/local-document-ingress.ts";
import {
  registerTauriLocalDocumentLaunch,
  type TauriDocumentBridge,
} from "../src/host/tauri-local-document-launch.ts";

type OpenedDocument = { id: string; name: string };

function bridgeWithPending(pending: readonly OpenedDocument[]): {
  bridge: TauriDocumentBridge;
  invoke: ReturnType<typeof vi.fn>;
  deliver: (documents: OpenedDocument[]) => Promise<void>;
} {
  let listener: ((event: { payload: OpenedDocument[] }) => void | Promise<void>) | undefined;
  const invoke = vi.fn(async (command: string, args?: Record<string, unknown>): Promise<unknown> => {
    if (command === "take_pending_opened_documents") return [...pending];
    if (command === "read_opened_document") {
      const id = args?.id;
      if (id === "first") return [0, 255, 12];
      throw new Error(`unknown document grant: ${String(id)}`);
    }
    if (command === "release_opened_documents") return undefined;
    throw new Error(`unexpected command: ${command}`);
  });
  return {
    bridge: {
      invoke,
      async listen(_event, next) {
        listener = next;
        return () => undefined;
      },
    },
    invoke,
    async deliver(documents) {
      if (listener === undefined) throw new Error("native event listener was not registered");
      await listener({ payload: documents });
    },
  };
}

describe("Tauri local document launch transport", () => {
  it("leaves ordinary Web/PWA startup untouched", async () => {
    const consume = vi.fn(async () => undefined);

    await expect(registerTauriLocalDocumentLaunch({}, consume)).resolves.toBe(false);
    expect(consume).not.toHaveBeenCalled();
  });

  it("consumes the one-shot cold-start grant through the existing file handle boundary", async () => {
    const harness = bridgeWithPending([{ id: "first", name: "Moonfall.ro" }]);
    const consume = vi.fn(async (handles: readonly LocalDocumentHandle[]) => {
      const document = await readSingleLocalRoDocument(handles);
      expect(document.name).toBe("Moonfall.ro");
      expect([...new Uint8Array(document.bytes)]).toEqual([0, 255, 12]);
    });

    await expect(
      registerTauriLocalDocumentLaunch(
        { __TAURI_INTERNALS__: {} },
        consume,
        async () => harness.bridge,
      ),
    ).resolves.toBe(true);

    expect(harness.invoke).toHaveBeenCalledWith("read_opened_document", { id: "first" });
    expect(harness.invoke).toHaveBeenLastCalledWith("release_opened_documents", { ids: ["first"] });
  });

  it("preserves a warm multi-file event for shared ingress rejection before either grant is read", async () => {
    const harness = bridgeWithPending([]);
    const consume = vi.fn(async (handles: readonly LocalDocumentHandle[]) => {
      await expect(readSingleLocalRoDocument(handles)).rejects.toThrow("exactly one");
    });

    await registerTauriLocalDocumentLaunch(
      { __TAURI_INTERNALS__: {} },
      consume,
      async () => harness.bridge,
    );
    await harness.deliver([
      { id: "first", name: "Moonfall.ro" },
      { id: "second", name: "Moonfall-copy.ro" },
    ]);

    expect(harness.invoke).not.toHaveBeenCalledWith("read_opened_document", expect.anything());
    expect(harness.invoke).toHaveBeenLastCalledWith("release_opened_documents", {
      ids: ["first", "second"],
    });
  });
});
