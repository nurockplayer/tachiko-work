import { createHash } from "node:crypto";
import { readFile, readdir } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { expect, test } from "@playwright/test";

const CAPTURE_DIR = process.env.TACHIKO_V3_ACCEPTANCE_CAPTURE_DIR;
const CANDIDATE_HEAD = process.env.TACHIKO_V3_ACCEPTANCE_CANDIDATE_HEAD;
const RUN_ID = process.env.TACHIKO_V3_ACCEPTANCE_RUN_ID;
const KIT_ID = process.env.TACHIKO_V3_ACCEPTANCE_KIT_ID;
const SEED_ROOT = fileURLToPath(new URL("../../../acceptance/date-definition-save-v3/fixtures", import.meta.url));
const KIT_ROOT = fileURLToPath(new URL("../../../examples/experimental-designer-client/vendor/tachiko", import.meta.url));
const CSV_PATH = join(SEED_ROOT, "mixed.csv");
const DEFINITION_ID = "acceptance-orders-summary";
const PROFILES = [
  { name: "date-definition", date: true, definition: true },
  { name: "date-only", date: true, definition: false },
  { name: "definition-only", date: false, definition: true },
  { name: "neither", date: false, definition: false },
] as const;
const REQUIRED_NATIVE_CASES = [
  "date_and_saved_definition_lossless_reopen", "date_only_lossless_reopen",
  "definition_only_lossless_reopen", "neither_lossless_reopen",
  "ordinary_export_keeps_the_mixed_legacy_refusal_control", "frozen_codec_and_bridge_profile_controls",
  "legacy_date_only_ingress_edit_reexport", "legacy_definition_only_ingress_edit_reexport",
  "legacy_neither_ingress_edit_reexport", "fresh_candidate_ordinary_legacy_exports_remain_v1_v2_compatible",
  "candidate_inspect_and_open_refuse_valid_v3_text_and_number_constraints",
  "exact_full_catalogue_opened_projection_boundary_and_plus_one",
  "version_gate_and_opaque_transfer_malformed_vectors_are_distinct_and_atomic",
  "successful_legacy_replacement_clears_v3_origin_marker",
  "successful_v3_replacement_preserves_origin_marker",
  "transfer_limit_and_oversized_inspect_preserve_the_current_v3_occurrence",
];
const REQUIRED_CAPTURE_PROFILES = [
  "date-definition", "date-only", "definition-only", "neither",
  "legacy-ingress-date-only", "legacy-ingress-definition-only", "legacy-ingress-neither",
  "ordinary-date-only", "ordinary-definition-only", "ordinary-neither",
  "constrained-text", "constrained-number", "boundary-fresh-65536", "boundary-fresh-65537-probe",
];

function requireEnvironment(value: string | undefined, name: string): string {
  if (value === undefined || value.length === 0) throw new Error(`NOTRUN_MISSING_${name}: acceptance evidence is incomplete`);
  return value;
}

function hex(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}

async function verifyNativeCapture(): Promise<{
  dateOnly: Uint8Array; dateDefinition: Uint8Array; mixedCsv: Uint8Array; constrainedText: Uint8Array; constrainedNumber: Uint8Array;
  boundaryAt: Uint8Array; boundaryOver: Uint8Array;
}> {
  const dir = requireEnvironment(CAPTURE_DIR, "CANDIDATE_CAPTURE");
  const candidateHead = requireEnvironment(CANDIDATE_HEAD, "CANDIDATE_HEAD");
  const runId = requireEnvironment(RUN_ID, "CAPTURE_RUN_ID");
  const kitId = requireEnvironment(KIT_ID, "KIT_ID");
  if (!/^[0-9a-f]{40}$/.test(candidateHead)) throw new Error("NOTRUN_INVALID_CANDIDATE_HEAD: expected exact commit identity");
  const kitManifestBytes = new Uint8Array(await readFile(join(KIT_ROOT, "artifact-manifest.json")));
  if (hex(kitManifestBytes) !== kitId) throw new Error("NOTRUN_STALE_KIT_IDENTITY: kit manifest digest differs from the runner receipt");
  const kitManifest = JSON.parse(new TextDecoder().decode(kitManifestBytes)) as { sourceCommit?: string; files?: Array<{ path: string; sha256: string }> };
  if (kitManifest.sourceCommit !== candidateHead || !Array.isArray(kitManifest.files)) throw new Error("NOTRUN_STALE_KIT_SOURCE: exported kit does not name the candidate commit");
  const kitFiles: string[] = [];
  const walkKit = async (relative = ""): Promise<void> => {
    for (const entry of await readdir(join(KIT_ROOT, relative), { withFileTypes: true })) {
      const child = relative.length === 0 ? entry.name : `${relative}/${entry.name}`;
      if (entry.isSymbolicLink()) throw new Error(`NOTRUN_KIT_SYMLINK: ${child}`);
      if (entry.isDirectory()) await walkKit(child);
      else if (entry.isFile() && child !== "artifact-manifest.json") kitFiles.push(child);
      else if (!entry.isFile()) throw new Error(`NOTRUN_KIT_NONREGULAR: ${child}`);
    }
  };
  await walkKit(); kitFiles.sort();
  const declaredKitFiles = kitManifest.files.map((file) => file.path).sort();
  if (JSON.stringify(kitFiles) !== JSON.stringify(declaredKitFiles)) throw new Error("NOTRUN_STALE_OR_PARTIAL_KIT: kit file inventory differs from its manifest");
  for (const file of kitManifest.files) {
    if (hex(new Uint8Array(await readFile(join(KIT_ROOT, file.path)))) !== file.sha256) throw new Error(`NOTRUN_KIT_HASH_MISMATCH: ${file.path}`);
  }
  const lease = JSON.parse(await readFile(join(dir, "capture-lease.json"), "utf8")) as Record<string, unknown>;
  const result = JSON.parse(await readFile(join(dir, "native-result.json"), "utf8")) as {
    status?: string; candidate_head?: string; run_id?: string; required_case_count?: number;
    required_cases?: string[]; artifacts?: Record<string, string>;
  };
  if (lease.state !== "native_complete" || lease.candidate_head !== candidateHead || lease.run_id !== runId) {
    throw new Error("NOTRUN_STALE_OR_PARTIAL_CAPTURE: capture lease does not match this candidate run");
  }
  if (result.status !== "native_cases_passed" || result.candidate_head !== candidateHead || result.run_id !== runId ||
      result.required_case_count !== REQUIRED_NATIVE_CASES.length ||
      JSON.stringify(result.required_cases) !== JSON.stringify(REQUIRED_NATIVE_CASES)) {
    throw new Error("NOTRUN_NATIVE_CASES_INCOMPLETE: required native acceptance cases were not all recorded");
  }
  const expectedArtifacts = REQUIRED_CAPTURE_PROFILES.flatMap((name) => [`${name}.source.json`, `${name}.twd`]).sort();
  if (JSON.stringify(Object.keys(result.artifacts ?? {}).sort()) !== JSON.stringify(expectedArtifacts)) {
    throw new Error("NOTRUN_REQUIRED_CAPTURE_MISSING: native manifest omits one or more mandatory candidate outputs");
  }
  const actualFiles = (await readdir(dir)).filter((file) => !["native-result.json", "capture-lease.json", "worker-acceptance.log", "worker-completion.json"].includes(file)).sort();
  if (JSON.stringify(actualFiles) !== JSON.stringify(expectedArtifacts)) throw new Error("NOTRUN_PARTIAL_OR_STALE_CAPTURE_FILES: capture directory inventory differs from its manifest");
  for (const [name, digest] of Object.entries(result.artifacts ?? {})) {
    const bytes = new Uint8Array(await readFile(join(dir, name)));
    if (hex(bytes) !== digest) throw new Error(`NOTRUN_CAPTURE_HASH_MISMATCH: ${name}`);
    if (name.endsWith(".source.json")) {
      const receipt = JSON.parse(new TextDecoder().decode(bytes)) as Record<string, unknown>;
      if (receipt.producer_head !== candidateHead || receipt.capture_run_id !== runId) {
        throw new Error(`NOTRUN_STALE_PRODUCER_RECEIPT: ${name}`);
      }
      const profile = name.slice(0, -".source.json".length);
      const kind = profile.startsWith("ordinary-") ? "ordinary_legacy_export" :
        profile.startsWith("constrained-") ? "valid_storage_v3_constraint_input" :
          profile.startsWith("boundary-fresh-65537") ? "fresh_admission_probe" : "selected_v3_export";
      if (receipt.profile !== profile || receipt.kind !== kind) throw new Error(`NOTRUN_CAPTURE_RECEIPT_KIND_MISMATCH: ${name}`);
    }
  }
  const dateOnly = new Uint8Array(await readFile(join(dir, "date-only.twd")));
  const dateDefinition = new Uint8Array(await readFile(join(dir, "date-definition.twd")));
  const constrainedText = new Uint8Array(await readFile(join(dir, "constrained-text.twd")));
  const constrainedNumber = new Uint8Array(await readFile(join(dir, "constrained-number.twd")));
  const boundaryAt = new Uint8Array(await readFile(join(dir, "boundary-fresh-65536.twd")));
  const boundaryOver = new Uint8Array(await readFile(join(dir, "boundary-fresh-65537-probe.twd")));
  const mixedCsv = new Uint8Array(await readFile(CSV_PATH));
  return { dateOnly, dateDefinition, mixedCsv, constrainedText, constrainedNumber, boundaryAt, boundaryOver };
}

test("actual Worker imports, explicitly exports and reopens every v3 profile", async ({ page }) => {
  const { mixedCsv } = await verifyNativeCapture();
  await page.goto("/");
  const results = await page.evaluate(async (args) => {
    const modulePath: string = "/vendor/tachiko/experimental-client.js";
    const module = await import(modulePath) as {
      createExperimentalDesignerClient: () => any;
      DesignerRuntimeError: new (...args: any[]) => Error & { failure: { code: string; current_revision: string } };
    };
    const check = (condition: boolean, message: string): void => { if (!condition) throw new Error(message); };
    const stable = (value: any): any => Array.isArray(value) ? value.map(stable) :
      value !== null && typeof value === "object" ? Object.fromEntries(Object.keys(value).sort().map((key) => [key, stable(value[key])])) : value;
    const equal = (actual: unknown, expected: unknown, message: string): void => {
      if (JSON.stringify(stable(actual)) !== JSON.stringify(stable(expected))) throw new Error(`${message}: ${JSON.stringify(actual)}`);
    };
    const withUuid = async <T>(uuid: string, operation: () => Promise<T>): Promise<T> => {
      const original = crypto.randomUUID;
      Object.defineProperty(crypto, "randomUUID", { configurable: true, value: () => uuid });
      try { return await operation(); }
      finally { Object.defineProperty(crypto, "randomUUID", { configurable: true, value: original }); }
    };
    const expectCode = async (operation: () => Promise<unknown>, code: string, revision: string): Promise<void> => {
      try { await operation(); } catch (error) {
        if (!(error instanceof module.DesignerRuntimeError)) throw new Error(`Expected typed ${code}; got ${String(error)}`);
        equal(error.failure.code, code, `wrong typed failure; expected ${code}`);
        equal(error.failure.current_revision, revision, `typed ${code} reported a different resident revision`);
        return;
      }
      throw new Error(`Expected typed ${code} refusal but operation succeeded`);
    };
    const noProjectOpen = async (client: any): Promise<void> => {
      try { await client.observeOccurrence(); } catch (error) {
        if (!(error instanceof module.DesignerRuntimeError)) throw new Error(`Expected typed no_project_open; got ${String(error)}`);
        equal(error.failure.code, "no_project_open", "Close left an unexpected runtime error");
        equal(error.failure.current_revision, "unavailable", "closed Worker still exposes a resident revision");
        return;
      }
      throw new Error("observeOccurrence succeeded after close; a later replacement could mask a resident runtime");
    };
    const prefix = "import_00000000-0000-4000-8000-000000000001_";
    const literalIds = {
      document: `${prefix}0001`, schema: `${prefix}0002`,
      fields: ["0003", "0004", "0005", "0006", "0007"].map((id) => `${prefix}${id}`),
      rows: [`${prefix}0008`, `${prefix}0009`],
    };
    const assertOpened = (opened: any, input: any): void => {
      const collection = { id: literalIds.schema, key: "sheet_1", entity_count: 2 };
      equal(opened.bootstrap, {
        title: "Imported workbook", revision: input.revision, default_collection: "sheet_1",
        collections: [collection], keyed_grouped_sum_definition_ids: input.catalogue,
      }, "complete Bootstrap oracle changed");
      equal(opened.table.revision, input.revision, "table revision changed");
      equal(opened.table.collection, collection, "complete table collection changed");
      equal(opened.table.columns, literalIds.fields.map((id: string, i: number) => ({
        id, key: `column_${i + 1}`, field_type: ["text", "text", "number", "number", input.date ? "date" : "text"][i],
      })), "complete literal field identities/keys/types changed");
      const values = [
        ["PEN", "Stationery", input.quantity, 200, input.dateValue],
        ["NOTE", "Paper", 5, 200, "2026-09-27"],
      ];
      equal(opened.table.rows, literalIds.rows.map((id: string, ri: number) => ({
        id, key: `sheet_1_row_${ri + 1}`,
        fields: literalIds.fields.map((field: string, fi: number) => {
          const kind = fi < 2 ? "text" : fi < 4 ? "number" : input.date ? "date" : "text";
          return {
            target: { entity: id, field }, address: `sheet_1_row_${ri + 1}.column_${fi + 1}`,
            stored: { kind, value: values[ri]![fi] }, formula: null, calculated: null,
            diagnostics: [], editable_scalar: kind,
          };
        }),
      })), "complete literal field target/address/value/formula/diagnostic projection changed");
    };
    const assertCurrent = async (client: any, input: any): Promise<void> => {
      const occurrence = await client.observeOccurrence();
      equal(occurrence, {
        scope: `designer-occurrence/${input.occurrence}/${literalIds.document}`,
        revision: input.revision,
      }, "resident occurrence/revision changed");
      const opened = { bootstrap: await client.bootstrap(), table: await client.queryTable("sheet_1") };
      assertOpened(opened, input);
    };
    const assertSummary = async (client: any, revision: string, stationery: number): Promise<void> => {
      const result = await client.queryKeyedGroupedSum(DEFINITION_ID);
      equal([result.definition_id, result.revision, result.diagnostics,
        result.groups.map((g: any) => [g.category, g.value]).sort()],
      [DEFINITION_ID, revision, [], [["Paper", 1000], ["Stationery", stationery]].sort()],
      "definition summary differs from the literal q oracle");
    };
    const manifestVersion = (bytes: Uint8Array): number => {
      if (new TextDecoder().decode(bytes.slice(0, 8)) === "TWDPROJ2") return 2;
      check(new TextDecoder().decode(bytes.slice(0, 8)) === "TWDPROJ1", "legacy export framing changed");
      const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
      const count = view.getUint32(8, true); let offset = 12; let manifest: any;
      for (let i = 0; i < count; i += 1) {
        const pathLength = view.getUint16(offset, true); offset += 2;
        const bodyLength = view.getUint32(offset, true); offset += 4;
        const path = new TextDecoder().decode(bytes.slice(offset, offset + pathLength)); offset += pathLength;
        const body = bytes.slice(offset, offset + bodyLength); offset += bodyLength;
        if (path === "manifest.json") manifest = JSON.parse(new TextDecoder().decode(body));
      }
      check(offset === bytes.length && manifest !== undefined, "legacy export framing or manifest incomplete");
      return manifest.format_version;
    };
    const assertLegacyGuard = async (client: any, input: any): Promise<void> => {
      const before = await client.observeOccurrence();
      if (input.date && input.definition) {
        await expectCode(() => client.exportProject(input.revision), "invalid_project", input.revision);
      } else {
        const saved = await client.exportProject(input.revision);
        const expectedVersion = input.date || input.definition ? 2 : 1;
        equal(saved.revision, input.revision, "ordinary export changed its source revision");
        equal(manifestVersion(new Uint8Array(saved.bytes)), expectedVersion, "legacy origin selected a different ordinary format");
      }
      await assertCurrent(client, input);
      equal(await client.observeOccurrence(), before, "ordinary legacy guard changed occurrence");
    };
    const assertV3Guard = async (client: any, input: any): Promise<void> => {
      const before = await client.observeOccurrence();
      await expectCode(() => client.exportProject(input.revision), "unsupported_project", input.revision);
      await assertCurrent(client, input);
      equal(await client.observeOccurrence(), before, "v3 downgrade guard changed occurrence");
    };
    const assertV3 = (buffer: ArrayBuffer, input: any): void => {
      const bytes = new Uint8Array(buffer); const view = new DataView(buffer); let offset = 8;
      check(new TextDecoder().decode(bytes.slice(0, 8)) === "TWDPROJ1", "selected export has wrong framing");
      const count = view.getUint32(offset, true); offset += 4; const files = new Map<string, Uint8Array>();
      for (let i = 0; i < count; i += 1) {
        const pathLength = view.getUint16(offset, true); offset += 2; const bodyLength = view.getUint32(offset, true); offset += 4;
        const path = new TextDecoder().decode(bytes.slice(offset, offset + pathLength)); offset += pathLength;
        check(!files.has(path), `selected export duplicated ${path}`);
        files.set(path, bytes.slice(offset, offset + bodyLength)); offset += bodyLength;
      }
      check(count === 19 && offset === bytes.length, "selected export file count/trailing data changed");
      const paths = ["manifest.json", "schemas.json", "definitions.json", ..."0123456789abcdef".split("").map((n) => `entities/${n}.jsonl`)];
      equal([...files.keys()], paths, "exact ordered selected-v3 paths changed");
      const read = (path: string): any => JSON.parse(new TextDecoder().decode(files.get(path)!));
      equal(read("manifest.json"), { format: "tachiko.roproj", format_version: 3,
        document: { id: literalIds.document, title: "Imported workbook" } }, "literal manifest changed");
      const schemas = read("schemas.json"); check(schemas.length === 1, "schema cardinality changed");
      equal(schemas[0], { id: literalIds.schema, key: "sheet_1", fields: literalIds.fields.map((id: string, i: number) => ({
        id, key: `column_${i + 1}`, field_type: { type: ["text", "text", "number", "number", input.date ? "date" : "text"][i] },
        required: false, constraint: { type: "none" },
      })) }, "literal schema type/key/required/constraint changed");
      const entities = [...files.entries()].filter(([path]) => path.startsWith("entities/"))
        .flatMap(([, body]) => new TextDecoder().decode(body).trim().split("\n").filter(Boolean).map((line) => JSON.parse(line)));
      check(entities.length === 2 && new Set(entities.map((row: any) => row.id)).size === 2, "entity count/identity changed");
      const rows = new Map(entities.map((row: any) => [row.id, row]));
      const values = [["PEN", "Stationery", input.quantity, 200, input.dateValue], ["NOTE", "Paper", 5, 200, "2026-09-27"]];
      literalIds.rows.forEach((id: string, ri: number) => {
        const row = rows.get(id) as any; check(row !== undefined, `missing entity ${id}`);
        equal([row.key, row.schema, Object.keys(row.fields).sort()], [`sheet_1_row_${ri + 1}`, literalIds.schema, [...literalIds.fields].sort()], "literal entity identity/schema/field set changed");
        literalIds.fields.forEach((field: string, fi: number) => equal(row.fields[field], {
          kind: fi < 2 ? "text" : fi < 4 ? "number" : input.date ? "date" : "text", value: values[ri]![fi],
        }, "literal canonical field kind/value changed"));
      });
      const defs = input.catalogue.length === 0 ? [] : [{ id: DEFINITION_ID,
        orders: { schema: literalIds.schema, lookup_key_field: literalIds.fields[0]!, quantity_field: literalIds.fields[2]! },
        products: { schema: literalIds.schema, key_field: literalIds.fields[0]!, category_field: literalIds.fields[1]!, price_field: literalIds.fields[3]! } }];
      equal(read("definitions.json"), defs, "literal definition catalogue/bindings changed");
    };

    const results = [];
    for (const profile of args.profiles) {
      const client = module.createExperimentalDesignerClient() as any;
      const requireMethod = (name: string): void => {
        if (typeof client[name] !== "function") throw new Error(`NOTRUN_MISSING_METHOD: Worker capability '${name}' is absent`);
      };
      for (const name of ["exportProjectV3", "exportProject", "importSpreadsheet", "createKeyedGroupedSum", "queryKeyedGroupedSum", "editDate", "editText", "editNumber", "trackerCommand", "openProject", "closeProject", "observeOccurrence", "bootstrap", "queryTable"]) requireMethod(name);
      try {
        const imported: any = await withUuid<any>("00000000-0000-4000-8000-000000000001", () => client.importSpreadsheet(
          new Uint8Array(args.csv).buffer, "csv", { delimiter: ",", header: true }, {
            column_types: [["text", "text", "number", "number", profile.date ? "date" : "text"]], extra_columns: [[]],
          },
        ));
        equal(imported.opened.bootstrap.revision, "resident/0", "import revision changed");
        const expectedInput = (dateValue: string, quantity: number, catalogue: string[], revision: string, occurrence: string) => ({
          date: profile.date, definition: catalogue.length === 1, dateValue, quantity, catalogue, revision, occurrence,
        });
        const ids = literalIds;
        let state = expectedInput("2026-09-26", 4, [], "resident/0", "00000000-0000-4000-8000-000000000001");
        await assertCurrent(client, state);
        await assertLegacyGuard(client, state);

        const dateChanged = profile.date
          ? await client.editDate(state.revision, { entity: ids.rows[0], field: ids.fields[4] }, "2026-09-28")
          : await client.editText(state.revision, { entity: ids.rows[0], field: ids.fields[4] }, "2026-09-28");
        state = { ...state, dateValue: "2026-09-28", revision: "resident/1" };
        equal(dateChanged.resulting_revision, state.revision, "explicit Date/Text import edit did not publish resident/1");
        await assertCurrent(client, state);
        await assertLegacyGuard(client, state);
        if (profile.definition) {
          const definition = await client.createKeyedGroupedSum(state.revision, {
            id: DEFINITION_ID, orders_schema: ids.schema, order_lookup_key_field: ids.fields[0],
            order_quantity_field: ids.fields[2], products_schema: ids.schema,
            product_key_field: ids.fields[0], product_category_field: ids.fields[1], product_price_field: ids.fields[3],
          });
          state = { ...state, definition: true, catalogue: [DEFINITION_ID], revision: "resident/2" };
          equal(definition.publication.resulting_revision, state.revision, "definition publication revision changed");
          await assertCurrent(client, state);
          await assertSummary(client, state.revision, 800);
          await assertLegacyGuard(client, state);
        }

        // Establish U=[Date/Text], R=[quantity] before successful selected export.
        const base = profile.definition ? 2 : 1;
        const quantityEdit = await client.editNumber(state.revision, { entity: ids.rows[0], field: ids.fields[2] }, "6");
        equal(quantityEdit.resulting_revision, `resident/${base + 1}`, "quantity edit revision changed");
        state = { ...state, quantity: 6, revision: `resident/${base + 1}` };
        await assertCurrent(client, state);
        await assertLegacyGuard(client, state);
        const quantityUndo = await client.trackerCommand({ type: "undo", expected_revision: state.revision });
        equal(quantityUndo.resulting_revision, `resident/${base + 2}`, "quantity Undo revision changed");
        state = { ...state, quantity: 4, revision: `resident/${base + 2}` };
        await assertCurrent(client, state);
        await assertLegacyGuard(client, state);

        const selected = await client.exportProjectV3(state.revision);
        equal(selected.revision, state.revision, "selected export returned a different source revision");
        assertV3(selected.bytes, state);
        await assertCurrent(client, state);
        await assertLegacyGuard(client, state);
        const staleSelected = async (): Promise<unknown> => client.exportProjectV3("resident/0");
        await expectCode(staleSelected, "stale_revision", state.revision);
        await assertCurrent(client, state);
        await assertLegacyGuard(client, state);

        // The successful export above must retain A and B. Replay the real
        // history before close, then install the captured q4 bytes fresh.
        let op = await client.trackerCommand({ type: "redo", expected_revision: state.revision });
        state = { ...state, quantity: 6, revision: `resident/${base + 3}` };
        equal(op.resulting_revision, state.revision, "Redo B revision changed"); await assertCurrent(client, state); await assertLegacyGuard(client, state);
        op = await client.trackerCommand({ type: "undo", expected_revision: state.revision });
        state = { ...state, quantity: 4, revision: `resident/${base + 4}` };
        equal(op.resulting_revision, state.revision, "Undo B revision changed"); await assertCurrent(client, state); await assertLegacyGuard(client, state);
        op = await client.trackerCommand({ type: "undo", expected_revision: state.revision });
        state = { ...state, dateValue: "2026-09-26", revision: `resident/${base + 5}` };
        equal(op.resulting_revision, state.revision, "Undo A revision changed"); await assertCurrent(client, state); await assertLegacyGuard(client, state);
        op = await client.trackerCommand({ type: "redo", expected_revision: state.revision });
        state = { ...state, dateValue: "2026-09-28", revision: `resident/${base + 6}` };
        equal(op.resulting_revision, state.revision, "Redo A revision changed"); await assertCurrent(client, state); await assertLegacyGuard(client, state);
        op = await client.trackerCommand({ type: "redo", expected_revision: state.revision });
        state = { ...state, quantity: 6, revision: `resident/${base + 7}` };
        equal(op.resulting_revision, state.revision, "Redo B revision changed"); await assertCurrent(client, state); await assertLegacyGuard(client, state);

        await client.closeProject();
        await noProjectOpen(client);
        const firstOpen = await withUuid("00000000-0000-4000-8000-000000000002", () => client.openProject(selected.bytes.slice(0)));
        state = expectedInput("2026-09-28", 4, profile.definition ? [DEFINITION_ID] : [], "resident/0", "00000000-0000-4000-8000-000000000002");
        assertOpened(firstOpen, state);
        await assertCurrent(client, state);
        await assertV3Guard(client, state);
        const firstReexport = await client.exportProjectV3("resident/0");
        assertV3(firstReexport.bytes, state);
        if (profile.definition) await assertSummary(client, state.revision, 800);
        const stale = async (): Promise<unknown> => client.exportProjectV3("resident/999");
        await expectCode(stale, "stale_revision", "resident/0");
        await assertCurrent(client, state);
        await assertV3Guard(client, state);

        // H at v3 origin: A, B, Undo B, X, then finish the fixed replay.
        const reopenedA = profile.date
          ? await client.editDate(state.revision, { entity: ids.rows[0], field: ids.fields[4] }, "2026-09-29")
          : await client.editText(state.revision, { entity: ids.rows[0], field: ids.fields[4] }, "2026-09-29");
        state = { ...state, dateValue: "2026-09-29", revision: "resident/1" };
        equal(reopenedA.resulting_revision, state.revision, "v3-origin A revision changed"); await assertCurrent(client, state); await assertV3Guard(client, state);
        const reopenedB = await client.editNumber(state.revision, { entity: ids.rows[0], field: ids.fields[2] }, "6");
        state = { ...state, quantity: 6, revision: "resident/2" };
        equal(reopenedB.resulting_revision, state.revision, "v3-origin B revision changed"); await assertCurrent(client, state); await assertV3Guard(client, state);
        if (profile.definition) await assertSummary(client, state.revision, 1200);
        op = await client.trackerCommand({ type: "undo", expected_revision: state.revision });
        state = { ...state, quantity: 4, revision: "resident/3" };
        equal(op.resulting_revision, state.revision, "v3-origin Undo B revision changed"); await assertCurrent(client, state); await assertV3Guard(client, state);
        if (profile.definition) await assertSummary(client, state.revision, 800);
        const historySave = await client.exportProjectV3(state.revision);
        equal(historySave.revision, state.revision, "history-state selected export changed revision");
        assertV3(historySave.bytes, state);
        await assertCurrent(client, state); await assertV3Guard(client, state);

        op = await client.trackerCommand({ type: "redo", expected_revision: state.revision });
        state = { ...state, quantity: 6, revision: "resident/4" };
        equal(op.resulting_revision, state.revision, "v3-origin Redo B revision changed"); await assertCurrent(client, state); await assertV3Guard(client, state);
        if (profile.definition) await assertSummary(client, state.revision, 1200);
        op = await client.trackerCommand({ type: "undo", expected_revision: state.revision });
        state = { ...state, quantity: 4, revision: "resident/5" };
        equal(op.resulting_revision, state.revision, "v3-origin Undo B revision changed"); await assertCurrent(client, state); await assertV3Guard(client, state);
        op = await client.trackerCommand({ type: "undo", expected_revision: state.revision });
        state = { ...state, dateValue: "2026-09-28", revision: "resident/6" };
        equal(op.resulting_revision, state.revision, "v3-origin Undo A revision changed"); await assertCurrent(client, state); await assertV3Guard(client, state);
        op = await client.trackerCommand({ type: "redo", expected_revision: state.revision });
        state = { ...state, dateValue: "2026-09-29", revision: "resident/7" };
        equal(op.resulting_revision, state.revision, "v3-origin Redo A revision changed"); await assertCurrent(client, state); await assertV3Guard(client, state);
        op = await client.trackerCommand({ type: "redo", expected_revision: state.revision });
        state = { ...state, quantity: 6, revision: "resident/8" };
        equal(op.resulting_revision, state.revision, "v3-origin Redo B revision changed"); await assertCurrent(client, state); await assertV3Guard(client, state);
        if (profile.definition) await assertSummary(client, state.revision, 1200);
        const finalSave = await client.exportProjectV3(state.revision);
        equal(finalSave.revision, state.revision, "final selected export revision changed");
        assertV3(finalSave.bytes, state); await assertCurrent(client, state); await assertV3Guard(client, state);

        await client.closeProject();
        await noProjectOpen(client);
        const secondOpen = await withUuid("00000000-0000-4000-8000-000000000003", () => client.openProject(finalSave.bytes.slice(0)));
        const final = expectedInput("2026-09-29", 6, profile.definition ? [DEFINITION_ID] : [], "resident/0", "00000000-0000-4000-8000-000000000003");
        assertOpened(secondOpen, final); await assertCurrent(client, final); await assertV3Guard(client, final);
        const secondReexport = await client.exportProjectV3("resident/0");
        assertV3(secondReexport.bytes, final);
        if (profile.definition) await assertSummary(client, state.revision, 1200);
        results.push(profile.name);
      } finally {
        await client.closeProject().catch(() => undefined);
        await client.close();
      }
    }
    return results;
  }, { csv: Array.from(mixedCsv), profiles: PROFILES });
  expect(results).toEqual(PROFILES.map((profile) => profile.name));
});

test("Worker preserves typed failures and resets origin after replacement or close", async ({ page }) => {
  const { dateOnly, dateDefinition, mixedCsv, constrainedText, constrainedNumber, boundaryAt, boundaryOver } = await verifyNativeCapture();
  const legacyNeither = new Uint8Array(await readFile(join(SEED_ROOT, "legacy/neither.twd")));
  const legacyDefinitionOnly = new Uint8Array(await readFile(join(SEED_ROOT, "legacy/definition-only.twd")));
  await page.goto("/");
  const result = await page.evaluate(async (input) => {
    const modulePath: string = "/vendor/tachiko/experimental-client.js";
    const module = await import(modulePath) as {
      createExperimentalDesignerClient: () => any;
      DesignerRuntimeError: new (...args: any[]) => Error & { failure: { code: string; current_revision: string } };
    };
    const client = module.createExperimentalDesignerClient() as any;
    const check = (condition: boolean, message: string): void => { if (!condition) throw new Error(message); };
    const stable = (value: any): any => Array.isArray(value) ? value.map(stable) :
      value !== null && typeof value === "object" ? Object.fromEntries(Object.keys(value).sort().map((key) => [key, stable(value[key])])) : value;
    const equal = (actual: unknown, expected: unknown, label: string): void => {
      if (JSON.stringify(stable(actual)) !== JSON.stringify(stable(expected))) throw new Error(`${label}: ${JSON.stringify(actual)}`);
    };
    const withUuid = async <T>(uuid: string, action: () => Promise<T>): Promise<T> => {
      const original = crypto.randomUUID;
      Object.defineProperty(crypto, "randomUUID", { configurable: true, value: () => uuid });
      try { return await action(); }
      finally { Object.defineProperty(crypto, "randomUUID", { configurable: true, value: original }); }
    };
    const expectClosed = async (): Promise<void> => {
      try { await client.observeOccurrence(); } catch (error) {
        if (!(error instanceof module.DesignerRuntimeError) || error.failure.code !== "no_project_open" || error.failure.current_revision !== "unavailable") throw new Error("Close did not expose typed no_project_open/unavailable");
        return;
      }
      throw new Error("Close left an observable occurrence before the next replacement");
    };
    for (const name of ["exportProjectV3", "openProject", "inspectProject", "exportProject", "newTable", "importSpreadsheet", "observeOccurrence", "closeProject", "trackerCommand", "editNumber", "editDate", "editText", "queryTable", "queryKeyedGroupedSum"]) {
      if (typeof client[name] !== "function") throw new Error(`NOTRUN_MISSING_METHOD: Worker capability '${name}' is absent`);
    }
    const expectCode = async (operation: () => Promise<unknown>, code: string, revision: string): Promise<void> => {
      try { await operation(); } catch (error) {
        if (!(error instanceof module.DesignerRuntimeError)) throw new Error(`Expected typed ${code}; got ${String(error)}`);
        if (error.failure.code !== code) throw new Error(`Expected typed ${code}; got ${error.failure.code}`);
        if (error.failure.current_revision !== revision) throw new Error(`Typed ${code} reported ${error.failure.current_revision}, expected ${revision}`);
        return;
      }
      throw new Error(`Expected typed ${code} refusal but operation succeeded`);
    };
    // Each open transfers and detaches its argument. Keep immutable masters
    // and provide a fresh transferable buffer at every call site.
    const v3Master = Uint8Array.from(input.v3);
    const legacyNeitherMaster = Uint8Array.from(input.legacyNeither);
    const constrainedMasters = [Uint8Array.from(input.constrainedText), Uint8Array.from(input.constrainedNumber)];
    const boundaryAtMaster = Uint8Array.from(input.boundaryAt);
    const boundaryOverMaster = Uint8Array.from(input.boundaryOver);
    const dateDefinitionMaster = Uint8Array.from(input.dateDefinition);
    const legacyDefinitionOnlyMaster = Uint8Array.from(input.legacyDefinitionOnly);
    const transferCopy = (master: Uint8Array): ArrayBuffer => {
      const copy = new Uint8Array(master.byteLength);
      copy.set(master);
      return copy.buffer;
    };
    const openFresh = (master: Uint8Array): Promise<any> => client.openProject(transferCopy(master));
    const assertDateProjection = async (opened: any, input: { dateValue: string; quantity: number; catalogue: string[]; revision: string; occurrence: string; identityUuid?: string; dateType?: boolean }): Promise<void> => {
      const prefix = `import_${input.identityUuid ?? "00000000-0000-4000-8000-000000000001"}_`;
      const fields = ["0003", "0004", "0005", "0006", "0007"].map((id) => `${prefix}${id}`);
      const rows = [`${prefix}0008`, `${prefix}0009`];
      const collection = { id: `${prefix}0002`, key: "sheet_1", entity_count: 2 };
      const values = [["PEN", "Stationery", input.quantity, 200, input.dateValue], ["NOTE", "Paper", 5, 200, "2026-09-27"]];
      const expected = {
        bootstrap: { title: "Imported workbook", revision: input.revision, default_collection: "sheet_1", collections: [collection], keyed_grouped_sum_definition_ids: input.catalogue },
        table: { revision: input.revision, collection, columns: fields.map((id, i) => ({ id, key: `column_${i + 1}`, field_type: ["text", "text", "number", "number", input.dateType === false ? "text" : "date"][i] })),
          rows: rows.map((id, ri) => ({ id, key: `sheet_1_row_${ri + 1}`, fields: fields.map((field, fi) => {
            const kind = fi < 2 ? "text" : fi < 4 ? "number" : input.dateType === false ? "text" : "date";
            return { target: { entity: id, field }, address: `sheet_1_row_${ri + 1}.column_${fi + 1}`, stored: { kind, value: values[ri]![fi] }, formula: null, calculated: null, diagnostics: [], editable_scalar: kind };
          }) })) },
      };
      equal(opened, expected, "full literal Date-only projection changed");
      equal(await client.observeOccurrence(), { scope: `designer-occurrence/${input.occurrence}/${prefix}0001`, revision: input.revision }, "literal Date-only occurrence changed");
      equal({ bootstrap: await client.bootstrap(), table: await client.queryTable("sheet_1") }, expected, "resident Date-only projection changed");
    };
    const residentState = async (collection = "sheet_1"): Promise<any> => ({ occurrence: await client.observeOccurrence(), bootstrap: await client.bootstrap(), table: await client.queryTable(collection) });
    const unchanged = async (before: any, label: string, collection = "sheet_1"): Promise<void> => {
      const after = await residentState(collection);
      if (JSON.stringify(after) !== JSON.stringify(before)) throw new Error(`${label} changed full occurrence/bootstrap/table state`);
    };
    const decodeEntries = (master: Uint8Array): Array<[string, Uint8Array]> => {
      const view = new DataView(master.buffer, master.byteOffset, master.byteLength);
      check(new TextDecoder().decode(master.slice(0, 8)) === "TWDPROJ1", "negative control is not framed v3");
      let offset = 8; const count = view.getUint32(offset, true); offset += 4;
      const entries: Array<[string, Uint8Array]> = [];
      for (let i = 0; i < count; i += 1) {
        const pathLength = view.getUint16(offset, true); offset += 2;
        const bodyLength = view.getUint32(offset, true); offset += 4;
        const path = new TextDecoder().decode(master.slice(offset, offset + pathLength)); offset += pathLength;
        entries.push([path, master.slice(offset, offset + bodyLength)]); offset += bodyLength;
      }
      check(offset === master.byteLength, "negative control has malformed framing");
      return entries;
    };
    const encodeEntries = (entries: Array<[string, Uint8Array]>): Uint8Array => {
      const encoder = new TextEncoder(); const encoded = entries.map(([path, body]) => [encoder.encode(path), body] as const);
      const length = 12 + encoded.reduce((sum, [path, body]) => sum + 6 + path.length + body.length, 0);
      const bytes = new Uint8Array(length); bytes.set(encoder.encode("TWDPROJ1"));
      const view = new DataView(bytes.buffer); view.setUint32(8, encoded.length, true); let offset = 12;
      for (const [path, body] of encoded) {
        view.setUint16(offset, path.length, true); offset += 2; view.setUint32(offset, body.length, true); offset += 4;
        bytes.set(path, offset); offset += path.length; bytes.set(body, offset); offset += body.length;
      }
      return bytes;
    };
    const boundaryPrefix = "import_00000000-0000-4000-8000-000000000001_";
    const boundaryEntityIds = [`${boundaryPrefix}0008`, `${boundaryPrefix}0009`];
    const boundaryFieldIds = ["0003", "0004", "0005", "0006", "0007"].map((id) => `${boundaryPrefix}${id}`);
    const boundaryProjection = (revision: string, ids: string[], dateValue = "2026-09-28", quantity = 4): any => {
      const collection = { id: `${boundaryPrefix}0002`, key: "sheet_1", entity_count: 2 };
      const columns = boundaryFieldIds.map((id, i) => ({ id, key: `column_${i + 1}`, field_type: ["text", "text", "number", "number", "date"][i] }));
      const sourceRows = [
        ["PEN", "Stationery", quantity, 200, dateValue],
        ["NOTE", "Paper", 5, 200, "2026-09-27"],
      ];
      const rows = boundaryEntityIds.map((entity, ri) => ({
        id: entity, key: `sheet_1_row_${ri + 1}`,
        fields: boundaryFieldIds.map((field, fi) => {
          const kind = fi < 2 ? "text" : fi < 4 ? "number" : "date";
          return { target: { entity, field }, address: `sheet_1_row_${ri + 1}.column_${fi + 1}`,
            stored: { kind, value: sourceRows[ri]![fi] }, formula: null, calculated: null, diagnostics: [], editable_scalar: kind };
        }),
      }));
      return {
        bootstrap: { title: "Imported workbook", revision, default_collection: "sheet_1", collections: [collection], keyed_grouped_sum_definition_ids: ids },
        table: { revision, collection, columns, rows },
      };
    };
    const rustProjectionBytes = (projection: any): number => new TextEncoder().encode(
      JSON.stringify(projection).replace(/("value":)(-?\d+)([,}])/g, "$1$2.0$3"),
    ).byteLength;
    const exactBoundaryIds = (target = 65536): string[] => {
      const prefixes = Array.from({ length: 16 }, (_, i) => `v3-budget-${String(i).padStart(2, "0")}-`);
      const ids = prefixes.slice(0, 13).map((prefix) => `${prefix}${"x".repeat(4096 - prefix.length)}`);
      ids.push(...prefixes.slice(13));
      let remaining = target - rustProjectionBytes(boundaryProjection("resident/0", ids));
      for (let i = 13; i < 16; i += 1) {
        const extra = Math.min(remaining, 4096 - prefixes[i]!.length);
        ids[i] = `${ids[i]!}${"x".repeat(extra)}`;
        remaining -= extra;
      }
      if (remaining !== 0 || rustProjectionBytes(boundaryProjection("resident/0", ids)) !== target) throw new Error(`Independent fresh resident/0 catalogue oracle does not equal ${target} bytes`);
      return ids;
    };
    const assertBoundaryV3 = (buffer: ArrayBuffer, dateValue: string, quantity: number, ids: string[]): void => {
      const entries = decodeEntries(new Uint8Array(buffer));
      const expectedPaths = ["manifest.json", "schemas.json", "definitions.json", ..."0123456789abcdef".split("").map((n) => `entities/${n}.jsonl`)];
      equal(entries.map(([path]) => path), expectedPaths, "selected boundary-v3 path order changed");
      const read = (path: string): any => JSON.parse(new TextDecoder().decode(entries.find(([name]) => name === path)![1]));
      const prefix = "import_00000000-0000-4000-8000-000000000001_";
      const fields = ["0003", "0004", "0005", "0006", "0007"].map((id) => `${prefix}${id}`);
      equal(read("manifest.json"), { format: "tachiko.roproj", format_version: 3, document: { id: `${prefix}0001`, title: "Imported workbook" } }, "boundary manifest changed");
      equal(read("schemas.json"), [{ id: `${prefix}0002`, key: "sheet_1", fields: fields.map((id, i) => ({ id, key: `column_${i + 1}`, field_type: { type: ["text", "text", "number", "number", "date"][i] }, required: false, constraint: { type: "none" } })) }], "boundary schema/type/flags changed");
      equal(read("definitions.json"), ids.map((id) => ({ id,
        orders: { schema: `${prefix}0002`, lookup_key_field: fields[0]!, quantity_field: fields[2]! },
        products: { schema: `${prefix}0002`, key_field: fields[0]!, category_field: fields[1]!, price_field: fields[3]! },
      })), "boundary definition catalogue/bindings changed");
      const entities = entries.filter(([path]) => path.startsWith("entities/"))
        .flatMap(([, body]) => new TextDecoder().decode(body).split("\n").filter(Boolean).map((line) => JSON.parse(line)));
      equal(entities.sort((left: any, right: any) => left.id.localeCompare(right.id)), [
        { id: `${prefix}0008`, key: "sheet_1_row_1", schema: `${prefix}0002`, fields: {
          [fields[0]!]: { kind: "text", value: "PEN" }, [fields[1]!]: { kind: "text", value: "Stationery" },
          [fields[2]!]: { kind: "number", value: quantity }, [fields[3]!]: { kind: "number", value: 200 }, [fields[4]!]: { kind: "date", value: dateValue },
        } },
        { id: `${prefix}0009`, key: "sheet_1_row_2", schema: `${prefix}0002`, fields: {
          [fields[0]!]: { kind: "text", value: "NOTE" }, [fields[1]!]: { kind: "text", value: "Paper" },
          [fields[2]!]: { kind: "number", value: 5 }, [fields[3]!]: { kind: "number", value: 200 }, [fields[4]!]: { kind: "date", value: "2026-09-27" },
        } },
      ], "boundary entity identities, keys, typed values, or bindings changed");
    };
    const matrixUuid = "00000000-0000-4000-8000-000000000100";
    const v3Open = await withUuid(matrixUuid, () => openFresh(dateDefinitionMaster));
    await assertDateProjection(v3Open, { dateValue: "2026-09-28", quantity: 4, catalogue: [DEFINITION_ID], revision: "resident/0", occurrence: matrixUuid });
    await expectCode(() => client.exportProject(v3Open.bootstrap.revision), "unsupported_project", v3Open.bootstrap.revision);
    await client.editDate("resident/0", {
      entity: "import_00000000-0000-4000-8000-000000000001_0008",
      field: "import_00000000-0000-4000-8000-000000000001_0007",
    }, "2026-09-29");
    await client.editNumber("resident/1", {
      entity: "import_00000000-0000-4000-8000-000000000001_0008",
      field: "import_00000000-0000-4000-8000-000000000001_0005",
    }, "6");
    await client.trackerCommand({ type: "undo", expected_revision: "resident/2" });
    let occurrence = await client.observeOccurrence();
    if (occurrence.revision !== "resident/3") throw new Error("Worker negative matrix did not begin with original Date Undo and quantity Redo entries");
    const ordinaryV3: Array<{ label: string; code: string; bytes: Uint8Array }> = [{ label: "well-formed unsupported version", code: "unsupported_project", bytes: (() => {
      const entries = decodeEntries(dateDefinitionMaster);
      const manifestIndex = entries.findIndex(([path]) => path === "manifest.json");
      if (manifestIndex < 0) throw new Error("version candidate has no manifest.json");
      const manifest = new TextDecoder().decode(entries[manifestIndex]![1]);
      if (manifest.split('"format_version": 3').length !== 2) throw new Error("well-formed unsupported-version mutator did not match once");
      entries[manifestIndex] = ["manifest.json", new TextEncoder().encode(manifest.replace('"format_version": 3', '"format_version": 99'))];
      return encodeEntries(entries);
    })() }];
    const baseEntries = decodeEntries(dateDefinitionMaster);
    ordinaryV3.push(
      { label: "truncated header", code: "invalid_project", bytes: v3Master.slice(0, 8) },
      { label: "truncated count", code: "invalid_project", bytes: v3Master.slice(0, 10) },
      { label: "truncated payload", code: "invalid_project", bytes: v3Master.slice(0, -1) },
      { label: "trailing byte", code: "invalid_project", bytes: Uint8Array.from([...v3Master, 0]) },
      { label: "wrong file order", code: "invalid_project", bytes: encodeEntries([baseEntries[1]!, baseEntries[0]!, ...baseEntries.slice(2)]) },
      { label: "duplicate path", code: "invalid_project", bytes: encodeEntries([[baseEntries[0]![0], baseEntries[0]![1]], [baseEntries[0]![0], baseEntries[1]![1]], ...baseEntries.slice(2)]) },
      { label: "missing entity path", code: "invalid_project", bytes: encodeEntries(baseEntries.filter(([path]) => path !== "entities/f.jsonl")) },
      { label: "extra path", code: "invalid_project", bytes: encodeEntries([...baseEntries, ["unexpected.txt", new Uint8Array([1])]]) },
      { label: "unsafe path", code: "invalid_project", bytes: encodeEntries([["../manifest.json", baseEntries[0]![1]], ...baseEntries.slice(1)]) },
      { label: "absolute path", code: "invalid_project", bytes: encodeEntries([["/manifest.json", baseEntries[0]![1]], ...baseEntries.slice(1)]) },
      { label: "malformed representation", code: "invalid_project", bytes: encodeEntries(baseEntries.map(([path, body]) => (path === "schemas.json" ? [path, new TextEncoder().encode("{broken")] : [path, body]) as [string, Uint8Array])) },
    );
    const definitionEntries = decodeEntries(dateDefinitionMaster);
    const definitionIndex = definitionEntries.findIndex(([path]) => path === "definitions.json");
    if (definitionIndex < 0) throw new Error("Native fixture has no definitions.json candidate");
    const canonicalDefinitions = new TextDecoder().decode(definitionEntries[definitionIndex]![1]);
    const changedDefinition = (label: string, from: string, to: string): void => {
      if (canonicalDefinitions.split(from).length !== 2) throw new Error(`Acceptance mutator for ${label} did not match exactly once`);
      const entries = definitionEntries.map(([path, body]) => [path, body] as [string, Uint8Array]);
      entries[definitionIndex] = ["definitions.json", new TextEncoder().encode(canonicalDefinitions.replace(from, to))];
      ordinaryV3.push({ label, code: "invalid_project", bytes: encodeEntries(entries) });
    };
    changedDefinition("empty definition id", '"id": "acceptance-orders-summary"', '"id": ""');
    changedDefinition("missing definition id", '    "id": "acceptance-orders-summary",\n', "");
    changedDefinition("missing definition schema reference", '"orders": {\n      "schema": "import_00000000-0000-4000-8000-000000000001_0002"', '"orders": {\n      "schema": "missing-schema"');
    changedDefinition("missing definition field reference", '"lookup_key_field": "import_00000000-0000-4000-8000-000000000001_0003"', '"lookup_key_field": "missing-field"');
    changedDefinition("wrong bound definition field type", '"quantity_field": "import_00000000-0000-4000-8000-000000000001_0005"', '"quantity_field": "import_00000000-0000-4000-8000-000000000001_0007"');
    const missingQuantityBinding = canonicalDefinitions.replace('      "lookup_key_field": "import_00000000-0000-4000-8000-000000000001_0003",\n      "quantity_field": "import_00000000-0000-4000-8000-000000000001_0005"', '      "lookup_key_field": "import_00000000-0000-4000-8000-000000000001_0003"');
    if (missingQuantityBinding === canonicalDefinitions || JSON.parse(missingQuantityBinding).length !== 1) throw new Error("missing binding mutator did not produce one valid definition array");
    const missingBindingEntries = definitionEntries.map(([path, body]) => [path, body] as [string, Uint8Array]);
    missingBindingEntries[definitionIndex] = ["definitions.json", new TextEncoder().encode(missingQuantityBinding)];
    ordinaryV3.push({ label: "missing definition binding member", code: "invalid_project", bytes: encodeEntries(missingBindingEntries) });
    const duplicateDefinition = `${canonicalDefinitions.trimEnd().slice(0, -1)},${canonicalDefinitions.trimEnd().slice(1, -1)}]\n`;
    const duplicateEntries = definitionEntries.map(([path, body]) => [path, body] as [string, Uint8Array]);
    duplicateEntries[definitionIndex] = ["definitions.json", new TextEncoder().encode(duplicateDefinition)];
    ordinaryV3.push({ label: "duplicate definition record", code: "invalid_project", bytes: encodeEntries(duplicateEntries) });
    const unknownCatalogueEntries = definitionEntries.map(([path, body]) => [path, body] as [string, Uint8Array]);
    unknownCatalogueEntries[definitionIndex] = ["definitions.json", new TextEncoder().encode('[\n  "not-a-definition-record"\n]\n')];
    ordinaryV3.push({ label: "unknown catalogue item shape", code: "invalid_project", bytes: encodeEntries(unknownCatalogueEntries) });
    const resourceEntries = decodeEntries(dateDefinitionMaster).map(([path, body]) => [path, body] as [string, Uint8Array]);
    const manifestIndex = resourceEntries.findIndex(([path]) => path === "manifest.json");
    if (manifestIndex < 0) throw new Error("Native fixture has no manifest.json");
    const canonicalManifest = new TextDecoder().decode(resourceEntries[manifestIndex]![1]);
    const titleCandidate = (length: number): Uint8Array => {
      const source = '"title": "Imported workbook"';
      if (canonicalManifest.split(source).length !== 2) throw new Error("Canonical manifest title anchor did not match exactly once");
      const entries = resourceEntries.map(([path, body]) => [path, body] as [string, Uint8Array]);
      entries[manifestIndex] = ["manifest.json", new TextEncoder().encode(canonicalManifest.replace(source, `"title": "${"x".repeat(length)}"`))];
      return encodeEntries(entries);
    };
    const titleAtLimit = titleCandidate(4096);
    const titleOverLimit = titleCandidate(4097);
    ordinaryV3.push({ label: "over title resource profile", code: "unsupported_project", bytes: titleOverLimit });
    const exactLimit = new Uint8Array(64 * 1024 * 1024);
    const overLimit = new Uint8Array(64 * 1024 * 1024 + 1);
    let matrixUuidCounter = 200;
    const nextMatrixUuid = (): string => `00000000-0000-4000-8000-${(matrixUuidCounter++).toString(16).padStart(12, "0")}`;
    const matrixProfile = (marked: boolean, occurrenceUuid: string, dateValue: string, quantity: number, revision: string): any => ({
      dateValue, quantity, catalogue: [DEFINITION_ID], revision, occurrence: occurrenceUuid, dateType: marked,
    });
    const checkMatrixResident = async (marked: boolean, occurrenceUuid: string, dateValue: string, quantity: number, revision: string): Promise<void> => {
      const profile = matrixProfile(marked, occurrenceUuid, dateValue, quantity, revision);
      await assertDateProjection({ bootstrap: await client.bootstrap(), table: await client.queryTable("sheet_1") }, profile);
      const summary = await client.queryKeyedGroupedSum(DEFINITION_ID);
      if (summary.revision !== revision || JSON.stringify(summary.groups.map((group: any) => [group.category, group.value]).sort()) !== JSON.stringify([["Paper", 1000], ["Stationery", quantity === 6 ? 1200 : 800]].sort())) throw new Error(`T2 literal catalogue summary changed at ${revision}`);
      if (marked) await expectCode(() => client.exportProject(revision), "unsupported_project", revision);
      else {
        const saved = await client.exportProject(revision);
        const manifest = JSON.parse(new TextDecoder().decode(decodeEntries(new Uint8Array(saved.bytes)).find(([path]) => path === "manifest.json")![1]));
        if (saved.revision !== revision || manifest.format_version !== 2) throw new Error(`legacy T2 origin changed at ${revision}`);
      }
    };
    const prepareMatrixResident = async (marked: boolean, occurrenceUuid: string): Promise<void> => {
      await client.closeProject();
      await expectClosed();
      const opened = marked
        ? await withUuid(occurrenceUuid, () => openFresh(dateDefinitionMaster))
        : await withUuid(occurrenceUuid, () => client.openProject(transferCopy(legacyDefinitionOnlyMaster)));
      await assertDateProjection(opened, matrixProfile(marked, occurrenceUuid, "2026-09-28", 4, "resident/0"));
      if (marked) await expectCode(() => client.exportProject("resident/0"), "unsupported_project", "resident/0");
      else {
        const saved = await client.exportProject("resident/0");
        const manifest = JSON.parse(new TextDecoder().decode(decodeEntries(new Uint8Array(saved.bytes)).find(([path]) => path === "manifest.json")![1]));
        if (manifest.format_version !== 2) throw new Error("definition-only resident lost its ordinary v2 origin");
      }
      const field = boundaryFieldIds[4]; const entity = boundaryEntityIds[0];
      const a = marked
        ? await client.editDate("resident/0", { entity, field }, "2026-09-29")
        : await client.editText("resident/0", { entity, field }, "2026-09-29");
      if (a.resulting_revision !== "resident/1") throw new Error("T2 Date/Text A revision changed");
      await checkMatrixResident(marked, occurrenceUuid, "2026-09-29", 4, "resident/1");
      const b = await client.editNumber("resident/1", { entity, field: boundaryFieldIds[2] }, "6");
      if (b.resulting_revision !== "resident/2") throw new Error("T2 quantity B revision changed");
      await checkMatrixResident(marked, occurrenceUuid, "2026-09-29", 6, "resident/2");
      await client.trackerCommand({ type: "undo", expected_revision: "resident/2" });
      await checkMatrixResident(marked, occurrenceUuid, "2026-09-29", 4, "resident/3");
    };
    const replayMatrixHistory = async (marked: boolean, occurrenceUuid: string): Promise<void> => {
      for (const [type, dateValue, quantity, revision] of [
        ["redo", "2026-09-29", 6, "resident/4"], ["undo", "2026-09-29", 4, "resident/5"],
        ["undo", "2026-09-28", 4, "resident/6"], ["redo", "2026-09-29", 4, "resident/7"],
        ["redo", "2026-09-29", 6, "resident/8"],
      ] as const) {
        const operation = await client.trackerCommand({ type, expected_revision: `resident/${Number(revision.split("/")[1]) - 1}` });
        if (operation.resulting_revision !== revision) throw new Error(`T2 ${marked ? "v3" : "legacy"} history lost ${type}`);
        await checkMatrixResident(marked, occurrenceUuid, dateValue, quantity, revision);
      }
    };
    const matrixCandidates: Array<{ label: string; code: string; bytes: Uint8Array; revision?: string }> = [
      ...ordinaryV3.map((candidate) => ({ label: candidate.label, code: candidate.code, bytes: candidate.bytes })),
      ...constrainedMasters.map((bytes, index) => ({ label: `valid constrained v3 ${index}`, code: "unsupported_project", bytes })),
      { label: "exact transfer limit", code: "invalid_project", bytes: exactLimit },
      { label: "over transfer limit", code: "request_too_large", revision: "unavailable", bytes: overLimit },
      { label: "malformed legacy candidate", code: "invalid_project", bytes: new TextEncoder().encode("TWDPROJ2not-json") },
      { label: "65,537-byte projection", code: "query_too_large", bytes: boundaryOverMaster },
    ];
    for (const candidate of matrixCandidates) {
      for (const marked of [true, false]) {
        const inspectUuid = nextMatrixUuid();
        await prepareMatrixResident(marked, inspectUuid);
        const beforeInspect = await residentState();
        await expectCode(() => client.inspectProject(transferCopy(candidate.bytes)), candidate.code, candidate.revision ?? "resident/3");
        await unchanged(beforeInspect, `${candidate.label} Inspect / ${marked ? "v3" : "legacy"}`);
        await checkMatrixResident(marked, inspectUuid, "2026-09-29", 4, "resident/3");
        await replayMatrixHistory(marked, inspectUuid);

        const openUuid = nextMatrixUuid();
        await prepareMatrixResident(marked, openUuid);
        const beforeOpen = await residentState();
        await expectCode(() => client.openProject(transferCopy(candidate.bytes)), candidate.code, candidate.revision ?? "resident/3");
        await unchanged(beforeOpen, `${candidate.label} Open / ${marked ? "v3" : "legacy"}`);
        await checkMatrixResident(marked, openUuid, "2026-09-29", 4, "resident/3");
        await replayMatrixHistory(marked, openUuid);
      }
    }
    occurrence = await client.observeOccurrence();
    const titleExpected = boundaryProjection("resident/0", [DEFINITION_ID]);
    titleExpected.bootstrap.title = "x".repeat(4096);
    if (rustProjectionBytes(titleExpected) >= 65536) throw new Error("canonical 4,096-byte title control exceeds the unrelated projection bound");
    for (const marked of [true, false]) {
      const titleOriginUuid = nextMatrixUuid();
      await prepareMatrixResident(marked, titleOriginUuid);
      const beforeTitleInspect = await residentState();
      equal(await client.inspectProject(transferCopy(titleAtLimit)), titleExpected, "canonical 4,096-byte title Inspect differs from independent projection");
      await unchanged(beforeTitleInspect, `canonical 4,096-byte title Inspect / ${marked ? "v3" : "legacy"}`);
      await checkMatrixResident(marked, titleOriginUuid, "2026-09-29", 4, "resident/3");
      await replayMatrixHistory(marked, titleOriginUuid);
    }
    const titleReplacementOriginUuid = nextMatrixUuid();
    await prepareMatrixResident(true, titleReplacementOriginUuid);
    const titleOpen = await withUuid("00000000-0000-4000-8000-000000000012", () => openFresh(titleAtLimit));
    equal(titleOpen, titleExpected, "canonical 4,096-byte title Open differs from independent projection");
    equal(await residentState(), { occurrence: { scope: "designer-occurrence/00000000-0000-4000-8000-000000000012/import_00000000-0000-4000-8000-000000000001_0001", revision: "resident/0" }, ...titleExpected }, "canonical title Open did not install the full candidate");
    await expectCode(() => client.exportProject("resident/0"), "unsupported_project", "resident/0");
    for (const type of ["undo", "redo"] as const) {
      const before = await residentState();
      await expectCode(() => client.trackerCommand({ type, expected_revision: "resident/0" }), "invalid_tracker_operation", "resident/0");
      await unchanged(before, `canonical title Open empty ${type}`);
    }
    const boundaryIds = exactBoundaryIds();
    const freshBoundaryExpected = boundaryProjection("resident/0", boundaryIds);
    if (rustProjectionBytes(freshBoundaryExpected) !== 65536) throw new Error("fresh admission oracle is not exactly 65,536 bytes");
    for (const marked of [true, false]) {
      const atLimitInspectUuid = nextMatrixUuid();
      await prepareMatrixResident(marked, atLimitInspectUuid);
      const beforeAtLimitInspect = await residentState();
      const freshBoundary = await client.inspectProject(transferCopy(boundaryAtMaster));
      equal(freshBoundary, freshBoundaryExpected, "65,536-byte fresh Inspect omitted or changed the literal catalogue/table projection");
      await unchanged(beforeAtLimitInspect, `successful at-limit Inspect / ${marked ? "v3" : "legacy"}`);
      await checkMatrixResident(marked, atLimitInspectUuid, "2026-09-29", 4, "resident/3");
      await replayMatrixHistory(marked, atLimitInspectUuid);
    }
    await client.closeProject();
    await expectClosed();
    const dateImportUuid = "00000000-0000-4000-8000-000000000001";
    const dateImport: any = await withUuid<any>(dateImportUuid, () => client.importSpreadsheet(
      new Uint8Array(input.mixedCsv).buffer, "csv", { delimiter: ",", header: true }, {
        column_types: [["text", "text", "number", "number", "date"]], extra_columns: [[]],
      },
    ));
    await assertDateProjection(dateImport.opened, { dateValue: "2026-09-26", quantity: 4, catalogue: [], revision: "resident/0", occurrence: dateImportUuid, identityUuid: dateImportUuid });
    const importedV3 = await client.exportProjectV3("resident/0");
    const importedEntries = decodeEntries(new Uint8Array(importedV3.bytes));
    if (JSON.parse(new TextDecoder().decode(importedEntries.find(([path]) => path === "definitions.json")![1])).length !== 0) throw new Error("T3 Date-only seed unexpectedly has definitions");
    await client.closeProject();
    await expectClosed();
    const t3OpenUuid = "00000000-0000-4000-8000-000000000002";
    const t3Opened = await withUuid(t3OpenUuid, () => client.openProject(importedV3.bytes.slice(0)));
    await assertDateProjection(t3Opened, { dateValue: "2026-09-26", quantity: 4, catalogue: [], revision: "resident/0", occurrence: t3OpenUuid });
    await expectCode(() => client.exportProject("resident/0"), "unsupported_project", "resident/0");
    for (let index = 0; index < boundaryIds.length; index += 1) {
      const id = boundaryIds[index];
      const publication = await client.createKeyedGroupedSum(`resident/${index}`, {
        id, orders_schema: `${boundaryPrefix}0002`, order_lookup_key_field: boundaryFieldIds[0],
        order_quantity_field: boundaryFieldIds[2], products_schema: `${boundaryPrefix}0002`,
        product_key_field: boundaryFieldIds[0], product_category_field: boundaryFieldIds[1], price_field: boundaryFieldIds[3],
      });
      if (publication.publication.resulting_revision !== `resident/${index + 1}`) throw new Error(`Definition publication ${id} did not produce resident/${index + 1}`);
      await expectCode(() => client.exportProject(`resident/${index + 1}`), "unsupported_project", `resident/${index + 1}`);
    }
    const afterDefinitions = boundaryProjection("resident/16", boundaryIds, "2026-09-26", 4);
    equal({ bootstrap: await client.bootstrap(), table: await client.queryTable("sheet_1") }, afterDefinitions, "real Worker publication changed the literal T3 resident");
    for (const id of boundaryIds) {
      const summary = await client.queryKeyedGroupedSum(id);
      if (summary.definition_id !== id || JSON.stringify(summary.groups.map((group: any) => [group.category, group.value]).sort()) !== JSON.stringify([["Paper", 1000], ["Stationery", 800]].sort())) throw new Error(`Fresh at-limit definition ${id} has incomplete/incorrect query result`);
    }
    await client.editDate("resident/16", { entity: boundaryEntityIds[0], field: boundaryFieldIds[4] }, "2026-09-28");
    await client.editNumber("resident/17", { entity: boundaryEntityIds[0], field: boundaryFieldIds[2] }, "6");
    await client.trackerCommand({ type: "undo", expected_revision: "resident/18" });
    const atLimitResident = await residentState();
    const atLimitExpected = boundaryProjection("resident/19", boundaryIds, "2026-09-28", 4);
    if (rustProjectionBytes(atLimitExpected) !== 65538) throw new Error("live resident/3 projection does not independently measure exactly two bytes above fresh admission");
    const wrapperBytes = rustProjectionBytes(atLimitExpected) - rustProjectionBytes(atLimitExpected.bootstrap) - rustProjectionBytes(atLimitExpected.table);
    if (wrapperBytes !== 23 || rustProjectionBytes(atLimitExpected.bootstrap) >= 65536 || rustProjectionBytes(atLimitExpected.table) >= 65536) throw new Error("Separate Worker Bootstrap/QueryTable response budgets or the 23-byte composite wrapper changed");
    if (JSON.stringify({ bootstrap: atLimitResident.bootstrap, table: atLimitResident.table }) !== JSON.stringify(atLimitExpected)) throw new Error("Live resident/3 boundary state differs from full independent projection");
    const boundaryOccurrence = await client.observeOccurrence();
    equal(boundaryOccurrence, { scope: `designer-occurrence/${t3OpenUuid}/import_${dateImportUuid}_0001`, revision: "resident/19" }, "at-limit live occurrence/revision changed");
    const boundarySave = await client.exportProjectV3("resident/19");
    if (boundarySave.revision !== "resident/19") throw new Error("Worker selected boundary export changed the source revision");
    assertBoundaryV3(boundarySave.bytes, "2026-09-28", 4, boundaryIds);
    equal(await residentState(), { occurrence: boundaryOccurrence, ...atLimitExpected }, "successful Worker boundary export changed full live state");
    await expectCode(() => client.exportProject("resident/19"), "unsupported_project", "resident/19");
    for (const [type, dateValue, quantity, revision] of [
      ["redo", "2026-09-28", 6, "resident/20"], ["undo", "2026-09-28", 4, "resident/21"],
      ["undo", "2026-09-26", 4, "resident/22"], ["redo", "2026-09-28", 4, "resident/23"],
      ["redo", "2026-09-28", 6, "resident/24"],
    ] as const) {
      const liveOccurrence = await client.observeOccurrence();
      const operation = await client.trackerCommand({ type, expected_revision: liveOccurrence.revision });
      if (operation.resulting_revision !== revision) throw new Error(`History ${type} entry was lost after 65,537-byte failure`);
      const expected = boundaryProjection(revision, boundaryIds, dateValue, quantity);
      const current = await residentState();
      if (JSON.stringify({ bootstrap: current.bootstrap, table: current.table }) !== JSON.stringify(expected)) throw new Error(`History ${type} changed full boundary projection`);
      for (const id of boundaryIds) {
        const summary = await client.queryKeyedGroupedSum(id);
        const total = quantity === 4 ? 800 : 1200;
        if (JSON.stringify(summary.groups.map((group: any) => [group.category, group.value]).sort()) !== JSON.stringify([["Paper", 1000], ["Stationery", total]].sort())) throw new Error(`History ${type} changed catalogue result ${id}`);
      }
      await expectCode(() => client.exportProject(revision), "unsupported_project", revision);
    }
    await client.closeProject();
    await expectClosed();
    const boundaryReopenUuid = "00000000-0000-4000-8000-000000000013";
    const boundaryReopened = await withUuid(boundaryReopenUuid, () => client.openProject(boundarySave.bytes.slice(0)));
    const boundaryFreshExpected = boundaryProjection("resident/0", boundaryIds, "2026-09-28", 4);
    if (rustProjectionBytes(boundaryFreshExpected) !== 65536) throw new Error("fresh resident/0 close/reopen boundary stopped measuring 65,536 bytes");
    equal(boundaryReopened, boundaryFreshExpected, "fresh at-limit reopen changed full expected projection");
    equal(await residentState(), { occurrence: { scope: `designer-occurrence/${boundaryReopenUuid}/import_${dateImportUuid}_0001`, revision: "resident/0" }, ...boundaryFreshExpected }, "fresh at-limit resident differs after close/reopen");
    const boundaryReexport = await client.exportProjectV3("resident/0");
    assertBoundaryV3(boundaryReexport.bytes, "2026-09-28", 4, boundaryIds);
    for (const id of boundaryIds) {
      const summary = await client.queryKeyedGroupedSum(id);
      if (JSON.stringify(summary.groups.map((group: any) => [group.category, group.value]).sort()) !== JSON.stringify([["Paper", 1000], ["Stationery", 800]].sort())) throw new Error(`Fresh reopened catalogue result changed for ${id}`);
    }
    await expectCode(() => client.exportProject("resident/0"), "unsupported_project", "resident/0");
    for (const type of ["undo", "redo"] as const) await expectCode(() => client.trackerCommand({ type, expected_revision: "resident/0" }), "invalid_tracker_operation", "resident/0");
    const afterBoundaryReplay = await client.observeOccurrence();
    await expectCode(() => client.exportProject(afterBoundaryReplay.revision), "unsupported_project", afterBoundaryReplay.revision);
    // Construct a live 65,537-byte fresh-admission catalogue on Worker. The
    // resident is made through real publications and remains only 2 bytes
    // larger at resident/19 due to the revision token.
    const overIds = exactBoundaryIds(65537);
    assertBoundaryV3(transferCopy(boundaryOverMaster), "2026-09-28", 4, overIds);
    await client.closeProject();
    await expectClosed();
    const overSeedUuid = "00000000-0000-4000-8000-000000000014";
    const overSeed = await withUuid(overSeedUuid, () => client.openProject(importedV3.bytes.slice(0)));
    await assertDateProjection(overSeed, { dateValue: "2026-09-26", quantity: 4, catalogue: [], revision: "resident/0", occurrence: overSeedUuid });
    for (let index = 0; index < overIds.length; index += 1) {
      const publication = await client.createKeyedGroupedSum(`resident/${index}`, {
        id: overIds[index], orders_schema: `${boundaryPrefix}0002`, order_lookup_key_field: boundaryFieldIds[0],
        order_quantity_field: boundaryFieldIds[2], products_schema: `${boundaryPrefix}0002`,
        product_key_field: boundaryFieldIds[0], product_category_field: boundaryFieldIds[1], price_field: boundaryFieldIds[3],
      });
      if (publication.publication.resulting_revision !== `resident/${index + 1}`) throw new Error("over-budget catalogue publication revision changed");
    }
    await client.editDate("resident/16", { entity: boundaryEntityIds[0], field: boundaryFieldIds[4] }, "2026-09-28");
    await client.editNumber("resident/17", { entity: boundaryEntityIds[0], field: boundaryFieldIds[2] }, "6");
    await client.trackerCommand({ type: "undo", expected_revision: "resident/18" });
    const overLiveExpected = boundaryProjection("resident/19", overIds, "2026-09-28", 4);
    if (rustProjectionBytes(boundaryProjection("resident/0", overIds)) !== 65537 || rustProjectionBytes(overLiveExpected) !== 65539) throw new Error("live over-budget catalogue sizing did not independently equal fresh 65,537/live 65,539 bytes");
    equal({ bootstrap: await client.bootstrap(), table: await client.queryTable("sheet_1") }, overLiveExpected, "live over-budget catalogue full projection changed");
    await expectCode(() => client.exportProjectV3("resident/19"), "query_too_large", "resident/19");
    equal({ occurrence: await client.observeOccurrence(), bootstrap: await client.bootstrap(), table: await client.queryTable("sheet_1") }, {
      occurrence: { scope: `designer-occurrence/${overSeedUuid}/import_${dateImportUuid}_0001`, revision: "resident/19" }, ...overLiveExpected,
    }, "failed selected Worker export changed live over-budget state");
    await expectCode(() => client.exportProject("resident/19"), "unsupported_project", "resident/19");
    for (const [type, dateValue, quantity, revision] of [
      ["redo", "2026-09-28", 6, "resident/20"], ["undo", "2026-09-28", 4, "resident/21"],
      ["undo", "2026-09-26", 4, "resident/22"], ["redo", "2026-09-28", 4, "resident/23"],
      ["redo", "2026-09-28", 6, "resident/24"],
    ] as const) {
      const operation = await client.trackerCommand({ type, expected_revision: revision === "resident/20" ? "resident/19" : `resident/${Number(revision.split("/")[1]) - 1}` });
      if (operation.resulting_revision !== revision) throw new Error(`over-budget failure lost ${type} history`);
      const projection = boundaryProjection(revision, overIds, dateValue, quantity);
      equal({ bootstrap: await client.bootstrap(), table: await client.queryTable("sheet_1") }, projection, `over-budget history ${type} changed literal state`);
      for (const id of overIds) {
        const summary = await client.queryKeyedGroupedSum(id);
        if (summary.definition_id !== id || summary.revision !== revision || JSON.stringify(summary.groups.map((group: any) => [group.category, group.value]).sort()) !== JSON.stringify([["Paper", 1000], ["Stationery", quantity === 4 ? 800 : 1200]].sort())) throw new Error(`over-budget history ${type} changed catalogue result ${id}`);
      }
      await expectCode(() => client.exportProject(revision), "unsupported_project", revision);
    }

    const establishMarkedHistory = async (occurrenceUuid: string): Promise<void> => {
      await client.closeProject();
      await expectClosed();
      const opened = await withUuid(occurrenceUuid, () => openFresh(dateDefinitionMaster));
      const profile = { dateValue: "2026-09-28", quantity: 4, catalogue: [DEFINITION_ID], revision: "resident/0", occurrence: occurrenceUuid };
      await assertDateProjection(opened, profile);
      const checkResident = async (dateValue: string, quantity: number, revision: string): Promise<void> => {
        await assertDateProjection({ bootstrap: await client.bootstrap(), table: await client.queryTable("sheet_1") }, { ...profile, dateValue, quantity, revision });
        const summary = await client.queryKeyedGroupedSum(DEFINITION_ID);
        if (summary.revision !== revision || JSON.stringify(summary.groups.map((group: any) => [group.category, group.value]).sort()) !== JSON.stringify([["Paper", 1000], ["Stationery", quantity === 6 ? 1200 : 800]].sort())) throw new Error(`replacement seed H state lost literal summary at ${revision}`);
        await expectCode(() => client.exportProject(revision), "unsupported_project", revision);
      };
      await client.editDate("resident/0", { entity: boundaryEntityIds[0], field: boundaryFieldIds[4] }, "2026-09-29");
      await checkResident("2026-09-29", 4, "resident/1");
      await client.editNumber("resident/1", { entity: boundaryEntityIds[0], field: boundaryFieldIds[2] }, "6");
      await checkResident("2026-09-29", 6, "resident/2");
      await client.trackerCommand({ type: "undo", expected_revision: "resident/2" });
      await checkResident("2026-09-29", 4, "resident/3");
    };

    // T4: v3-to-v3 replacement installs a different semantic candidate and clears history.
    await withUuid("00000000-0000-4000-8000-000000000003", () => openFresh(boundaryAtMaster));
    await client.editNumber("resident/0", { entity: boundaryEntityIds[0], field: boundaryFieldIds[2] }, "6");
    const q6 = await client.exportProjectV3("resident/1");
    const q6Entries = decodeEntries(new Uint8Array(q6.bytes));
    const q6Manifest = JSON.parse(new TextDecoder().decode(q6Entries.find(([path]) => path === "manifest.json")![1]));
    const q6Definitions = JSON.parse(new TextDecoder().decode(q6Entries.find(([path]) => path === "definitions.json")![1]));
    if (q6Manifest.format_version !== 3 || q6Definitions.length !== boundaryIds.length) throw new Error("v3-to-v3 replacement candidate lost the selected format or catalogue");
    equal(q6Definitions, boundaryIds.map((id) => ({ id,
      orders: { schema: `${boundaryPrefix}0002`, lookup_key_field: boundaryFieldIds[0], quantity_field: boundaryFieldIds[2] },
      products: { schema: `${boundaryPrefix}0002`, key_field: boundaryFieldIds[0], category_field: boundaryFieldIds[1], price_field: boundaryFieldIds[3] },
    })), "v3-to-v3 encoded definition bindings changed");
    await establishMarkedHistory("00000000-0000-4000-8000-000000000015");
    await withUuid("00000000-0000-4000-8000-000000000004", () => client.openProject(q6.bytes.slice(0)));
    const q6Expected = boundaryProjection("resident/0", boundaryIds, "2026-09-28", 6);
    equal({ bootstrap: await client.bootstrap(), table: await client.queryTable("sheet_1") }, q6Expected, "successful v3 replacement did not install the distinct q6 candidate");
    equal(await client.observeOccurrence(), { scope: "designer-occurrence/00000000-0000-4000-8000-000000000004/import_00000000-0000-4000-8000-000000000001_0001", revision: "resident/0" }, "v3 replacement scope/revision changed");
    await expectCode(() => client.exportProject("resident/0"), "unsupported_project", "resident/0");
    for (const type of ["undo", "redo"] as const) {
      const before = await residentState();
      await expectCode(() => client.trackerCommand({ type, expected_revision: "resident/0" }), "invalid_tracker_operation", "resident/0");
      await unchanged(before, `v3 replacement empty ${type}`);
      await expectCode(() => client.exportProject("resident/0"), "unsupported_project", "resident/0");
    }

    // A real successful legacy replacement clears origin; use the immutable neither control.
    await establishMarkedHistory("00000000-0000-4000-8000-000000000016");
    const legacyOpen = await withUuid("00000000-0000-4000-8000-000000000005", () => openFresh(legacyNeitherMaster));
    await assertDateProjection(legacyOpen, { dateValue: "2026-09-28", quantity: 4, catalogue: [], revision: "resident/0", occurrence: "00000000-0000-4000-8000-000000000005", dateType: false });
    const legacySaved = await client.exportProject("resident/0");
    if (legacySaved.bytes.byteLength < 8 || new TextDecoder().decode(new Uint8Array(legacySaved.bytes).slice(0, 8)) !== "TWDPROJ1") throw new Error("legacy neither replacement did not select ordinary framed export");
    const legacyEntries = decodeEntries(new Uint8Array(legacySaved.bytes));
    const legacyManifest = JSON.parse(new TextDecoder().decode(legacyEntries.find(([path]) => path === "manifest.json")![1]));
    if (legacyManifest.format_version !== 1) throw new Error("legacy neither replacement did not reset to v1 origin");
    for (const type of ["undo", "redo"] as const) {
      const before = await residentState();
      await expectCode(() => client.trackerCommand({ type, expected_revision: "resident/0" }), "invalid_tracker_operation", "resident/0");
      await unchanged(before, `legacy replacement empty ${type}`);
    }

    // New replaces the marked occurrence with its exact native-table identity map.
    await establishMarkedHistory("00000000-0000-4000-8000-000000000017");
    await expectCode(() => client.exportProject("resident/3"), "unsupported_project", "resident/3");
    const newUuid = "00000000-0000-4000-8000-000000000007";
    const newOpen: any = await withUuid<any>(newUuid, () => client.newTable("reset", [{ name: "value", field_type: "text" }]));
    const newDocument = `native_table_document_0001_${newUuid}`;
    const newSchema = `native_table_schema_0002_${newUuid}`;
    const newField = `native_table_field_0003_${newUuid}`;
    const newExpected = {
      bootstrap: { title: "reset", revision: "resident/0", default_collection: "reset", collections: [{ id: newSchema, key: "reset", entity_count: 0 }], keyed_grouped_sum_definition_ids: [] },
      table: { revision: "resident/0", native_table_profile: true, collection: { id: newSchema, key: "reset", entity_count: 0 }, columns: [{ id: newField, key: "value", field_type: "text" }], rows: [] },
    };
    equal(newOpen, newExpected, "New did not produce the literal native table document");
    equal(await client.observeOccurrence(), { scope: `designer-occurrence/${newUuid}/${newDocument}`, revision: "resident/0" }, "New scope/document identity changed");
    const newSaved = await client.exportProject("resident/0");
    const newEntries = decodeEntries(new Uint8Array(newSaved.bytes));
    const newManifest = JSON.parse(new TextDecoder().decode(newEntries.find(([path]) => path === "manifest.json")![1]));
    if (newManifest.format_version !== 1 || newManifest.document.id !== newDocument || newManifest.document.title !== "reset") throw new Error("New encoded document identity/title/origin changed");
    const newSchemas = JSON.parse(new TextDecoder().decode(newEntries.find(([path]) => path === "schemas.json")![1]));
    equal(newSchemas, [{ id: newSchema, key: "reset", fields: [{ id: newField, key: "value", field_type: { type: "text" }, required: true }] }], "New encoded schema/field flags changed");
    if (newEntries.some(([path]) => path.startsWith("entities/") && newEntries.find(([candidate]) => candidate === path)![1].length !== 0)) throw new Error("New created an unexpected entity");
    for (const type of ["undo", "redo"] as const) {
      const before = await residentState("reset");
      await expectCode(() => client.trackerCommand({ type, expected_revision: "resident/0" }), "invalid_tracker_operation", "resident/0");
      await unchanged(before, `New empty ${type}`, "reset");
    }

    // Import uses the immutable CSV (whose first event_date is September 26) and a fresh marked replacement.
    await establishMarkedHistory("00000000-0000-4000-8000-000000000018");
    await expectCode(() => client.exportProject("resident/3"), "unsupported_project", "resident/3");
    const csv = new Uint8Array(input.mixedCsv).buffer;
    const importUuid = "00000000-0000-4000-8000-000000000009";
    const imported: any = await withUuid<any>(importUuid, () => client.importSpreadsheet(csv, "csv", { delimiter: ",", header: true }, {
      column_types: [["text", "text", "number", "number", "date"]], extra_columns: [[]],
    }));
    await assertDateProjection(imported.opened, { dateValue: "2026-09-26", quantity: 4, catalogue: [], revision: "resident/0", occurrence: importUuid, identityUuid: importUuid });
    const importSaved = await client.exportProject("resident/0");
    if (new TextDecoder().decode(new Uint8Array(importSaved.bytes).slice(0, 8)) !== "TWDPROJ2") throw new Error("Worker Import did not clear v3 origin to direct-v2 legacy output");
    for (const type of ["undo", "redo"] as const) {
      const before = await residentState();
      await expectCode(() => client.trackerCommand({ type, expected_revision: "resident/0" }), "invalid_tracker_operation", "resident/0");
      await unchanged(before, `Import empty ${type}`);
    }

    const closeSource = await withUuid("00000000-0000-4000-8000-000000000010", () => openFresh(v3Master));
    await expectCode(() => client.exportProject(closeSource.bootstrap.revision), "unsupported_project", closeSource.bootstrap.revision);
    await client.closeProject();
    await expectClosed();
    const afterClose: any = await withUuid<any>("00000000-0000-4000-8000-000000000011", () => client.newTable("after close", [{ name: "value", field_type: "text" }]));
    await client.closeProject();
    await expectClosed();
    await client.close();
    return { legacy: legacyOpen.bootstrap.revision, newDocument: newOpen.bootstrap.revision, imported: imported.opened.bootstrap.revision, afterClose: afterClose.bootstrap.revision };
  }, {
    v3: Array.from(dateOnly),
    dateDefinition: Array.from(dateDefinition),
    legacyNeither: Array.from(legacyNeither),
    legacyDefinitionOnly: Array.from(legacyDefinitionOnly),
    mixedCsv: Array.from(mixedCsv),
    constrainedText: Array.from(constrainedText), constrainedNumber: Array.from(constrainedNumber),
    boundaryAt: Array.from(boundaryAt), boundaryOver: Array.from(boundaryOver),
  });
  expect(result.legacy).toBe("resident/0");
  expect(result.newDocument).toBe("resident/0");
  expect(result.imported).toBe("resident/0");
  expect(result.afterClose).toBe("resident/0");
});
