import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import * as entry from "../src/experimental-client.ts";

// The packaged kit manifest advertises the exposed client methods and entry
// functions, plus projection feature markers. Keep that inventory in parity with the actual
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

// Inspect the object the public factory actually returns, walking its whole
// prototype chain so subclass overrides and additions are covered too.
function publicSurface(): string[] {
  const original = (globalThis as { Worker?: unknown }).Worker;
  class StubWorker {
    addEventListener(): void {}
    postMessage(): void {}
    terminate(): void {}
  }
  (globalThis as { Worker?: unknown }).Worker = StubWorker;
  try {
    const client = entry.createExperimentalDesignerClient();
    const methods = new Set<string>();
    for (let prototype = Object.getPrototypeOf(client); prototype && prototype !== Object.prototype; prototype = Object.getPrototypeOf(prototype)) {
      for (const name of Object.getOwnPropertyNames(prototype)) {
        if (name !== "constructor" && typeof (prototype as Record<string, unknown>)[name] === "function") methods.add(name);
      }
    }
    client.close();
    const functions = Object.entries(entry)
      .filter(([name, value]) => typeof value === "function" && !/^[A-Z]/.test(name))
      .map(([name]) => name);
    return [...new Set([...methods, ...functions])];
  } finally {
    (globalThis as { Worker?: unknown }).Worker = original;
  }
}

const projectionMarkers = ["columnDisplayNameExplicit", "columnConstraintProjection"];

describe("kit manifest capabilities", () => {
  it("lists every public client method and entry function exactly once", () => {
    const capabilities = manifestCapabilities();
    expect(new Set(capabilities).size).toBe(capabilities.length);
    expect(capabilities).toContain("queryCapacity");
    expect(capabilities).toContain("isCapacityFailure");
    expect([...capabilities].sort()).toEqual([...publicSurface(), ...projectionMarkers].sort());
  });

  it("advertises optional-field projection features without inventing methods", () => {
    for (const marker of projectionMarkers) {
      expect(manifestCapabilities()).toContain(marker);
      expect(publicSurface()).not.toContain(marker);
    }
  });
});
