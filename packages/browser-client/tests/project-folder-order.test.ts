import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { execFileSync } from "node:child_process";

import { describe, expect, it } from "vitest";

import { projectTransferFromFiles, projectTransferToEntries } from "../src/host/project-transfer.ts";

function paths(version: number): string[] {
  const rust = readFileSync(resolve(`../../crates/storage/src/roproj/v${version}.rs`), "utf8");
  const declaration = rust.match(/pub const ROPROJ_V\d_PATHS: \[&str; \d+\] = \[([\s\S]*?)\];/);
  if (!declaration) throw new Error("Missing Rust path authority");
  return Array.from(declaration[1]!.matchAll(/"([^"]+)"/g), match => match[1]!);
}

function file(path: string, body = "", root = "project.roproj"): File {
  const result = new File([body], path.split("/").at(-1)!);
  Object.defineProperty(result, "webkitRelativePath", { value: `${root}/${path}` });
  return result;
}

function selection(files: readonly File[]): FileList {
  return { length: files.length, item: (index: number) => files[index] ?? null,
    [Symbol.iterator]: () => files[Symbol.iterator]() } as unknown as FileList;
}

function shuffle<T>(input: T[], seed: number): T[] {
  const output = [...input];
  for (let index = output.length - 1; index > 0; index -= 1) {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    const next = seed % (index + 1);
    [output[index], output[next]] = [output[next]!, output[index]!];
  }
  return output;
}

describe("Work directory roots", () => {
  const expected = paths(3);
  const fixture = expected.map(path => ({ path, body: readFileSync(resolve(
    "../../docs/research/fixtures/issue-519-portable-package-v1-v3/input.roproj", path,
  ), "utf8") }));
  const files = (root: string) => fixture.map(({ path, body }) => file(path, body, root));

  it("accepts a backslash in the discarded root of a canonical v3 directory", async () => {
    const transfer = await projectTransferFromFiles(selection(files("Forecast\\Final.roproj")));
    expect(projectTransferToEntries(transfer).map(entry => entry.path)).toEqual(expected);
    const canonical = await projectTransferFromFiles(selection(files("project.roproj")));
    expect(new Uint8Array(transfer)).toEqual(new Uint8Array(canonical));
  });

  it("refuses a backslash in a relative path below the discarded root", async () => {
    const selected = files("Forecast\\Final.roproj");
    selected[3] = file("entities\\0.jsonl", "", "Forecast\\Final.roproj");
    await expect(projectTransferFromFiles(selection(selected))).rejects.toThrow("one safe project directory");
  });

  it("refuses an empty root component", async () => {
    await expect(projectTransferFromFiles(selection(files("")))).rejects.toThrow(
      "Select the complete .roproj directory",
    );
  });
});

describe("Work directory ordering", () => {
  it("uses a generated copy of the Rust storage path authority", () => {
    expect(() => execFileSync(process.execPath, ["scripts/generate-project-paths.mjs", "--check"])).not.toThrow();
  });
  for (const version of [1, 2, 3]) {
    const expected = paths(version);
    const files = expected.map(path => file(path, path === "manifest.json"
      ? JSON.stringify({ format: "tachiko.roproj", format_version: version }) : `opaque ${path}`));
    it(`packages every permutation of v${version} identically in Rust order`, async () => {
      const canonical = await projectTransferFromFiles(selection(files));
      for (const selected of [[...files].reverse(), [...files.slice(7), ...files.slice(0, 7)],
        ...[1, 42, 536].map(seed => shuffle(files, seed))]) {
        const transfer = await projectTransferFromFiles(selection(selected));
        expect(projectTransferToEntries(transfer).map(entry => entry.path)).toEqual(expected);
        expect(new Uint8Array(transfer)).toEqual(new Uint8Array(canonical));
      }
    });
    it(`refuses damaged v${version} path sets instead of repairing them`, async () => {
      for (const [selected, message] of [
        [files.slice(0, -1), /exactly 18 or 19 files|unexpected path|missing/],
        [[...files, file("extra.txt")], /exactly 18 or 19 files|unexpected path/],
        [[...files.slice(0, -1), files[0]!], /duplicate path/],
        [[...files.slice(0, -1), file("unknown.json")], /unexpected path/],
        [[...files.slice(0, -1), file("entities/nested/f.jsonl")], /unexpected path/],
        [[...files.slice(0, -1), file("../escape")], /one safe project directory/],
        [[...files.slice(0, -1), file("entities\\f.jsonl")], /one safe project directory/],
      ] as const) {
        await expect(projectTransferFromFiles(selection(selected))).rejects.toThrow(message);
      }
      const mixed = [...files];
      mixed[0] = file("manifest.json", JSON.stringify({ format_version: version === 1 ? 3 : 1 }));
      await expect(projectTransferFromFiles(selection(mixed))).rejects.toThrow();
    });
    it(`refuses malformed or unsupported v${version} manifest dispatch without rewriting bytes`, async () => {
      for (const body of ["{", "null", "[]", '{"format_version":4}', '{"format_version":"3"}']) {
        await expect(projectTransferFromFiles(selection([file("manifest.json", body), ...files.slice(1)]))).rejects.toThrow(/manifest/);
      }
    });
  }
});
