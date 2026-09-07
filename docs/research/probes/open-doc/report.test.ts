import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { chmodSync, existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { parseArgs, renderTemplate, run } from './report.js';

const ids = {
  schema: '6e594d33-70eb-5755-8b9f-f19b948d39ce',
  sword: '24ab8d17-bff2-53fc-9632-45617effe270',
  name: '866dded6-ba9f-542c-b328-bea19ee0f80f',
  damage: 'fa616e90-705e-5fa8-b735-2a6e84d03354',
  dps: '89f0fd5e-dfc9-53bf-b008-85f78c403420',
} as const;

function nativeFixture(root: string, failField = false): { binary: string; calls: string } {
  const binary = path.join(root, 'native.mjs');
  const calls = path.join(root, 'calls.jsonl');
  writeFileSync(binary, `#!/usr/bin/env node
import { appendFileSync, readFileSync, statSync, writeFileSync } from 'node:fs';
const args = process.argv.slice(2);
const mode = args[1];
const snapshot = args[2];
const source = { document_id: 'unit-document', source_label: args.at(-1) };
appendFileSync(${JSON.stringify(calls)}, JSON.stringify({ mode, args, bytes: readFileSync(snapshot, 'utf8'), modeBits: statSync(snapshot).mode & 0o777 }) + '\\n');
if (mode === 'document') {
  writeFileSync(process.env.UNIT_ORIGINAL_SOURCE, 'mutated-after-snapshot');
  console.log(JSON.stringify({ source, schemas: [{ id: ${JSON.stringify(ids.schema)}, fields: [
    { id: ${JSON.stringify(ids.name)}, key: 'name' },
    { id: ${JSON.stringify(ids.damage)}, key: 'damage' },
    { id: ${JSON.stringify(ids.dps)}, key: 'dps' },
  ] }], entities: [{ id: ${JSON.stringify(ids.sword)}, key: 'unit_sword', schema: ${JSON.stringify(ids.schema)} }] }));
} else if (mode === 'field') {
  if (${failField} && args[3].endsWith('.damage')) process.exit(2);
  const key = args[3].split('.').at(-1);
  const values = { name: 'Snapshot sword', damage: 20, dps: 40 };
  const fieldIds = { name: ${JSON.stringify(ids.name)}, damage: ${JSON.stringify(ids.damage)}, dps: ${JSON.stringify(ids.dps)} };
  console.log(JSON.stringify({ source, field: { entity: ${JSON.stringify(ids.sword)}, field: fieldIds[key] }, stored_value: key === 'dps' ? undefined : { value: values[key] }, calculated_value: key === 'dps' ? values[key] : undefined }));
} else if (mode === 'validation') {
  console.log(JSON.stringify({ source, is_valid: true, findings: [] }));
} else process.exit(3);
`);
  chmodSync(binary, 0o755);
  return { binary, calls };
}

function withRoot(t: test.TestContext): string {
  const root = mkdtempSync(path.join(os.tmpdir(), 'tachiko-open-doc-unit-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  return root;
}

test('unit: command parser rejects duplicate or incomplete flags', () => {
  assert.throws(() => parseArgs(['--source', 'a', '--source', 'b']), { code: 'USAGE' });
  assert.throws(() => parseArgs(['--source', 'a', '--out', 'b', '--tachiko', 'c', '--theme', 'dark']), { code: 'USAGE' });
});
test('unit: fixed templates carry only presentation variation', () => {
  const plain = renderTemplate('plain');
  const compact = renderTemplate('compact');
  for (const template of [plain, compact]) {
    assert.match(template, /import projection from '..\/..\/projection.json'/);
    assert.match(template, /data-tachiko-source-sha256/);
    assert.match(template, /data-tachiko-field-id/);
    assert.match(template, /satisfies DocEntry\[\]/);
    assert.doesNotMatch(template, /globalThis\.__injected/);
  }
  assert.notEqual(plain, compact);
});

test('unit: immutable staging is the sole native authority for a successful projection', (t) => {
  const root = withRoot(t);
  const source = path.join(root, 'source.ro');
  const output = path.join(root, 'output');
  writeFileSync(source, 'original-snapshot-bytes');
  const { binary, calls } = nativeFixture(root);
  const prior = process.env.UNIT_ORIGINAL_SOURCE;
  process.env.UNIT_ORIGINAL_SOURCE = source;
  try {
    run({ source, out: output, tachiko: binary, theme: 'plain' });
  } finally {
    if (prior === undefined) delete process.env.UNIT_ORIGINAL_SOURCE;
    else process.env.UNIT_ORIGINAL_SOURCE = prior;
  }
  const digest = createHash('sha256').update('original-snapshot-bytes').digest('hex');
  const nativeCalls = readFileSync(calls, 'utf8').trim().split('\n').map((line) => JSON.parse(line));
  assert.equal(readFileSync(source, 'utf8'), 'mutated-after-snapshot');
  assert.deepEqual(nativeCalls.map((call) => call.mode), ['document', 'field', 'field', 'field', 'validation']);
  for (const call of nativeCalls) {
    assert.equal(call.bytes, 'original-snapshot-bytes');
    assert.equal(call.modeBits, 0o400);
    assert.ok(call.args.includes(`sha256:${digest}`));
  }
  assert.equal(JSON.parse(readFileSync(path.join(output, 'projection.json'), 'utf8')).source.sha256, digest);
});

test('unit: native failure clears only its owned output workspace and staging input', (t) => {
  const root = withRoot(t);
  const source = path.join(root, 'source.ro');
  const output = path.join(root, 'output');
  writeFileSync(source, 'failure-snapshot-bytes');
  const { binary } = nativeFixture(root, true);
  const scratch = path.join(path.dirname(fileURLToPath(import.meta.url)), '.scratch');
  const before = existsSync(scratch) ? readdirSync(scratch).filter((entry) => entry.startsWith('snapshot-')).sort() : [];
  assert.throws(() => run({ source, out: output, tachiko: binary, theme: 'plain' }), { code: 'SOURCE_REJECTED' });
  assert.equal(existsSync(output), false);
  const after = existsSync(scratch) ? readdirSync(scratch).filter((entry) => entry.startsWith('snapshot-')).sort() : [];
  assert.deepEqual(after, before);
  assert.equal(statSync(source).isFile(), true);
});
