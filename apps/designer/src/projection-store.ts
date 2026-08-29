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
  let pending = new Set<string>();

  const allFields = (): FieldProjection[] =>
    table.rows.flatMap((row) => row.fields);

  return {
    snapshot: () => ({ table, control, currentness, failure }),
    field: (address) => allFields().find((field) => field.address === address),
    beginPublication: (publication) => {
      if (publication.base_revision !== table.revision) {
        throw new Error("Publication does not match the visible table revision.");
      }
      const targets = new Map<string, FieldTarget>();
      for (const target of [
        ...publication.fields,
        ...publication.affected_calculations,
      ]) {
        targets.set(fieldTargetKey(target), target);
      }
      pending = new Set(targets.keys());
      table = { ...table, revision: publication.resulting_revision };
      if (!pending.has(fieldTargetKey(control.target))) {
        control = { ...control, revision: publication.resulting_revision };
      }
      currentness = "refreshing";
      failure = null;
      return [...targets.values()];
    },
    finishRefresh: (refresh) => {
      if (refresh.revision !== table.revision) {
        throw new Error("Refresh does not match the published revision.");
      }
      const replacements = new Map(
        refresh.fields.map((field) => [fieldTargetKey(field.target), field]),
      );
      for (const target of pending) {
        if (!replacements.has(target)) {
          throw new Error(`Refresh omitted invalidated field '${target}'.`);
        }
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
      const controlReplacement = replacements.get(fieldTargetKey(control.target));
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
