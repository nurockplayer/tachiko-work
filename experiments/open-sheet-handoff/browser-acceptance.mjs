// Actual UI + a separate real public-client observer; no mocked runtime replies.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { resolve } from "node:path";
import test from "node:test";

const require = createRequire(new URL("../../apps/designer/package.json", import.meta.url));
const { chromium, expect } = require("@playwright/test");
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const targetValues = { "e-jan": [800, 400], "e-feb": [800, 400], "e-mar": [1000, 500] };

test("human handoff: consent, real editing, truthful impact, rejection and saved-state reopen", {
  timeout: 120_000,
}, async () => {
  assert.ok(process.env.HANDOFF_DEMO_URL, "ENVIRONMENT UNVERIFIED: HANDOFF_DEMO_URL required");
  const url = new URL(process.env.HANDOFF_DEMO_URL);
  assert.ok(["localhost", "127.0.0.1", "[::1]"].includes(url.hostname));
  assert.match(process.env.HANDOFF_HEAD ?? "", /^[0-9a-f]{40}$/u);
  const head = execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: new URL("../../", import.meta.url), encoding: "utf8",
  }).trim();
  assert.equal(process.env.HANDOFF_HEAD, head, "Evidence must match this checkout HEAD");
  assert.ok(process.env.HANDOFF_ARTIFACT_DIR, "Use a new artifact directory");
  const output = resolve(process.env.HANDOFF_ARTIFACT_DIR);
  mkdirSync(output); // Never overwrite previous evidence.
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ acceptDownloads: true });
    const errors = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.goto(url.href);
    const button = (name) => page.getByRole("button", { name, exact: true });
    const save = async (name) => {
      const pending = page.waitForEvent("download");
      await button("Save work").click();
      const download = await pending;
      const path = resolve(output, `${name}.twdproj`);
      await download.saveAs(path);
      assert.equal(await download.failure(), null);
      return { path, bytes: readFileSync(path) };
    };
    await expect(button("Save work")).toBeDisabled();
    await expect(page.getByTestId("validation")).toHaveText("Validation pending");
    await button("Prepare handoff").click();
    const preview = page.getByRole("region", { name: "Handoff proposal" });
    await expect(preview).toContainText("New identities");
    await expect(preview).toContainText("Explicit schema");
    await expect(preview).toContainText("Exact formula profile");
    await expect(preview).toContainText("Presentation only");
    await expect(button("Save work")).toBeDisabled();
    await button("Cancel handoff").click();
    await expect(button("Save work")).toBeDisabled();
    await expect(page.getByLabel("Tax rate", { exact: true })).toBeDisabled();
    await button("Prepare handoff").click();
    await page.evaluate(() => {
      const button = document.querySelector("#accept");
      button?.dispatchEvent(new MouseEvent("click"));
      button?.dispatchEvent(new MouseEvent("click"));
      button?.dispatchEvent(new MouseEvent("click"));
    });
    await expect(page.getByTestId("operation-error")).toContainText("open operation is already in progress");
    await expect(button("Accept handoff")).toBeDisabled();
    await expect(button("Open saved work")).toBeDisabled();
    const rate = page.getByLabel("Tax rate", { exact: true });
    await expect(rate).toHaveValue("0.25");
    const revision = page.getByTestId("current-revision");
    const beforeRevision = (await revision.innerText()).trim();
    assert.ok(beforeRevision);
    const before = await save("before");
    await page.screenshot({ path: resolve(output, "01-accepted.png"), fullPage: true });
    await page.getByLabel("Earlier draft tax rate", { exact: true }).fill("0.75");
    await button("Freeze earlier draft").click();
    await rate.fill("0.5000");
    await button("Apply tax rate").click();
    await expect(revision).not.toHaveText(beforeRevision);
    const currentRevision = (await revision.innerText()).trim();
    await expect(page.getByTestId("changed-field")).toHaveCount(1);
    await expect(page.getByTestId("changed-field")).toHaveText("Tax rate: 0.25 → 0.5");
    await expect(rate).toHaveValue("0.5");
    for (const [id, [gross, net]] of Object.entries(targetValues)) {
      await expect(page.getByTestId(`gross-${id}`)).toHaveText(String(gross));
      await expect(page.getByTestId(`net-${id}`)).toHaveText(String(net));
    }
    const impact = page.getByRole("region", { name: "Last accepted change" });
    await expect(impact.getByRole("listitem")).toHaveCount(3);
    for (const [id, label, prior, next] of [
      ["e-jan", "January", 600, 400], ["e-feb", "February", 600, 400],
      ["e-mar", "March", 750, 500],
    ]) {
      await expect(page.getByTestId(`impact-${id}`)).toHaveText(
        new RegExp(`^${label} net: ${prior} → ${next}$`, "u"),
      );
    }
    await expect(page.getByTestId("validation")).toHaveText("No diagnostics (authoritative projection)");
    const acceptedCard = await impact.innerText();
    const after = await save("after");
    assert.notDeepEqual(after.bytes, before.bytes);
    await page.screenshot({ path: resolve(output, "02-changed.png"), fullPage: true });
    await button("Apply earlier draft").click();
    await expect(page.getByTestId("operation-error")).toContainText("stale_revision");
    assert.equal((await revision.innerText()).trim(), currentRevision);
    assert.deepEqual((await save("after-stale-rejection")).bytes, after.bytes);
    await rate.fill("not-a-number");
    await button("Apply tax rate").click();
    await expect(page.getByTestId("operation-error")).toContainText("invalid_number");
    assert.equal((await revision.innerText()).trim(), currentRevision);
    assert.equal(await impact.innerText(), acceptedCard);
    assert.deepEqual((await save("after-invalid-rejection")).bytes, after.bytes);
    await button("Close work").click();
    await expect(button("Open saved work")).toBeEnabled();
    await page.getByLabel("Saved work", { exact: true }).setInputFiles(after.path);
    await page.evaluate(() => {
      const button = document.querySelector("#open-saved");
      button?.dispatchEvent(new MouseEvent("click"));
      button?.dispatchEvent(new MouseEvent("click"));
      button?.dispatchEvent(new MouseEvent("click"));
    });
    await expect(page.getByTestId("operation-error")).toContainText("open operation is already in progress");
    await expect(rate).toHaveValue("0.5");
    assert.deepEqual((await save("reopened")).bytes, after.bytes);

    // Independently admit the downloaded bytes through an exported kit client.
    // UI-provided "success" or a hand-written JSON evidence object is insufficient.
    const observed = await page.evaluate(async (bytes) => {
      const { createExperimentalDesignerClient } = await import("/vendor/tachiko/experimental-client.js");
      const client = createExperimentalDesignerClient();
      try {
        const opened = await client.openProject(new Uint8Array(bytes).buffer);
        const tables = [];
        for (const collection of opened.bootstrap.collections) {
          tables.push(await client.queryTable(collection.key));
        }
        const exported = await client.exportProject(opened.bootstrap.revision);
        return { tables, bytes: Array.from(new Uint8Array(exported.bytes)) };
      } finally {
        await client.close();
      }
    }, Array.from(after.bytes));
    assert.deepEqual(Buffer.from(observed.bytes), after.bytes);
    const fields = observed.tables.flatMap((table) => table.rows.flatMap((row) => row.fields));
    assert.equal(fields.length, 16);
    const tax = fields.find((field) => field.target.entity === "e-tax" && field.target.field === "f-rate");
    assert.deepEqual(tax.stored, { kind: "number", value: 0.5 });
    for (const [id, [gross, net]] of Object.entries(targetValues)) {
      for (const [field, value] of [["f-gross", gross], ["f-net", net]]) {
        const observed = fields.find((item) => item.target.entity === id && item.target.field === field);
        assert.deepEqual(observed.calculated, { status: "value", value });
        assert.ok(observed.formula);
        assert.deepEqual(observed.diagnostics, []);
      }
    }
    assert.deepEqual(errors, []);
    writeFileSync(resolve(output, "browser-evidence.json"), JSON.stringify({
      head: process.env.HANDOFF_HEAD,
      before_sha256: hash(before.bytes), after_sha256: hash(after.bytes),
      accepted_revision: currentRevision,
      stale_rejection: "canonical-bytes-unchanged",
      invalid_rejection: "canonical-bytes-unchanged",
      reopened: "independently-admitted-and-byte-equal",
      history_claim: "last accepted change only; not durable audit history",
    }, null, 2) + "\n");
  } finally {
    await browser.close();
  }
});
