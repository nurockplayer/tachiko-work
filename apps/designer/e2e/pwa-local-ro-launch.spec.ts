import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

import { expect, test, type Page } from "@playwright/test";

const GAME_BALANCE_RO = fileURLToPath(
  new URL("../../../examples/game-balance/game-balance.ro", import.meta.url),
);

type LaunchEntry = { name: string; bytes: number[] };
type LaunchHarness = {
  hasConsumer(): boolean;
  deliver(entries: LaunchEntry[]): Promise<void>;
};

async function installLaunchQueueHarness(page: Page): Promise<void> {
  await page.addInitScript(() => {
    type Consumer = (launchParams: {
      files: Array<{
        kind: "file";
        name: string;
        getFile(): Promise<File>;
      }>;
      targetURL: string;
    }) => void | Promise<void>;

    let consumer: Consumer | null = null;
    const queue = {
      setConsumer(next: Consumer) {
        consumer = next;
      },
    };
    const harness: LaunchHarness = {
      hasConsumer: () => consumer !== null,
      async deliver(entries) {
        if (consumer === null) throw new Error("Tachiko did not register a launchQueue consumer.");
        const handles = entries.map(({ name, bytes }) => ({
          kind: "file" as const,
          name,
          async getFile() {
            return new File([new Uint8Array(bytes)], name, {
              type: "application/octet-stream",
            });
          },
        }));
        await consumer({ files: handles, targetURL: window.location.href });
      },
    };
    Object.defineProperty(window, "launchQueue", {
      configurable: true,
      value: queue,
    });
    Object.defineProperty(window, "__tachikoLaunchHarness", {
      configurable: true,
      value: harness,
    });
  });
}

async function launchHarness(page: Page): Promise<LaunchHarness> {
  return page.evaluate(() => {
    const harness = Reflect.get(window, "__tachikoLaunchHarness") as LaunchHarness | undefined;
    if (harness === undefined) throw new Error("launchQueue acceptance harness is missing");
    return {
      hasConsumer: () => harness.hasConsumer(),
      deliver: async () => {
        throw new Error("The in-page launch harness must be invoked through page.evaluate.");
      },
    };
  });
}

async function hasLaunchConsumer(page: Page): Promise<boolean> {
  return page.evaluate(() => {
    const harness = Reflect.get(window, "__tachikoLaunchHarness") as LaunchHarness | undefined;
    return harness?.hasConsumer() ?? false;
  });
}

async function deliverLaunch(page: Page, entries: LaunchEntry[]): Promise<void> {
  await page.evaluate(async (payload) => {
    const harness = Reflect.get(window, "__tachikoLaunchHarness") as LaunchHarness | undefined;
    if (harness === undefined) throw new Error("launchQueue acceptance harness is missing");
    await harness.deliver(payload);
  }, entries);
}

test("built Designer exposes an installable manifest with one .ro file handler", async ({ page }) => {
  await page.goto("/");

  const manifestHref = await page.locator('link[rel="manifest"]').getAttribute("href");
  expect(manifestHref, "the shipped Designer page must link its web app manifest").toBeTruthy();

  const result = await page.evaluate(async (href) => {
    const response = await fetch(href ?? "");
    return {
      status: response.status,
      contentType: response.headers.get("content-type") ?? "",
      text: await response.text(),
    };
  }, manifestHref);
  expect(result.status).toBe(200);
  expect(result.contentType).toMatch(/json|manifest/);

  const manifest = JSON.parse(result.text) as {
    name?: string;
    start_url?: string;
    display?: string;
    file_handlers?: Array<{ action?: string; accept?: Record<string, string[]> }>;
  };
  expect(manifest.name).toBeTruthy();
  expect(manifest.start_url).toBeTruthy();
  expect(manifest.display).toBeTruthy();
  expect(manifest.file_handlers).toHaveLength(1);
  const handler = manifest.file_handlers?.[0];
  expect(handler?.action).toBeTruthy();
  expect(Object.values(handler?.accept ?? {}).flat()).toContain(".ro");
});

test("installed-PWA launchQueue opens raw .ro bytes through the existing Designer authority", async ({ page }) => {
  const bytes = [...await readFile(GAME_BALANCE_RO)];
  await installLaunchQueueHarness(page);
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Moonfall Balance" })).toBeVisible();
  await expect.poll(() => hasLaunchConsumer(page)).toBe(true);

  await deliverLaunch(page, [{ name: "game-balance.ro", bytes }]);
  await expect(page.getByRole("heading", { name: "Moonfall: starter balance" })).toBeVisible();
  await expect(page.getByTestId("revision")).toContainText("resident/0");
  await expect(page.locator('[data-field="iron_sword.dps"] output')).toHaveText("40");
});

test("warm PWA launches respect dirty consent and reject bad or multi-file input without replacing current work", async ({ page }) => {
  const bytes = [...await readFile(GAME_BALANCE_RO)];
  await installLaunchQueueHarness(page);
  await page.goto("/");
  await expect.poll(() => hasLaunchConsumer(page)).toBe(true);

  await page.getByLabel("Damage for Iron Sword").fill("45");
  await page
    .locator('[data-field="iron_sword.damage"]')
    .getByRole("button", { name: "Apply" })
    .click();
  await expect(page.getByTestId("durability")).toContainText("Unsaved changes");

  page.once("dialog", async (dialog) => dialog.dismiss());
  await deliverLaunch(page, [{ name: "game-balance.ro", bytes }]);
  await expect(page.getByRole("heading", { name: "Moonfall Balance" })).toBeVisible();
  await expect(page.getByLabel("Damage for Iron Sword")).toHaveValue("45");

  page.once("dialog", async (dialog) => dialog.accept());
  await deliverLaunch(page, [{ name: "corrupt.ro", bytes: [1, 2, 3] }]);
  await expect(page.getByRole("alert")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Moonfall Balance" })).toBeVisible();
  await expect(page.getByLabel("Damage for Iron Sword")).toHaveValue("45");

  await deliverLaunch(page, [
    { name: "one.ro", bytes },
    { name: "two.ro", bytes },
  ]);
  await expect(page.getByRole("alert")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Moonfall Balance" })).toBeVisible();
  await expect(page.getByLabel("Damage for Iron Sword")).toHaveValue("45");
});

test("ordinary Web startup remains usable when launchQueue is absent", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Moonfall Balance" })).toBeVisible();
  await expect(page.getByLabel("Damage for Iron Sword")).toHaveValue("36");
});
