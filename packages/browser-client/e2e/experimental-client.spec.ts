import { fileURLToPath } from "node:url";

import { expect, test } from "@playwright/test";

const PRODUCT_GAPS_PROJECT = fileURLToPath(
  new URL("../../../dogfood/product-gaps.roproj", import.meta.url),
);

test("exported experimental kit owns the complete external-client workflow", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByLabel("Canonical project directory").setInputFiles(
    PRODUCT_GAPS_PROJECT,
  );

  const result = page.getByTestId("result");
  await expect(result).toHaveAttribute("data-status", "passed");
  const evidence = JSON.parse((await result.textContent()) ?? "null") as {
    title: string;
    collection: string;
    rowCount: number;
    publication: { base: string; resulting: string };
    calculation: { status: string; value?: number };
    diagnostics: string[];
    staleFailure: { code: string; currentRevision: string };
    canonicalStateUnchanged: boolean;
    roundTrip: { revision: string; impact: number; priority: number; bytesEqual: boolean };
  };

  expect(evidence).toEqual({
    title: "Tachiko Work Product Gaps",
    collection: "product_gaps",
    rowCount: 3,
    publication: { base: "resident/0", resulting: "resident/1" },
    calculation: { status: "value", value: 8 },
    diagnostics: [],
    staleFailure: { code: "stale_revision", currentRevision: "resident/1" },
    canonicalStateUnchanged: true,
    roundTrip: {
      revision: "resident/0",
      impact: 3,
      priority: 8,
      bytesEqual: true,
    },
  });
});
