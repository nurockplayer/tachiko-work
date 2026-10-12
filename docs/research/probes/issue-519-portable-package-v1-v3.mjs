// Independent spec-only ZIP32/CRC/root encoder. No Rust codec or ZIP library.
// Usage: node <this-file> [--write]; default verifies every checked-in byte.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../fixtures/issue-519-portable-package-v1-v3');
const paths = ['manifest.json', 'schemas.json', 'definitions.json',
  ...'0123456789abcdef'.split('').map(n => `entities/${n}.jsonl`)];
const sha = b => createHash('sha256').update(b).digest();
const u16 = n => { const b = Buffer.alloc(2); b.writeUInt16LE(n); return b; };
const u32 = n => { const b = Buffer.alloc(4); b.writeUInt32LE(n); return b; };
const cat = (...b) => Buffer.concat(b);
const json = o => Buffer.from(`${JSON.stringify(o, null, 2)}\n`);
function crc(b) {
  let c = 0xffffffff;
  for (const byte of b) {
    c ^= byte;
    for (let bit = 0; bit < 8; bit++) c = (c >>> 1) ^ ((c & 1) ? 0xedb88320 : 0);
  }
  return (~c) >>> 0;
}
assert.equal(crc(Buffer.from('123456789')), 0xcbf43926);
assert.equal(crc(Buffer.alloc(0)), 0);
const input = await Promise.all(paths.map(async p => [p, await readFile(resolve(root, 'input.roproj', p))]));
assert.equal(JSON.parse(input[0][1]).format_version, 3);
assert(input.slice(3).some(([, b]) => b.length === 0));
function integrity(tree) {
  return sha(cat(Buffer.from('tachiko.portable-package/v1\0tachiko.roproj/v3\0'),
    ...tree.map(([p, b]) => sha(cat(Buffer.from(p), Buffer.from([0]), b))))).toString('hex');
}
function entries(tree = input, overrides = {}) {
  return [['package.json', json({format: 'tachiko.portable-package', format_version: 1,
    payload_format: 'tachiko.roproj', payload_format_version: 3,
    payload_root_sha256: integrity(tree), ...overrides})],
    ...tree.map(([p, b]) => [`payload/${p}`, b])];
}
function zip(records, { flags = 0x0800, method = 0, time = 0, date = 0x21, descriptor = false } = {}) {
  const locals = [], centrals = [];
  let offset = 0;
  for (const [index, [p, b]] of records.entries()) {
    const entryFlags = descriptor && index === records.length - 1 ? 0x0808 : flags;
    const name = Buffer.from(p);
    const h = cat(u32(0x04034b50), u16(10), u16(entryFlags), u16(method), u16(time), u16(date),
      u32(crc(b)), u32(b.length), u32(b.length), u16(name.length), u16(0), name);
    locals.push(h, b);
    centrals.push(cat(u32(0x02014b50), u16(20), u16(10), u16(entryFlags), u16(method),
      u16(time), u16(date), u32(crc(b)), u32(b.length), u32(b.length), u16(name.length),
      u16(0), u16(0), u16(0), u16(0), u32(0), u32(offset), name));
    offset += h.length + b.length;
    if (descriptor && index === records.length - 1) {
      // f.jsonl is empty; zero local CRC/sizes are also the correct deferred fields.
      assert.equal(b.length, 0);
      const dataDescriptor = cat(u32(0x08074b50), u32(0), u32(0), u32(0));
      locals.push(dataDescriptor);
      offset += dataDescriptor.length;
    }
  }
  const central = cat(...centrals);
  return cat(...locals, central, u32(0x06054b50), u16(0), u16(0), u16(records.length),
    u16(records.length), u32(central.length), u32(offset), u16(0));
}
// Separate readback walk checks sizes, offsets, central agreement, CRC and root.
function selfCheck(bytes) {
  const end = bytes.length - 22;
  assert.equal(bytes.readUInt32LE(end), 0x06054b50);
  assert.equal(bytes.readUInt16LE(end + 10), 20);
  let l = 0, c = bytes.readUInt32LE(end + 16);
  const readback = [];
  for (let i = 0; i < 20; i++) {
    assert.equal(bytes.readUInt32LE(l), 0x04034b50);
    assert.equal(bytes.readUInt32LE(c), 0x02014b50);
    const length = bytes.readUInt16LE(l + 26), size = bytes.readUInt32LE(l + 18);
    const name = bytes.subarray(l + 30, l + 30 + length).toString();
    const body = bytes.subarray(l + 30 + length, l + 30 + length + size);
    assert.equal(bytes.subarray(c + 46, c + 46 + length).toString(), name);
    assert.equal(bytes.readUInt32LE(c + 42), l);
    assert.equal(bytes.readUInt32LE(l + 14), crc(body));
    assert.equal(bytes.readUInt32LE(c + 16), crc(body));
    readback.push([name, body]);
    l += 30 + length + size;
    c += 46 + length;
  }
  assert.equal(l, bytes.readUInt32LE(end + 16));
  assert.equal(c, end);
  assert.equal(c - l, bytes.readUInt32LE(end + 12));
  assert.deepEqual(readback.map(([p]) => p), ['package.json', ...paths.map(p => `payload/${p}`)]);
  assert.equal(readback[0][1].length, 228);
  const manifest = JSON.parse(readback[0][1]);
  const leaves = readback.slice(1).map(([p, b]) => {
    const hash = createHash('sha256'); hash.update(p.slice(8)); hash.update(Buffer.from([0])); hash.update(b);
    return hash.digest();
  });
  const rootHash = createHash('sha256');
  rootHash.update('tachiko.portable-package/v1'); rootHash.update(Buffer.from([0]));
  rootHash.update('tachiko.roproj/v3'); rootHash.update(Buffer.from([0]));
  for (const leaf of leaves) rootHash.update(leaf);
  assert.equal(rootHash.digest('hex'), manifest.payload_root_sha256);
  assert.equal(manifest.payload_root_sha256, integrity(input));
}
const golden = zip(entries());
selfCheck(golden);
const control = await Promise.all(paths.map(async p => [p, await readFile(resolve(root, 'runtime-control.roproj', p))]));
const vectors = new Map([['golden', golden], ['runtime-control', zip(entries(control))]]);
const cases = [];
function refusal(name, code, bytes) { vectors.set(name, bytes); cases.push({name, code: `portable_package.${code}`}); }
refusal('developmental', 'unsupported_payload_representation', Buffer.from((await readFile(resolve(root, '../issue-43-portable-package-v1/empty-package-v1.hex'), 'utf8')).trim(), 'hex'));
for (const [name, code, change] of [
  ['unknown-profile', 'invalid_manifest', {format: 'other.package'}],
  ['unknown-version', 'unsupported_version', {format_version: 2}],
  ['unknown-payload-profile', 'unsupported_payload_representation', {payload_format: 'other.payload'}],
  ['unknown-payload-version', 'unsupported_payload_representation', {payload_format_version: 4}],
  ['old-payload-claims-before-body', 'unsupported_payload_representation', {payload_format_version: 1}],
  ['integrity', 'integrity_mismatch', {payload_root_sha256: '0'.repeat(64)}],
]) refusal(name, code, zip(entries(input, change)));
const base = entries();
refusal('missing-entry', 'entry_set_mismatch', zip(base.slice(0, -1)));
refusal('extra-entry', 'entry_set_mismatch', zip([...base, ['payload/extra.json', Buffer.from('{}\n')]]));
refusal('duplicate-entry', 'entry_set_mismatch', zip([...base, base[2]]));
refusal('alias-entry', 'entry_set_mismatch', zip(base.map(([p, b]) => [p === 'payload/schemas.json' ? 'payload\\schemas.json' : p, b])));
const reordered = [...base]; [reordered[1], reordered[2]] = [reordered[2], reordered[1]];
refusal('entry-order', 'noncanonical_container', zip(reordered));
// Corrupt payload instead of manifest so dispatch succeeds before CRC verification.
const firstPayload = 30 + 'package.json'.length + 228;
const crcCorrupt = Buffer.from(golden); crcCorrupt[firstPayload + 30 + 'payload/manifest.json'.length + 5] ^= 1;
refusal('crc', 'crc_mismatch', crcCorrupt);
const mutateTree = (p, f) => input.map(([n, b]) => [n, n === p ? f(b) : b]);
refusal('inner-manifest', 'payload_manifest_mismatch', zip(entries(mutateTree('manifest.json', b => Buffer.from(b.toString().replace('"format_version": 3', '"format_version": 2'))))));
refusal('noncanonical-payload', 'noncanonical_payload', zip(entries(mutateTree('manifest.json', b => Buffer.from(`${JSON.stringify(JSON.parse(b))}\n`)))));
// Keep exact v3 DTO serialization and the ID-based shard; break entity-schema reference.
const semanticPath = input.find(([p,b]) => p.startsWith('entities/') && b.length > 0)[0];
const schemaMember = '"schema":"import_00000000-0000-4000-8000-000000000001_0002"';
assert(input.find(([p]) => p === semanticPath)[1].toString().includes(schemaMember));
refusal('invalid-semantic-payload', 'invalid_semantic_payload', zip(entries(mutateTree(semanticPath,
  b => Buffer.from(b.toString().replace(schemaMember, '"schema":"missing-schema"'))))));

for (const [name, options] of [
  ['compression', {method: 8}], ['encryption', {flags: 0x0801}],
  ['data-descriptor', {descriptor: true}], ['timestamp', {time: 1}],
]) refusal(name, 'noncanonical_container', zip(base, options));
refusal('truncated', 'invalid_container', golden.subarray(0, golden.length - 1));
refusal('trailing', 'invalid_container', cat(golden, Buffer.from([0])));
refusal('prepended', 'invalid_container', cat(Buffer.from([0]), golden));
const split = Buffer.from(golden); split.writeUInt16LE(1, split.length - 18);
refusal('split', 'invalid_container', split);
const zip64 = Buffer.from(golden); zip64.writeUInt32LE(0xffffffff, zip64.length - 6);
refusal('zip64-sentinel', 'invalid_container', zip64);
const future = depth => zip([['package.json', Buffer.from(`{"format":"tachiko.portable-package","format_version":2,"future":${'['.repeat(depth)}0${']'.repeat(depth)}}`)], ...base.slice(1)]);
refusal('manifest-depth-256', 'unsupported_version', future(255)); // outer object + 255 arrays
refusal('manifest-depth-257', 'resource_limit', future(256));
const manifestText = base[0][1].toString();
refusal('duplicate-manifest-member', 'invalid_manifest', zip([['package.json', Buffer.from(manifestText.replace('  "format":', '  "format": "tachiko.portable-package",\n  "format":'))], ...base.slice(1)]));
refusal('noncanonical-manifest', 'noncanonical_container', zip([['package.json', Buffer.from(JSON.stringify(JSON.parse(base[0][1]))+'\n')], ...base.slice(1)]));
refusal('malformed-version', 'invalid_manifest', zip([['package.json', Buffer.from(manifestText.replace('"format_version": 1', '"format_version": 1.0'))], ...base.slice(1)]));
const write = process.argv.slice(2).join(' ') === '--write';
assert(process.argv.length === 2 || write, 'usage: node probe.mjs [--write]');
for (const [name, bytes] of vectors) {
  const hex = `${bytes.toString('hex')}\n`;
  const path = resolve(root, `${name}.hex`);
  if (write) await writeFile(path, hex);
  else assert.equal(await readFile(path, 'utf8'), hex, `${name} fixture drift`);
}
const inventory = `${JSON.stringify(cases, null, 2)}\n`;
if (write) await writeFile(resolve(root, 'refusals.json'), inventory);
else assert.equal(await readFile(resolve(root, 'refusals.json'), 'utf8'), inventory);
console.log(JSON.stringify({profile:'tachiko.portable-package/v1', payload:'tachiko.roproj/v3',
  entries:20, bytes:golden.length, sha256:sha(golden).toString('hex'), integrity:integrity(input),
  emptyShards:input.slice(3).filter(([,b]) => b.length === 0).length, refusalVectors:cases.length}));
