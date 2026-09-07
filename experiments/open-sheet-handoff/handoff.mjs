// Closed #341 ingress profile. It reads public compiler output only; Rust owns
// semantic admission and calculation of the direct-ro/v2 candidate.
const ledger = [
  { classification: "exact", code: "closed_formula_profile" },
  { classification: "strengthening", code: "explicit_schema" },
  { classification: "strengthening", code: "new_identity" },
  { classification: "presentation", code: "presentation_not_semantics" },
];

const reject = code => { throw Object.assign(new Error(code), { code, ledger: [...ledger, { classification: "unsupported", code }] }); };
const object = (value, code = "invalid_mapping") => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) reject(code);
  return value;
};
const map = (value, code = "invalid_mapping") => {
  if (!(value instanceof Map)) reject(code);
  return value;
};
const finite = value => typeof value === "number" && Number.isFinite(value);
const unique = (values, code = "invalid_mapping") => { if (new Set(values).size !== values.length) reject(code); };
const sourceCell = (sheet, row, column) => map(sheet.cells, "unmapped_source").get(`${row},${column}`);
const exactProperties = (value, names) => {
  for (const name of Object.keys(value)) if (!names.includes(name)) reject("unmapped_source");
};

function mapping(manifest) {
  const input = object(manifest);
  const document = object(input.document);
  const assumptions = object(input.assumptions);
  const plan = object(input.plan);
  const schema = object(assumptions.schema);
  const entity = object(assumptions.entity);
  const rate = object(assumptions.field);
  const planSchema = object(plan.schema);
  if (!Array.isArray(plan.rows) || !Array.isArray(plan.fields)
      || typeof assumptions.block !== "string" || typeof assumptions.key !== "string"
      || typeof plan.block !== "string" || typeof plan.row_label_column !== "string"
      || typeof document.id !== "string" || typeof document.title !== "string"
      || typeof schema.id !== "string" || typeof schema.key !== "string"
      || typeof planSchema.id !== "string" || typeof planSchema.key !== "string"
      || typeof entity.id !== "string" || typeof entity.key !== "string"
      || typeof rate.id !== "string" || typeof rate.key !== "string" || rate.type !== "number" || rate.required !== true) reject("invalid_mapping");
  const rows = plan.rows.map(row => object(row));
  const fields = plan.fields.map(field => object(field));
  if (rows.some(row => typeof row.label !== "string" || typeof row.id !== "string" || typeof row.key !== "string")
      || fields.some(field => typeof field.id !== "string" || typeof field.key !== "string"
        || !["text", "number"].includes(field.type) || field.required !== true)) reject("invalid_mapping");
  unique([schema.id, planSchema.id]);
  unique([entity.id, ...rows.map(row => row.id)]);
  unique([rate.id, ...fields.map(field => field.id)]);
  unique(rows.map(row => row.label)); unique(rows.map(row => row.key)); unique(fields.map(field => field.key));
  return { document, assumptions, plan, schema, entity, rate, planSchema, rows, fields };
}

function shape(book, checked) {
  const sourceBook = object(book, "unmapped_source");
  exactProperties(sourceBook, ["sheets", "registry", "definedNames"]);
  const registry = map(sourceBook.registry, "unmapped_source");
  const assumptions = registry.get(checked.assumptions.block);
  const plan = registry.get(checked.plan.block);
  if (registry.size !== 2 || !object(assumptions, "unmapped_source") || !object(plan, "unmapped_source")
      || assumptions.kind !== "keyValue" || plan.kind !== "table" || plan.rowCount !== checked.rows.length) reject("unmapped_source");
  const sheets = Array.isArray(sourceBook.sheets) ? sourceBook.sheets : reject("unmapped_source");
  const assumptionSheet = sheets.find(sheet => sheet?.name === assumptions.sheet);
  const planSheet = sheets.find(sheet => sheet?.name === plan.sheet);
  const rate = map(assumptions.keys, "unmapped_source").get(checked.assumptions.key);
  const columns = map(plan.columns, "unmapped_source");
  if (!assumptionSheet || !planSheet || !object(rate, "unmapped_source")
      || !Number.isInteger(plan.firstDataRow) || !Number.isInteger(columns.get(checked.plan.row_label_column))) reject("unmapped_source");
  exactProperties(assumptions, ["kind", "name", "sheet", "rect", "keys"]);
  exactProperties(plan, ["kind", "name", "sheet", "rect", "firstDataRow", "lastDataRow", "rowCount", "columns", "headerRow"]);
  const definedNames = map(sourceBook.definedNames, "unmapped_source");
  const namedRate = object(definedNames.get(checked.assumptions.key), "unmapped_source");
  if (definedNames.size !== 1 || namedRate.sheet !== assumptions.sheet || namedRate.owner !== checked.assumptions.block
      || namedRate.addr?.r !== rate.r || namedRate.addr?.c !== rate.c) reject("unmapped_source");
  exactProperties(namedRate, ["sheet", "addr", "owner"]);
  exactProperties(object(namedRate.addr, "unmapped_source"), ["r", "c"]);
  for (const field of checked.fields) if (!Number.isInteger(columns.get(field.key))) reject("unmapped_source");
  return { assumptions, plan, assumptionSheet, planSheet, rate, columns };
}

function direct(source, type) {
  if (!object(source, "unsupported_value") || source.expr !== undefined || !("value" in source)) reject("unsupported_value");
  sourceMetadata(source);
  if ((type === "number" && !finite(source.value)) || (type === "text" && typeof source.value !== "string")) reject("unsupported_value");
  return source.value;
}

function nativeFormula(source, rows, checked) {
  if (!object(source, "unsupported_formula") || source.expr === undefined || "value" in source) reject("unsupported_formula");
  sourceMetadata(source);
  const translate = expression => {
    if (!object(expression, "unsupported_formula")) reject("unsupported_formula");
    if (expression.k === "lit") {
      if (!finite(expression.v)) reject("unsupported_formula");
      return { op: "number", args: expression.v };
    }
    if (expression.k === "ref") {
      const target = object(expression.target, "unsupported_formula");
      if (target.kind === "name" && target.block === checked.assumptions.block && target.key === checked.assumptions.key) {
        return { op: "reference", args: { entity: checked.entity.id, field: checked.rate.id } };
      }
      if (target.kind === "cell" && target.block === checked.plan.block && target.part === "data"
          && typeof target.column === "string" && Number.isInteger(target.row)) {
        const row = rows[target.row];
        const field = checked.fields.find(candidate => candidate.key === target.column);
        if (!row || !field || field.type !== "number") reject("unsupported_formula");
        return { op: "reference", args: { entity: row.id, field: field.id } };
      }
      reject("unsupported_formula");
    }
    if (expression.k === "op" && ["+", "-", "*", "/"].includes(expression.op)) {
      const operation = { "+": "add", "-": "subtract", "*": "multiply", "/": "divide" }[expression.op];
      return { op: operation, args: { left: translate(expression.l), right: translate(expression.r) } };
    }
    reject("unsupported_formula");
  };
  return { kind: "formula", value: translate(source.expr) };
}

function sourceMetadata(source) {
  for (const name of Object.keys(source)) {
    if (!["value", "expr", "style", "format"].includes(name)) reject("unmapped_source");
  }
  for (const name of ["style", "format"]) {
    if (source[name] !== undefined && typeof source[name] !== "string") reject("unmapped_source");
  }
}

function presentation(source) {
  if (!object(source, "unmapped_source") || source.expr !== undefined || typeof source.value !== "string") reject("unmapped_source");
  sourceMetadata(source);
}

function claimAllSource(book, shape, checked) {
  const claimed = new Set();
  const claim = (sheet, row, column, check) => {
    check(sourceCell(sheet, row, column));
    claimed.add(`${sheet.name}\u0000${row},${column}`);
  };
  const claimPresentationRectangle = (sheet, rectangle, semantic) => {
    if (!object(rectangle, "unmapped_source")) reject("unmapped_source");
    for (let row = rectangle.r; row < rectangle.r + rectangle.rows; row += 1) {
      for (let column = rectangle.c; column < rectangle.c + rectangle.cols; column += 1) {
        if (!semantic.has(`${row},${column}`)) claim(sheet, row, column, presentation);
      }
    }
  };
  if (shape.assumptions.keys.size !== 1 || shape.plan.columns.size !== checked.fields.length
      || shape.assumptions.rect.rows !== 1 || shape.assumptions.rect.cols !== 2
      || shape.plan.rect.rows !== checked.rows.length + 1 || shape.plan.rect.cols !== checked.fields.length) reject("unmapped_source");
  const assumptionSemantic = new Set([`${shape.rate.r},${shape.rate.c}`]);
  claimPresentationRectangle(shape.assumptionSheet, shape.assumptions.rect, assumptionSemantic);
  claim(shape.assumptionSheet, shape.rate.r, shape.rate.c, source => sourceMetadata(object(source, "unmapped_source")));
  const planSemantic = new Set();
  for (const field of checked.fields) {
    const column = shape.columns.get(field.key);
    for (let index = 0; index < checked.rows.length; index += 1) planSemantic.add(`${shape.plan.firstDataRow + index},${column}`);
  }
  claimPresentationRectangle(shape.planSheet, shape.plan.rect, planSemantic);
  for (const address of planSemantic) {
    const [row, column] = address.split(",").map(Number);
    claim(shape.planSheet, row, column, source => sourceMetadata(object(source, "unmapped_source")));
  }
  for (const sheet of book.sheets) {
    for (const name of Object.keys(object(sheet, "unmapped_source"))) {
      if (!["name", "cells", "columnWidths", "conditionalFormats", "charts", "sparklines", "autoFilters", "printArea", "pageBreaks", "bounds"].includes(name)) reject("unmapped_source");
    }
    for (const [address, source] of map(sheet.cells, "unmapped_source")) {
      if (!claimed.has(`${sheet.name}\u0000${address}`)) reject("unmapped_source");
    }
    for (const name of ["conditionalFormats", "charts", "sparklines", "autoFilters", "printArea", "pageBreaks"]) {
      if (Array.isArray(sheet[name]) && sheet[name].length !== 0) reject("unmapped_source");
    }
  }
}

export function proposeHandoff(compiledWorkbook, manifest) {
  const checked = mapping(manifest);
  const source = shape(compiledWorkbook, checked);
  claimAllSource(compiledWorkbook, source, checked);
  const rate = direct(sourceCell(source.assumptionSheet, source.rate.r, source.rate.c), "number");
  const labelColumn = source.columns.get(checked.plan.row_label_column);
  const rows = [];
  for (let index = 0; index < source.plan.rowCount; index += 1) {
    const label = direct(sourceCell(source.planSheet, source.plan.firstDataRow + index, labelColumn), "text");
    const target = checked.rows.find(row => row.label === label);
    if (!target || rows.some(row => row.id === target.id)) reject("unmapped_source");
    rows.push(target);
  }
  if (rows.length !== checked.rows.length) reject("unmapped_source");
  const schemas = {
    [checked.schema.id]: { id: checked.schema.id, key: checked.schema.key, fields: {
      [checked.rate.id]: { id: checked.rate.id, key: checked.rate.key, field_type: { type: "number" }, required: true },
    } },
    [checked.planSchema.id]: { id: checked.planSchema.id, key: checked.planSchema.key, fields: Object.fromEntries(checked.fields.map(field => [field.id,
      { id: field.id, key: field.key, field_type: { type: field.type }, required: true },
    ])) },
  };
  const entities = {
    [checked.entity.id]: { id: checked.entity.id, key: checked.entity.key, schema: checked.schema.id, fields: {
      [checked.rate.id]: { kind: "number", value: rate },
    } },
  };
  for (let index = 0; index < rows.length; index += 1) {
    const row = rows[index];
    const fields = {};
    for (const field of checked.fields) {
      const sourceCellValue = sourceCell(source.planSheet, source.plan.firstDataRow + index, source.columns.get(field.key));
      fields[field.id] = sourceCellValue?.expr === undefined
        ? { kind: field.type, value: direct(sourceCellValue, field.type) }
        : nativeFormula(sourceCellValue, rows, checked);
    }
    entities[row.id] = { id: row.id, key: row.key, schema: checked.planSchema.id, fields };
  }
  return { status: "proposal", document: {
    format_version: 2, id: checked.document.id, title: checked.document.title, schemas, entities,
  }, ledger: ledger.map(entry => ({ ...entry })) };
}
