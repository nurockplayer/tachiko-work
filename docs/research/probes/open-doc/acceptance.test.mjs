import assert from 'node:assert/strict';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import {
  assertProjection, base, binary, buffed, digest, hostileName, ids, inspection,
  invoke, native, nativeJson, nativeRows, project, run, scratch, validation,
} from './support.mjs';

// These are prerequisite tests, not mocked evidence of the missing integration.
test('fixture: native Moonfall values, stable IDs and validation are admitted', () => {
  assert.equal(nativeJson('calculate', base)['iron_sword.dps'], 40);
  assert.equal(nativeJson('calculate', buffed)['iron_sword.dps'], 50);
  assert.deepEqual(nativeRows(base), [{
    entityId: ids.sword, name: 'Iron Sword', damage: 36, dps: 40,
  }]);
  assert.equal(validation(base).is_valid, true);
  assert.equal(inspection(base).source.source_label, `sha256:${digest(base)}`);
});

test('fixture: changed formulas use native calculation; invalid input is diagnosed', (t) => {
  const dir = scratch(t);
  const formula = path.join(dir, 'formula.ro');
  native('formula', 'set', base, 'iron_sword.dps', '--expression',
    'min(60, [iron_sword.damage] / [iron_sword.attack_interval] + 5)', '--output', formula);
  assert.equal(nativeJson('calculate', formula)['iron_sword.dps'], 45);
  const invalid = path.join(dir, 'invalid.ro');
  const fixture = JSON.parse(readFileSync(base, 'utf8'));
  // Deliberate test-only invalid-input construction; never an authoring path.
  fixture.entities.iron_sword.fields.attack_interval.value = 0;
  writeFileSync(invalid, JSON.stringify(fixture));
  assert.notEqual(run(binary, ['validate', invalid]).status, 0);
  const findings = validation(invalid);
  assert.equal(findings.is_valid, false);
  assert.ok(findings.diagnostics.length > 0);
});

test('projection: emits exact native values, stable bindings and full validation', (t) => {
  const out = path.join(scratch(t), 'report');
  const data = project(base, out);
  assertProjection(data, base);
  assert.ok(existsSync(path.join(out, 'docs/moonfall/index.tsx')));
});

test('projection: relevant source change updates the metric and table together', (t) => {
  const dir = scratch(t);
  const first = project(base, path.join(dir, 'base'));
  const next = project(buffed, path.join(dir, 'buffed'));
  assertProjection(first, base);
  assertProjection(next, buffed);
  assert.equal(first.metric.value, 40);
  assert.equal(next.metric.value, 50);
  assert.notEqual(first.source.sha256, next.source.sha256);
});

test('projection: changed formula defeats a hard-coded JS damage/interval calculator', (t) => {
  const dir = scratch(t);
  const source = path.join(dir, 'formula.ro');
  native('formula', 'set', base, 'iron_sword.dps', '--expression',
    'min(60, [iron_sword.damage] / [iron_sword.attack_interval] + 5)', '--output', source);
  const data = project(source, path.join(dir, 'report'));
  assertProjection(data, source);
  assert.equal(data.metric.value, 45);
});

test('projection: unrelated change updates snapshot evidence, not bound values', (t) => {
  const dir = scratch(t);
  const source = path.join(dir, 'economy.ro');
  native('set', base, 'shop.gold_per_match', '100', '--output', source);
  const first = project(base, path.join(dir, 'base'));
  const next = project(source, path.join(dir, 'next'));
  assertProjection(next, source);
  assert.deepEqual(next.rows, first.rows);
  assert.deepEqual(next.metric, first.metric);
  assert.notEqual(next.source.sha256, first.source.sha256);
  // This proves unchanged visible content, not a second dependency/invalidation engine.
});

test('projection: duplicate and rename cannot retarget the metric to a row position', (t) => {
  const dir = scratch(t);
  const duplicate = path.join(dir, 'duplicate.ro');
  const tuned = path.join(dir, 'tuned.ro');
  const renamed = path.join(dir, 'renamed.ro');
  native('entity', 'duplicate', base, 'iron_sword', 'aaa_sword', '--output', duplicate);
  native('set', duplicate, 'aaa_sword.damage', '18', '--output', tuned);
  native('entity', 'rename', tuned, 'iron_sword', 'zzz_sword', '--output', renamed);
  const data = project(renamed, path.join(dir, 'report'));
  assertProjection(data, renamed);
  assert.equal(data.rows.length, 2);
  assert.deepEqual(data.rows.map((row) => row.dps).sort((a, b) => a - b), [20, 40]);
  assert.equal(data.metric.entityId, ids.sword);
  assert.equal(data.metric.value, 40);
});

test('projection: presentation-only theme change leaves projection bytes unchanged', (t) => {
  const dir = scratch(t);
  const plain = path.join(dir, 'plain');
  const compact = path.join(dir, 'compact');
  project(base, plain, 'plain');
  project(base, compact, 'compact');
  assert.deepEqual(readFileSync(path.join(plain, 'projection.json')),
    readFileSync(path.join(compact, 'projection.json')));
});

test('projection: exact input repeated at different output paths is reproducible', (t) => {
  const dir = scratch(t);
  const a = path.join(dir, 'a');
  const b = path.join(dir, 'b');
  project(base, a);
  project(base, b);
  for (const file of ['projection.json', 'docs/moonfall/index.tsx']) {
    assert.deepEqual(readFileSync(path.join(a, file)), readFileSync(path.join(b, file)));
  }
});

test('projection: hostile source text stays data and does not rewrite executable TSX', (t) => {
  const dir = scratch(t);
  const source = path.join(dir, 'text.ro');
  native('set', base, 'iron_sword.name', hostileName, '--output', source);
  const normal = path.join(dir, 'normal');
  const changed = path.join(dir, 'changed');
  project(base, normal);
  const data = project(source, changed);
  assertProjection(data, source);
  assert.equal(data.rows[0].name, hostileName);
  assert.deepEqual(readFileSync(path.join(normal, 'docs/moonfall/index.tsx')),
    readFileSync(path.join(changed, 'docs/moonfall/index.tsx')));
});

test('projection: invalid calculation refuses publication rather than substituting zero', (t) => {
  const dir = scratch(t);
  const source = path.join(dir, 'invalid.ro');
  const fixture = JSON.parse(readFileSync(base, 'utf8'));
  fixture.entities.iron_sword.fields.attack_interval.value = 0;
  writeFileSync(source, JSON.stringify(fixture));
  const out = path.join(dir, 'report');
  const result = invoke(source, out);
  assert.notEqual(result.status, 0);
  assert.equal(JSON.parse(result.stderr).code, 'SOURCE_REJECTED');
  assert.equal(existsSync(out), false);
});

test('projection: malformed source cannot produce a plausible empty report', (t) => {
  const dir = scratch(t);
  const source = path.join(dir, 'invalid.ro');
  writeFileSync(source, 'not a Tachiko document');
  const out = path.join(dir, 'report');
  const result = invoke(source, out);
  assert.notEqual(result.status, 0);
  assert.equal(JSON.parse(result.stderr).code, 'SOURCE_REJECTED');
  assert.equal(existsSync(out), false);
});

test('projection: an existing output is preserved byte for byte', (t) => {
  const out = path.join(scratch(t), 'report');
  mkdirSync(out);
  const sentinel = path.join(out, 'projection.json');
  writeFileSync(sentinel, 'previous reviewed report\n');
  const result = invoke(base, out);
  assert.notEqual(result.status, 0);
  assert.equal(JSON.parse(result.stderr).code, 'OUTPUT_EXISTS');
  assert.equal(readFileSync(sentinel, 'utf8'), 'previous reviewed report\n');
});
