// Test wiring around genuine codec/runtime boundaries; no production parser here.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {fileURLToPath,pathToFileURL} from 'node:url';
const name=process.env.WORK_STORAGE_DRIVER;
if(!name){console.error('BLOCKED: WORK_STORAGE_DRIVER must wrap the real consumer kit and core storage codec.');process.exit(78);}
const {createDriver}=await import(pathToFileURL(name).href);
const x=JSON.parse(await readFile(new URL('./fixtures/expected.json',import.meta.url),'utf8'));
const fixture=fileURLToPath(new URL('./fixtures/release-plan.roproj',import.meta.url));
const decoder=new TextDecoder(),encoder=new TextEncoder();
const cases=[
 ['C1-01 canonical export is a complete v1 tree',async d=>{
  await d.editImpact(3);await d.editNotes(x.editedNotes);const files=await d.exportCanonicalTree();
  assert.deepEqual(files.map(f=>f.path).sort(),['manifest.json','schemas.json',...[...'0123456789abcdef'].map(s=>`entities/${s}.jsonl`)].sort());
  const manifest=JSON.parse(decoder.decode(files.find(f=>f.path==='manifest.json').bytes));assert.equal(manifest.format,'tachiko.roproj');assert.equal(manifest.format_version,1);
  const records=files.filter(f=>f.path.startsWith('entities/')).flatMap(f=>decoder.decode(f.bytes).split('\n').filter(Boolean).map(line=>JSON.parse(line)));
  const entity=records.find(e=>e.id===x.entity);assert.equal(entity.fields[x.impact].value,3);assert.equal(entity.fields[x.notes].value,x.editedNotes);assert.equal(entity.fields[x.priority].kind,'formula');
 }],
 ['C1-02 .ro is codec output, not relabeled transfer bytes',async d=>{
  await d.editImpact(3);const bytes=await d.exportRo();assert.equal(decoder.decode(bytes.slice(0,7)).startsWith('TWDPROJ'),false);
  const report=await d.verifyRoUsingCore(bytes);assert.equal(report.accepted,true);assert.equal(report.impact,3);assert.equal(report.priority,8);
 }],
 ['C1-03 fresh canonical source defeats a saved-view cache',async d=>{
  await d.editImpact(3);const files=await d.exportCanonicalTree();
  const shard=files.find(f=>f.path==='entities/2.jsonl');const e=JSON.parse(decoder.decode(shard.bytes).trim());e.fields[x.impact].value=4;shard.bytes=encoder.encode(JSON.stringify(e)+'\n');
  await d.reopenCanonicalWithOldPresentationCache(files);const o=await d.observe();assert.equal(o.impact,4);assert.equal(o.priority,9);
 }],
 ['C1-04 rejected input does not destroy the active work',async d=>{
  const before=await d.observe();const files=await d.exportCanonicalTree();const invalid=files.filter(f=>f.path!=='schemas.json');
  assert.equal((await d.tryOpenCanonical(invalid)).outcome,'rejected');assert.deepEqual(await d.observe(),before);
 }],
];
let failures=0;
for(const [name,run] of cases){let d;try{d=await createDriver({fixture});await run(d);console.log(JSON.stringify({case:name,status:'PASS'}));}catch(e){failures++;console.error(JSON.stringify({case:name,status:'FAIL',message:String(e.stack??e)}));}finally{await d?.close();}}
if(failures)process.exit(1);
