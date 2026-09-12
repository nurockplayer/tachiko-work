import { readFile } from "node:fs/promises";
import { expect, test, type Page } from "@playwright/test";
import type { DesignerWireReply } from "../src/runtime/protocol.ts";

const shortcut = process.platform === "darwin" ? "Meta" : "Control";
const inventoryRows = "0012\t3\ttrue\t2024-02-29\nノート\t0\tfalse\t2026-09-13\n紙\t-2\ttrue\t2026-01-01";
const inventoryRequest = {
  type: "new_table",
  occurrence_id: "00000000-0000-4000-8000-000000000315",
  name: "Inventory",
  columns: [
    { name: "item", field_type: "text" },
    { name: "quantity", field_type: "number" },
    { name: "active", field_type: "boolean" },
    { name: "received", field_type: "date" },
  ],
};

type DesignerAbi = WebAssembly.Exports & {
  memory: WebAssembly.Memory;
  tachiko_designer_request_reserve(length: number): number;
  tachiko_designer_request_run(): void;
  tachiko_designer_response_ptr(): number;
  tachiko_designer_response_len(): number;
};
const encoder = new TextEncoder();
const decoder = new TextDecoder();

test.use({ permissions: ["clipboard-read", "clipboard-write"] });

async function paste(page: Page, text: string): Promise<void> {
  await page.evaluate(value => navigator.clipboard.writeText(value), text);
  await page.keyboard.press(`${shortcut}+V`);
}

async function wasmRequest(request: unknown): Promise<DesignerWireReply> {
  const wasm = new Uint8Array(await readFile(new URL("../public/designer_runtime.wasm", import.meta.url)));
  const { instance } = await WebAssembly.instantiate(wasm, {});
  const abi = instance.exports as DesignerAbi;
  const bytes = encoder.encode(JSON.stringify(request));
  const pointer = abi.tachiko_designer_request_reserve(bytes.length);
  new Uint8Array(abi.memory.buffer, pointer, bytes.length).set(bytes);
  abi.tachiko_designer_request_run();
  return JSON.parse(decoder.decode(new Uint8Array(abi.memory.buffer, abi.tachiko_designer_response_ptr(), abi.tachiko_designer_response_len()))) as DesignerWireReply;
}

async function createInventory(page: Page): Promise<void> {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Moonfall Balance" })).toBeVisible();
  const newTable = page.getByRole("button", { name: "New Table", exact: true });
  await expect(newTable).toBeVisible();
  await newTable.click();
  const dialog = page.getByRole("dialog", { name: "New table", exact: true });
  await expect(dialog).toBeVisible();
  await dialog.getByLabel("Table name", { exact: true }).fill("Inventory");
  for (const [index, name, type] of [
    [0, "item", "Text"],
    [1, "quantity", "Number"],
    [2, "active", "Boolean"],
    [3, "received", "Date"],
  ] as const) {
    if (index > 0) await dialog.getByRole("button", { name: "Add column", exact: true }).click();
    await dialog.getByLabel("Column name", { exact: true }).nth(index).fill(name);
    await dialog.getByLabel("Column type", { exact: true }).nth(index).selectOption({ label: type });
  }
  await dialog.getByRole("button", { name: "Create table", exact: true }).click();
  await expect(page.getByRole("grid", { name: "Inventory cells", exact: true })).toBeVisible();
}

test("Driver creates Inventory, pastes typed rows, saves, reopens, and continues the same table", async ({ page }) => {
  await createInventory(page);
  await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "true");
  const headers = page.getByRole("columnheader");
  await expect(headers).toHaveText(["item", "quantity", "active", "received"]);
  await page.getByRole("gridcell", { name: "Paste rows here, or choose Append row." }).click();
  await paste(page, inventoryRows);
  await expect(page.getByRole("gridcell", { name: "0012", exact: true })).toBeVisible();
  await expect(page.getByRole("gridcell", { name: "2024-02-29", exact: true })).toBeVisible();
  await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "true");
  page.once("dialog", dialog => dialog.accept("inventory.roproj"));
  await page.getByRole("button", { name: "Save As", exact: true }).click();
  await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "false");
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await page.getByLabel("Saved project", { exact: true }).selectOption("inventory.roproj");
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(page.getByRole("gridcell", { name: "ノート", exact: true })).toBeVisible();
  await page.getByRole("gridcell", { name: "0012", exact: true }).click();
  await page.getByLabel("Cell value", { exact: true }).fill("0013");
  await page.getByRole("button", { name: "Apply to selection", exact: true }).click();
  await expect(page.getByRole("gridcell", { name: "0013", exact: true })).toBeVisible();
});

test("cancelled New Table preserves a dirty current occurrence", async ({ page }) => {
  await page.goto("/");
  page.once("dialog", dialog => dialog.accept());
  await page.getByRole("button", { name: "New Tracker", exact: true }).click();
  await expect(page.getByRole("grid", { name: "Tracker cells", exact: true })).toBeVisible();
  await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "true");

  const confirmation = page.waitForEvent("dialog");
  await page.getByRole("button", { name: "New Table", exact: true }).click();
  await (await confirmation).dismiss();

  await expect(page.getByRole("grid", { name: "Tracker cells", exact: true })).toBeVisible();
  await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "true");
});

test("invalid New Table candidates leave the current saved occurrence and durability state unchanged", async ({ page }) => {
  await page.goto("/");
  page.once("dialog", dialog => dialog.accept());
  await page.getByRole("button", { name: "New Tracker", exact: true }).click();
  page.once("dialog", dialog => dialog.accept("resident.roproj"));
  await page.getByRole("button", { name: "Save As", exact: true }).click();
  await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "false");
  const newTable = page.getByRole("button", { name: "New Table", exact: true });
  await expect(newTable).toBeVisible();
  await newTable.click();
  const dialog = page.getByRole("dialog", { name: "New table", exact: true });
  await dialog.getByLabel("Table name", { exact: true }).fill("Inventory");
  await dialog.getByLabel("Column name", { exact: true }).nth(0).fill("item");
  await dialog.getByRole("button", { name: "Add column", exact: true }).click();
  await dialog.getByLabel("Column name", { exact: true }).nth(1).fill("item");
  await dialog.getByRole("button", { name: "Create table", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText(/duplicate|invalid/i);
  await expect(page.getByRole("grid", { name: "Tracker cells", exact: true })).toBeVisible();
  await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "false");
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await page.getByLabel("Saved project", { exact: true }).selectOption("resident.roproj");
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(page.getByRole("grid", { name: "Tracker cells", exact: true })).toBeVisible();
  await expect(page.getByTestId("durability")).toHaveAttribute("data-dirty", "false");
});

test("invalid typed paste does not publish into newly created Inventory", async ({ page }) => {
  await createInventory(page);
  await page.getByRole("gridcell", { name: "Paste rows here, or choose Append row." }).click();
  await paste(page, "0012\t3\ttrue\t2024-02-29");
  const revision = await page.getByTestId("revision").textContent();
  await paste(page, "valid\t3\ttrue\t2024-02-29\ninvalid\tNaN\tyes\t2026-02-30");
  await expect(page.getByRole("alert")).toContainText(/invalid|rejected/i);
  await expect(page.getByTestId("revision")).toHaveText(revision ?? "");
  await expect(page.getByRole("gridcell", { name: "0012", exact: true })).toBeVisible();
});

test("built Designer WASM admits the same Inventory candidate as the native seed", async () => {
  const reply = await wasmRequest(inventoryRequest);
  expect(reply.status, JSON.stringify(reply)).toBe("ok");
  if (reply.status !== "ok") throw new Error(JSON.stringify(reply.error));
  expect(reply.response.type).toBe("opened");
  if (reply.response.type !== "opened") throw new Error(JSON.stringify(reply.response));
  expect(reply.response.payload.table.columns.map(column => [column.key, column.field_type])).toEqual([
    ["item", "text"],
    ["quantity", "number"],
    ["active", "boolean"],
    ["received", "date"],
  ]);
});
