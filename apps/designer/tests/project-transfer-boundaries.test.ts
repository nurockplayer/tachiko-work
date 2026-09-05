import { describe, expect, it, vi } from "vitest";

import { projectTransferFromFiles } from "../src/host/project-transfer.ts";

function entry(path: string, bytes = new Uint8Array(0)) {
  return {
    name: path.split("/").at(-1) ?? "",
    webkitRelativePath: path,
    size: bytes.byteLength,
    arrayBuffer: vi.fn(async () => bytes.slice().buffer),
  };
}

function entries() {
  return Array.from({ length: 18 }, (_, index) => entry(`project.roproj/f${String(index).padStart(2, "0")}`));
}

function fileList(selected: ReturnType<typeof entry>[]): FileList {
  return {
    length: selected.length,
    item: (index: number) => selected[index] ?? null,
    [Symbol.iterator]: () => selected[Symbol.iterator](),
  } as unknown as FileList;
}

describe("Designer directory transfer boundaries", () => {
  it("rejects an empty selection", async () => {
    await expect(projectTransferFromFiles(fileList([]))).rejects.toThrow("No project directory was selected.");
  });

  it("rejects a missing entry before reading any file", async () => {
    const selected = entries().slice(0, 17);
    await expect(projectTransferFromFiles(fileList(selected))).rejects.toThrow("exactly 18 files");
    for (const file of selected) expect(file.arrayBuffer).not.toHaveBeenCalled();
  });

  it.each(["f00", "/f00"])("rejects a first entry without a selected directory root: %s", async path => {
    const selected = entries();
    selected[0] = entry(path);
    await expect(projectTransferFromFiles(fileList(selected))).rejects.toThrow("Select the complete .roproj directory");
    for (const file of selected) expect(file.arrayBuffer).not.toHaveBeenCalled();
  });

  it.each([
    "other.roproj/f01",
    "project.roproj//f01",
    "project.roproj/./f01",
    "project.roproj/../f01",
    "project.roproj/nested/",
  ])("rejects an unsafe entry before reading it or later entries: %s", async path => {
    const selected = entries();
    selected[1] = entry(path);
    await expect(projectTransferFromFiles(fileList(selected))).rejects.toThrow("one safe project directory");
    for (const file of selected.slice(1)) expect(file.arrayBuffer).not.toHaveBeenCalled();
  });

  it("accepts a 65,535-byte UTF-8 path and encodes its byte length without truncation", async () => {
    const selected = entries();
    const path = "測".repeat(21_845);
    selected[0] = entry(`project.roproj/${path}`);
    const output = await projectTransferFromFiles(fileList(selected));
    expect(new DataView(output).getUint16(12, true)).toBe(65_535);
    expect(new Uint8Array(output, 18, 65_535)).toEqual(new TextEncoder().encode(path));
  });

  it("rejects a 65,536-byte UTF-8 path even though its character count fits", async () => {
    const selected = entries();
    selected[0] = entry(`project.roproj/${"測".repeat(21_845)}a`);
    await expect(projectTransferFromFiles(fileList(selected))).rejects.toThrow("path exceeds the private host transfer profile");
    for (const file of selected) expect(file.arrayBuffer).not.toHaveBeenCalled();
  });

  it("accepts exactly 64 MiB including the complete framing overhead", async () => {
    // Magic/count = 12 bytes; 18 entries each have a 6-byte header + 3-byte path.
    const overhead = 174;
    const payload = new Uint8Array(64 * 1024 * 1024 - overhead);
    payload[0] = 17;
    payload[payload.length - 1] = 29;
    const selected = entries();
    selected[17] = entry("project.roproj/f17", payload);
    const output = new Uint8Array(await projectTransferFromFiles(fileList(selected)));
    expect(output.byteLength).toBe(64 * 1024 * 1024);
    expect(output[overhead]).toBe(17);
    expect(output[output.length - 1]).toBe(29);
  });

  it("rejects one byte over the total budget before reading the oversized entry", async () => {
    const selected = entries();
    const oversized = entry("project.roproj/f17");
    // Metadata-only rejection fixture: it must never allocate/read this payload.
    oversized.size = 64 * 1024 * 1024 - 174 + 1;
    oversized.arrayBuffer.mockRejectedValue(new Error("Oversized payload must not be read"));
    selected[17] = oversized;
    await expect(projectTransferFromFiles(fileList(selected))).rejects.toThrow("64 MiB host transfer boundary");
    expect(oversized.arrayBuffer).not.toHaveBeenCalled();
  });

  it("propagates a file read failure without reading the remaining entries", async () => {
    const selected = entries();
    const unreadable = entry("project.roproj/f01");
    const failure = new Error("File read failed");
    unreadable.arrayBuffer.mockRejectedValue(failure);
    selected[1] = unreadable;
    await expect(projectTransferFromFiles(fileList(selected))).rejects.toBe(failure);
    for (const file of selected.slice(2)) expect(file.arrayBuffer).not.toHaveBeenCalled();
  });

  it("packs exact little-endian framing, UTF-8 paths and binary contents deterministically", async () => {
    const selected = entries();
    selected[0] = entry("project.roproj/f00", new Uint8Array([0, 255]));
    selected[17] = entry("project.roproj/あ", new Uint8Array([1, 2, 3]));
    // Fixed private transport vector, not a .roproj parser or semantic oracle.
    const hex = [
      "54574450524f4a3112000000",
      "03000200000066303000ff",
      "030000000000663031", "030000000000663032", "030000000000663033",
      "030000000000663034", "030000000000663035", "030000000000663036",
      "030000000000663037", "030000000000663038", "030000000000663039",
      "030000000000663130", "030000000000663131", "030000000000663132",
      "030000000000663133", "030000000000663134", "030000000000663135",
      "030000000000663136", "030003000000e38182010203",
    ].join("");
    const expected = Uint8Array.from(hex.match(/../g) ?? [], pair => Number.parseInt(pair, 16));
    expect(new Uint8Array(await projectTransferFromFiles(fileList(selected)))).toEqual(expected);
    expect(new Uint8Array(await projectTransferFromFiles(fileList(selected)))).toEqual(expected);
  });
});
