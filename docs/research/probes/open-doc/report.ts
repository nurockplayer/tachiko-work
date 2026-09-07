import { createHash } from 'node:crypto';
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const ids = {
  schema: '6e594d33-70eb-5755-8b9f-f19b948d39ce',
  metricEntity: '24ab8d17-bff2-53fc-9632-45617effe270',
  name: '866dded6-ba9f-542c-b328-bea19ee0f80f',
  damage: 'fa616e90-705e-5fa8-b735-2a6e84d03354',
  dps: '89f0fd5e-dfc9-53bf-b008-85f78c403420',
} as const;

type Theme = 'plain' | 'compact';
type Options = { source: string; out: string; tachiko: string; theme: Theme };
type Json = Record<string, unknown>;
type NativeField = { field: { entity: string; field: string }; stored_value?: { value?: unknown }; calculated_value?: unknown; source: unknown };
type Row = { entityId: string; name: string; damage: number; dps: number };

class ProbeError extends Error {
  constructor(readonly code: string, message: string) { super(message); }
}

function object(value: unknown, context: string): Json {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) throw new ProbeError('NATIVE_SHAPE_REJECTED', `${context} was not an object`);
  return value as Json;
}
function string(value: unknown, context: string): string {
  if (typeof value !== 'string') throw new ProbeError('NATIVE_SHAPE_REJECTED', `${context} was not a string`);
  return value;
}
function number(value: unknown, context: string): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) throw new ProbeError('NATIVE_SHAPE_REJECTED', `${context} was not a finite number`);
  return value;
}

export function parseArgs(argv: string[]): Options {
  const values = new Map<string, string>();
  for (let index = 0; index < argv.length; index += 2) {
    const flag = argv[index];
    const value = argv[index + 1];
    if (!['--source', '--out', '--tachiko', '--theme'].includes(flag) || value === undefined || values.has(flag)) throw new ProbeError('USAGE', 'expected --source, --out, --tachiko and --theme exactly once');
    values.set(flag, value);
  }
  const theme = values.get('--theme');
  if (theme !== 'plain' && theme !== 'compact') throw new ProbeError('USAGE', 'theme must be plain or compact');
  return { source: path.resolve(required(values, '--source')), out: path.resolve(required(values, '--out')), tachiko: path.resolve(required(values, '--tachiko')), theme };
}
function required(values: Map<string, string>, key: string): string {
  const value = values.get(key);
  if (!value) throw new ProbeError('USAGE', 'expected --source, --out, --tachiko and --theme exactly once');
  return value;
}
function nativeJson(binary: string, args: string[], failureCode: string): Json {
  const result = spawnSync(binary, args, { encoding: 'utf8', timeout: 60_000, maxBuffer: 8 * 1024 * 1024 });
  if (result.error) throw new ProbeError('NATIVE_UNAVAILABLE', result.error.message);
  if (result.status !== 0) throw new ProbeError(failureCode, result.stderr.trim() || `native command failed: ${args.join(' ')}`);
  try { return object(JSON.parse(result.stdout), 'native response'); }
  catch (error) {
    if (error instanceof ProbeError) throw error;
    throw new ProbeError(failureCode, 'native command did not emit JSON');
  }
}
function assertedSource(value: unknown, documentId: string, sourceLabel: string): void {
  const source = object(value, 'native source');
  if (source.document_id !== documentId || source.source_label !== sourceLabel) throw new ProbeError('NATIVE_SHAPE_REJECTED', 'native response did not preserve the staged source identity');
}
function fieldValue(fact: NativeField, key: keyof Pick<Row, 'name' | 'damage' | 'dps'>): string | number {
  if (key === 'dps') return number(fact.calculated_value, 'calculated DPS');
  const stored = object(fact.stored_value, `stored ${key}`);
  return key === 'name' ? string(stored.value, 'stored name') : number(stored.value, `stored ${key}`);
}
function collectProjection(binary: string, snapshot: string, digest: string): Json {
  const sourceLabel = `sha256:${digest}`;
  const document = nativeJson(binary, ['analyze', 'document', snapshot, '--source-state', sourceLabel], 'SOURCE_REJECTED');
  const source = object(document.source, 'document source');
  const documentId = string(source.document_id, 'document id');
  assertedSource(source, documentId, sourceLabel);
  const schemas = document.schemas;
  const entities = document.entities;
  if (!Array.isArray(schemas) || !Array.isArray(entities)) throw new ProbeError('SOURCE_REJECTED', 'document lacked schemas or entities');
  const schema = schemas.map((item) => object(item, 'schema')).find((item) => item.id === ids.schema);
  if (!schema || !Array.isArray(schema.fields)) throw new ProbeError('SOURCE_REJECTED', 'Moonfall weapons schema was not found');
  const schemaFields = schema.fields;
  const fields = Object.fromEntries(['name', 'damage', 'dps'].map((key) => {
    const id = ids[key as keyof Pick<typeof ids, 'name' | 'damage' | 'dps'>];
    const field = schemaFields.map((item) => object(item, 'field')).find((item) => item.id === id);
    if (!field) throw new ProbeError('SOURCE_REJECTED', `Moonfall ${key} field was not found`);
    return [key, { id, key: string(field.key, `${key} field key`) }];
  }));
  const rows: Row[] = [];
  for (const rawEntity of entities.map((item) => object(item, 'entity')).filter((item) => item.schema === ids.schema)) {
    const entityId = string(rawEntity.id, 'entity id');
    const entityKey = string(rawEntity.key, 'entity key');
    const row: Partial<Row> = { entityId };
    for (const key of ['name', 'damage', 'dps'] as const) {
      const field = fields[key] as { id: string; key: string };
      const fact = nativeJson(binary, ['analyze', 'field', snapshot, `${entityKey}.${field.key}`, '--source-state', sourceLabel], 'SOURCE_REJECTED') as NativeField;
      assertedSource(fact.source, documentId, sourceLabel);
      if (fact.field.entity !== entityId || fact.field.field !== field.id) throw new ProbeError('SOURCE_REJECTED', 'native field analysis did not preserve its declared subject');
      row[key] = fieldValue(fact, key) as never;
    }
    rows.push(row as Row);
  }
  rows.sort((left, right) => left.entityId.localeCompare(right.entityId));
  const metricRow = rows.find((row) => row.entityId === ids.metricEntity);
  if (!metricRow) throw new ProbeError('SOURCE_REJECTED', 'Moonfall original sword was not found');
  const validation = nativeJson(binary, ['analyze', 'validation', snapshot, '--source-state', sourceLabel], 'SOURCE_REJECTED');
  assertedSource(validation.source, documentId, sourceLabel);
  if (validation.is_valid !== true) throw new ProbeError('SOURCE_REJECTED', 'native validation rejected the snapshot');
  return { profile: 'tachiko.open-doc-research/v0', source: { kind: 'snapshot', documentId, sha256: digest }, schemaId: ids.schema, fields: { name: ids.name, damage: ids.damage, dps: ids.dps }, rows, metric: { entityId: ids.metricEntity, fieldId: ids.dps, value: metricRow.dps }, validation };
}

export function renderTemplate(theme: Theme): string {
  const compact = theme === 'compact';
  return `import { DataTable, flow, type DocEntry, type DocMeta } from '@open-document/core';
import projection from '../../projection.json';

const columns = [
  { key: 'name', label: 'Weapon', format: (value: unknown, row: Record<string, unknown>) => <span data-tachiko-entity-id={String(row.entityId)} data-tachiko-field-id={projection.fields.name}>{String(value)}</span> },
  { key: 'damage', label: 'Damage', align: 'right' as const, format: (value: unknown, row: Record<string, unknown>) => <span data-tachiko-entity-id={String(row.entityId)} data-tachiko-field-id={projection.fields.damage}>{String(value)}</span> },
  { key: 'dps', label: 'DPS', align: 'right' as const, format: (value: unknown, row: Record<string, unknown>) => <span data-tachiko-entity-id={String(row.entityId)} data-tachiko-field-id={projection.fields.dps}>{String(value)}</span> },
];

export const meta: DocMeta = { title: 'Moonfall balance report' };

export default [flow(
  <main style={{ fontFamily: 'var(--od-font-body)', lineHeight: 1.5 }}>
    <p style={{ color: 'var(--od-muted)', margin: '0 0 8px' }}>Tachiko open-doc research prototype</p>
    <h1 style={{ margin: '0 0 12px' }}>Moonfall balance report</h1>
    <p data-tachiko-source-sha256={projection.source.sha256} style={{ fontFamily: 'monospace', fontSize: 11, overflowWrap: 'anywhere' }}>Snapshot: {projection.source.sha256}</p>
    <p><strong>Original sword DPS: </strong><span data-tachiko-metric>{projection.metric.value}</span></p>
    <p data-tachiko-validation="valid">Native validation: valid</p>
    <DataTable rows={projection.rows} columns={columns} caption="Native calculated weapon values" compact={${compact}} />
  </main>,
  { padding: ${compact ? '36' : '56'} },
)] satisfies DocEntry[];
`;
}
function configTemplate(): string {
  return `import type { OpenDocConfig } from '@open-document/core';

export default { docsDir: 'docs', assetsDir: 'assets' } satisfies OpenDocConfig;
`;
}
export function run(options: Options): void {
  let ownsOutput = false;
  const sourceBytes = readFileSync(options.source);
  const digest = createHash('sha256').update(sourceBytes).digest('hex');
  const scratchRoot = path.join(here, '.scratch');
  mkdirSync(scratchRoot, { recursive: true });
  const staging = mkdtempSync(path.join(scratchRoot, 'snapshot-'));
  const snapshot = path.join(staging, 'source.ro');
  try {
    writeFileSync(snapshot, sourceBytes, { mode: 0o400 });
    chmodSync(snapshot, 0o400);
    const projection = collectProjection(options.tachiko, snapshot, digest);
    mkdirSync(path.dirname(options.out), { recursive: true });
    try {
      mkdirSync(options.out);
      ownsOutput = true;
    } catch (error) {
      if (error && typeof error === 'object' && 'code' in error && error.code === 'EEXIST') {
        throw new ProbeError('OUTPUT_EXISTS', 'output workspace already exists');
      }
      throw error;
    }
    mkdirSync(path.join(options.out, 'docs', 'moonfall'), { recursive: true });
    writeFileSync(path.join(options.out, 'projection.json'), `${JSON.stringify(projection, null, 2)}\n`);
    writeFileSync(path.join(options.out, 'open-doc.config.ts'), configTemplate());
    writeFileSync(path.join(options.out, 'docs', 'moonfall', 'index.tsx'), renderTemplate(options.theme));
  } catch (error) {
    if (ownsOutput) rmSync(options.out, { recursive: true, force: true });
    throw error;
  } finally { rmSync(staging, { recursive: true, force: true }); }
}
export function main(argv: string[]): void {
  try { run(parseArgs(argv)); }
  catch (error) {
    const probeError = error instanceof ProbeError ? error : new ProbeError('SOURCE_REJECTED', String(error));
    process.stderr.write(`${JSON.stringify({ code: probeError.code })}\n`);
    process.exitCode = 1;
  }
}
