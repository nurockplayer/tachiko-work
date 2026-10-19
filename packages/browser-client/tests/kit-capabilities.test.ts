import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import * as entry from "../src/experimental-client.ts";
import { WorkerDesignerClient } from "../src/runtime/worker-client.ts";

// The packaged kit manifest advertises the exposed client methods and entry
// functions. Keep that hand-maintained inventory in parity with the actual
// public surface: client instance methods plus exported entry functions
// (exported classes such as DesignerRuntimeError are types, not capabilities).
function manifestCapabilities(): string[] {
  const script = readFileSync(
    // Vitest runs from packages/browser-client (its config root).
    path.resolve(process.cwd(), "../../scripts/package-experimental-designer-client.sh"),
    "utf8",
  );
  const block = /capabilities: \[([\s\S]*?)\]/.exec(script);
  if (!block?.[1]) throw new Error("capabilities inventory not found in the kit packager");
  return [...block[1].matchAll(/"([A-Za-z0-9]+)"/g)].map((match) => match[1]!);
}

function publicSurface(): string[] {
  const prototype = WorkerDesignerClient.prototype as unknown as Record<string, unknown>;
  const methods = Object.getOwnPropertyNames(prototype).filter(
    (name) => name !== "constructor" && typeof prototype[name] === "function",
  );
  const functions = Object.entries(entry)
    .filter(([name, value]) => typeof value === "function" && !/^[A-Z]/.test(name))
    .map(([name]) => name);
  return [...new Set([...methods, ...functions])];
}

describe("kit manifest capabilities", () => {
  it("lists every public client method and entry function exactly once", () => {
    const capabilities = manifestCapabilities();
    expect(new Set(capabilities).size).toBe(capabilities.length);
    expect([...capabilities].sort()).toEqual([...publicSurface()].sort());
  });
});
