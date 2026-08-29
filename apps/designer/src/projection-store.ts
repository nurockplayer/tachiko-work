import type {
  FieldBatchProjection,
  FieldProjection,
  FieldTarget,
  PublicationProjection,
  TableProjection,
} from "./runtime/protocol.ts";
import { fieldTargetKey } from "./runtime/protocol.ts";

export type ProjectionCurrentness =
  | "current"
  | "refreshing"
  | "refresh_failed";

export type ControlProjection = {
  target: FieldTarget;
  value: number;
  revision: string;
};

export type ProjectionSnapshot = {
  table: TableProjection;
  control: ControlProjection;
  currentness: ProjectionCurrentness;
  failure: string | null;
};

export type ProjectionStore = {
  snapshot(): ProjectionSnapshot;
  field(address: string): FieldProjection | undefined;
  beginPublication(publication: PublicationProjection): FieldTarget[];
  finishRefresh(refresh: FieldBatchProjection): void;
  failRefresh(message: string): void;
};

export function createProjectionStore(
  initialTable: TableProjection,
  initialControl: ControlProjection,
): ProjectionStore {
  let table = structuredClone(initialTable);
  let control = structuredClone(initialControl);
  let currentness: ProjectionCurrentness = "current";
  let failure: string | null = null;
  const pending = new Map<string, FieldTarget>();

  const allFields = (): FieldProjection[] =>
    table.rows.flatMap((row) => row.fields);

  return {
    snapshot: () => ({ table, control, currentness, failure }),
    field: (address) => allFields().find((field) => field.address === address),
    beginPublication: (publication) => {
      if (publication.base_revision !== table.revision) {
        throw new Error("Publication does not match the visible table revision.");
      }
      for (const target of [
        ...publication.fields,
        ...publication.affected_calculations,
      ]) {
        pending.set(fieldTargetKey(target), target);
      }
      table = { ...table, revision: publication.resulting_revision };
      if (!pending.has(fieldTargetKey(control.target))) {
        control = { ...control, revision: publication.resulting_revision };
      }
      currentness = "refreshing";
      failure = null;
      return [...pending.values()];
    },
    finishRefresh: (refresh) => {
      if (refresh.revision !== table.revision) {
        throw new Error("Refresh does not match the published revision.");
      }
      const replacements = new Map(
        refresh.fields.map((field) => [fieldTargetKey(field.target), field]),
      );
      for (const target of pending.keys()) {
        if (!replacements.has(target)) {
          throw new Error(`Refresh omitted invalidated field '${target}'.`);
        }
      }
      const controlReplacement = replacements.get(fieldTargetKey(control.target));
      if (
        controlReplacement !== undefined &&
        controlReplacement.calculated?.status !== "value"
      ) {
        throw new Error("The invalidated control projection is unavailable after refresh.");
      }
      table = {
        ...table,
        rows: table.rows.map((row) => ({
          ...row,
          fields: row.fields.map(
            (field) => replacements.get(fieldTargetKey(field.target)) ?? field,
          ),
        })),
      };
      if (controlReplacement?.calculated?.status === "value") {
        control = {
          ...control,
          value: controlReplacement.calculated.value,
          revision: refresh.revision,
        };
      }
      pending.clear();
      currentness = "current";
      failure = null;
    },
    failRefresh: (message) => {
      currentness = "refresh_failed";
      failure = message;
    },
  };
}
