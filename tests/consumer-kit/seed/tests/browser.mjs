// Steward acceptance, run with real production composition plus observation/fault hooks.
// Hooks may observe authoritative state or fail a host/transport operation; never fake results.
import assert from 'node:assert/strict';
import {mkdtemp,rm,readFile,readdir} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {linked,rejected,saveFailed,reopened,unknown} from './oracles.mjs';
const block=message=>{console.error(`BLOCKED: ${message}`);process.exit(78);};
const url=process.env.WORK_CLIENT_URL;
if(!url)block('WORK_CLIENT_URL is required; no real client/canary has been supplied.');
let chromium;
try{({chromium}=await import(process.env.WORK_PLAYWRIGHT_MODULE??'playwright-core'));}
catch{block('Install the pinned Playwright dependency or supply WORK_PLAYWRIGHT_MODULE.');}
const fixture=fileURLToPath(new URL('./fixtures/release-plan.roproj',import.meta.url));
const expected=JSON.parse(await readFile(new URL('./fixtures/expected.json',import.meta.url),'utf8'));
const cell=field=>`cell:${expected.entity}:${field}`;
const launchOptions={headless:true,...(process.env.WORK_CHROMIUM?{executablePath:process.env.WORK_CHROMIUM}:{})};
async function sourceHash(){
 const h=createHash('sha256');
 async function scan(dir,prefix=''){for(const name of (await readdir(dir)).sort()){const rel=prefix+name;if(name==='entities'){await scan(path.join(dir,name),rel+'/');continue;}h.update(rel);h.update('\0');h.update(await readFile(path.join(dir,name)));}}
 await scan(fixture);return h.digest('hex');
}
async function shown(page,id,value){
 await page.waitForFunction(({id,value})=>Array.from(document.querySelectorAll('[data-testid]')).some(el=>el.dataset.testid===id&&el.textContent.trim()===String(value)),{id,value});
}
async function open(page){await page.goto(url);await page.getByTestId('open-project').setInputFiles(fixture);await page.getByTestId('project-ready').waitFor();}
async function snapshot(page){return page.evaluate(()=>window.__tachikoAcceptance.observe());}
async function rendered(page,view){
 const id=f=>view==='table'?cell(f):`brief:${expected.entity}:${f}`;
 const a=page.getByTestId(id(expected.impact)), b=page.getByTestId(id(expected.priority));
 await a.waitFor();await b.waitFor();
 const attributes=['data-work-occurrence','data-work-revision','data-work-entity','data-work-currentness'];
 const facts={};
 for(const attribute of attributes){facts[attribute]=await a.getAttribute(attribute);assert.equal(await b.getAttribute(attribute),facts[attribute],`Mixed ${attribute} within one view`);}
 return {occurrence:facts[attributes[0]],revision:facts[attributes[1]],entity:facts[attributes[2]],currentness:facts[attributes[3]],impact:Number((await a.textContent()).trim()),priority:Number((await b.textContent()).trim())};
}
async function edit(page,value){
 await page.getByTestId(cell(expected.impact)).dblclick();
 const input=page.getByRole('textbox',{name:'Edit cell',exact:true});await input.fill(value);await input.press('Enter');
}
async function notes(page){
 await page.getByRole('tab',{name:'Brief',exact:true}).click();
 const input=page.getByRole('textbox',{name:'Decision notes',exact:true});
 await input.fill(expected.editedNotes);await page.getByRole('button',{name:'Apply notes',exact:true}).click();
 await page.waitForFunction(text=>window.__tachikoAcceptance.observe().then(s=>s.notes===text),expected.editedNotes);
}
async function save(page,name){
 await page.getByRole('button',{name:'Save a copy',exact:true}).click();
 await page.getByRole('textbox',{name:'Copy name',exact:true}).fill(name);
 await page.getByRole('button',{name:'Create copy',exact:true}).click();
}
if(process.argv.includes('--canary')){
 const browser=await chromium.launch(launchOptions);
 try{
  const page=await browser.newPage();await page.goto(url);await page.locator('#fixture').setInputFiles(fixture);await page.locator('#run').click();
  await page.waitForFunction(()=>['complete','failed'].includes(document.querySelector('#result').dataset.status));
  assert.equal(await page.locator('#result').getAttribute('data-status'),'complete',await page.locator('#result').textContent());
  const o=JSON.parse(await page.locator('#result').textContent());
  assert.equal(o.initialImpact,5);assert.equal(o.initialPriority,10);assert.equal(o.initialNotes,expected.initialNotes);
  assert.equal(o.impact,3);assert.equal(o.priority,8);assert.equal(o.baseChanged,true);
  assert.equal(o.staleCode,'stale_revision');assert.equal(o.staleUnchanged,true);
  assert.equal(o.reopenedImpact,3);assert.equal(o.reopenedPriority,8);assert.equal(o.rowCount,3);
  console.log(JSON.stringify({case:'real-runtime-canary',status:'PASS',observation:o}));
 }finally{await browser.close();}
 process.exit(0);
}
const humanCases=[
 ['M1-01 linked views and restart durability',async env=>{
  let {page}=env;const initial=await snapshot(page);await edit(page,'3');await shown(page,cell(expected.priority),8);await notes(page);
  const confirmed=await snapshot(page);
  await page.getByRole('tab',{name:'Table',exact:true}).click();const sheet=await rendered(page,'table');
  await page.getByRole('tab',{name:'Brief',exact:true}).click();const brief=await rendered(page,'brief');
  linked({sheet,brief,occurrence:confirmed.occurrence,revision:confirmed.revision,baseRevision:initial.revision,entity:expected.entity,notes:confirmed.notes,sourceHashBefore:env.source,sourceHashAfter:await sourceHash()});
  await save(page,'review-copy');await shown(page,'save-status','Saved on this device');
  await env.restart();page=env.page;await page.goto(url);
  await page.getByRole('button',{name:'Open saved review-copy',exact:true}).click();await page.getByTestId('project-ready').waitFor();
  reopened({before:confirmed,after:await snapshot(page),browserProcessRestarted:true,authoritativeRead:true});
 }],
 ['M1-02 invalid edit is atomic and retains draft',async({page})=>{
  const before=await snapshot(page);await edit(page,'not a number');await page.getByRole('alert').waitFor();
  const input=page.getByRole('textbox',{name:'Edit cell',exact:true});
  rejected({before,after:await snapshot(page),draftRetained:(await input.inputValue())==='not a number'});
  await input.press('Escape');await shown(page,cell(expected.impact),5);
 }],
 ['M1-03 IME composition does not publish',async({page})=>{
  await page.getByRole('tab',{name:'Brief',exact:true}).click();const before=await snapshot(page);
  const input=page.getByRole('textbox',{name:'Decision notes',exact:true});await input.focus();
  await input.evaluate(el=>{el.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true}));el.value='試玩';el.dispatchEvent(new InputEvent('input',{bubbles:true,inputType:'insertCompositionText',data:'試玩',isComposing:true}));el.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',code:'Enter',bubbles:true,isComposing:true}));});
  assert.deepEqual(await snapshot(page),before);assert.equal(await input.inputValue(),'試玩');
  await input.evaluate(el=>el.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true,data:'試玩'})));
 }],
 ['M1-04 failed save keeps work and destination',async({page})=>{
  await edit(page,'3');await shown(page,cell(expected.priority),8);const before=await snapshot(page);
  const destination=await page.evaluate(()=>window.__tachikoAcceptance.savedHash('quota-copy'));
  await page.evaluate(()=>window.__tachikoAcceptance.failNextSave());await save(page,'quota-copy');await page.getByRole('alert').waitFor();
  const after=await snapshot(page);const ui=await page.evaluate(()=>window.__tachikoAcceptance.saveObservation());
  saveFailed({...ui,semanticRevision:after.revision,publishedRevision:before.revision,destinationHashBefore:destination,destinationHashAfter:await page.evaluate(()=>window.__tachikoAcceptance.savedHash('quota-copy'))});
  assert.equal(after.canonicalHash,before.canonicalHash);
 }],
 ['M1-05 lost Execute reply is unknown, not retried',async({page})=>{
  const start=await page.evaluate(()=>window.__tachikoAcceptance.executeRequestCount());
  await page.evaluate(()=>window.__tachikoAcceptance.loseNextExecuteReply());await edit(page,'3');await shown(page,'operation-outcome','Outcome unknown');
  // The hook resolves only after the real transport's bounded reconciliation window.
  await page.evaluate(()=>window.__tachikoAcceptance.settleFaultWindow());
  const o=await page.evaluate(()=>window.__tachikoAcceptance.unknownObservation());
  unknown({...o,executeRequests:(await page.evaluate(()=>window.__tachikoAcceptance.executeRequestCount()))-start});
 }],
 ['M1-06 keyboard cancel and safe close',async({page})=>{
  const before=await snapshot(page);await page.getByTestId(cell(expected.impact)).focus();await page.keyboard.press('Enter');
  const input=page.getByRole('textbox',{name:'Edit cell',exact:true});await input.fill('7');await input.press('Escape');
  assert.deepEqual(await snapshot(page),before);assert.equal(await page.getByTestId(cell(expected.impact)).evaluate(el=>el===document.activeElement),true);
  await edit(page,'3');await shown(page,cell(expected.priority),8);await page.getByRole('button',{name:'Close project',exact:true}).click();
  const dialog=page.getByRole('dialog',{name:'Unsaved work',exact:true});await dialog.waitFor();await dialog.getByRole('button',{name:'Keep editing',exact:true}).click();await shown(page,cell(expected.impact),3);
 }],
];
const agentCases=[
 ['M2-UI-01 review is non-mutating; explicit approved apply publishes',async({page})=>{
  const before=await snapshot(page);
  await page.getByRole('button',{name:'Suggest a change',exact:true}).click();
  const review=page.getByRole('dialog',{name:'Proposed changes',exact:true});await review.waitFor();
  assert.deepEqual(await snapshot(page),before);
  assert.equal(await review.getByTestId('proposal-source').textContent(),'Deterministic test provider');
  assert.equal(await review.getByTestId('proposal-target').getAttribute('data-work-entity'),expected.entity);
  assert.equal(await review.getByTestId('proposal-before').textContent(),'5');assert.equal(await review.getByTestId('proposal-after').textContent(),'3');
  await review.getByRole('button',{name:'Approve and apply',exact:true}).click();await shown(page,cell(expected.priority),8);
  const receipt=await page.evaluate(()=>window.__tachikoAcceptance.lastReceipt());assert.equal(receipt.origin,'delegated');assert.equal(receipt.outcome,'published');assert.equal(receipt.externalEffects,0);
 }],
 ['M2-UI-02 rejecting a suggestion changes nothing',async({page})=>{
  const before=await snapshot(page);await page.getByRole('button',{name:'Suggest a change',exact:true}).click();
  const review=page.getByRole('dialog',{name:'Proposed changes',exact:true});await review.waitFor();await review.getByRole('button',{name:'Reject suggestion',exact:true}).click();
  assert.deepEqual(await snapshot(page),before);
 }],
 ['M2-UI-03 stale review requires a new proposal',async({page})=>{
  await page.getByRole('button',{name:'Suggest a change',exact:true}).click();await page.getByRole('dialog',{name:'Proposed changes',exact:true}).waitFor();
  await page.getByRole('button',{name:'Keep proposal for later',exact:true}).click();await edit(page,'4');await shown(page,cell(expected.priority),9);const before=await snapshot(page);
  await page.getByRole('button',{name:'Review pending proposal',exact:true}).click();const review=page.getByRole('dialog',{name:'Proposed changes',exact:true});await review.waitFor();
  const apply=review.getByRole('button',{name:'Approve and apply',exact:true});assert.equal(await apply.isDisabled(),true);
  await review.getByText('Work changed; request a new proposal',{exact:true}).waitFor();assert.deepEqual(await snapshot(page),before);
 }],
];
const cases=process.argv.includes('--agent')?agentCases:humanCases;
let failures=0;
for(const [name,run] of cases){
 const profile=await mkdtemp(path.join(tmpdir(),'tachiko-client-acceptance-'));
 let context;const remote=[];
 try{
  const env={source:await sourceHash(),page:null,restart:async()=>{if(context)await context.close();context=await chromium.launchPersistentContext(profile,launchOptions);
   await context.route('**/*',route=>{const requested=new URL(route.request().url());if(['http:','https:','ws:','wss:'].includes(requested.protocol)&&requested.origin!==new URL(url).origin){remote.push(requested.origin);return route.abort();}return route.continue();});
   env.page=context.pages()[0]??await context.newPage();}};
  await env.restart();await open(env.page);
  const wired=await env.page.evaluate(()=>Boolean(window.__tachikoAcceptance?.observe));
  if(!wired)throw new Error('UNQUALIFIED HARNESS: observation/fault driver is missing; not behavioral RED');
  await run(env);assert.equal(await sourceHash(),env.source);assert.deepEqual(remote,[],'Default work sent data to an external origin');
  console.log(JSON.stringify({case:name,status:'PASS'}));
 }catch(error){failures++;console.error(JSON.stringify({case:name,status:'FAIL',message:String(error.stack??error)}));}
 finally{if(context)await context.close();await rm(profile,{recursive:true,force:true});}
}
if(failures)process.exit(1);
