const DATABASE_NAME = "tachiko-designer-projects";
const DATABASE_VERSION = 2;
const PROJECT_STORE = "projects";
const PROJECT_SUMMARY_STORE = "project_summaries";

type ProjectRecord = {
  name: string;
  bytes: ArrayBuffer;
  saved_at: string;
  presentation?: string;
};

export type SavedProjectSummary = {
  name: string;
  byte_length: number;
  saved_at: string;
};

export interface DesignerProjectHost {
  list(): Promise<SavedProjectSummary[]>;
  read(name: string): Promise<ArrayBuffer>;
  readSnapshot?(name: string): Promise<{ bytes: ArrayBuffer; presentation?: string }>;
  publish(name: string, bytes: ArrayBuffer, presentation?: string): Promise<void>;
  readPresentation?(name: string): Promise<string | undefined>;
  update?(name: string, bytes: ArrayBuffer, expectedBytes: ArrayBuffer, presentation?: string, expectedPresentation?: string): Promise<void>;
}

export class ProjectHostError extends Error {
  readonly code: "destination_exists" | "not_found" | "invalid_name" | "stale_project" | "host_failure";

  constructor(
    code: ProjectHostError["code"],
    message: string,
    options?: ErrorOptions,
  ) {
    super(message, options);
    this.name = "ProjectHostError";
    this.code = code;
  }
}

/// Browser composition for opaque canonical project payloads.
///
/// IndexedDB's single-record read/write transaction is the host commit point.
/// `add` is deliberately create-only: an existing name aborts the transaction,
/// and no partially visible destination is possible.
export class BrowserProjectHost implements DesignerProjectHost {
  readonly #database: Promise<IDBDatabase>;

  constructor() {
    this.#database = openDatabase();
  }

  async list(): Promise<SavedProjectSummary[]> {
    const database = await this.#database;
    const transaction = database.transaction(PROJECT_SUMMARY_STORE, "readonly");
    const request = transaction
      .objectStore(PROJECT_SUMMARY_STORE)
      .getAll() as IDBRequest<SavedProjectSummary[]>;
    const [records] = await Promise.all([
      requestResult<SavedProjectSummary[]>(request),
      transactionComplete(transaction),
    ]);
    return records.sort((left, right) => left.name.localeCompare(right.name));
  }

  async read(name: string): Promise<ArrayBuffer> {
    return (await this.readSnapshot(name)).bytes;
  }

  async readSnapshot(name: string): Promise<{ bytes: ArrayBuffer; presentation?: string }> {
    const canonicalName = projectName(name);
    const database = await this.#database;
    const transaction = database.transaction(PROJECT_STORE, "readonly");
    const request = transaction.objectStore(PROJECT_STORE).get(
      canonicalName,
    ) as IDBRequest<ProjectRecord | undefined>;
    const [record] = await Promise.all([
      requestResult<ProjectRecord | undefined>(request),
      transactionComplete(transaction),
    ]);
    if (record === undefined) {
      throw new ProjectHostError(
        "not_found",
        `Saved project '${canonicalName}' is no longer available.`,
      );
    }
    return {
      bytes: record.bytes.slice(0),
      ...(record.presentation === undefined ? {} : { presentation: record.presentation }),
    };
  }

  async readPresentation(name: string): Promise<string | undefined> {
    return (await this.readSnapshot(name)).presentation;
  }

  async publish(name: string, bytes: ArrayBuffer, presentation?: string): Promise<void> {
    const canonicalName = projectName(name);
    const database = await this.#database;
    const transaction = database.transaction(
      [PROJECT_STORE, PROJECT_SUMMARY_STORE],
      "readwrite",
    );
    const savedAt = new Date().toISOString();
    const projectRequest = transaction.objectStore(PROJECT_STORE).add({
      name: canonicalName,
      bytes: bytes.slice(0),
      saved_at: savedAt,
      ...(presentation === undefined ? {} : { presentation }),
    } satisfies ProjectRecord);
    const summaryRequest = transaction.objectStore(PROJECT_SUMMARY_STORE).add({
      name: canonicalName,
      byte_length: bytes.byteLength,
      saved_at: savedAt,
    } satisfies SavedProjectSummary);
    try {
      await Promise.all([
        requestResult(projectRequest),
        requestResult(summaryRequest),
        transactionComplete(transaction),
      ]);
    } catch (error) {
      if (error instanceof DOMException && error.name === "ConstraintError") {
        throw new ProjectHostError(
          "destination_exists",
          `'${canonicalName}' already exists. Save As never overwrites a project.`,
          { cause: error },
        );
      }
      throw new ProjectHostError(
        "host_failure",
        `The browser could not publish '${canonicalName}'.`,
        { cause: error },
      );
    }
  }

  async update(name: string, bytes: ArrayBuffer, expectedBytes: ArrayBuffer, presentation?: string, expectedPresentation?: string): Promise<void> {
    const canonicalName = projectName(name);
    // Snapshot caller-owned buffers before the first asynchronous boundary.
    const candidate = bytes.slice(0);
    const expected = expectedBytes.slice(0);
    const database = await this.#database;
    const transaction = database.transaction(
      [PROJECT_STORE, PROJECT_SUMMARY_STORE],
      "readwrite",
    );
    const completion = transactionComplete(transaction);
    let failure: unknown;
    const store = transaction.objectStore(PROJECT_STORE);
    const request = store.get(canonicalName) as IDBRequest<ProjectRecord | undefined>;
    // Compare and replace in the same transaction: concurrent tabs cannot both
    // publish against the same prior bytes, or resurrect a removed project.
    request.addEventListener("success", () => {
      try {
        const record = request.result;
        if (record === undefined) {
          throw new ProjectHostError(
            "not_found",
            `Saved project '${canonicalName}' is no longer available. Use Save As to create a new project.`,
          );
        }
        if (!sameBytes(record.bytes, expected) || record.presentation !== expectedPresentation) {
          throw new ProjectHostError(
            "stale_project",
            `Saved project '${canonicalName}' changed elsewhere. Reopen it or use Save As to preserve your edits.`,
          );
        }
        const savedAt = new Date().toISOString();
        store.put({
          name: canonicalName,
          bytes: candidate,
          saved_at: savedAt,
          ...(presentation === undefined ? {} : { presentation }),
        } satisfies ProjectRecord);
        transaction.objectStore(PROJECT_SUMMARY_STORE).put({
          name: canonicalName,
          byte_length: candidate.byteLength,
          saved_at: savedAt,
        } satisfies SavedProjectSummary);
      } catch (error) {
        failure = error;
        transaction.abort();
      }
    });
    try {
      await completion;
    } catch (error) {
      if (failure instanceof ProjectHostError) throw failure;
      throw new ProjectHostError(
        "host_failure",
        `The browser could not save '${canonicalName}'.`,
        { cause: failure ?? error },
      );
    }
  }
}

function sameBytes(left: ArrayBuffer, right: ArrayBuffer): boolean {
  if (left.byteLength !== right.byteLength) return false;
  const rightBytes = new Uint8Array(right);
  return new Uint8Array(left).every((value, index) => value === rightBytes[index]);
}

function projectName(input: string): string {
  const name = input.trim();
  if (
    name.length === 0 ||
    name.length > 128 ||
    name === "." ||
    name === ".." ||
    name.includes("/") ||
    name.includes("\\") ||
    !name.endsWith(".roproj")
  ) {
    throw new ProjectHostError(
      "invalid_name",
      "Choose a simple project name ending in .roproj (maximum 128 characters).",
    );
  }
  return name;
}

function openDatabase(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DATABASE_NAME, DATABASE_VERSION);
    request.addEventListener("upgradeneeded", (event) => {
      const transaction = request.transaction;
      if (transaction === null) {
        throw new Error("IndexedDB upgrade transaction is unavailable.");
      }
      const projectStore = request.result.objectStoreNames.contains(PROJECT_STORE)
        ? transaction.objectStore(PROJECT_STORE)
        : request.result.createObjectStore(PROJECT_STORE, { keyPath: "name" });
      if (!request.result.objectStoreNames.contains(PROJECT_SUMMARY_STORE)) {
        const summaryStore = request.result.createObjectStore(PROJECT_SUMMARY_STORE, {
          keyPath: "name",
        });
        if (event.oldVersion > 0) {
          const cursorRequest = projectStore.openCursor();
          cursorRequest.addEventListener("success", () => {
            const cursor = cursorRequest.result;
            if (cursor === null) return;
            const record = cursor.value as ProjectRecord;
            summaryStore.add({
              name: record.name,
              byte_length: record.bytes.byteLength,
              saved_at: record.saved_at,
            } satisfies SavedProjectSummary);
            cursor.continue();
          });
        }
      }
    });
    request.addEventListener("success", () => { resolve(request.result); });
    request.addEventListener("error", () => { reject(request.error ?? new Error("IndexedDB could not be opened.")); },
    );
    request.addEventListener("blocked", () => { reject(new Error("IndexedDB upgrade was blocked by another Designer tab.")); },
    );
  });
}

function requestResult<Result>(request: IDBRequest<Result>): Promise<Result> {
  return new Promise((resolve, reject) => {
    request.addEventListener("success", () => { resolve(request.result); });
    request.addEventListener("error", () => { reject(request.error ?? new Error("IndexedDB request failed.")); },
    );
  });
}

function transactionComplete(transaction: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    transaction.addEventListener("complete", () => { resolve(); });
    transaction.addEventListener("abort", () => { reject(transaction.error ?? new Error("IndexedDB transaction aborted.")); },
    );
    transaction.addEventListener("error", () => { reject(transaction.error ?? new Error("IndexedDB transaction failed.")); },
    );
  });
}
