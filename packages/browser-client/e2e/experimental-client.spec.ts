import { fileURLToPath } from "node:url";
import { readFile, writeFile } from "node:fs/promises";

import { expect, test } from "@playwright/test";

const PRODUCT_GAPS_PROJECT = fileURLToPath(
  new URL("../../../dogfood/product-gaps.roproj", import.meta.url),
);

for (const [version, directory] of [
  [1, "dogfood/product-gaps.roproj"],
  [3, "acceptance/native-display-names/tree"],
] as const) {
  test(`#536 exported kit opens a real v${version} directory through shuffled input FileLists`, async ({ page }) => {
    const rust = await readFile(fileURLToPath(new URL(`../../../crates/storage/src/roproj/v${version}.rs`, import.meta.url)), "utf8");
    const declaration = rust.match(/pub const ROPROJ_V\d_PATHS: \[&str; \d+\] = \[([\s\S]*?)\];/);
    if (!declaration) throw new Error("Missing Rust path authority");
    const paths = Array.from(declaration[1]!.matchAll(/"([^"]+)"/g), match => match[1]!);
    await page.goto("/");
    await page.evaluate(() => {
      const input = document.createElement("input");
      input.type = "file";
      input.webkitdirectory = true;
      input.id = "folder-order-regression";
      document.body.append(input);
    });
    await page.locator("#folder-order-regression").setInputFiles(fileURLToPath(new URL(`../../../${directory}`, import.meta.url)));
    const results = await page.evaluate(async paths => {
      const entry = "/vendor/tachiko/experimental-client.js";
      const hostEntry = "/vendor/tachiko/host/project-transfer.js";
      const kit = await import(entry) as typeof import("../src/experimental-client.ts");
      const host = await import(hostEntry) as typeof import("../src/host/project-transfer.ts");
      const input = document.querySelector<HTMLInputElement>("#folder-order-regression")!;
      const relative = (file: File) => file.webkitRelativePath.split("/").slice(1).join("/");
      const selected = Array.from(input.files!);
      const canonical = paths.map(path => {
        const file = selected.find(file => relative(file) === path);
        if (!file) throw new Error(`Real directory is missing ${path}`);
        return file;
      });
      const permutations = [selected, canonical, [...canonical].reverse(), [...canonical.slice(7), ...canonical.slice(0, 7)]];
      for (let seed of [1, 42, 536]) {
        const shuffled = [...canonical];
        for (let index = shuffled.length - 1; index > 0; index -= 1) {
          seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
          const next = seed % (index + 1);
          [shuffled[index], shuffled[next]] = [shuffled[next]!, shuffled[index]!];
        }
        permutations.push(shuffled);
      }
      const observations = [];
      for (const files of permutations) {
        const list = new DataTransfer();
        for (const file of files) list.items.add(file);
        input.files = list.files;
        const inputPaths = Array.from(input.files).map(relative);
        const transfer = await host.projectTransferFromFiles(input.files);
        const entries = host.projectTransferToEntries(transfer);
        const client = kit.createExperimentalDesignerClient();
        try {
          const inspected = await client.inspectProject(transfer.slice(0));
          const opened = await client.openProject(transfer.slice(0));
          const exported = await client.exportCanonicalTree(opened.bootstrap.revision);
          observations.push({ inputPaths, paths: entries.map(file => file.path), inspected, opened,
            exactBytes: entries.every((file, index) => file.path === exported.files[index]?.path &&
              JSON.stringify(Array.from(new Uint8Array(file.bytes))) === JSON.stringify(Array.from(new Uint8Array(exported.files[index]!.bytes)))) });
        } finally { await client.close(); }
      }
      return observations;
    }, paths);
    expect(results).toHaveLength(7);
    expect(results.some(result => JSON.stringify(result.inputPaths) !== JSON.stringify(paths))).toBe(true);
    for (const result of results) {
      expect(result.paths).toEqual(paths);
      expect(result.opened).toEqual(results[0]!.opened);
      expect(result.inspected).toEqual(results[0]!.inspected);
      expect(result.exactBytes).toBe(true);
    }
  });
}

test("#529 exported kit creates named native tables and reopens every carrier in a fresh Worker", async ({ page }) => {
  const golden = Array.from(await readFile(fileURLToPath(new URL("../../../acceptance/native-display-names/named.ro", import.meta.url))));
  const workers: string[] = [];
  page.on("worker", worker => workers.push(worker.url()));
  await page.goto("/");
  const evidence = await page.evaluate(async golden => {
    const entry = "/vendor/tachiko/experimental-client.js";
    const kit = await import(entry) as typeof import("../src/experimental-client.ts");
    const client = kit.createExperimentalDesignerClient();
    if (!client.newTable || !client.trackerCommand) throw new Error("Native naming capabilities missing");
    const opened = await client.newTable("My Expenses", [{name: "Item Name", field_type: "text"}, {name: "項目", field_type: "text"}]);
    const columns = opened.table.columns.map(column => ({id: column.id, key: column.key, display_name: column.display_name, required: (column as typeof column & { required: boolean }).required}));
    const renamed = await client.trackerCommand({type: "rename_column", expected_revision: "resident/0", collection: "table_1", field: columns[0]!.id, name: "Receipt 🧾"});
    const after = await client.queryTable("table_1");
    await client.trackerCommand({type: "undo", expected_revision: renamed.resulting_revision});
    const undone = await client.queryTable("table_1");
    await client.trackerCommand({type: "redo", expected_revision: undone.revision});
    const redone = await client.queryTable("table_1");
    let refusal = "";
    try { await client.exportProject(redone.revision); } catch (error) { refusal = (error as InstanceType<typeof kit.DesignerRuntimeError>).failure.code; }
    const tree = await client.exportCanonicalTree(redone.revision);
    const opaque = await client.exportProjectV3(redone.revision);
    const portable = await client.exportPortableRo(redone.revision);
    const portableExpected = Array.from(new Uint8Array(portable.bytes));
    await client.close();
    const reopened = [];
    for (const kind of ["tree", "opaque", "portable", "golden"] as const) {
      const fresh = kit.createExperimentalDesignerClient();
      try {
        const projection = kind === "tree" ? await fresh.openCanonicalTree(tree.files.map(file => ({...file, bytes: file.bytes.slice(0)}))) : kind === "opaque" ? await fresh.openProject(opaque.bytes.slice(0)) : await fresh.openPortableRo(kind === "golden" ? new Uint8Array(golden).buffer : portable.bytes.slice(0));
        const repacked = await fresh.exportPortableRo("resident/0");
        const expected = kind === "golden" ? golden : portableExpected;
        reopened.push({kind, revision: projection.table.revision, collection: projection.table.collection.display_name, keys: projection.table.columns.map(column => column.key), names: projection.table.columns.map(column => column.display_name), exactRepack: JSON.stringify(Array.from(new Uint8Array(repacked.bytes))) === JSON.stringify(expected)});
      } finally { await fresh.close(); }
    }
    return {collection: opened.table.collection, columns, after: after.columns, undone: undone.columns, redone: redone.columns, paths: tree.files.map(file => file.path), refusal, reopened};
  }, golden);
  expect(evidence.collection.key).toBe("table_1");
  expect(evidence.collection.display_name).toBe("My Expenses");
  expect(evidence.columns.map(column => column.key)).toEqual(["column_1", "column_2"]);
  expect(evidence.columns.map(column => column.display_name)).toEqual(["Item Name", "項目"]);
  expect(evidence.columns.map(column => column.required)).toEqual([true, true]);
  expect(evidence.after.map(column => column.id)).toEqual(evidence.columns.map(column => column.id));
  expect(evidence.after.map(column => column.key)).toEqual(["column_1", "column_2"]);
  expect(evidence.after.map(column => column.display_name)).toEqual(["Receipt 🧾", "項目"]);
  expect(evidence.undone.map(column => column.display_name)).toEqual(["Item Name", "項目"]);
  expect(evidence.redone).toEqual(evidence.after);
  expect(evidence.paths).toHaveLength(19);
  expect(evidence.refusal).toBe("unsupported_project");
  expect(evidence.reopened).toEqual([
    ...["tree", "opaque", "portable"].map(kind => ({kind, revision: "resident/0", collection: "My Expenses", keys: ["column_1", "column_2"], names: ["Receipt 🧾", "項目"], exactRepack: true})),
    {kind: "golden", revision: "resident/0", collection: "My Expenses", keys: ["column_1", "column_2", "column_3"], names: ["項目", "Item Name", "column_1"], exactRepack: true},
  ]);
  expect(workers).toHaveLength(5);
  expect(workers.every(url => new URL(url).pathname === "/vendor/tachiko/experimental-client.worker.js")).toBe(true);
});

test("#515 actual kit module Worker matches native field explanations", async ({ page }) => {
  const capturePath = process.env.TACHIKO_FIELD_EXPLANATION_CAPTURE;
  if (!capturePath) throw new Error("Run the native worker_parity_capture case first; see SEED-515.md");
  const capture = JSON.parse(await readFile(capturePath, "utf8")) as {
    bytes: number[];
    requests: Array<{ type: string; expected_revision: string; target: { entity: string; field: string } }>;
    responses: unknown[];
  };
  expect(capture.requests).toEqual([
    { type: "explain_field", expected_revision: "resident/0", target: { entity: "weapon", field: "f02_damage" } },
    { type: "explain_field", expected_revision: "resident/0", target: { entity: "weapon", field: "f03_dps" } },
  ]);
  const workerUrls: string[] = [];
  page.on("worker", worker => workerUrls.push(worker.url()));
  await page.goto("/");
  const responses = await page.evaluate(async ({ bytes, requests }) => {
    const entry = "/vendor/tachiko/experimental-client.js";
    const kit = await import(entry) as typeof import("../src/experimental-client.ts");
    const client = kit.createExperimentalDesignerClient();
    try {
      await client.openProject(new Uint8Array(bytes).buffer);
      const responses = [];
      for (const request of requests) {
        responses.push({ type: "field_explanation", payload: await client.explainField(request.expected_revision, request.target) });
      }
      return responses;
    } finally { await client.close(); }
  }, capture);
  expect(workerUrls).toHaveLength(1);
  expect(workerUrls[0]).toContain("/vendor/tachiko/experimental-client.worker.js");
  expect(responses).toEqual(capture.responses);
});

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
    const withUuid = async <T>(uuid: string, action: () => Promise<T>): Promise<T> => {
      const original = crypto.randomUUID;
      Object.defineProperty(crypto, "randomUUID", { configurable: true, value: () => uuid });
      try { return await action(); }
      finally { Object.defineProperty(crypto, "randomUUID", { configurable: true, value: original }); }
    };
    const records = [];
    try {
      // Include an unavailable-only inventory: evaluated groups are not the catalogue.
      for (const missingLookups of [[], [false], [true], [true, true], [false, true]]) {
        // Stable import identities make RED/GREEN captures the same saved snapshot.
        const imported = await withUuid(`00000000-0000-4000-8000-${String(records.length + 1).padStart(12, "0")}`, () => importSpreadsheet(
          new TextEncoder().encode("P-100,hardware,2,3\n").buffer, "csv",
          { delimiter: ",", header: false },
          { column_types: [["text", "text", "number", "number"]], extra_columns: [[]] },
        ));
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

test("#533 named cross-table summary survives edit, history and fresh Worker save/reopen", async ({ page }) => {
  const workers: string[] = [];
  page.on("worker", worker => workers.push(worker.url()));
  await page.goto("/");
  const evidence = await page.evaluate(async () => {
    const kit = await import("/vendor/tachiko/experimental-client.js" as string) as typeof import("../src/experimental-client.ts");
    const client = kit.createExperimentalDesignerClient();
    if (!client.newTable || !client.duplicateCollection || !client.createKeyedGroupedSum || !client.queryKeyedGroupedSum || !client.trackerCommand) throw new Error("Required native table/summary/history capability missing");
    const opened = await client.newTable("My Expenses", [
      {name: "Item Name", field_type: "text"}, {name: "項目", field_type: "text"}, {name: "Amount", field_type: "number"},
    ]);
    const ordersKey = opened.table.collection.key;
    const columns = opened.table.columns;
    await client.trackerCommand({type: "paste_cells", expected_revision: "resident/0", collection: ordersKey, start_entity: null, start_field: columns[0]!.id, rows: [["P-100", "hardware", "2"]]});
    const duplicated = await client.duplicateCollection((await client.observeOccurrence()).revision, ordersKey, "Products");
    const products = await client.queryTable(duplicated.collection.key);
    const productsTarget = products.rows[0]!.fields.find(field => field.target.field === products.columns[2]!.id)!.target;
    await client.editNumber(products.revision, productsTarget, "3");
    const definition = {
      id: "expenses-summary", orders_schema: opened.table.collection.id, products_schema: products.collection.id,
      order_lookup_key_field: columns[0]!.id, order_quantity_field: columns[2]!.id,
      product_key_field: products.columns[0]!.id, product_category_field: products.columns[1]!.id, product_price_field: products.columns[2]!.id,
    };
    const created = await client.createKeyedGroupedSum((await client.observeOccurrence()).revision, definition);
    await client.trackerCommand({type: "undo", expected_revision: created.publication.resulting_revision});
    const afterCreationUndoIds = (await client.bootstrap()).keyed_grouped_sum_definition_ids;
    const productsAfterCreationUndo = await client.queryTable(products.collection.key);
    const priceAfterCreationUndo = productsAfterCreationUndo.rows[0]!.fields.find(field => field.target.field === products.columns[2]!.id)!.stored;
    await client.trackerCommand({type: "redo", expected_revision: productsAfterCreationUndo.revision});
    const afterCreationRedo = await client.queryKeyedGroupedSum(definition.id);
    const orders = await client.queryTable(ordersKey);
    const quantity = orders.rows[0]!.fields.find(field => field.target.field === columns[2]!.id)!.target;
    const edited = await client.editNumber(orders.revision, quantity, "4");
    const afterEdit = await client.queryKeyedGroupedSum(definition.id);
    await client.trackerCommand({type: "undo", expected_revision: edited.resulting_revision});
    const afterUndo = await client.queryKeyedGroupedSum(definition.id);
    await client.trackerCommand({type: "redo", expected_revision: afterUndo.revision});
    const afterRedo = await client.queryKeyedGroupedSum(definition.id);
    const saved = await client.exportProjectV3(afterRedo.revision);
    const savedBytes = Array.from(new Uint8Array(saved.bytes));
    const tree = await client.exportCanonicalTree(afterRedo.revision);
    const treeBytes = tree.files.map(file => ({path: file.path, bytes: Array.from(new Uint8Array(file.bytes))}));
    await client.close();
    const fresh = kit.createExperimentalDesignerClient();
    try {
      const reopened = await fresh.openProject(saved.bytes);
      if (!fresh.queryKeyedGroupedSum) throw new Error("Fresh summary capability missing");
      const summary = await fresh.queryKeyedGroupedSum(definition.id);
      const repacked = await fresh.exportProjectV3("resident/0");
      const reopenedTree = await fresh.exportCanonicalTree("resident/0");
      const canonicalBytesEqual = JSON.stringify(reopenedTree.files.map(file => ({path: file.path, bytes: Array.from(new Uint8Array(file.bytes))}))) === JSON.stringify(treeBytes);
      const opaqueBytesEqual = JSON.stringify(savedBytes) === JSON.stringify(Array.from(new Uint8Array(repacked.bytes)));
      return {
        tableName: opened.table.collection.display_name, columns: columns.map(column => column.display_name),
        crossTable: definition.orders_schema !== definition.products_schema,
        created: created.result.groups, afterCreationUndoIds, priceAfterCreationUndo, afterCreationRedo: afterCreationRedo.groups,
        afterEdit: afterEdit.groups, afterUndo: afterUndo.groups, afterRedo: afterRedo.groups,
        reopenedRevision: reopened.table.revision, summary, ids: (await fresh.bootstrap()).keyed_grouped_sum_definition_ids, canonicalBytesEqual, opaqueBytesEqual,
      };
    } finally {await fresh.close();}
  });
  expect(workers).toHaveLength(2);
  expect(workers.every(url => url.includes("/vendor/tachiko/experimental-client.worker.js"))).toBe(true);
  expect(evidence).toEqual({
    tableName: "My Expenses", columns: ["Item Name", "項目", "Amount"], crossTable: true,
    created: [{category: "hardware", value: 6}],
    afterCreationUndoIds: [], priceAfterCreationUndo: {kind: "number", value: 3}, afterCreationRedo: [{category: "hardware", value: 6}],
    afterEdit: [{category: "hardware", value: 12}], afterUndo: [{category: "hardware", value: 6}], afterRedo: [{category: "hardware", value: 12}],
    reopenedRevision: "resident/0", summary: {definition_id: "expenses-summary", revision: "resident/0", groups: [{category: "hardware", value: 12}], diagnostics: []},
    ids: ["expenses-summary"], canonicalBytesEqual: true, opaqueBytesEqual: true,
  });
});
