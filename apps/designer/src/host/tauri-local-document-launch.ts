import type { LocalDocumentHandle } from "./local-document-ingress.ts";

const OPENED_DOCUMENTS_EVENT = "macos-local-ro-opened";

type OpenedDocument = {
  id: string;
  name: string;
};

export type TauriDocumentBridge = {
  invoke(command: string, args?: Record<string, unknown>): Promise<unknown>;
  listen(
    event: string,
    handler: (event: { payload: OpenedDocument[] }) => void | Promise<void>,
  ): Promise<() => void>;
};

function isOpenedDocument(value: unknown): value is OpenedDocument {
  if (value === null || typeof value !== "object") return false;
  const candidate = value as Record<string, unknown>;
  return typeof candidate.id === "string" && typeof candidate.name === "string";
}

function openedDocuments(value: unknown): OpenedDocument[] {
  if (!Array.isArray(value)) throw new Error("The native open event did not provide a document list.");
  return value.map((document) => {
    if (!isOpenedDocument(document)) {
      throw new Error("The native open event did not provide a readable document.");
    }
    return document;
  });
}

function openedDocumentBytes(value: unknown): ArrayBuffer {
  if (!Array.isArray(value) || !value.every(byte => typeof byte === "number" && Number.isInteger(byte) && byte >= 0 && byte <= 255)) {
    throw new Error("The native opened document did not provide valid bytes.");
  }
  return Uint8Array.from(value).buffer;
}

/**
 * Turn a native, OS-granted document token into the existing opaque file
 * handle. The native host permits one read only for this token; it never
 * accepts a frontend-provided filesystem path.
 */
function documentHandle(
  bridge: TauriDocumentBridge,
  document: OpenedDocument,
): LocalDocumentHandle {
  return {
    kind: "file",
    name: document.name,
    async getFile(): Promise<File> {
      const bytes = openedDocumentBytes(await bridge.invoke("read_opened_document", { id: document.id }));
      return new File([bytes], document.name);
    },
  };
}

async function releaseDocuments(
  bridge: TauriDocumentBridge,
  documents: readonly OpenedDocument[],
): Promise<void> {
  try {
    await bridge.invoke("release_opened_documents", { ids: documents.map(document => document.id) });
  } catch {
    // Releasing an already-read one-shot grant is cleanup only. The Designer
    // must retain the real local-document success or visible rejection.
  }
}

async function consumeDocuments(
  bridge: TauriDocumentBridge,
  documents: readonly OpenedDocument[],
  consume: (handles: readonly LocalDocumentHandle[]) => Promise<void>,
): Promise<void> {
  if (documents.length === 0) return;
  try {
    await consume(documents.map(document => documentHandle(bridge, document)));
  } finally {
    await releaseDocuments(bridge, documents);
  }
}

function hasTauriRuntime(target: object): boolean {
  return "__TAURI_INTERNALS__" in target;
}

/**
 * Register the macOS-only Tauri transport without changing ordinary Web/PWA
 * startup. Listening before consuming the native cold-start latch prevents a
 * warm OS event from being lost at the frontend-ready boundary.
 */
export async function registerTauriLocalDocumentLaunch(
  target: object,
  consume: (handles: readonly LocalDocumentHandle[]) => Promise<void>,
  loadBridge: () => Promise<TauriDocumentBridge> = async () => {
    const [{ invoke }, { listen }] = await Promise.all([
      import("@tauri-apps/api/core"),
      import("@tauri-apps/api/event"),
    ]);
    return { invoke, listen };
  },
): Promise<boolean> {
  if (!hasTauriRuntime(target)) return false;

  const bridge = await loadBridge();
  await bridge.listen(OPENED_DOCUMENTS_EVENT, async ({ payload }) => {
    await consumeDocuments(bridge, payload, consume);
  });
  const pending = openedDocuments(await bridge.invoke("take_pending_opened_documents"));
  await consumeDocuments(bridge, pending, consume);
  return true;
}
