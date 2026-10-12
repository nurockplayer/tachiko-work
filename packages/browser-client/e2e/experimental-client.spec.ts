import { fileURLToPath } from "node:url";
import { writeFile } from "node:fs/promises";

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

test("spreadsheet export discloses saved definitions through the actual Worker", async ({ page }, testInfo) => {
  await page.goto("/");
  const workerUrls: string[] = [];
  page.on("worker", worker => workerUrls.push(worker.url()));
  const records = await page.evaluate(async () => {
    const entry = "/vendor/tachiko/experimental-client.js";
    const kit = await import(entry) as typeof import("../src/experimental-client.ts");
    const client = kit.createExperimentalDesignerClient();
    if (!client.importSpreadsheet || !client.exportSpreadsheet || !client.createKeyedGroupedSum || !client.queryKeyedGroupedSum || !client.trackerCommand) throw new Error("Spreadsheet/summary/history capability missing");
    const importSpreadsheet = client.importSpreadsheet.bind(client);
    const exportSpreadsheet = client.exportSpreadsheet.bind(client);
    const createSummary = client.createKeyedGroupedSum.bind(client);
    const querySummary = client.queryKeyedGroupedSum.bind(client);
    const history = client.trackerCommand.bind(client);
    const same = (actual: unknown, expected: unknown, label: string): void => {
      if (JSON.stringify(actual) !== JSON.stringify(expected)) throw new Error(label);
    };
    const bytes = (value: ArrayBuffer): number[] => Array.from(new Uint8Array(value));
    const records = [];
    try {
      // Include an unavailable-only inventory: evaluated groups are not the catalogue.
      for (const missingLookups of [[], [false], [true], [true, true], [false, true]]) {
        const imported = await importSpreadsheet(
          new TextEncoder().encode("P-100,hardware,2,3\n").buffer, "csv",
          { delimiter: ",", header: false },
          { column_types: [["text", "text", "number", "number"]], extra_columns: [[]] },
        );
        const metadata = imported.metadata;
        const sheet = metadata.sheets[0]!;
        const collection = sheet.schema_id;
        for (const [index, missing] of missingLookups.entries()) {
          const publication = await createSummary((await client.observeOccurrence()).revision, {
            id: `summary-${index}`, orders_schema: collection, products_schema: collection,
            order_lookup_key_field: sheet.columns[Number(missing)]!.field_id,
            order_quantity_field: sheet.columns[2]!.field_id,
            product_key_field: sheet.columns[0]!.field_id,
            product_category_field: sheet.columns[1]!.field_id,
            product_price_field: sheet.columns[3]!.field_id,
          });
          same(publication.result.groups, missing ? [] : [{ category: "hardware", value: 6 }], "summary fixture result");
          same(publication.result.diagnostics.map(d => d.code), missing ? ["lookup.missing_key"] : [], "summary fixture diagnostic");
        }
        const target = { entity: sheet.rows[0]!.entity_id, field: sheet.columns[2]!.field_id };
        await client.editNumber((await client.observeOccurrence()).revision, target, "4");
        await history({ type: "undo", expected_revision: (await client.observeOccurrence()).revision });
        const occurrence = await client.observeOccurrence();
        const revision = occurrence.revision;
        const native = await client.exportProject(revision);
        const table = await client.queryTable(collection);
        const summaries = await Promise.all(missingLookups.map((_, index) => querySummary(`summary-${index}`)));
        const exports = [];
        for (const format of ["csv", "xlsx"] as const) {
          const exported = await exportSpreadsheet(revision, metadata, format, collection);
          const repeated = await exportSpreadsheet(revision, metadata, format, collection);
          same({ ...repeated, bytes: bytes(repeated.bytes) }, { ...exported, bytes: bytes(exported.bytes) }, "repeat export accumulated findings or changed bytes");
          exports.push({ format, bytes: bytes(exported.bytes), ledger: exported.ledger });
        }
        same(await client.observeOccurrence(), occurrence, "export changed occurrence/revision");
        same(bytes((await client.exportProject(revision)).bytes), bytes(native.bytes), "export changed native save");
        same(await client.queryTable(collection), table, "export changed table");
        same(await Promise.all(missingLookups.map((_, index) => querySummary(`summary-${index}`))), summaries, "export changed summary facts");
        // Export must leave the pre-existing redo entry usable.
        await history({ type: "redo", expected_revision: revision });
        same((await client.queryFields((await client.observeOccurrence()).revision, [target])).fields[0]!.stored,
          { kind: "number", value: 4 }, "export changed redo history");
        await history({ type: "undo", expected_revision: (await client.observeOccurrence()).revision });
        same(bytes((await client.exportProject((await client.observeOccurrence()).revision)).bytes), bytes(native.bytes), "history did not restore native bytes");
        await client.openProject(native.bytes.slice(0));
        same(bytes((await client.exportProject("resident/0")).bytes), bytes(native.bytes), "native fresh reopen changed definitions/bindings");
        same((await client.bootstrap()).keyed_grouped_sum_definition_ids, summaries.map(summary => summary.definition_id), "reopen lost catalogue");
        for (const summary of summaries) {
          same(await querySummary(summary.definition_id), { ...summary, revision: "resident/0" }, "reopen changed recomputation");
        }
        records.push({ missingLookups, native: bytes(native.bytes), exports });
      }
      return records;
    } finally { await client.close(); }
  });
  // Preserve byte evidence even on RED so the same snapshots can be compared to GREEN.
  await writeFile(testInfo.outputPath("spreadsheet-exports.json"), JSON.stringify(records));
  expect(workerUrls).toHaveLength(1);
  expect(workerUrls[0]).toContain("/vendor/tachiko/experimental-client.worker.js");
  for (const record of records) for (const exported of record.exports) {
    const base = {
      category: "converted", code: "bound_formula_absolute_a1", location: "workbook",
      message: "Export is rebuilt from current stable identities and authoritative values. Formula references use current absolute worksheet coordinates; deleted rows are omitted.", blocking: false,
    };
    const formatFinding = exported.ledger.find(finding => finding.code === (exported.format === "csv" ? "csv_values_only" : "xlsx_header_inserted"));
    expect(formatFinding).toBeDefined();
    const expected = [base];
    if (record.missingLookups.length > 0) expected.push({
      category: "lossy_on_export", code: "saved_keyed_grouped_sum_definitions_not_exported", location: "workbook",
      message: "Saved grouped-summary definitions are not included in this spreadsheet export. Keep a Tachiko saved copy to preserve their bindings and recomputation.", blocking: false,
    });
    expect(exported.ledger).toEqual([...expected, formatFinding]);
  }
});
