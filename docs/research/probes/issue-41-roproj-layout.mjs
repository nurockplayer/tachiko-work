#!/usr/bin/env node
// DISPOSABLE, NON-PRODUCTION architecture probe for Issue #41.
// Node standard library only. This is evidence gathering, not a product
// reader/writer or a proposal of production .roproj semantics.

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { cp, mkdtemp, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, relative, resolve } from 'node:path';

const VARIANTS = ['monolith_json', 'mutable_key_jsonl', 'per_object_json', 'range_jsonl', 'hash_jsonl'];
const RANGE_SIZE = 256;
const BUCKETS = '0123456789abcdef';
const DOCUMENT = { id: 'doc_J4rN9x', title: 'Skyforge Balance Lab' };
const MANIFEST = { format: 'tachiko.roproj-probe', format_version: 1, document: DOCUMENT };
const compareUtf8 = (a, b) => Buffer.compare(Buffer.from(a, 'utf8'), Buffer.from(b, 'utf8'));
const ordered = (items) => [...items].sort((a, b) => compareUtf8(a.id, b.id));
const compact = (value) => JSON.stringify(value);
const pretty = (value) => `${JSON.stringify(value, null, 2)}\n`;
const sha256 = (value) => createHash('sha256').update(value).digest('hex');
const bucketFor = (id) => sha256(Buffer.from(id, 'utf8'))[0];
const portableRelativePath = (root, file) => relative(root, file).replaceAll('\\', '/');

function base32hex(value) {
  const alphabet = '0123456789abcdefghijklmnopqrstuv';
  let bits = 0;
  let accumulator = 0;
  let output = '';
  for (const byte of Buffer.from(value, 'utf8')) {
    accumulator = (accumulator << 8) | byte;
    bits += 8;
    while (bits >= 5) {
      bits -= 5;
      output += alphabet[(accumulator >>> bits) & 31];
    }
  }
  if (bits > 0) output += alphabet[(accumulator << (5 - bits)) & 31];
  return output;
}

async function put(root, path, body) {
  const file = join(root, path);
  await mkdir(dirname(file), { recursive: true });
  await writeFile(file, body, 'utf8');
}

function schema(id, key, fields) {
  return {
    id,
    key,
    fields: fields.map(([fieldId, fieldKey, fieldType]) => ({
      id: fieldId,
      key: fieldKey,
      fieldType,
    })),
  };
}

const numberType = { type: 'number' };
const textType = { type: 'text' };
const numberValue = (value) => ({ kind: 'number', value: value === 0 ? 0 : value });
const textValue = (value) => ({ kind: 'text', value });
const referenceValue = (target) => ({ kind: 'reference', value: target });
const formulaValue = (value) => ({ kind: 'formula', value });
const referenceExpression = (entity, field) => ({
  op: 'reference',
  args: { entity, field },
});
const numberExpression = (value) => ({ op: 'number', args: value === 0 ? 0 : value });
const binaryExpression = (op, left, right) => ({ op, args: { left, right } });

function fixture(entityCount) {
  const schemas = [
    schema('sch_A3pK7q', 'characters', [
      ['fld_C0hp', 'health', numberType],
      ['fld_C0sp', 'speed', numberType],
      ['fld_C0wp', 'starter_weapon', { type: 'reference', schema: 'sch_B8mV2d' }],
    ]),
    schema('sch_B8mV2d', 'weapons', [
      ['fld_W0dm', 'damage', numberType],
      ['fld_W0cd', 'cooldown', numberType],
      ['fld_W0lb', 'label', textType],
      ['fld_W0dp', 'damage_per_second', numberType],
    ]),
    schema('sch_D1yT6h', 'items', [
      ['fld_I0vl', 'value', numberType],
      ['fld_I0nm', 'name', textType],
    ]),
    schema('sch_E5rQ4n', 'economy', [
      ['fld_E0pr', 'price', numberType],
      ['fld_E0it', 'featured_item', { type: 'reference', schema: 'sch_D1yT6h' }],
      ['fld_E0rt', 'price_ratio', numberType],
    ]),
  ];
  const weaponId = 'ent_00000000_Weapon';
  const itemId = 'ent_00000001_Item';
  const entities = [
    {
      id: weaponId,
      schemaId: 'sch_B8mV2d',
      key: 'starter_blade',
      fields: {
        fld_W0dm: numberValue(12),
        fld_W0cd: numberValue(0.000001),
        fld_W0lb: textValue('Élan'),
        fld_W0dp: formulaValue(binaryExpression(
          'divide',
          referenceExpression(weaponId, 'fld_W0dm'),
          referenceExpression(weaponId, 'fld_W0cd'),
        )),
      },
    },
    {
      id: itemId,
      schemaId: 'sch_D1yT6h',
      key: 'starter_potion',
      fields: { fld_I0vl: numberValue(-0), fld_I0nm: textValue('E\u0301lan') },
    },
    {
      id: 'ent_00000002_Economy',
      schemaId: 'sch_E5rQ4n',
      key: 'starter_market',
      fields: {
        fld_E0pr: numberValue(25),
        fld_E0it: referenceValue(itemId),
        fld_E0rt: formulaValue(binaryExpression(
          'multiply',
          referenceExpression('ent_00000002_Economy', 'fld_E0pr'),
          numberExpression(1),
        )),
      },
    },
    {
      id: 'ent_00000003_Character',
      schemaId: 'sch_A3pK7q',
      key: 'starter_hero',
      fields: {
        fld_C0hp: numberValue(100),
        fld_C0sp: numberValue(2.5),
        fld_C0wp: referenceValue(weaponId),
      },
    },
  ];
  for (let index = entities.length; index < entityCount; index += 1) {
    const schemaIndex = index % schemas.length;
    const id = `ent_${String(index).padStart(8, '0')}_Qz`;
    if (schemaIndex === 0) entities.push({
      id,
      schemaId: 'sch_A3pK7q',
      key: `character_${index}`,
      fields: {
        fld_C0hp: numberValue(100 + index),
        fld_C0sp: numberValue(Number((2.5 + index / 100).toFixed(2))),
        fld_C0wp: referenceValue(weaponId),
      },
    });
    if (schemaIndex === 1) entities.push({
      id,
      schemaId: 'sch_B8mV2d',
      key: `weapon_${index}`,
      fields: {
        fld_W0dm: numberValue(12 + index),
        fld_W0cd: numberValue(Number((0.2 + index / 1000).toFixed(6))),
        fld_W0lb: textValue(`Weapon ${index}`),
        fld_W0dp: formulaValue(binaryExpression(
          'divide',
          referenceExpression(id, 'fld_W0dm'),
          referenceExpression(id, 'fld_W0cd'),
        )),
      },
    });
    if (schemaIndex === 2) entities.push({
      id,
      schemaId: 'sch_D1yT6h',
      key: `item_${index}`,
      fields: { fld_I0vl: numberValue(index), fld_I0nm: textValue(`Item ${index}`) },
    });
    if (schemaIndex === 3) entities.push({
      id,
      schemaId: 'sch_E5rQ4n',
      key: `economy_${index}`,
      fields: {
        fld_E0pr: numberValue(index),
        fld_E0it: referenceValue(itemId),
        fld_E0rt: formulaValue(binaryExpression(
          'multiply',
          referenceExpression(id, 'fld_E0pr'),
          numberExpression(1),
        )),
      },
    });
  }
  return { document: DOCUMENT, schemas, entities };
}

function clone(value) { return structuredClone(value); }
function mutation(model, name) {
  const next = clone(model);
  const first = ordered(next.entities)[0];
  switch (name) {
    case 'scalar_edit': first.fields.fld_W0dm = numberValue(13); break;
    case 'formula_edit': first.fields.fld_W0dp = formulaValue(binaryExpression(
      'divide',
      referenceExpression(first.id, 'fld_W0dm'),
      binaryExpression('add', referenceExpression(first.id, 'fld_W0cd'), numberExpression(1)),
    )); break;
    case 'entity_key_rename': first.key = 'starter_blade_renamed'; break;
    case 'schema_key_rename': next.schemas.find(({ id }) => id === 'sch_A3pK7q').key = 'heroes'; break;
    case 'field_key_rename': next.schemas.find(({ id }) => id === 'sch_B8mV2d').fields.find(({ id }) => id === 'fld_W0dm').key = 'base_damage'; break;
    case 'add': next.entities.push({ id: 'ent_Z9Added_Entity', schemaId: 'sch_D1yT6h', key: 'added_item', fields: { fld_I0vl: numberValue(99), fld_I0nm: textValue('Added Item') } }); break;
    // Keep the referenced starter item intact: deletion must not introduce a
    // dangling typed reference in this fixture.
    case 'delete': next.entities = next.entities.filter(({ id }) => id !== 'ent_00000006_Qz'); break;
    case 'large_scalar_edit': first.fields.fld_W0dm = numberValue(99); break;
    case 'large_beginning_insertion': next.entities.push(beginningBucketId(next.entities)); break;
    default: throw new Error(`unknown mutation ${name}`);
  }
  return next;
}

function beginningBucketId(existing) {
  const firstId = ordered(existing)[0].id;
  for (let index = 0; ; index += 1) {
    const id = `ent_00000000_A${index.toString(36)}`;
    if (compareUtf8(id, firstId) < 0 && bucketFor(id) === '0') return {
      id,
      schemaId: 'sch_D1yT6h',
      key: 'inserted_at_beginning',
      fields: { fld_I0vl: numberValue(1), fld_I0nm: textValue('Beginning Item') },
    };
  }
}

function entityRecord(entity) {
  const fields = Object.fromEntries(Object.entries(entity.fields).sort(([left], [right]) => compareUtf8(left, right)));
  return { id: entity.id, key: entity.key, schema: entity.schemaId, fields };
}
function schemaRecord(schemaValue) {
  return {
    id: schemaValue.id,
    key: schemaValue.key,
    fields: ordered(schemaValue.fields).map((field) => ({
      id: field.id,
      key: field.key,
      field_type: field.fieldType ?? field.field_type,
      required: field.required ?? true,
    })),
  };
}
function schemaRecords(model) { return ordered(model.schemas).map(schemaRecord); }
function baseManifest(model) { return { ...MANIFEST, document: model.document }; }

async function materialize(variant, model, root) {
  const entities = ordered(model.entities);
  if (variant === 'monolith_json') {
    return put(root, 'project.json', pretty({ manifest: baseManifest(model), schemas: schemaRecords(model), entities: entities.map(entityRecord) }));
  }
  await put(root, 'manifest.json', pretty(baseManifest(model)));
  if (variant === 'mutable_key_jsonl') {
    for (const schemaValue of ordered(model.schemas)) {
      await put(root, `schemas/${schemaValue.key}.json`, pretty(schemaRecord(schemaValue)));
      const records = entities.filter(({ schemaId }) => schemaId === schemaValue.id).map(entityRecord);
      await put(root, `entities/${schemaValue.key}.jsonl`, jsonl(records));
    }
    return;
  }
  if (variant === 'per_object_json') {
    for (const schemaValue of ordered(model.schemas)) await put(root, `schemas/id-${base32hex(schemaValue.id)}.json`, pretty(schemaRecord(schemaValue)));
    for (const entity of entities) await put(root, `entities/id-${base32hex(entity.id)}.json`, pretty(entityRecord(entity)));
    return;
  }
  await put(root, 'schemas.json', pretty(schemaRecords(model)));
  if (variant === 'range_jsonl') {
    for (let index = 0; index < entities.length; index += RANGE_SIZE) await put(root, `entities/${String(index / RANGE_SIZE).padStart(4, '0')}.jsonl`, jsonl(entities.slice(index, index + RANGE_SIZE).map(entityRecord)));
    return;
  }
  if (variant === 'hash_jsonl') {
    for (const bucket of BUCKETS) await put(root, `entities/${bucket}.jsonl`, jsonl(entities.filter((entity) => bucketFor(entity.id) === bucket).map(entityRecord)));
    return;
  }
  throw new Error(`unknown variant ${variant}`);
}

function jsonl(records) { return records.length === 0 ? '' : `${records.map(compact).join('\n')}\n`; }
async function files(root) {
  const result = [];
  async function walk(directory) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const file = join(directory, entry.name);
      if (entry.isDirectory()) await walk(file); else result.push(file);
    }
  }
  await walk(root);
  return result.sort((a, b) => compareUtf8(portableRelativePath(root, a), portableRelativePath(root, b)));
}
function frameLength(length) {
  const frame = Buffer.alloc(8);
  frame.writeBigUInt64BE(BigInt(length));
  return frame;
}
function percentile(values, ratio) { return values.length === 0 ? 0 : values[Math.ceil(values.length * ratio) - 1]; }
async function treeMetrics(root) {
  const paths = await files(root);
  const digest = createHash('sha256');
  const sizes = [];
  for (const file of paths) {
    const path = Buffer.from(portableRelativePath(root, file), 'utf8');
    const body = await readFile(file);
    // Length-framed UTF-8 relative path + exact bytes. Modes are excluded:
    // all probe files are regular non-executable files.
    digest.update(frameLength(path.length)); digest.update(path); digest.update(frameLength(body.length)); digest.update(body);
    sizes.push(body.length);
  }
  sizes.sort((a, b) => a - b);
  return { fileCount: paths.length, totalBytes: sizes.reduce((sum, size) => sum + size, 0), maxFileBytes: sizes.at(-1) || 0, medianFileBytes: percentile(sizes, 0.5), p95FileBytes: percentile(sizes, 0.95), treeSha256: digest.digest('hex') };
}
async function hashDistribution(root) {
  const result = {};
  for (const bucket of BUCKETS) {
    const body = await readFile(join(root, 'entities', `${bucket}.jsonl`));
    result[bucket] = { records: body.length === 0 ? 0 : body.toString('utf8').trimEnd().split('\n').length, bytes: body.length };
  }
  return result;
}

function git(args, options = {}) {
  try { return execFileSync('git', args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], ...options }); }
  catch (error) { throw new Error(`git ${args.join(' ')} failed (${error.status ?? 'unknown'}): ${error.stderr || error.message}`); }
}
async function copyTreeContents(from, to) {
  for (const entry of await readdir(from, { withFileTypes: true })) await cp(join(from, entry.name), join(to, entry.name), { recursive: true });
}
async function clearWorkTree(repo) {
  for (const entry of await readdir(repo, { withFileTypes: true })) if (entry.name !== '.git') await rm(join(repo, entry.name), { recursive: true, force: true });
}
function fixedCommitEnv() {
  return { ...process.env, GIT_AUTHOR_NAME: 'Issue 41 Probe', GIT_AUTHOR_EMAIL: 'probe@example.invalid', GIT_AUTHOR_DATE: '2000-01-01T00:00:00Z', GIT_COMMITTER_NAME: 'Issue 41 Probe', GIT_COMMITTER_EMAIL: 'probe@example.invalid', GIT_COMMITTER_DATE: '2000-01-01T00:00:00Z' };
}
function parseNumstat(output) {
  return output.split('\n').filter(Boolean).reduce((result, line) => {
    const match = /^(\d+|-)\t(\d+|-)\t/.exec(line); assert(match, `unexpected git numstat line: ${line}`); result.addedLines += Number(match[1]) || 0; result.deletedLines += Number(match[2]) || 0; return result;
  }, { addedLines: 0, deletedLines: 0 });
}
function parseStatus(output) {
  return output.split('\n').filter(Boolean).filter((line) => !line.startsWith('---')).map((line) => {
    const match = /^([ACDMRTUXB][0-9]*)\t(.+)$/.exec(line); assert(match, `unexpected git name-status line: ${line}`); return { status: match[1], paths: match[2].split('\t') };
  });
}
function pathCounts(pathStatus) {
  const counts = { changed: 0, added: 0, deleted: 0, renamed: 0 };
  for (const entry of pathStatus) { if (entry.status.startsWith('R')) counts.renamed += 1; else if (entry.status === 'A') counts.added += 1; else if (entry.status === 'D') counts.deleted += 1; else counts.changed += 1; }
  return counts;
}
async function diffMetrics(work, left, right) {
  const repo = await mkdtemp(join(work, 'metric-repo-'));
  try {
    git(['-C', repo, 'init']);
    await copyTreeContents(left, repo);
    git(['-C', repo, 'add', '.']); git(['-C', repo, 'commit', '-m', 'before'], { env: fixedCommitEnv() });
    await clearWorkTree(repo); await copyTreeContents(right, repo);
    git(['-C', repo, 'add', '.']); git(['-C', repo, 'commit', '-m', 'after'], { env: fixedCommitEnv() });
    const common = ['-C', repo, 'diff'];
    const inferred = parseStatus(git([...common, '--name-status', '-M', 'HEAD~1', 'HEAD']));
    return { metricSource: 'git_commits', ...parseNumstat(git([...common, '--numstat', '-M', 'HEAD~1', 'HEAD'])), pathCounts: pathCounts(inferred), pathStatus: inferred, pathStatusNoRenames: parseStatus(git([...common, '--name-status', '--no-renames', 'HEAD~1', 'HEAD'])) };
  } finally { await rm(repo, { recursive: true, force: true }); }
}

async function assertDeterminism(work, label, model) {
  const permuted = { ...model, schemas: [...model.schemas].reverse(), entities: [...model.entities].reverse() };
  for (const variant of VARIANTS) {
    const normal = join(work, 'determinism', label, variant, 'normal'); const reversed = join(work, 'determinism', label, variant, 'reversed');
    await materialize(variant, model, normal); await materialize(variant, permuted, reversed);
    assert.equal((await treeMetrics(normal)).treeSha256, (await treeMetrics(reversed)).treeSha256, `${label}/${variant} is nondeterministic`);
  }
}
function parseJsonl(body, path) {
  if (body === '') return [];
  const lines = body.split('\n');
  assert.equal(lines.pop(), '', `${path} must have exactly one final LF`);
  return lines.map((line, index) => { assert.notEqual(line, '', `${path}:${index + 1} is a blank JSONL record`); return JSON.parse(line); });
}
async function canonicalizeHash(input, output) {
  const manifest = JSON.parse(await readFile(join(input, 'manifest.json'), 'utf8'));
  const schemaValues = JSON.parse(await readFile(join(input, 'schemas.json'), 'utf8'));
  const records = [];
  const entityRoot = join(input, 'entities');
  for (const file of await files(entityRoot)) if (file.endsWith('.jsonl')) records.push(...parseJsonl(await readFile(file, 'utf8'), portableRelativePath(input, file)));
  const ids = new Set(records.map(({ id }) => id)); assert.equal(ids.size, records.length, 'duplicate stable entity record IDs fail closed');
  assert.equal(manifest.format, MANIFEST.format, 'unsupported probe profile');
  assert.equal(manifest.format_version, MANIFEST.format_version, 'unsupported probe version');
  const entities = records.map((record) => ({
    id: record.id,
    key: record.key,
    schemaId: record.schema,
    fields: record.fields,
  }));
  await materialize('hash_jsonl', { document: manifest.document, schemas: schemaValues, entities }, output);
}
async function canonicalizerEvidence(work, model, expected) {
  const input = join(work, 'canonicalizer-input'); const output = join(work, 'canonicalizer-output');
  await put(input, 'manifest.json', `{\n  \"format_version\" : 1, \"document\" : ${compact(model.document)}, \"format\" : \"tachiko.roproj-probe\"\n}\n`);
  await put(input, 'schemas.json', `${JSON.stringify([...schemaRecords(model)].reverse(), null, 4)}\n`);
  const reversed = ordered(model.entities).reverse();
  for (let index = 0; index < reversed.length; index += 257) {
    const records = reversed.slice(index, index + 257).map((value) => {
      const encoded = compact(entityRecord(value));
      return index % 2 === 0 ? encoded.replaceAll(',', ', ') : encoded;
    });
    await put(input, `entities/misplaced/part-${String(index / 257).padStart(3, '0')}.jsonl`, `${records.join('\n')}\n`);
  }
  await put(input, 'entities/misplaced/empty.jsonl', '');
  await canonicalizeHash(input, output);
  assert.equal((await treeMetrics(output)).treeSha256, (await treeMetrics(expected)).treeSha256, 'canonicalizer tree digest differs');
  const duplicate = join(work, 'canonicalizer-duplicate'); await mkdir(duplicate, { recursive: true }); await cp(input, duplicate, { recursive: true });
  await put(duplicate, 'entities/misplaced/duplicate.jsonl', `${compact(entityRecord(model.entities[0]))}\n`);
  await assert.rejects(() => canonicalizeHash(duplicate, join(work, 'canonicalizer-duplicate-output')), /duplicate stable entity record IDs/);
  const blank = join(work, 'canonicalizer-blank'); await mkdir(blank, { recursive: true }); await cp(input, blank, { recursive: true });
  await put(blank, 'entities/000-blank.jsonl', `${compact(entityRecord(model.entities[0]))}\n\n`);
  await assert.rejects(() => canonicalizeHash(blank, join(work, 'canonicalizer-blank-output')), /blank JSONL record/);
  return {
    sourceRecordFiles: (await files(input)).filter((file) => file.endsWith('.jsonl')).length,
    canonicalTreeSha256: (await treeMetrics(output)).treeSha256,
    duplicateIdsRejected: true,
    blankRecordsRejected: true,
  };
}

async function run(work) {
  const small = fixture(16); const large = fixture(4096);
  await assertDeterminism(work, 'small', small); await assertDeterminism(work, 'large', large);
  const evidence = { probe: 'issue-41-roproj-layout', variants: VARIANTS, rangeSize: RANGE_SIZE, fixtures: {} };
  for (const [name, model, scenarios] of [['small', small, ['scalar_edit', 'formula_edit', 'entity_key_rename', 'schema_key_rename', 'field_key_rename', 'add', 'delete']], ['large', large, ['large_scalar_edit', 'large_beginning_insertion']]]) {
    const base = join(work, name, 'base'); evidence.fixtures[name] = { entityCount: model.entities.length, treeMetrics: {}, mutations: {} };
    for (const variant of VARIANTS) { const root = join(base, variant); await materialize(variant, model, root); evidence.fixtures[name].treeMetrics[variant] = await treeMetrics(root); if (variant === 'hash_jsonl') evidence.fixtures[name].hashBucketDistribution = await hashDistribution(root); }
    for (const scenario of scenarios) { const changed = mutation(model, scenario); evidence.fixtures[name].mutations[scenario] = {}; for (const variant of VARIANTS) { const target = join(work, name, scenario, variant); await materialize(variant, changed, target); evidence.fixtures[name].mutations[scenario][variant] = await diffMetrics(work, join(base, variant), target); } }
  }
  evidence.hashCanonicalizer = await canonicalizerEvidence(work, large, join(work, 'large', 'base', 'hash_jsonl'));
  return evidence;
}
function keepDirectoryArgument(argv) { if (argv.length === 0) return null; if (argv.length === 2 && argv[0] === '--keep-dir') return resolve(argv[1]); throw new Error('usage: node docs/research/probes/issue-41-roproj-layout.mjs [--keep-dir <path>]'); }

const keep = keepDirectoryArgument(process.argv.slice(2));
const work = keep || await mkdtemp(join(tmpdir(), 'issue-41-roproj-layout-'));
// A retained evidence directory must be a new explicit path. Refusing an
// existing target keeps this disposable probe from overwriting user data.
if (keep) await mkdir(work);
try { process.stdout.write(`${JSON.stringify(await run(work))}\n`); } finally { if (!keep) await rm(work, { recursive: true, force: true }); }
