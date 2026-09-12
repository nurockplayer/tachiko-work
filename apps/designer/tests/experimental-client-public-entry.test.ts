import { describe, expect, it, vi } from "vitest";

import {
  openCanonicalTreeFromEntries,
  type ExperimentalDesignerClient,
} from "../src/experimental-client.ts";

const canonicalPaths = [
  "manifest.json",
  "schemas.json",
  ..."0123456789abcdef".split("").map((digit) => `entities/${digit}.jsonl`),
];

function entries(): { path: string; bytes: ArrayBuffer }[] {
  return canonicalPaths.map((path) => ({path, bytes: new ArrayBuffer(0)}));
}

function opened(revision: string) {
  return {
    bootstrap: {
      title: "fixture",
      revision,
      default_collection: "entities",
      collections: [],
      keyed_grouped_sum_definition_ids: [],
    },
    table: {revision, collection: {id: "entities", key: "entities", entity_count: 0}, columns: [], rows: []},
  };
}

describe("public canonical-entry preflight", () => {
  it("dispatches one public open for a complete canonical set", async () => {
    let resident = "resident/0";
    const openProject = vi.fn(async () => {
      resident = "resident/1";
      return opened(resident);
    });
    const client = {openProject} as unknown as Pick<ExperimentalDesignerClient, "openProject">;

    const result = await openCanonicalTreeFromEntries(client, entries());

    expect(openProject).toHaveBeenCalledTimes(1);
    expect(result.bootstrap.revision).toBe("resident/1");
    expect(resident).toBe("resident/1");
  });

  it.each([
    ["duplicate", () => { const value = entries(); value[17] = value[0]!; return value; }],
    ["unsafe path", () => { const value = entries(); value[0] = {path: "../manifest.json", bytes: new ArrayBuffer(0)}; return value; }],
    ["over file count", () => [...entries(), {path: "entities/extra.jsonl", bytes: new ArrayBuffer(0)}]],
    ["over transfer size", () => { const value = entries(); value[17] = {path: "entities/f.jsonl", bytes: new ArrayBuffer(64 * 1024 * 1024)}; return value; }],
  ])("rejects %s before dispatch and leaves the resident occurrence unchanged", async (_name, makeEntries) => {
    const openProject = vi.fn(async () => opened("unexpected"));
    const client = {openProject} as unknown as Pick<ExperimentalDesignerClient, "openProject">;
    const resident = "resident/0";

    await expect(openCanonicalTreeFromEntries(client, makeEntries())).rejects.toThrow();

    expect(openProject).not.toHaveBeenCalled();
    expect(resident).toBe("resident/0");
  });
});
