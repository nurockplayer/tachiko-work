// Steward assertions, not a semantic engine or product implementation.
import assert from 'node:assert/strict';
export const ENTITY = '1d37df46-01f6-4b05-8fd9-064718dc91ea';
export const IMPACT = '52a0f7e9-4d6b-4628-ad7d-6b649f8c8a77';
export const NOTES = 'b4284d96-4370-4f42-9c0e-4478f947e159';
export const NOTE_A = '先完成團隊每天會用到的流程，再增加進階功能。';
export const NOTE_B = '先完成試玩回饋，再決定下一版範圍。';
export const FILES = ['manifest.json', 'schemas.json', ...'0123456789abcdef'.split('').map(x => `entities/${x}.jsonl`)].sort();
export function snapshot(s) {
  for (const key of ['documentId', 'occurrence', 'revision']) assert.ok(typeof s[key] === 'string' && s[key], key);
  assert.deepEqual(Object.keys(s.canonicalFiles).sort(), FILES, 'complete v1 codec output, including empty shards');
  for (const value of Object.values(s.canonicalFiles)) assert.equal(typeof value, 'string');
}
export function values(s, impact, priority, notes) {
  snapshot(s);
  assert.equal(s.impact, impact);
  assert.equal(s.priority, priority, 'runtime-calculated fact');
  assert.equal(s.notes, notes);
}
export function unchanged(a, b) {
  snapshot(a); snapshot(b);
  assert.equal(b.documentId, a.documentId);
  assert.equal(b.occurrence, a.occurrence);
  assert.equal(b.revision, a.revision);
  assert.deepEqual(b.canonicalFiles, a.canonicalFiles);
}
export function equivalent(a, b) {
  snapshot(a); snapshot(b);
  assert.equal(b.documentId, a.documentId);
  assert.deepEqual(b.canonicalFiles, a.canonicalFiles, 'exact authoritative canonical state');
  assert.deepEqual([b.impact, b.priority, b.notes], [a.impact, a.priority, a.notes]);
}
export function forward(a, b, restored) {
  equivalent(a, restored);
  assert.equal(restored.occurrence, b.occurrence, 'restore is a publication, not reopening A');
  assert.notEqual(restored.revision, b.revision, 'new current-base revision');
}
export function checkpoint(c, s) {
  assert.ok(typeof c.id === 'string' && c.id);
  assert.equal(c.documentId, s.documentId);
  assert.equal(c.profile, 'snapshot-only');
  assert.equal(c.committed, true);
  assert.deepEqual(c.canonicalFiles, s.canonicalFiles);
}
export function exactDiff(p, current, oldNotes = NOTE_A) {
  assert.equal(p.complete, true, 'never publish an incomplete diff as whole restore');
  assert.deepEqual(p.base, { occurrence: current.occurrence, revision: current.revision });
  const expected = [
    { entity: ENTITY, field: IMPACT, before: {kind:'number',value:5}, after: {kind:'number',value:3} },
    { entity: ENTITY, field: NOTES, before: {kind:'text',value:oldNotes}, after: {kind:'text',value:NOTE_B} },
  ];
  const sort = xs => [...xs].sort((a,b) => { const x=`${a.entity}/${a.field}`, y=`${b.entity}/${b.field}`; return x < y ? -1 : x > y ? 1 : 0; });
  assert.deepEqual(sort(p.changes), sort(expected), 'net stored changes only; no fabricated formula edit');
}
export function committed(r) {
  assert.equal(r.semantic, 'published');
  assert.equal(r.persistence, 'committed');
}
export function savedFiles(p, s) {
  assert.equal(p.status, 'confirmed');
  assert.deepEqual(p.canonicalFiles, s.canonicalFiles);
}
