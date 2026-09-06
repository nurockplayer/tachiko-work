import { cp, lstat, mkdtemp, readFile, realpath, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { basename, dirname, isAbsolute, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import { execFile as execFileCallback } from 'node:child_process';
import * as core from '@open-sheet/core';
import { XlsxWriter } from '@open-sheet/core/node';

const execFile = promisify(execFileCallback);
const UPSTREAM = '6ab72ddb50bb87dad11cc321d87b5e9bcc3c5689';
const CLI = process.env.TACHIKO_BIN ?? 'tachiko';

class Rejection extends Error {
  constructor(code, ledger = []) {
    super(code);
    this.code = code;
    this.ledger = ledger;
  }
}

function reject(code, detail = {}) {
  throw new Rejection(code, [{ code, ...detail }]);
}

function targetKey(entity, field) {
  return `${entity}.${field}`;
}

function isWithin(parent, candidate) {
  const path = relative(parent, candidate);
  return path === '' || (!path.startsWith('..') && !isAbsolute(path));
}

async function cliJson(...args) {
  try {
    const { stdout } = await execFile(CLI, args, { encoding: 'utf8', maxBuffer: 1_000_000 });
    return JSON.parse(stdout);
  } catch (error) {
    const message = error?.stderr?.trim() || error?.message || 'native query failed';
    throw new Error(`native query failed: ${message}`);
  }
}

async function cli(...args) {
  try {
    await execFile(CLI, args, { encoding: 'utf8', maxBuffer: 1_000_000 });
  } catch (error) {
    const message = error?.stderr?.trim() || error?.message || 'native query failed';
    throw new Error(`native query failed: ${message}`);
  }
}

async function captureSnapshot(source) {
  const canonicalSource = await realpath(source);
  if (!(await lstat(canonicalSource)).isDirectory()) {
    throw new Error('source must be a canonical .roproj directory');
  }
  const directory = await mkdtemp(resolve(tmpdir(), 'tachiko-open-sheet-snapshot-'));
  const snapshot = resolve(directory, basename(canonicalSource));
  try {
    await cp(canonicalSource, snapshot, { recursive: true, force: false, errorOnExist: true });
    await cli('roproj', 'validate', snapshot);
    return { directory, snapshot, canonicalSource };
  } catch (error) {
    await rm(directory, { recursive: true, force: true });
    throw error;
  }
}

function projectionLocations(projection) {
  const locations = new Map();
  projection.sheets.forEach((sheet, sheetIndex) => {
    sheet.rows.forEach((entity, row) => {
      sheet.columns.forEach((field, column) => {
        const key = targetKey(entity, field);
        if (locations.has(key)) reject('ambiguous_projection', { target: key });
        locations.set(key, {
          block: `projection_${sheetIndex}`,
          field,
          row,
          sheet: sheet.name,
          coordinate: { r: sheet.header_row + 1 + row, c: sheet.left_column + column },
        });
      });
    });
  });
  return locations;
}

function scalar(value, target) {
  if (!value || !['number', 'text', 'boolean'].includes(value.kind)) {
    reject('unsupported_value', { target, actual: value?.kind ?? 'missing' });
  }
  if (typeof value.value !== ({ number: 'number', text: 'string', boolean: 'boolean' })[value.kind]) {
    reject('unsupported_value', { target, actual: value.kind });
  }
  return value.value;
}

export function translateExpression(expression, locations, coreApi) {
  if (!expression || typeof expression !== 'object') reject('unsupported_value', { actual: 'formula' });
  if (expression.op === 'number') {
    if (typeof expression.args !== 'number' || !Number.isFinite(expression.args)) {
      reject('unsupported_value', { actual: 'formula_number' });
    }
    return expression.args;
  }
  if (expression.op === 'reference') {
    const target = targetKey(expression.args?.entity, expression.args?.field);
    const location = locations.get(target);
    if (!location) reject('unprojected_reference', { target });
    return coreApi.ref(location.block).cell(location.field, location.row);
  }
  const left = translateExpression(expression.args?.left, locations, coreApi);
  const right = translateExpression(expression.args?.right, locations, coreApi);
  const operation = {
    add: coreApi.add,
    subtract: coreApi.sub,
    multiply: coreApi.mul,
    divide: coreApi.div,
    minimum: coreApi.min,
    maximum: coreApi.max,
  }[expression.op];
  if (!operation) reject('unsupported_value', { actual: `formula_${expression.op}` });
  return operation(left, right);
}

async function collectSnapshot(snapshot, projection, locations) {
  const document = await cliJson('analyze', 'document', snapshot);
  const entities = new Map(document.entities.map((entity) => [entity.id, entity]));
  const schemas = new Map(document.schemas.map((schema) => [schema.id, schema]));
  const facts = new Map();
  let revision;

  for (const sheet of projection.sheets) {
    const schema = schemas.get(sheet.schema);
    if (!schema) reject('unsupported_value', { actual: 'unknown_schema', schema: sheet.schema });
    const fields = new Map(schema.fields.map((field) => [field.id, field]));
    for (const entityId of sheet.rows) {
      const entity = entities.get(entityId);
      if (!entity || entity.schema !== sheet.schema) {
        reject('unsupported_value', { target: entityId, actual: 'schema_mismatch' });
      }
      for (const fieldId of sheet.columns) {
        const field = fields.get(fieldId);
        const target = targetKey(entityId, fieldId);
        if (!field) reject('unsupported_value', { target, actual: 'unknown_field' });
        const stored = await cliJson('analyze', 'field', snapshot, `${entity.key}.${field.key}`);
        const inspected = await cliJson('formula', 'inspect', snapshot, target);
        revision ??= inspected.source_revision;
        if (inspected.source_revision !== revision) reject('stale_source_revision');
        const value = stored.stored_value;
        facts.set(target, {
          key: field.key,
          value: value?.kind === 'formula'
            ? { kind: 'formula', expression: inspected.outcome?.expression }
            : { kind: 'scalar', value: scalar(value, target) },
        });
      }
    }
  }
  if (!revision) reject('unsupported_value', { actual: 'empty_projection' });
  return { facts, revision };
}

function compileWorkbook(projection, locations, facts, coreApi) {
  const registry = new Map();
  const sheets = projection.sheets.map((sheet, sheetIndex) => {
    const cells = new Map();
    const columns = new Map();
    sheet.columns.forEach((field, index) => columns.set(field, sheet.left_column + index));
    registry.set(`projection_${sheetIndex}`, {
      kind: 'table',
      name: `projection_${sheetIndex}`,
      sheet: sheet.name,
      rect: { r: sheet.header_row, c: sheet.left_column, rows: sheet.rows.length + 1, cols: sheet.columns.length },
      headerRow: sheet.header_row,
      firstDataRow: sheet.header_row + 1,
      lastDataRow: sheet.header_row + sheet.rows.length,
      rowCount: sheet.rows.length,
      columns,
    });
    sheet.columns.forEach((fieldId, column) => {
      const first = facts.get(targetKey(sheet.rows[0], fieldId));
      cells.set(coreApi.cellKey(sheet.header_row, sheet.left_column + column), {
        value: first.key,
        style: 'tableHeader',
      });
    });
    sheet.rows.forEach((entityId, row) => {
      sheet.columns.forEach((fieldId, column) => {
        const target = targetKey(entityId, fieldId);
        const fact = facts.get(target);
        const cell = fact.value.kind === 'formula'
          ? { expr: translateExpression(fact.value.expression, locations, coreApi) }
          : { value: fact.value.value };
        const format = sheet.number_formats?.[fieldId];
        if (format) cell.format = format;
        cells.set(coreApi.cellKey(sheet.header_row + 1 + row, sheet.left_column + column), cell);
      });
    });
    return {
      name: sheet.name,
      cells,
      columnWidths: new Map(),
      conditionalFormats: [],
      charts: [],
      autoFilters: [],
      sparklines: [],
      printArea: [],
      pageBreaks: [],
      bounds: {
        rows: sheet.header_row + sheet.rows.length + 1,
        cols: sheet.left_column + sheet.columns.length,
      },
    };
  });
  return { sheets, registry, definedNames: new Map() };
}

export async function exportSnapshot(request) {
  if (!request || typeof request !== 'object') reject('unsupported_value', { actual: 'request' });
  let output = resolve(request.output ?? '');
  const canonicalParent = await realpath(dirname(output)).catch(() => undefined);
  if (canonicalParent) output = resolve(canonicalParent, basename(output));
  if (await lstat(output).catch(() => undefined)) reject('output_exists');
  const capture = await captureSnapshot(request.source);
  try {
    if (isWithin(capture.canonicalSource, output)) reject('invalid_output');
    const locations = projectionLocations(request.projection);
    const { facts, revision } = await collectSnapshot(capture.snapshot, request.projection, locations);
    if (revision !== request.expected_source_revision) reject('stale_source_revision');
    const workbook = compileWorkbook(request.projection, locations, facts, core);
    const bytes = await new XlsxWriter().write(workbook, { cacheValues: false });
    await writeFile(output, bytes, { flag: 'wx' });
    return {
      status: 'exported',
      source_revision: revision,
      writer: { name: 'open-sheet', source_commit: UPSTREAM },
      ledger: [{ code: 'semantic_metadata_not_exported' }],
    };
  } finally {
    await rm(capture.directory, { recursive: true, force: true });
  }
}

async function main() {
  const requestIndex = process.argv.indexOf('--request');
  if (requestIndex < 0 || !process.argv[requestIndex + 1]) throw new Error('supply --request <request.json>');
  const request = JSON.parse(await readFile(process.argv[requestIndex + 1], 'utf8'));
  const result = await exportSnapshot(request);
  process.stdout.write(`${JSON.stringify(result)}\n`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    const result = error instanceof Rejection
      ? { status: 'rejected', code: error.code, ledger: error.ledger }
      : { status: 'rejected', code: 'unsupported_value', ledger: [{ code: 'unsupported_value' }] };
    process.stderr.write(`${error.stack ?? error.message}\n`);
    process.stdout.write(`${JSON.stringify(result)}\n`);
    process.exitCode = 1;
  });
}
