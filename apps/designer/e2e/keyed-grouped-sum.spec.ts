import { fileURLToPath } from "node:url";

import { expect, test, type Locator, type Page } from "@playwright/test";

const CANARY_PROJECT = fileURLToPath(
  new URL("./fixtures/keyed-grouped-sum-v1.roproj", import.meta.url),
);
const DATABASE_NAME = "tachiko-designer-projects";
const DATABASE_VERSION = 2;
const PROJECT_STORE = "projects";
const PROJECT_SUMMARY_STORE = "project_summaries";
const PRODUCT_A_ID = "30000000-0000-4000-8000-000000000001";
const ORDERS_SCHEMA = "10000000-0000-4000-8000-000000000001";
const ORDER_LOOKUP_FIELD = "10000000-0000-4000-8000-000000000101";
const ORDER_QUANTITY_FIELD = "10000000-0000-4000-8000-000000000102";
const PRODUCTS_SCHEMA = "20000000-0000-4000-8000-000000000001";
const PRODUCT_KEY_FIELD = "20000000-0000-4000-8000-000000000201";
const PRODUCT_CATEGORY_FIELD = "20000000-0000-4000-8000-000000000202";
const PRODUCT_PRICE_FIELD = "20000000-0000-4000-8000-000000000203";
const CANONICAL_V2_PATHS = [
  "manifest.json",
  "schemas.json",
  ...Array.from({ length: 16 }, (_, index) => `entities/${index.toString(16)}.jsonl`),
  "definitions.json",
].sort();

const groupedSummary = (page: Page) =>
  page.getByRole("region", { name: "Grouped summary", exact: true });

const groupedResult = (page: Page) =>
  page.getByRole("region", { name: "Grouped summary result", exact: true });

type TransferEntry = {
  path: string;
  bytes: Uint8Array;
};

type ProjectTransfer = {
  magic: Uint8Array;
  entries: TransferEntry[];
};

type KeyedGroupedSumDefinitionWire = {
  id: string;
  orders: {
    schema: string;
    lookup_key_field: string;
    quantity_field: string;
  };
  products: {
    schema: string;
    key_field: string;
    category_field: string;
    price_field: string;
  };
};

async function openCanary(page: Page): Promise<void> {
  await page.goto("/");
  page.once("dialog", async dialog => dialog.accept());
  await page.locator("[data-import-project]").setInputFiles(CANARY_PROJECT);
  const notice = page.locator(".notice").first();
  await expect(notice).toBeVisible();
  if (!(await notice.evaluate(element => element.classList.contains("success")))) {
    throw new Error(`Canary project was not admitted: ${await notice.innerText()}`);
  }
  await expect(notice).toContainText("Project opened");
  await expect(
    page.getByRole("heading", { name: "Keyed Grouped Sum Acceptance", exact: true }),
  ).toBeVisible();
  await expect(page.getByTestId("revision")).toContainText("resident/0");
}

async function configureGroupedSummary(panel: Locator): Promise<void> {
  await panel.getByLabel("Orders table", { exact: true }).selectOption({ label: "Orders" });
  await panel
    .getByLabel("Order lookup key", { exact: true })
    .selectOption({ label: "Product Code" });
  await panel
    .getByLabel("Order quantity", { exact: true })
    .selectOption({ label: "Quantity" });
  await panel.getByLabel("Products table", { exact: true }).selectOption({ label: "Products" });
  await panel
    .getByLabel("Product lookup key", { exact: true })
    .selectOption({ label: "Product Code" });
  await panel
    .getByLabel("Product category", { exact: true })
    .selectOption({ label: "Category" });
  await panel
    .getByLabel("Product price", { exact: true })
    .selectOption({ label: "Price" });
}

async function decideMigration(
  page: Page,
  panel: Locator,
  decision: "accept" | "dismiss",
): Promise<void> {
  const dialogPromise = page.waitForEvent("dialog");
  const clickPromise = panel
    .getByRole("button", { name: "Create grouped summary", exact: true })
    .click();
  const dialog = await dialogPromise;
  expect(dialog.message()).toMatch(/format 2/i);
  expect(dialog.message()).toMatch(/upgrade|migrate/i);
  if (decision === "accept") await dialog.accept();
  else await dialog.dismiss();
  await clickPromise;
}

async function applyField(page: Page, label: string, value: string): Promise<void> {
  const control = page.getByLabel(label, { exact: true });
  await control.fill(value);
  await control
    .locator("xpath=ancestor::form")
    .getByRole("button", { name: "Apply", exact: true })
    .click();
  await expect(page.locator(".notice.success")).toContainText("Publication complete");
}

async function expectGroups(
  result: Locator,
  hardware: string,
  services: string,
): Promise<void> {
  await expect(result).toContainText(
    new RegExp(`hardware\\s+${hardware}(?=\\s|$)`),
  );
  await expect(result).toContainText(
    new RegExp(`services\\s+${services}(?=\\s|$)`),
  );
}

async function saveAs(page: Page, name: string, storage?: string): Promise<void> {
  page.once("dialog", dialog => dialog.accept(name));
  await page.getByRole("button", { name: "Save As", exact: true }).click();
  const notice = page.locator(".notice.success");
  await expect(notice).toContainText("Save As complete");
  if (storage !== undefined) await expect(notice).toContainText(storage);
}

async function openSaved(page: Page, name: string): Promise<void> {
  await page.getByLabel("Saved project", { exact: true }).selectOption(name);
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(page.locator(".notice.success")).toContainText("Project opened");
}

async function reopen(page: Page, name: string): Promise<void> {
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await openSaved(page, name);
}

async function readSavedProjectBytes(page: Page, name: string): Promise<Uint8Array> {
  const values = await page.evaluate(
    async ({ databaseName, databaseVersion, projectStore, projectName }) =>
      new Promise<number[]>((resolve, reject) => {
        const request = indexedDB.open(databaseName, databaseVersion);
        request.addEventListener("error", () => reject(request.error ?? new Error("database open failed")));
        request.addEventListener("success", () => {
          const database = request.result;
          const transaction = database.transaction(projectStore, "readonly");
          const get = transaction.objectStore(projectStore).get(projectName) as IDBRequest<
            { bytes: ArrayBuffer } | undefined
          >;
          get.addEventListener("error", () => reject(get.error ?? new Error("project read failed")));
          get.addEventListener("success", () => {
            if (get.result === undefined) {
              reject(new Error(`saved project '${projectName}' not found`));
              return;
            }
            resolve([...new Uint8Array(get.result.bytes)]);
          });
        });
      }),
    {
      databaseName: DATABASE_NAME,
      databaseVersion: DATABASE_VERSION,
      projectStore: PROJECT_STORE,
      projectName: name,
    },
  );
  return Uint8Array.from(values);
}

async function replaceSavedProjectBytes(
  page: Page,
  name: string,
  bytes: Uint8Array,
): Promise<void> {
  await page.evaluate(
    async ({ databaseName, databaseVersion, projectStore, summaryStore, projectName, values }) =>
      new Promise<void>((resolve, reject) => {
        const request = indexedDB.open(databaseName, databaseVersion);
        request.addEventListener("error", () => reject(request.error ?? new Error("database open failed")));
        request.addEventListener("success", () => {
          const database = request.result;
          const transaction = database.transaction([projectStore, summaryStore], "readwrite");
          transaction.addEventListener("complete", () => resolve());
          transaction.addEventListener("abort", () => reject(transaction.error ?? new Error("project replacement aborted")));
          transaction.addEventListener("error", () => reject(transaction.error ?? new Error("project replacement failed")));
          const store = transaction.objectStore(projectStore);
          const get = store.get(projectName) as IDBRequest<
            { name: string; bytes: ArrayBuffer; saved_at: string; presentation?: string } | undefined
          >;
          get.addEventListener("error", () => transaction.abort());
          get.addEventListener("success", () => {
            if (get.result === undefined) {
              transaction.abort();
              return;
            }
            const savedAt = new Date().toISOString();
            store.put({
              ...get.result,
              bytes: Uint8Array.from(values).buffer,
              saved_at: savedAt,
            });
            transaction.objectStore(summaryStore).put({
              name: projectName,
              byte_length: values.length,
              saved_at: savedAt,
            });
          });
        });
      }),
    {
      databaseName: DATABASE_NAME,
      databaseVersion: DATABASE_VERSION,
      projectStore: PROJECT_STORE,
      summaryStore: PROJECT_SUMMARY_STORE,
      projectName: name,
      values: [...bytes],
    },
  );
}

function decodeProjectTransfer(bytes: Uint8Array): ProjectTransfer {
  if (bytes.byteLength < 12) throw new Error("project transfer is too short");
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const fileCount = view.getUint32(8, true);
  let offset = 12;
  const entries: TransferEntry[] = [];
  const seen = new Set<string>();
  const decoder = new TextDecoder();
  for (let index = 0; index < fileCount; index += 1) {
    if (offset + 6 > bytes.byteLength) throw new Error("project transfer header is truncated");
    const pathLength = view.getUint16(offset, true);
    offset += 2;
    const bodyLength = view.getUint32(offset, true);
    offset += 4;
    const end = offset + pathLength + bodyLength;
    if (end > bytes.byteLength) throw new Error("project transfer body is truncated");
    const path = decoder.decode(bytes.slice(offset, offset + pathLength));
    offset += pathLength;
    if (seen.has(path)) throw new Error(`duplicate project transfer path '${path}'`);
    seen.add(path);
    entries.push({ path, bytes: bytes.slice(offset, offset + bodyLength) });
    offset += bodyLength;
  }
  if (offset !== bytes.byteLength) throw new Error("project transfer has trailing bytes");
  return { magic: bytes.slice(0, 8), entries };
}

function encodeProjectTransfer(transfer: ProjectTransfer): Uint8Array {
  const encoder = new TextEncoder();
  const encoded = transfer.entries.map(entry => ({
    path: encoder.encode(entry.path),
    bytes: entry.bytes,
  }));
  const total = encoded.reduce(
    (size, entry) => size + 6 + entry.path.byteLength + entry.bytes.byteLength,
    12,
  );
  const output = new Uint8Array(total);
  const view = new DataView(output.buffer);
  output.set(transfer.magic, 0);
  view.setUint32(8, encoded.length, true);
  let offset = 12;
  for (const entry of encoded) {
    view.setUint16(offset, entry.path.byteLength, true);
    offset += 2;
    view.setUint32(offset, entry.bytes.byteLength, true);
    offset += 4;
    output.set(entry.path, offset);
    offset += entry.path.byteLength;
    output.set(entry.bytes, offset);
    offset += entry.bytes.byteLength;
  }
  return output;
}

function inspectCanonicalV2(bytes: Uint8Array): {
  transfer: ProjectTransfer;
  definitionId: string;
} {
  const decoder = new TextDecoder();
  const transfer = decodeProjectTransfer(bytes);
  expect(decoder.decode(transfer.magic)).not.toBe("TWDPROJ2");
  expect(transfer.entries.map(entry => entry.path).sort()).toEqual(CANONICAL_V2_PATHS);

  const manifestEntry = transfer.entries.find(entry => entry.path === "manifest.json");
  const definitionsEntry = transfer.entries.find(entry => entry.path === "definitions.json");
  expect(manifestEntry).toBeDefined();
  expect(definitionsEntry).toBeDefined();
  const manifest = JSON.parse(decoder.decode(manifestEntry!.bytes)) as {
    format: string;
    format_version: number;
  };
  expect(manifest.format).toBe("tachiko.roproj");
  expect(manifest.format_version).toBe(2);

  const definitionsText = decoder.decode(definitionsEntry!.bytes);
  const definitions = JSON.parse(definitionsText) as KeyedGroupedSumDefinitionWire[];
  expect(definitions).toHaveLength(1);
  const definition = definitions[0];
  expect(definition).toBeDefined();
  expect(definition!.id.length).toBeGreaterThan(0);
  const expected = {
    id: definition!.id,
    orders: {
      schema: ORDERS_SCHEMA,
      lookup_key_field: ORDER_LOOKUP_FIELD,
      quantity_field: ORDER_QUANTITY_FIELD,
    },
    products: {
      schema: PRODUCTS_SCHEMA,
      key_field: PRODUCT_KEY_FIELD,
      category_field: PRODUCT_CATEGORY_FIELD,
      price_field: PRODUCT_PRICE_FIELD,
    },
  } satisfies KeyedGroupedSumDefinitionWire;
  expect(definition).toEqual(expected);
  expect(definitionsText).toBe(`${JSON.stringify([expected], null, 2)}\n`);
  return { transfer, definitionId: expected.id };
}

function rewriteProductPrice(
  transfer: ProjectTransfer,
  from: number,
  to: number,
): ProjectTransfer {
  const decoder = new TextDecoder();
  const encoder = new TextEncoder();
  let rewritten = false;
  const entries = transfer.entries.map(entry => {
    if (!entry.path.startsWith("entities/")) return entry;
    const entity = JSON.parse(decoder.decode(entry.bytes)) as {
      id: string;
      fields: Record<string, { kind: string; value: unknown }>;
    };
    if (entity.id !== PRODUCT_A_ID) return entry;
    expect(entity.fields[PRODUCT_PRICE_FIELD]).toEqual({ kind: "number", value: from });
    entity.fields[PRODUCT_PRICE_FIELD] = { kind: "number", value: to };
    rewritten = true;
    return { path: entry.path, bytes: encoder.encode(`${JSON.stringify(entity)}\n`) };
  });
  expect(rewritten).toBe(true);
  return { magic: transfer.magic, entries };
}

test("keyed grouped-sum canary is a valid ordinary two-table project", async ({ page }) => {
  await openCanary(page);

  await page.getByLabel("Collection", { exact: true }).selectOption("orders");
  await expect(page.getByRole("heading", { name: "Orders", exact: true })).toBeVisible();
  await expect(page.getByLabel("Product Code for Order A", { exact: true })).toHaveValue("P-100");
  await expect(page.getByLabel("Quantity for Order A", { exact: true })).toHaveValue("-1");
  await expect(page.getByLabel("Product Code for Order B", { exact: true })).toHaveValue("P-200");
  await expect(page.getByLabel("Quantity for Order C", { exact: true })).toHaveValue("3");

  await page.getByLabel("Collection", { exact: true }).selectOption("products");
  await expect(page.getByRole("heading", { name: "Products", exact: true })).toBeVisible();
  await expect(page.getByLabel("Product Code for Product A", { exact: true })).toHaveValue("P-100");
  await expect(page.getByLabel("Category for Product A", { exact: true })).toHaveValue("hardware");
  await expect(page.getByLabel("Price for Product B", { exact: true })).toHaveValue("5");
});

test("Driver explicitly migrates to canonical v2, persists the definition, recomputes on reopen, and invalidates failures", async ({
  page,
}) => {
  await openCanary(page);

  const panel = groupedSummary(page);
  await expect(panel).toBeVisible();
  await configureGroupedSummary(panel);

  await decideMigration(page, panel, "dismiss");
  await expect(page.getByTestId("revision")).toContainText("resident/0");
  await saveAs(page, "keyed-grouped-sum-declined.roproj", "Storage: .roproj/v1");

  await configureGroupedSummary(groupedSummary(page));
  await decideMigration(page, groupedSummary(page), "accept");
  await expect(page.locator(".notice.success")).toContainText("Publication complete");

  let result = groupedResult(page);
  await expectGroups(result, "4", "10");

  await page.getByLabel("Collection", { exact: true }).selectOption("products");
  await applyField(page, "Price for Product A", "3");
  await expectGroups(result, "6", "10");

  const savedName = "keyed-grouped-sum.roproj";
  await saveAs(page, savedName, "Storage: .roproj/v2");
  const saved = inspectCanonicalV2(await readSavedProjectBytes(page, savedName));

  await page.getByRole("button", { name: "Close", exact: true }).click();
  const externallyEdited = rewriteProductPrice(saved.transfer, 3, 4);
  await replaceSavedProjectBytes(page, savedName, encodeProjectTransfer(externallyEdited));
  await openSaved(page, savedName);

  result = groupedResult(page);
  await expect(page.getByLabel("Price for Product A", { exact: true })).toHaveValue("4");
  await expectGroups(result, "8", "10");
  const reopened = inspectCanonicalV2(await readSavedProjectBytes(page, savedName));
  expect(reopened.definitionId).toBe(saved.definitionId);

  await page.getByLabel("Collection", { exact: true }).selectOption("products");
  await applyField(page, "Product Code for Product B", "P-100");
  await expect(result).toContainText("lookup.ambiguous_key");
  await expect(result).not.toContainText(/hardware\s+8(?=\s|$)/);
  await expect(result).not.toContainText(/services\s+10(?=\s|$)/);

  await applyField(page, "Product Code for Product B", "P-300");
  await expect(result).toContainText("lookup.missing_key");
  await expect(result).not.toContainText(/hardware\s+8(?=\s|$)/);
  await expect(result).not.toContainText(/services\s+10(?=\s|$)/);
});
