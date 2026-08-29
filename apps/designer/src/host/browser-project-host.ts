const DATABASE_NAME = "tachiko-designer-projects";
const DATABASE_VERSION = 1;
const PROJECT_STORE = "projects";

type ProjectRecord = {
  name: string;
  bytes: ArrayBuffer;
  saved_at: string;
};

export type SavedProjectSummary = {
  name: string;
  byte_length: number;
  saved_at: string;
};

export interface DesignerProjectHost {
  list(): Promise<SavedProjectSummary[]>;
  read(name: string): Promise<ArrayBuffer>;
  publish(name: string, bytes: ArrayBuffer): Promise<void>;
}

export class ProjectHostError extends Error {
  readonly code: "destination_exists" | "not_found" | "invalid_name" | "host_failure";

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
    const transaction = database.transaction(PROJECT_STORE, "readonly");
    const request = transaction.objectStore(PROJECT_STORE).getAll() as IDBRequest<
      ProjectRecord[]
    >;
    const [records] = await Promise.all([
      requestResult<ProjectRecord[]>(request),
      transactionComplete(transaction),
    ]);
    return records
      .map((record) => ({
        name: record.name,
        byte_length: record.bytes.byteLength,
        saved_at: record.saved_at,
      }))
      .sort((left, right) => left.name.localeCompare(right.name));
  }

  async read(name: string): Promise<ArrayBuffer> {
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
    return record.bytes.slice(0);
  }

  async publish(name: string, bytes: ArrayBuffer): Promise<void> {
    const canonicalName = projectName(name);
    const database = await this.#database;
    const transaction = database.transaction(PROJECT_STORE, "readwrite");
    const request = transaction.objectStore(PROJECT_STORE).add({
      name: canonicalName,
      bytes: bytes.slice(0),
      saved_at: new Date().toISOString(),
    } satisfies ProjectRecord);
    const requestFailure = new Promise<never>((_resolve, reject) => {
      request.addEventListener("error", () => {
        reject(request.error ?? new Error("IndexedDB create-only publication failed."));
      });
    });
    try {
      await Promise.race([transactionComplete(transaction), requestFailure]);
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
    request.addEventListener("upgradeneeded", () => {
      if (!request.result.objectStoreNames.contains(PROJECT_STORE)) {
        request.result.createObjectStore(PROJECT_STORE, { keyPath: "name" });
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
