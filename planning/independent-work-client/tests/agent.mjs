// Integration-driver names are test wiring, NOT a stable public API or authority.
// Supply a reviewed driver around the real Rust lifecycle and trusted host domain.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {fileURLToPath,pathToFileURL} from 'node:url';
const name=process.env.WORK_AGENT_DRIVER;
if(!name){console.error('BLOCKED: WORK_AGENT_DRIVER must provide real runtime/host wiring.');process.exit(78);}
const {createDriver}=await import(pathToFileURL(name).href);
const expected=JSON.parse(await readFile(new URL('./fixtures/expected.json',import.meta.url),'utf8'));
const fixture=fileURLToPath(new URL('./fixtures/release-plan.roproj',import.meta.url));
const intent={target:{entity:expected.entity,field:expected.impact},value:3};
const cases=[
 ['M2-01 propose/preview never publish',async d=>{const a=await d.observe();const p=await d.propose(intent);await d.preview(p);assert.deepEqual(await d.observe(),a);}],
 ['M2-02 delegated execute requires exact approval',async d=>{const p=await d.propose(intent);const a=await d.observe();assert.equal((await d.execute(p)).outcome,'denied');assert.deepEqual(await d.observe(),a);}],
 ['M2-03 approved publication and replay',async d=>{const p=await d.propose(intent);await d.approveAsHuman(p);const r=await d.execute(p);assert.equal(r.outcome,'published');const a=await d.observe();assert.equal(a.impact,3);assert.equal(a.priority,8);assert.notEqual((await d.execute(p)).outcome,'published');assert.deepEqual(await d.observe(),a);assert.equal(await d.externalEffectCount(),0);}],
 ['M2-04 intervening human edit invalidates exact proposal',async d=>{const p=await d.propose(intent);await d.approveAsHuman(p);await d.editAsHuman({...intent,value:4});const a=await d.observe();assert.notEqual((await d.execute(p)).outcome,'published');assert.deepEqual(await d.observe(),a);}],
 ['M2-05 revoked authority cannot publish',async d=>{const p=await d.propose(intent);await d.approveAsHuman(p);await d.revokeDelegatedAuthority();const a=await d.observe();assert.equal((await d.execute(p)).outcome,'denied');assert.deepEqual(await d.observe(),a);}],
 ['M2-06 unavailable scope reveals no evidence',async d=>{const p=await d.propose(intent);await d.revokeQueryAuthority();const r=await d.preview(p);assert.equal(r.outcome,'denied');assert.deepEqual(r.disclosedSubjects,[]);assert.deepEqual(r.disclosedValues,[]);}],
 ['M2-07 proposal content cannot change under one identity',async d=>{const p=await d.propose(intent);await d.approveAsHuman(p);const a=await d.observe();assert.equal((await d.executeAltered(p,{...intent,value:9})).outcome,'denied');assert.deepEqual(await d.observe(),a);}],
 ['M2-08 close/reopen prevents approval reuse',async d=>{const p=await d.propose(intent);await d.approveAsHuman(p);await d.reopenFixture();const a=await d.observe();assert.notEqual((await d.execute(p)).outcome,'published');assert.deepEqual(await d.observe(),a);}],
];
let failures=0;
for(const [name,run] of cases){let d;try{d=await createDriver({fixture});await run(d);console.log(JSON.stringify({case:name,status:'PASS'}));}catch(e){failures++;console.error(JSON.stringify({case:name,status:'FAIL',message:String(e.stack??e)}));}finally{await d?.close();}}
if(failures)process.exit(1);
