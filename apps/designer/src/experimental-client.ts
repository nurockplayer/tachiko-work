import { projectTransferFromFiles } from "./host/project-transfer.ts";
import { DesignerRuntimeError, type DesignerClient } from "./runtime/client.ts";
import { WorkerDesignerClient } from "./runtime/worker-client.ts";

export const EXPERIMENTAL_CLIENT_KIT_ID = "tachiko-designer-client-kit/v0-experimental";

export function createExperimentalDesignerClient(): DesignerClient {
  return new WorkerDesignerClient(
    () =>
      new Worker(new URL("./experimental-client.worker.js", import.meta.url), {
        type: "module",
        name: "tachiko-experimental-designer-runtime",
      }),
  );
}

export { DesignerRuntimeError, projectTransferFromFiles };
export type { DesignerClient };
export type {
  BootstrapProjection,
  CalculationProjection,
  CollectionSummary,
  DiagnosticProjection,
  FailureProjection,
  FieldBatchProjection,
  FieldProjection,
  FieldTarget,
  OpenedProjection,
  ProjectExport,
  PublicationProjection,
  StoredValueProjection,
  TableProjection,
} from "./runtime/protocol.ts";
