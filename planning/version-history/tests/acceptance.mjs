// Real-boundary acceptance. Driver wiring is reviewed test-only composition.
import assert from 'node:assert/strict';
import { access } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { test } from 'node:test';
import * as o from './oracles.mjs';
const fixture = process.env.TACHIKO_HISTORY_FIXTURE;
const modulePath = process.env.TACHIKO_HISTORY_DRIVER;
function blocked(message) { console.error(`BLOCKED: ${message}`); process.exit(78); }
if (!fixture || !modulePath) blocked('set TACHIKO_HISTORY_FIXTURE and TACHIKO_HISTORY_DRIVER; no product RED claimed');
try { for (const f of o.FILES) await access(path.join(fixture,f)); await access(modulePath); }
catch (e) { blocked(`missing fixture/driver: ${e.message}`); }
let createDriver;
try { ({ createDriver } = await import(pathToFileURL(path.resolve(modulePath)).href)); }
catch (e) { blocked(`driver import: ${e.message}`); }
if (typeof createDriver !== 'function') blocked('driver must export createDriver');
const methods = ['observe','persisted','keepVersion','versions','edit','compare','restore','restart','fault','retryPersistence','exportSnapshot','openIndependent','close'];
// Qualify setup/admission before registering behavioral tests. A missing seam
// in an admitted product may be RED; a broken adapter/fixture cannot be.
let probe;
try {
  probe = await createDriver({fixture});
  for (const m of methods) assert.equal(typeof probe[m], 'function', `missing driver method ${m}`);
  o.values(await probe.observe(), 5, 10, o.NOTE_A);
  await probe.edit({impact:3});
  o.values(await probe.observe(), 3, 8, o.NOTE_A);
} catch (e) {
  if (probe && typeof probe.close === 'function') await probe.close().catch(() => {});
  blocked(`existing-boundary qualification failed: ${e.message}`);
}
await probe.close();
console.log('Existing fixture admission and 5/10 -> 3/8 qualified; history cases follow.');
async function withDriver(fn) {
  const d = await createDriver({fixture});
  try {
    for (const m of methods) assert.equal(typeof d[m],'function',`unqualified driver: ${m}`);
    const a = await d.observe(); o.values(a,5,10,o.NOTE_A);
    await fn(d,a);
  } finally { await d.close(); }
}
async function two(d,a) {
  const ca = await d.keepVersion('調整前', {expectedOccurrence:a.occurrence,expectedRevision:a.revision});
  o.checkpoint(ca,a); o.unchanged(a,await d.observe());
  await d.edit({impact:3,notes:o.NOTE_B});
  const b=await d.observe(); o.values(b,3,8,o.NOTE_B);
  const cb=await d.keepVersion('調整後',{expectedOccurrence:b.occurrence,expectedRevision:b.revision});
  o.checkpoint(cb,b); o.unchanged(b,await d.observe());
  return {ca,cb,b};
}
const cases = [
  ['H01 named snapshots survive restart', async(d,a) => {
    const {ca,cb,b}=await two(d,a); const before=await d.versions();
    assert.ok(before.some(x=>x.id===ca.id)); assert.ok(before.some(x=>x.id===cb.id));
    o.savedFiles(await d.persisted(),b);
    await d.restart(); const after=await d.observe(); o.equivalent(b,after);
    assert.notEqual(after.occurrence,b.occurrence,'fresh runtime occurrence');
    assert.deepEqual(await d.versions(),before,'immutable retained checkpoints');
  }],
  ['H02 read-only exact net comparison', async(d,a) => {
    const {ca,b}=await two(d,a); const versions=await d.versions();
    const preview=await d.compare(ca.id); o.exactDiff(preview,b);
    o.unchanged(b,await d.observe()); assert.deepEqual(await d.versions(),versions);
  }],
  ['H03 forward restore retains B and persists', async(d,a) => {
    const {ca,cb,b}=await two(d,a); const prior=await d.versions();
    const p=await d.compare(ca.id); const r=await d.restore(ca.id,p); o.committed(r);
    const c=await d.observe(); o.forward(a,b,c); o.savedFiles(await d.persisted(),c);
    const after=await d.versions();
    for (const cp of prior) assert.deepEqual(after.find(x=>x.id===cp.id),cp);
    assert.ok(after.some(x=>x.id!==ca.id&&x.id!==cb.id),'new restore checkpoint');
    await d.restart(); o.equivalent(a,await d.observe());
    assert.deepEqual(await d.versions(),after);
  }],
  ['H04 stale preview cannot restore', async(d,a) => {
    const {ca}=await two(d,a); const p=await d.compare(ca.id);
    await d.edit({impact:4}); const current=await d.observe();
    const r=await d.restore(ca.id,p); assert.equal(r.semantic,'stale');
    o.unchanged(current,await d.observe());
  }],
  ['H05 safety-capture failure precedes mutation', async(d,a) => {
    const {ca}=await two(d,a); await d.edit({impact:4});
    const current=await d.observe(), saved=await d.persisted(), prior=await d.versions();
    const p=await d.compare(ca.id); await d.fault('fail-safety-capture');
    const r=await d.restore(ca.id,p); assert.equal(r.semantic,'not-published');
    assert.equal(r.persistence,'failed'); o.unchanged(current,await d.observe());
    assert.deepEqual(await d.persisted(),saved); assert.deepEqual(await d.versions(),prior);
  }],
  ['H06 post-publication persistence failure never replays mutation', async(d,a) => {
    const {ca,b}=await two(d,a); const saved=await d.persisted();
    const p=await d.compare(ca.id); await d.fault('fail-restore-commit');
    const r=await d.restore(ca.id,p);
    assert.equal(r.semantic,'published'); assert.equal(r.persistence,'failed');
    const c=await d.observe(); o.forward(a,b,c); assert.deepEqual(await d.persisted(),saved);
    const retry=await d.retryPersistence(); assert.equal(retry.persistence,'committed');
    o.unchanged(c,await d.observe()); o.savedFiles(await d.persisted(),c);
    await d.restart(); o.equivalent(a,await d.observe());
  }],
  ['H07 corrupt historical snapshot does not destroy good current', async(d,a) => {
    const {ca,b}=await two(d,a); await d.fault('corrupt-historical-snapshot',{checkpointId:ca.id});
    await d.restart(); const current=await d.observe(); o.equivalent(b,current);
    const p=await d.compare(ca.id); assert.equal(p.status,'history-unavailable');
    assert.notEqual(p.reason,'empty'); const r=await d.restore(ca.id,p);
    assert.equal(r.semantic,'not-published'); o.unchanged(current,await d.observe());
  }],
  ['H08 same content has distinct checkpoint identity, restore is NoChange', async(d,a) => {
    const ca=await d.keepVersion('同じ名前',{expectedOccurrence:a.occurrence,expectedRevision:a.revision});
    const cb=await d.keepVersion('同じ名前',{expectedOccurrence:a.occurrence,expectedRevision:a.revision});
    o.checkpoint(ca,a); o.checkpoint(cb,a); assert.notEqual(ca.id,cb.id);
    o.unchanged(a,await d.observe()); const p=await d.compare(ca.id);
    assert.equal(p.complete,true); assert.deepEqual(p.changes,[]);
    const r=await d.restore(ca.id,p); assert.equal(r.semantic,'nochange');
    o.unchanged(a,await d.observe());
  }],
  ['H09 export is snapshot-only and copy cannot inherit history implicitly', async(d,a) => {
    const {ca}=await two(d,a); const exported=await d.exportSnapshot(ca.id);
    assert.deepEqual(exported.canonicalFiles,a.canonicalFiles);
    const x=await d.openIndependent(exported);
    try {
      o.equivalent(a,await x.observe()); assert.deepEqual(await x.versions(),[]);
      const p=await x.compare(ca.id); assert.equal(p.status,'history-unavailable');
    } finally { await x.close(); }
  }],
  ['H10 unsupported history cannot authorize restore', async(d,a) => {
    const {ca,b}=await two(d,a); await d.fault('unsupported-history-version',{checkpointId:ca.id});
    await d.restart(); const current=await d.observe(); o.equivalent(b,current);
    const p=await d.compare(ca.id); assert.equal(p.status,'history-unavailable');
    assert.equal(p.reason,'unsupported-version');
    const r=await d.restore(ca.id,{...p,complete:false});
    assert.equal(r.semantic,'not-published'); o.unchanged(current,await d.observe());
  }],
];
for (const [name,fn] of cases) test(name,{concurrency:false,timeout:120000},()=>withDriver(fn));
