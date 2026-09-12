// Producer/consumer artifact integrity. This does not replace a real runtime canary.
import assert from 'node:assert/strict';
import {readFile,readdir,lstat} from 'node:fs/promises';
import path from 'node:path';
import {createHash} from 'node:crypto';
const root=process.env.WORK_CLIENT_KIT;
if(!root){console.error('BLOCKED: WORK_CLIENT_KIT is required.');process.exit(78);}
const manifest=JSON.parse(await readFile(path.join(root,'artifact-manifest.json'),'utf8'));
assert.equal(manifest.sourceRepository,'nurockplayer/tachiko-work');
assert.match(manifest.sourceCommit,/^[a-f0-9]{40}$/);
if(process.env.WORK_CORE_COMMIT)assert.equal(manifest.sourceCommit,process.env.WORK_CORE_COMMIT);
assert.equal(manifest.stability,'experimental');
assert.ok(Array.isArray(manifest.files)&&manifest.files.length>0);
const declared=new Set();
for(const file of manifest.files){
 assert.equal(typeof file.path,'string');assert.ok(file.path.length>0);
 assert.ok(!path.isAbsolute(file.path)&&!file.path.includes('\\')&&!file.path.split('/').some(p=>!p||p==='.'||p==='..'));
 assert.ok(!declared.has(file.path),'Duplicate asset entry');declared.add(file.path);
 assert.match(file.sha256,/^[a-f0-9]{64}$/);
 const absolute=path.join(root,file.path);assert.equal((await lstat(absolute)).isSymbolicLink(),false);
 const bytes=await readFile(absolute);assert.equal(createHash('sha256').update(bytes).digest('hex'),file.sha256,`Digest mismatch: ${file.path}`);
}
const actual=[];
async function walk(dir,prefix=''){for(const item of await readdir(dir,{withFileTypes:true})){assert.equal(item.isSymbolicLink(),false,'Symlink in artifact');const relative=prefix+item.name;if(item.isDirectory())await walk(path.join(dir,item.name),relative+'/');else actual.push(relative);}}
await walk(root);
assert.deepEqual(actual.filter(p=>p!=='artifact-manifest.json').sort(),[...declared].sort(),'Missing or undeclared asset');
assert.ok(declared.has(manifest.entry));assert.ok([...declared].some(p=>p.endsWith('.d.ts')));
assert.ok([...declared].some(p=>p.endsWith('.wasm')));assert.ok([...declared].some(p=>p.includes('worker')&&p.endsWith('.js')));
assert.ok(manifest.licenseNotices?.length>0);for(const item of manifest.licenseNotices)assert.ok(declared.has(item));
console.log(JSON.stringify({case:'C1 artifact integrity',status:'PASS',source:manifest.sourceCommit,assets:declared.size}));
