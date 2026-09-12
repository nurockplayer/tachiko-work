import {
  projectTransferFromEntries,
  projectTransferFromFiles,
  type CanonicalProjectTransferEntry,
} from "./host/project-transfer.ts";
import { DesignerRuntimeError, type DesignerClient } from "./runtime/client.ts";
import { WorkerDesignerClient } from "./runtime/worker-client.ts";
import type {
  CanonicalProjectFile,
  CanonicalTreeExport,
  OccurrenceProjection,
  OpenedProjection,
  ProjectExport,
} from "./runtime/protocol.ts";

export const EXPERIMENTAL_CLIENT_KIT_ID = "tachiko-designer-client-kit/v0-experimental";

const CANONICAL_PROJECT_FILE_COUNT = 18;
const MAX_PROJECT_TRANSFER_BYTES = 64 * 1024 * 1024;
const CANONICAL_PROJECT_PATHS = new Set([
  "manifest.json",
  "schemas.json",
  ..."0123456789abcdef".split("").map((digit) => `entities/${digit}.jsonl`),
]);

/**
 * Public client surface exposed by the experimental kit.
 *
 * The application client keeps these storage and occurrence operations
 * optional so lightweight in-app implementations can provide only the
 * capabilities they need. The actual Worker client supplies all of them,
 * and the vendored kit makes that stronger contract explicit for consumers.
 */
export interface ExperimentalDesignerClient extends DesignerClient {
  openCanonicalTree(files: readonly CanonicalProjectFile[]): Promise<OpenedProjection>;
  exportCanonicalTree(expectedRevision: string): Promise<CanonicalTreeExport>;
  exportPortableRo(expectedRevision: string): Promise<ProjectExport>;
  verifyPortableRo(bytes: ArrayBuffer): Promise<void>;
  openPortableRo(bytes: ArrayBuffer): Promise<OpenedProjection>;
  observeOccurrence(): Promise<OccurrenceProjection>;
}

/**
 * Check the public canonical-tree shape before any bytes are dispatched to a
 * Worker. This is transport admission only; Rust remains the `.roproj`
 * parser and source of meaning.
 */
export function preflightCanonicalProjectEntries(
  entries: readonly CanonicalProjectTransferEntry[],
): void {
  if (entries.length !== CANONICAL_PROJECT_FILE_COUNT) {
    throw new Error("A canonical .roproj/v1 directory must contain exactly 18 files.");
  }
  const paths = new Set<string>();
  let total = 8 + 4;
  for (const entry of entries) {
    if (!isSafeRelativePath(entry.path) || paths.has(entry.path)) {
      throw new Error("The canonical project entries do not form one safe project directory.");
    }
    paths.add(entry.path);
    const pathBytes = new TextEncoder().encode(entry.path);
    if (pathBytes.byteLength > 65_535) {
      throw new Error("A selected project path exceeds the private host transfer profile.");
    }
    const bytes = new Uint8Array(entry.bytes);
    total += 2 + 4 + pathBytes.byteLength + bytes.byteLength;
    if (total > MAX_PROJECT_TRANSFER_BYTES) {
      throw new Error("The selected project exceeds the 64 MiB host transfer boundary.");
    }
  }
  for (const path of CANONICAL_PROJECT_PATHS) {
    if (!paths.has(path)) {
      throw new Error("The canonical project entries do not contain the complete .roproj/v1 tree.");
    }
  }
}

function isSafeRelativePath(path: string): boolean {
  return path.length > 0 && !path.startsWith("/") && !path.includes("\\") &&
    path.split("/").every((component) => component.length > 0 && component !== "." && component !== "..");
}

/**
 * Preflight and open a canonical tree through the public `openProject`
 * capability. Invalid entries fail before the capability is invoked.
 */
export async function openCanonicalTreeFromEntries(
  client: Pick<ExperimentalDesignerClient, "openProject">,
  entries: readonly CanonicalProjectTransferEntry[],
): Promise<OpenedProjection> {
  preflightCanonicalProjectEntries(entries);
  return client.openProject(projectTransferFromEntries(entries));
}

class PublicExperimentalDesignerClient extends WorkerDesignerClient {
  override async openCanonicalTree(
    files: readonly CanonicalProjectFile[],
  ): Promise<OpenedProjection> {
    return openCanonicalTreeFromEntries(this, files);
  }
}

export function createExperimentalDesignerClient(): ExperimentalDesignerClient {
  return new PublicExperimentalDesignerClient(
    () =>
      new Worker(new URL("./experimental-client.worker.js", import.meta.url), {
        type: "module",
        name: "tachiko-experimental-designer-runtime",
      }),
  );
}

export {
  DesignerRuntimeError,
  projectTransferFromEntries,
  projectTransferFromFiles,
};
export type { DesignerClient };
export type { CanonicalProjectTransferEntry } from "./host/project-transfer.ts";
export type {
  BootstrapProjection,
  CanonicalProjectFile,
  CanonicalTreeExport,
  CalculationProjection,
  CollectionSummary,
  DiagnosticProjection,
  FailureProjection,
  FieldBatchProjection,
  FieldProjection,
  FieldTarget,
  OpenedProjection,
  OccurrenceProjection,
  ProjectExport,
  PublicationProjection,
  StoredValueProjection,
  TableProjection,
} from "./runtime/protocol.ts";
