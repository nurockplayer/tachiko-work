import { describe, expect, it, vi } from "vitest";

import { projectTransferFromFiles, projectTransferFromEntries, projectTransferToEntries } from "../src/host/project-transfer.ts";

const V3_PATHS = ["manifest.json", "schemas.json", "definitions.json",
  ..."0123456789abcdef".split("").map(digit => `entities/${digit}.jsonl`)];
const PRESENTATION = "presentation.jsonl";

function file(path: string, bytes = new Uint8Array(0)) {
  return {
    name: path.split("/").at(-1) ?? "",
    webkitRelativePath: `project.roproj/${path}`,
    size: bytes.byteLength,
    arrayBuffer: vi.fn(async () => bytes.slice().buffer),
  };
}

function fileList(selected: ReturnType<typeof file>[]): FileList {
  return {
    length: selected.length,
    item: (index: number) => selected[index] ?? null,
    [Symbol.iterator]: () => selected[Symbol.iterator](),
  } as unknown as FileList;
}

describe("Designer directory transfer admission", () => {
  it("rejects a noncanonical file count before reading any contents", async () => {
    const read = vi.fn(async () => new ArrayBuffer(0));
    const entries = Array.from({ length: 21 }, (_, index) => ({
      name: `file-${String(index)}`,
      size: 0,
      webkitRelativePath: `project.roproj/file-${String(index)}`,
      arrayBuffer: read,
    })) as unknown as File[];
    const files = {
      length: entries.length,
      item: (index: number) => entries[index] ?? null,
      [Symbol.iterator]: () => entries[Symbol.iterator](),
    } as unknown as FileList;

    await expect(projectTransferFromFiles(files)).rejects.toThrow(
      "A canonical .roproj directory must contain exactly 18, 19 or 20 files.",
    );
    expect(read).not.toHaveBeenCalled();
  });

  it("round trips a twenty-file v3 directory with the presentation member last", async () => {
    const selected = V3_PATHS.map(path => file(path, new Uint8Array([1])));
    selected[0] = file("manifest.json", new TextEncoder().encode('{"format_version":3}'));
    selected[19] = file(PRESENTATION, new Uint8Array([2, 3]));
    const transferred = await projectTransferFromFiles(fileList(selected));
    const entries = projectTransferToEntries(transferred);
    expect(entries.map(entry => entry.path)).toEqual([...V3_PATHS, PRESENTATION]);
    expect(new Uint8Array(entries[19]!.bytes)).toEqual(new Uint8Array([2, 3]));
  });

  it("refuses a twentieth file with another name", async () => {
    const selected = V3_PATHS.map(path => file(path, new Uint8Array([1])));
    selected[0] = file("manifest.json", new TextEncoder().encode('{"format_version":3}'));
    selected.push(file("extra.jsonl", new Uint8Array([2])));
    await expect(projectTransferFromFiles(fileList(selected))).rejects.toThrow(
      "unexpected path 'extra.jsonl'",
    );
  });

  it("refuses a twenty-file transfer whose presentation member is not last", () => {
    const entries = [...V3_PATHS, PRESENTATION].map(path => ({
      path, bytes: new Uint8Array(path === PRESENTATION ? [2] : [1]).buffer,
    }));
    const [manifest, schemas, definitions, ...rest] = entries;
    const entities = rest.filter(entry => entry.path !== PRESENTATION);
    const outOfOrder = [manifest!, schemas!, definitions!, entities[0]!, {
      path: PRESENTATION, bytes: new Uint8Array([2]).buffer,
    }, ...entities.slice(1)];
    const transferred = projectTransferFromEntries(outOfOrder);
    expect(() => projectTransferToEntries(transferred)).toThrow(
      "A twenty-file canonical project transfer must end with 'presentation.jsonl'.",
    );
  });
});
