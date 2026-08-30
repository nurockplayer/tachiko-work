import { describe, expect, it, vi } from "vitest";

import { projectTransferFromFiles } from "../src/host/project-transfer.ts";

describe("Designer directory transfer admission", () => {
  it("rejects a noncanonical file count before reading any contents", async () => {
    const read = vi.fn(async () => new ArrayBuffer(0));
    const entries = Array.from({ length: 19 }, (_, index) => ({
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
      "A canonical .roproj/v1 directory must contain exactly 18 files.",
    );
    expect(read).not.toHaveBeenCalled();
  });
});
