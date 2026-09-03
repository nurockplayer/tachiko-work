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

export type ProjectionSnapshot = {
  table: TableProjection;
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
): ProjectionStore {
  let table = structuredClone(initialTable);
  let currentness: ProjectionCurrentness = "current";
  let failure: string | null = null;
  const pending = new Map<string, FieldTarget>();

  const allFields = (): FieldProjection[] =>
    table.rows.flatMap((row) => row.fields);

  return {
    snapshot: () => ({ table, currentness, failure }),
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
      table = {
        ...table,
        rows: table.rows.map((row) => ({
          ...row,
          fields: row.fields.map(
            (field) => replacements.get(fieldTargetKey(field.target)) ?? field,
          ),
        })),
      };
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
