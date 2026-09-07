import { describe, expect, it, vi } from "vitest";

import {
  readSingleLocalRoDocument,
  type LocalDocumentHandle,
} from "../src/host/local-document-ingress.ts";

function handle(
  name: string,
  bytes: number[] = [1, 2, 3],
){
  const getFile = vi.fn<() => Promise<File>>(async () => new File([new Uint8Array(bytes)], name));
  const source: LocalDocumentHandle = { kind: "file", name, getFile };
  return { source, getFile };
}

describe("local document ingress", () => {
  it("passes the exact bytes of one .ro file without interpreting them", async () => {
    const { source, getFile } = handle("Moonfall.RO", [0, 255, 12]);

    const document = await readSingleLocalRoDocument([source]);

    expect(document.name).toBe("Moonfall.RO");
    expect([...new Uint8Array(document.bytes)]).toEqual([0, 255, 12]);
    expect(getFile).toHaveBeenCalledOnce();
  });

  it("rejects empty, multi-file, and non-.ro launches before reading a file", async () => {
    const first = handle("one.ro");
    const second = handle("two.ro");
    const wrongExtension = handle("not-a-project.txt");

    await expect(readSingleLocalRoDocument([])).rejects.toThrow("No local .ro file");
    await expect(readSingleLocalRoDocument([first.source, second.source])).rejects.toThrow("exactly one");
    await expect(readSingleLocalRoDocument([wrongExtension.source])).rejects.toThrow("Only local .ro");
    expect(first.getFile).not.toHaveBeenCalled();
    expect(second.getFile).not.toHaveBeenCalled();
    expect(wrongExtension.getFile).not.toHaveBeenCalled();
  });

  it("leaves unreadable-handle failures visible to the application", async () => {
    const source: LocalDocumentHandle = {
      kind: "file",
      name: "unreadable.ro",
      getFile: vi.fn(async () => { throw new Error("permission denied"); }),
    };

    await expect(readSingleLocalRoDocument([source])).rejects.toThrow("permission denied");
  });
});
