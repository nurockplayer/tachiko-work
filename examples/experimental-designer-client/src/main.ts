import {
  DesignerRuntimeError,
  createExperimentalDesignerClient,
  projectTransferFromFiles,
  type FieldProjection,
  type FieldTarget,
  type TableProjection,
} from "../vendor/tachiko/experimental-client.js";

const input = document.querySelector<HTMLInputElement>("#project");
const result = document.querySelector<HTMLElement>("[data-testid='result']");
if (input === null || result === null) throw new Error("Smoke UI is incomplete.");
const projectInput = input;
const resultOutput = result;

projectInput.addEventListener("change", () => {
  void run().catch((error: unknown) => {
    resultOutput.dataset.status = "failed";
    resultOutput.textContent =
      error instanceof Error ? error.stack ?? error.message : String(error);
  });
});

async function run(): Promise<void> {
  if (projectInput.files === null) throw new Error("No project directory was selected.");
  const client = createExperimentalDesignerClient();
  try {
    const project = await projectTransferFromFiles(projectInput.files);
    const opened = await client.openProject(project);
    const table = opened.table;
    const row = requireRow(table, "designer_profile_bound");
    const impact = requireField(table, row.fields, "impact");
    const title = requireField(table, row.fields, "title");
    const priority = requireField(table, row.fields, "priority");

    const publication = await client.editNumber(
      opened.bootstrap.revision,
      impact.target,
      "3",
    );
    const observed = await client.queryFields(publication.resulting_revision, [
      impact.target,
      priority.target,
    ]);
    const observedPriority = requireProjection(observed.fields, priority.target);
    const beforeStale = await client.exportProject(publication.resulting_revision);

    let staleFailure: { code: string; currentRevision: string } | undefined;
    try {
      await client.editText(opened.bootstrap.revision, title.target, "stale overwrite");
    } catch (error) {
      if (!(error instanceof DesignerRuntimeError)) throw error;
      staleFailure = {
        code: error.failure.code,
        currentRevision: error.failure.current_revision,
      };
    }
    if (staleFailure === undefined) throw new Error("A stale edit unexpectedly published.");

    const afterStale = await client.exportProject(publication.resulting_revision);
    const canonicalStateUnchanged = bytesEqual(beforeStale.bytes, afterStale.bytes);
    const roundTripInput = afterStale.bytes.slice(0);
    await client.closeProject();
    const reopened = await client.openProject(roundTripInput);
    const reopenedRow = requireRow(reopened.table, "designer_profile_bound");
    const reopenedImpact = requireField(reopened.table, reopenedRow.fields, "impact");
    const reopenedPriority = requireField(reopened.table, reopenedRow.fields, "priority");
    const roundTripExport = await client.exportProject(reopened.bootstrap.revision);

    resultOutput.dataset.status = "passed";
    resultOutput.textContent = JSON.stringify({
      title: opened.bootstrap.title,
      collection: table.collection.key,
      rowCount: table.rows.length,
      publication: {
        base: publication.base_revision,
        resulting: publication.resulting_revision,
      },
      calculation: observedPriority.calculated,
      diagnostics: observedPriority.diagnostics.map((diagnostic) => diagnostic.code),
      staleFailure,
      canonicalStateUnchanged,
      roundTrip: {
        revision: reopened.bootstrap.revision,
        impact: storedNumber(reopenedImpact),
        priority: calculatedNumber(reopenedPriority),
        bytesEqual: bytesEqual(beforeStale.bytes, roundTripExport.bytes),
      },
    });
  } finally {
    await client.closeProject().catch(() => undefined);
    await client.close();
  }
}

function requireRow(table: TableProjection, key: string): TableProjection["rows"][number] {
  const row = table.rows.find((candidate) => candidate.key === key);
  if (row === undefined) throw new Error(`Missing row '${key}'.`);
  return row;
}

function requireField(
  table: TableProjection,
  fields: FieldProjection[],
  key: string,
): FieldProjection {
  const column = table.columns.find((candidate) => candidate.key === key);
  if (column === undefined) throw new Error(`Missing column '${key}'.`);
  const field = fields.find((candidate) => candidate.target.field === column.id);
  if (field === undefined) throw new Error(`Missing field '${key}'.`);
  return field;
}

function requireProjection(
  fields: FieldProjection[],
  target: FieldTarget,
): FieldProjection {
  const field = fields.find(
    (candidate) =>
      candidate.target.entity === target.entity && candidate.target.field === target.field,
  );
  if (field === undefined) throw new Error("A requested field projection was omitted.");
  return field;
}

function storedNumber(field: FieldProjection): number {
  if (field.stored?.kind !== "number") throw new Error("Expected a stored Number.");
  return field.stored.value;
}

function calculatedNumber(field: FieldProjection): number {
  if (field.calculated?.status !== "value") throw new Error("Expected a calculated Number.");
  return field.calculated.value;
}

function bytesEqual(left: ArrayBuffer, right: ArrayBuffer): boolean {
  if (left.byteLength !== right.byteLength) return false;
  const leftBytes = new Uint8Array(left);
  const rightBytes = new Uint8Array(right);
  return leftBytes.every((byte, index) => byte === rightBytes[index]);
}
