// Test support only: expected values come from the real Rust CLI, never a JS calculator.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const probe = fileURLToPath(new URL('./', import.meta.url));
export const root = path.resolve(probe, '../../../..');
export const base = path.join(root, 'examples/game-balance/game-balance.ro');
export const buffed = path.join(root, 'examples/game-balance/buffed-sword.ro');
export const binary = process.env.TACHIKO_BIN ?? path.join(root, 'target/debug',
  process.platform === 'win32' ? 'tachiko.exe' : 'tachiko');
export const ids = Object.freeze({
  schema: '6e594d33-70eb-5755-8b9f-f19b948d39ce',
  sword: '24ab8d17-bff2-53fc-9632-45617effe270',
  name: '866dded6-ba9f-542c-b328-bea19ee0f80f',
  damage: 'fa616e90-705e-5fa8-b735-2a6e84d03354',
  dps: '89f0fd5e-dfc9-53bf-b008-85f78c403420',
});
export const hostileName = '台灣 <script>globalThis.__injected=1</script> & "鐵劍"';

export function scratch(t) {
  const parent = path.join(probe, '.scratch');
  mkdirSync(parent, { recursive: true });
  const dir = mkdtempSync(path.join(parent, 'acceptance-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  return dir;
}
export function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: root, encoding: 'utf8', timeout: 60_000, maxBuffer: 8 * 1024 * 1024,
    ...options,
  });
  if (result.error) throw result.error;
  return result;
}
export function native(...args) {
  const result = run(binary, args);
  assert.equal(result.status, 0,
    `Native prerequisite failed: ${JSON.stringify(args)}\n${result.stderr}`);
  return result.stdout;
}
export function nativeJson(...args) { return JSON.parse(native(...args)); }
export function digest(file) {
  return createHash('sha256').update(readFileSync(file)).digest('hex');
}
export function label(file) { return `sha256:${digest(file)}`; }
export function inspection(file) {
  return nativeJson('analyze', 'document', file, '--source-state', label(file));
}
export function validation(file) {
  return nativeJson('analyze', 'validation', file, '--source-state', label(file));
}
export function nativeRows(file) {
  const doc = inspection(file);
  const schema = doc.schemas.find((item) => item.id === ids.schema);
  assert.ok(schema, 'Native fixture must contain the declared stable weapons schema');
  return doc.entities.filter((entity) => entity.schema === ids.schema).map((entity) => {
    const row = { entityId: entity.id };
    for (const key of ['name', 'damage', 'dps']) {
      const field = schema.fields.find((item) => item.id === ids[key]);
      assert.ok(field, `Native fixture is missing the ${key} stable field`);
      const fact = nativeJson('analyze', 'field', file, `${entity.key}.${field.key}`,
        '--source-state', label(file));
      assert.deepEqual(fact.field, { entity: entity.id, field: field.id });
      assert.deepEqual(fact.source, doc.source);
      row[key] = key === 'dps' ? fact.calculated_value : fact.stored_value.value;
    }
    return row;
  }).sort((a, b) => a.entityId < b.entityId ? -1 : a.entityId > b.entityId ? 1 : 0);
}
export function invoke(file, output, theme = 'plain') {
  const before = readFileSync(file);
  const result = run(process.execPath, [path.join(probe, 'report.mjs'),
    '--source', file, '--out', output, '--tachiko', binary, '--theme', theme]);
  assert.deepEqual(readFileSync(file), before, 'Report generation must not mutate its source');
  return result;
}
export function project(file, output, theme = 'plain') {
  const result = invoke(file, output, theme);
  assert.equal(result.status, 0, `Report generation failed:\n${result.stderr}`);
  return JSON.parse(readFileSync(path.join(output, 'projection.json'), 'utf8'));
}
export function assertProjection(actual, file) {
  const doc = inspection(file);
  const rows = nativeRows(file);
  const sword = rows.find((row) => row.entityId === ids.sword);
  assert.ok(sword, 'The fixed metric must still target the original sword');
  assert.equal(actual.profile, 'tachiko.open-doc-research/v0');
  assert.deepEqual(actual.source, {
    kind: 'snapshot', documentId: doc.source.document_id, sha256: digest(file),
  });
  assert.equal(actual.schemaId, ids.schema);
  assert.deepEqual(actual.fields, { name: ids.name, damage: ids.damage, dps: ids.dps });
  assert.deepEqual(actual.rows, rows);
  assert.deepEqual(actual.metric, { entityId: ids.sword, fieldId: ids.dps, value: sword.dps });
  assert.deepEqual(actual.validation, validation(file));
}
