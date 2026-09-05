import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import ts from "typescript";
import type * as HostModule from "../src/host/browser-project-host.ts";

// Exercise the host against real IndexedDB, including transaction scheduling.
// Transpile the owning source without adding a production-only test entrypoint.
const source = await readFile(new URL("../src/host/browser-project-host.ts", import.meta.url), "utf8");
const moduleSource = ts.transpileModule(source, {
  compilerOptions: { target: ts.ScriptTarget.ES2024, module: ts.ModuleKind.ESNext },
}).outputText;

test("project Save commits bytes and presentation, rejects stale writers, and preserves Save As", async ({ page }) => {
  await page.goto("/");
  const result = await page.evaluate(async (sourceText) => {
    const { BrowserProjectHost } = await import(`data:text/javascript,${encodeURIComponent(sourceText)}`) as typeof HostModule;
    const host = new BrowserProjectHost();
    const initial = new Uint8Array([1, 2]).buffer;
    await host.publish("driver.roproj", initial, "initial view");
    const attempts = await Promise.allSettled([
      host.update("driver.roproj", new Uint8Array([3]).buffer, initial, "first view", "initial view"),
      host.update("driver.roproj", new Uint8Array([4]).buffer, initial, "second view", "initial view"),
    ]);
    const outcome = attempts.map((attempt) => attempt.status === "fulfilled" ? "saved" : (attempt.reason as { code: string }).code);
    const snapshot = await host.readSnapshot("driver.roproj");
    const saved = snapshot.bytes;
    const presentation = snapshot.presentation;
    const summary = (await host.list()).find((item) => item.name === "driver.roproj");
    let duplicate = "";
    try { await host.publish("driver.roproj", initial); } catch (error) { duplicate = (error as { code: string }).code; }
    let staleView = "";
    try { await host.update("driver.roproj", saved, saved, "stale overwrite", "initial view"); } catch (error) { staleView = (error as { code: string }).code; }
    let missing = "";
    try { await host.update("missing.roproj", initial, initial); } catch (error) { missing = (error as { code: string }).code; }
    return { outcome, saved: [...new Uint8Array(saved)], presentation, summary, duplicate, staleView, missing, finalPresentation: await host.readPresentation("driver.roproj") };
  }, moduleSource);
  expect(result.outcome).toEqual(["saved", "stale_project"]);
  expect(result.saved).toEqual([3]);
  expect(result.presentation).toBe("first view");
  expect(result.finalPresentation).toBe("first view");
  expect(result.summary?.byte_length).toBe(1);
  expect(result.duplicate).toBe("destination_exists");
  expect(result.staleView).toBe("stale_project");
  expect(result.missing).toBe("not_found");
});

test("project Save abort rolls back both the payload and presentation", async ({ page }) => {
  await page.goto("/");
  const result = await page.evaluate(async (sourceText) => {
    const { BrowserProjectHost } = await import(`data:text/javascript,${encodeURIComponent(sourceText)}`) as typeof HostModule;
    const host = new BrowserProjectHost();
    const initial = new Uint8Array([1, 2]).buffer;
    await host.publish("driver.roproj", initial, "initial view");
    // Keep the native method to restore it after injecting a transaction failure.
    // eslint-disable-next-line @typescript-eslint/unbound-method
    const originalPut = IDBObjectStore.prototype.put;
    IDBObjectStore.prototype.put = function (value: unknown, key?: IDBValidKey) {
      if (this.name === "project_summaries") throw new DOMException("Simulated storage failure", "QuotaExceededError");
      return originalPut.call(this, value, key);
    };
    let failure = "";
    try {
      await host.update("driver.roproj", new Uint8Array([3]).buffer, initial, "changed view", "initial view");
    } catch (error) {
      failure = (error as { code: string }).code;
    } finally {
      IDBObjectStore.prototype.put = originalPut;
    }
    return { failure, saved: [...new Uint8Array(await host.read("driver.roproj"))], presentation: await host.readPresentation("driver.roproj"), summary: (await host.list())[0] };
  }, moduleSource);
  expect(result.failure).toBe("host_failure");
  expect(result.saved).toEqual([1, 2]);
  expect(result.presentation).toBe("initial view");
  expect(result.summary?.byte_length).toBe(2);
});
