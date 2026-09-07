import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

import { expect, test, type Page } from "@playwright/test";

const GAME_BALANCE_RO = fileURLToPath(
  new URL("../../../examples/game-balance/game-balance.ro", import.meta.url),
);

type LaunchEntry = { name: string; bytes?: number[]; readError?: string };
type LaunchHarness = {
  hasConsumer(): boolean;
  deliver(entries: LaunchEntry[]): Promise<void>;
};

/** Inject the browser File Handling delivery boundary without implementing product behavior. */
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
        const handles = entries.map(({ name, bytes, readError }) => ({
          kind: "file" as const,
          name,
          async getFile() {
            if (readError !== undefined) throw new Error(readError);
            return new File([new Uint8Array(bytes ?? [])], name, {
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

/** Report whether production registered exactly the launch boundary under test. */
async function hasLaunchConsumer(page: Page): Promise<boolean> {
  return page.evaluate(() => {
    const harness = Reflect.get(window, "__tachikoLaunchHarness") as LaunchHarness | undefined;
    return harness?.hasConsumer() ?? false;
  });
}

/** Deliver deterministic OS-style file launch inputs to the registered product consumer. */
async function deliverLaunch(page: Page, entries: LaunchEntry[]): Promise<void> {
  await page.evaluate(async (payload) => {
    const harness = Reflect.get(window, "__tachikoLaunchHarness") as LaunchHarness | undefined;
    if (harness === undefined) throw new Error("launchQueue acceptance harness is missing");
    await harness.deliver(payload);
  }, entries);
}

/** Require one confirmation dialog and resolve only after the selected user decision is applied. */
function requireConfirmDialog(page: Page, decision: "accept" | "dismiss"): Promise<void> {
  return new Promise((resolve, reject) => {
    page.once("dialog", async (dialog) => {
      try {
        expect(dialog.type()).toBe("confirm");
        if (decision === "accept") await dialog.accept();
        else await dialog.dismiss();
        resolve();
      } catch (error) {
        reject(error);
      }
    });
  });
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

test("warm PWA launches require dirty consent and corrupt input preserves current work", async ({ page }) => {
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

  const rejectedReplacement = requireConfirmDialog(page, "dismiss");
  await deliverLaunch(page, [{ name: "game-balance.ro", bytes }]);
  await rejectedReplacement;
  await expect(page.getByRole("heading", { name: "Moonfall Balance" })).toBeVisible();
  await expect(page.getByLabel("Damage for Iron Sword")).toHaveValue("45");

  const acceptedAttempt = requireConfirmDialog(page, "accept");
  await deliverLaunch(page, [{ name: "corrupt.ro", bytes: [1, 2, 3] }]);
  await acceptedAttempt;
  await expect(page.getByRole("alert")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Moonfall Balance" })).toBeVisible();
  await expect(page.getByLabel("Damage for Iron Sword")).toHaveValue("45");
});

test("multi-file PWA launch creates a fresh visible failure and preserves the current occurrence", async ({ page }) => {
  const bytes = [...await readFile(GAME_BALANCE_RO)];
  await installLaunchQueueHarness(page);
  await page.goto("/");
  await expect.poll(() => hasLaunchConsumer(page)).toBe(true);
  await expect(page.getByRole("alert")).toHaveCount(0);

  await deliverLaunch(page, [
    { name: "one.ro", bytes },
    { name: "two.ro", bytes },
  ]);
  await expect(page.getByRole("alert")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Moonfall Balance" })).toBeVisible();
  await expect(page.getByLabel("Damage for Iron Sword")).toHaveValue("36");
});

test("empty and unreadable PWA launch inputs fail visibly without replacing current work", async ({ page }) => {
  await installLaunchQueueHarness(page);
  await page.goto("/");
  await expect.poll(() => hasLaunchConsumer(page)).toBe(true);
  await expect(page.getByRole("alert")).toHaveCount(0);

  await deliverLaunch(page, []);
  await expect(page.getByRole("alert")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Moonfall Balance" })).toBeVisible();

  await page.reload();
  await expect.poll(() => hasLaunchConsumer(page)).toBe(true);
  await expect(page.getByRole("alert")).toHaveCount(0);
  await deliverLaunch(page, [{ name: "unreadable.ro", readError: "permission denied" }]);
  await expect(page.getByRole("alert")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Moonfall Balance" })).toBeVisible();
});

test("ordinary Web startup remains usable when launchQueue is absent", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Moonfall Balance" })).toBeVisible();
  await expect(page.getByLabel("Damage for Iron Sword")).toHaveValue("36");
});
