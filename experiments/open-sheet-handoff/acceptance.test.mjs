// Steward-owned contract. Expected semantics come from the independently fixed
// #337 oracle, never from the adapter or either spreadsheet evaluator.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { pathToFileURL } from "node:url";
import { compileModel, SOURCE_ROWS } from "./model.mjs";
import { proposeHandoff } from "./handoff.mjs";

const require = createRequire(new URL("../open-sheet-export/package.json", import.meta.url));
const core = await import(pathToFileURL(require.resolve("@open-sheet/core")).href);
const fixture = JSON.parse(readFileSync(new URL("../open-sheet-export/canary.json", import.meta.url)));
const mapping = JSON.parse(readFileSync(new URL("./mapping.json", import.meta.url)));
const clone = (value) => structuredClone(value);
const planCell = (book, key, row = 0) => {
  const anchor = book.registry.get("plan");
  return book.sheets.find((sheet) => sheet.name === anchor.sheet).cells.get(
    core.cellKey(anchor.firstDataRow + row, anchor.columns.get(key)),
  );
};
const taxCell = (book) => {
  const anchor = book.registry.get("assumptions");
  const address = anchor.keys.get("rate");
  return book.sheets.find((sheet) => sheet.name === anchor.sheet).cells.get(
    core.cellKey(address.r, address.c),
  );
};

function native(...args) {
  assert.ok(process.env.TACHIKO_BIN, "ENVIRONMENT UNVERIFIED: TACHIKO_BIN is required");
  const result = spawnSync(process.env.TACHIKO_BIN, args, {
    encoding: "utf8", timeout: 30_000, maxBuffer: 4 * 1024 * 1024,
  });
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stderr);
  return result.stdout;
}

function withProject(document, inspect) {
  const directory = mkdtempSync(join(tmpdir(), "tachiko-handoff-"));
  try {
    const direct = join(directory, "candidate.ro");
    const project = join(directory, "candidate.roproj");
    writeFileSync(direct, JSON.stringify(document));
    native("validate", direct);
    native("roproj", "materialize", direct, project);
    native("roproj", "validate", project);
    return inspect(project);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

async function proposal(book = compileModel(core), manifest = mapping) {
  const before = clone(book);
  const beforeMapping = clone(manifest);
  const result = await proposeHandoff(book, manifest);
  assert.deepEqual(book, before, "A proposal must not mutate its authored source");
  assert.deepEqual(manifest, beforeMapping);
  assert.equal(result.status, "proposal", "Conversion is not acceptance/publication");
  assert.ok(Array.isArray(result.ledger));
  for (const [classification, code] of [
    ["exact", "closed_formula_profile"],
    ["strengthening", "explicit_schema"],
    ["strengthening", "new_identity"],
    ["presentation", "presentation_not_semantics"],
  ]) {
    assert.ok(result.ledger.some((entry) => entry.classification === classification
      && entry.code === code), `Missing truthful classification: ${code}`);
  }
  return result;
}

async function rejected(book, code, manifest = mapping) {
  const before = clone(book);
  const beforeMapping = clone(manifest);
  await assert.rejects(async () => proposeHandoff(book, manifest), (error) => {
    assert.equal(error.code, code);
    assert.ok(Array.isArray(error.ledger));
    assert.ok(error.ledger.some((entry) => entry.classification === "unsupported"
      && entry.code === code));
    assert.equal(error.document, undefined, "No partially successful candidate");
    return true;
  });
  assert.deepEqual(book, before);
  assert.deepEqual(manifest, beforeMapping);
}

test("preflight: real public compiler preserves symbolic source, not baked answers", () => {
  const book = compileModel(core);
  assert.equal(book.sheets.length, 2);
  assert.deepEqual([...book.registry.keys()].sort(), ["assumptions", "plan"]);
  assert.equal(book.registry.get("plan").rowCount, 3);
  assert.equal(taxCell(book).value, 0.25);
  assert.equal(planCell(book, "revenue").value, 1200);
  assert.equal(planCell(book, "net").expr.k, "op");
  assert.equal(planCell(book, "net").expr.op, "*");
  assert.equal(planCell(book, "net").expr.r.r.target.block, "assumptions");
  assert.equal(planCell(book, "net").value, undefined);
});

test("preflight: independent expected model reaches existing Rust storage and calculation", () => {
  withProject(fixture.document, (project) => {
    for (const [target, expected] of Object.entries(fixture.expected.base)) {
      const facts = JSON.parse(native("formula", "inspect", project, target));
      assert.equal(facts.outcome.kind, "formula");
      assert.deepEqual(facts.outcome.calculation, { kind: "value", value: expected });
    }
  });
});

test("handoff: actual compiler output yields the exact native proposal", async () => {
  const result = await proposal();
  assert.deepEqual(result.document, fixture.document);
  withProject(result.document, (project) => {
    const result = JSON.parse(native("formula", "inspect", project, "e-mar.f-net"));
    assert.deepEqual(result.outcome.calculation, { kind: "value", value: 750 });
  });
});

test("handoff: source input variations are read rather than replaced by the canary", async () => {
  for (const rate of [0, 0.5]) {
    const rows = SOURCE_ROWS.map((row) => ({ ...row }));
    rows[0].revenue = 1800;
    const result = await proposal(compileModel(core, { rate, rows }));
    const expected = clone(fixture.document);
    expected.entities["e-tax"].fields["f-rate"].value = rate;
    expected.entities["e-jan"].fields["f-revenue"].value = 1800;
    assert.deepEqual(result.document, expected);
  }
});

test("handoff: formula structure comes from source, not the known final numbers", async () => {
  for (const [operation, helper] of [["add", core.add], ["divide", core.div]]) {
    const result = await proposal(compileModel(core, {
      formulas: { gross: (row) => helper(row.cell("revenue"), row.cell("cost")) },
    }));
    const expected = clone(fixture.document);
    for (const id of ["e-jan", "e-feb", "e-mar"]) {
      expected.entities[id].fields["f-gross"].value.op = operation;
    }
    assert.deepEqual(result.document, expected);
  }
});

test("handoff: row, column, sheet and origin changes do not become semantic identity", async () => {
  const result = await proposal(compileModel(core, {
    rows: [...SOURCE_ROWS].reverse(),
    columns: ["net", "cost", "label", "gross", "revenue"],
    origin: { r: 3, c: 2 }, reverseSheets: true,
    planName: "計畫", assumptionsName: "假設 '2026",
  }));
  assert.deepEqual(result.document, fixture.document);
});

test("handoff: numeric-looking Text is not silently strengthened to Number", async () => {
  const book = compileModel(core);
  taxCell(book).value = "0.25";
  await rejected(book, "unsupported_value");
});

test("handoff: nonfinite and absent values have no implicit default", async () => {
  for (const value of [NaN, Infinity, null, undefined, false]) {
    const book = compileModel(core);
    taxCell(book).value = value;
    await rejected(book, "unsupported_value");
  }
});

test("handoff: duplicate, unknown or missing source rows cannot retarget identity", async () => {
  for (const labels of [["January", "January", "March"],
    ["January", "February", "April"], ["January", "February"]]) {
    const rows = labels.map((label, index) => ({ ...SOURCE_ROWS[index], label }));
    await rejected(compileModel(core, { rows }), "unmapped_source");
  }
});

test("handoff: duplicate target mapping is rejected, not last-writer-wins", async () => {
  const manifest = clone(mapping);
  manifest.plan.rows[1].id = manifest.plan.rows[0].id;
  await rejected(compileModel(core), "invalid_mapping", manifest);
});

test("handoff: unsupported expressions never become cached or approximate values", async () => {
  for (const expr of [core.now(), core.raw("XIRR(A1:A2,B1:B2)"),
    { k: "addr", ref: "B2" }, { k: "ref", target: core.ref("plan").column("revenue") },
    { k: "ref", target: core.ref("unknown").cell("missing", 0) }]) {
    const book = compileModel(core);
    planCell(book, "net").expr = expr;
    await rejected(book, "unsupported_formula");
  }
});

test("handoff: unclaimed formula content is not silently dropped", async () => {
  const book = compileModel(core);
  book.sheets[0].cells.set(core.cellKey(20, 20), { expr: core.now() });
  await rejected(book, "unmapped_source");
});

test("handoff: an added mapped-block column cannot be silently dropped", async () => {
  const book = compileModel(core, {
    columns: ["label", "revenue", "cost", "gross", "net", "bonus"],
    formulas: { bonus: (row) => core.add(row.cell("revenue"), 1) },
  });
  await rejected(book, "unmapped_source");
});

test("handoff: meaning-affecting metadata is never treated as presentation", async () => {
  const book = compileModel(core);
  Object.assign(taxCell(book), { decimal: { min: 1500 }, error: "Minimum 1500", style: "error" });
  await rejected(book, "unmapped_source");
});

test("handoff: unknown workbook metadata, cells and registry anchors fail closed", async () => {
  const cases = [
    (book) => { book.sheets[0].protect = true; },
    (book) => { book.sheets[0].cells.set(core.cellKey(20, 20), { validate: { decimal: { min: 1 } } }); },
    (book) => { book.registry.set("unmapped", { kind: "table" }); },
  ];
  for (const change of cases) {
    const book = compileModel(core);
    change(book);
    await rejected(book, "unmapped_source");
  }
});

test("handoff: appendable public-table metadata cannot be silently dropped", async () => {
  await rejected(compileModel(core, { appendable: true }), "unmapped_source");
});

test("handoff: cycles are not blessed by conversion and Rust refuses publication input", async () => {
  const book = compileModel(core, {
    formulas: { gross: (row) => core.add(row.cell("net"), 1) },
  });
  // Translation is syntax-only. It must not grow a second dependency evaluator.
  const result = await proposal(book);
  const directory = mkdtempSync(join(tmpdir(), "tachiko-handoff-invalid-"));
  try {
    const direct = join(directory, "cycle.ro");
    writeFileSync(direct, JSON.stringify(result.document));
    const validation = spawnSync(process.env.TACHIKO_BIN, ["validate", direct], {
      encoding: "utf8", timeout: 30_000,
    });
    assert.ifError(validation.error);
    assert.notEqual(validation.status, 0, "Rust admission must reject cyclic meaning");
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
