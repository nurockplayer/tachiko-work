// Real browser UI acceptance. No test hook performs semantic mutation.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { mkdtemp, rm, readFile, access } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import * as o from './oracles.mjs';
function blocked(s) { console.error(`BLOCKED: ${s}`); process.exit(78); }
const fixture=process.env.TACHIKO_HISTORY_FIXTURE;
const url=process.env.WORK_CLIENT_URL;
if (!fixture || !url) blocked('set TACHIKO_HISTORY_FIXTURE and WORK_CLIENT_URL; no browser RED claimed');
let chromium;
try {
  for (const f of o.FILES) await access(path.join(fixture,f));
  const spec=process.env.WORK_PLAYWRIGHT_MODULE;
  ({chromium}=await import(spec ? pathToFileURL(path.resolve(spec)).href : 'playwright'));
} catch (e) { blocked(`fixture/Playwright prerequisite: ${e.message}`); }
const original=Object.fromEntries(await Promise.all(o.FILES.map(async f=>[f,await readFile(path.join(fixture,f),'utf8')])));
async function launch(profile) {
  return chromium.launchPersistentContext(profile,{
    headless:true,
    ...(process.env.WORK_CHROMIUM ? {executablePath:process.env.WORK_CHROMIUM} : {}),
  });
}
async function openFixture(page) {
  await page.goto(url);
  await page.getByTestId('open-project').setInputFiles(path.resolve(fixture));
  await page.getByTestId('project-ready').waitFor();
}
async function observe(page) { return page.evaluate(()=>window.__tachikoHistoryAcceptance.observe()); }
async function versions(page) { return page.evaluate(()=>window.__tachikoHistoryAcceptance.versions()); }
async function persisted(page) { return page.evaluate(()=>window.__tachikoHistoryAcceptance.persisted()); }
async function keep(page,name) {
  await page.getByRole('button',{name:'Keep version',exact:true}).click();
  const dialog=page.getByRole('dialog',{name:'Keep version',exact:true});
  await dialog.getByLabel('Version name',{exact:true}).fill(name);
  await dialog.getByRole('button',{name:'Keep version',exact:true}).click();
  await page.waitForFunction(async label => (await window.__tachikoHistoryAcceptance.versions()).some(v=>v.name===label),name);
  return (await versions(page)).find(v=>v.name===name);
}
async function impact(page,value) {
  await page.getByRole('tab',{name:'Table',exact:true}).click();
  await page.getByTestId(`cell:${o.ENTITY}:${o.IMPACT}`).dblclick();
  const editor=page.getByRole('textbox',{name:'Edit cell',exact:true});
  await editor.fill(String(value)); await editor.press('Enter');
  await page.waitForFunction(async n => (await window.__tachikoHistoryAcceptance.observe()).impact===n,value);
}
async function notes(page,text) {
  await page.getByRole('tab',{name:'Brief',exact:true}).click();
  await page.getByRole('textbox',{name:'Decision notes',exact:true}).fill(text);
  await page.getByRole('button',{name:'Apply notes',exact:true}).click();
  await page.waitForFunction(async t => (await window.__tachikoHistoryAcceptance.observe()).notes===t,text);
}
async function restoreDialog(page,id) {
  await page.getByRole('button',{name:'Versions',exact:true}).click();
  await page.getByTestId(`version:${id}`).getByRole('button',{name:'Restore version',exact:true}).click();
  const dialog=page.getByRole('dialog',{name:'Restore version',exact:true});
  await dialog.waitFor(); return dialog;
}
// Missing application or observation hook is setup BLOCKED, not a product defect.
const preflightProfile=await mkdtemp(path.join(os.tmpdir(),'tachiko-history-preflight-'));
let preflight;
try {
  preflight=await launch(preflightProfile); const p=await preflight.newPage(); await openFixture(p);
  assert.equal(await p.evaluate(()=>typeof window.__tachikoHistoryAcceptance?.observe),'function');
  assert.equal(await p.evaluate(()=>typeof window.__tachikoHistoryAcceptance?.versions),'function');
  assert.equal(await p.evaluate(()=>typeof window.__tachikoHistoryAcceptance?.persisted),'function');
  o.values(await observe(p),5,10,o.NOTE_A);
} catch(e) {
  if(preflight) await preflight.close(); await rm(preflightProfile,{recursive:true,force:true});
  blocked(`existing client/hook/fixture qualification: ${e.message}`);
}
await preflight.close(); await rm(preflightProfile,{recursive:true,force:true});
async function inBrowser(fn) {
  const profile=await mkdtemp(path.join(os.tmpdir(),'tachiko-history-ui-'));
  let context;
  const host={
    async start(){context=await launch(profile); const page=await context.newPage(); await openFixture(page); return page;},
    async restart(workId){
      await context.close(); context=await launch(profile); const page=await context.newPage(); await page.goto(url);
      await page.getByTestId(`open-history:${workId}`).click(); await page.getByTestId('project-ready').waitFor(); return page;
    },
  };
  try {await fn(host);}
  finally {
    if(context) await context.close();
    await rm(profile,{recursive:true,force:true});
    const after=Object.fromEntries(await Promise.all(o.FILES.map(async f=>[f,await readFile(path.join(fixture,f),'utf8')])));
    assert.deepEqual(after,original,'input fixture/source remains unchanged');
  }
}
test('HU01 real controls, linked work, forward restore and full process restart',{timeout:180000},()=>inBrowser(async host=>{
  let page=await host.start(); const a=await observe(page); o.values(a,5,10,o.NOTE_A);
  const ca=await keep(page,'調整前'); await impact(page,3); await notes(page,o.NOTE_B);
  const b=await observe(page); o.values(b,3,8,o.NOTE_B);
  const cb=await keep(page,'調整後'); const state=await observe(page);
  assert.ok(typeof state.localWorkId==='string' && state.localWorkId);
  o.savedFiles(await persisted(page),b);
  page=await host.restart(state.localWorkId); const reopened=await observe(page);
  o.equivalent(b,reopened); assert.notEqual(reopened.occurrence,b.occurrence);
  await page.getByRole('button',{name:'Versions',exact:true}).click();
  await page.getByTestId(`version:${ca.id}`).getByRole('button',{name:'Compare with current',exact:true}).click();
  const changes=page.getByRole('region',{name:'Version comparison',exact:true});
  await changes.waitFor(); assert.match(await changes.innerText(),/Impact/);
  o.unchanged(reopened,await observe(page));
  const dialog=await restoreDialog(page,ca.id);
  await dialog.getByRole('button',{name:'Restore version',exact:true}).click();
  await page.waitForFunction(async()=>{
    const h=window.__tachikoHistoryAcceptance;
    const s=await h.observe(), p=await h.persisted();
    return s.impact===5 && p.status==='confirmed' && Object.keys(p.canonicalFiles).length===Object.keys(s.canonicalFiles).length && Object.entries(s.canonicalFiles).every(([k,v])=>p.canonicalFiles[k]===v);
  });
  const restored=await observe(page); o.forward(a,reopened,restored);
  const retained=await versions(page); assert.ok(retained.some(v=>v.id===ca.id)); assert.ok(retained.some(v=>v.id===cb.id));
  page=await host.restart(state.localWorkId); o.equivalent(a,await observe(page));
  assert.deepEqual(await versions(page),retained);
}));
test('HU02 uncommitted Brief draft and cancel are non-destructive',{timeout:120000},()=>inBrowser(async host=>{
  const page=await host.start(); const ca=await keep(page,'調整前'); await impact(page,3);
  await page.getByRole('tab',{name:'Brief',exact:true}).click();
  const draft='まだ保存していないメモ。';
  await page.getByRole('textbox',{name:'Decision notes',exact:true}).fill(draft);
  const before=await observe(page);
  // Pending-draft handling may show this confirmation instead of a preview.
  await page.getByRole('button',{name:'Versions',exact:true}).click();
  await page.getByTestId(`version:${ca.id}`).getByRole('button',{name:'Restore version',exact:true}).click();
  await page.getByRole('dialog').getByRole('button',{name:'Keep editing',exact:true}).click();
  o.unchanged(before,await observe(page));
  await page.getByRole('tab',{name:'Brief',exact:true}).click();
  assert.equal(await page.getByRole('textbox',{name:'Decision notes',exact:true}).inputValue(),draft);
}));
