// Test-only data materialization. Admission by the real Rust codec is mandatory.
import { mkdir, writeFile, access } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
const root = fileURLToPath(new URL('../tests/fixtures/release-plan.roproj', import.meta.url));
try { await access(root); throw new Error(`Refusing to replace existing fixture: ${root}`); }
catch (error) { if (error.code !== 'ENOENT') throw error; }
const schema = 'd8b3db6e-a2ca-48f1-82f5-4e44630418dc';
const fields = [
 ['197ae0df-85bd-4c9a-8a9d-0ea27a599881','title','text'],
 ['3f49496c-1764-438b-a106-5563246ad8d3','area','text'],
 ['52a0f7e9-4d6b-4628-ad7d-6b649f8c8a77','impact','number'],
 ['6b529d6d-f003-42ff-aa22-5e317a3e5f37','friction','number'],
 ['75f3d634-452c-4838-925b-8c52e54fc972','priority','number'],
 ['8e1647d7-1a4c-48f7-bbf0-14527730619f','confirmed','boolean'],
 ['9c3c9df1-ac56-4211-9d9d-846ce8c4afbd','source','text'],
 ['b4284d96-4370-4f42-9c0e-4478f947e159','notes','text'],
];
const records = [
 ['2','1d37df46-01f6-4b05-8fd9-064718dc91ea','release_scope','完成下一版的核心流程','發版',5,5,'先完成團隊每天會用到的流程，再增加進階功能。'],
 ['a','3c7d2a33-66b4-4f02-b590-8c6f2d9ecdf1','import_bridge','評估資料匯入的阻力','資料',4,4,'保留來源檔案，說明不支援的內容。'],
 ['e','2a80fa52-df19-4e75-992c-ed92b7194970','playtest_notes','整理試玩回饋','測試',5,4,'把回饋與要修改的資料放在同一份工作裡。'],
];
await mkdir(path.join(root,'entities'),{recursive:true});
const write = (name, content) => writeFile(path.join(root,name), content, {flag:'wx'});
await write('manifest.json',JSON.stringify({format:'tachiko.roproj',format_version:1,document:{id:'c84e78bc-5d28-4d7e-9b57-f104fce5a164',title:'下一版工作計畫'}},null,2)+'\n');
await write('schemas.json',JSON.stringify([{id:schema,key:'release_items',fields:fields.map(([id,key,type])=>({id,key,field_type:{type},required:true}))}],null,2)+'\n');
for (const shard of '0123456789abcdef') {
 const record=records.find(r=>r[0]===shard);
 if (!record) { await write(`entities/${shard}.jsonl`,''); continue; }
 const [,id,key,title,area,impact,friction,notes]=record;
 const values=[{kind:'text',value:title},{kind:'text',value:area},{kind:'number',value:impact},{kind:'number',value:friction},
  {kind:'formula',value:{op:'add',args:{left:{op:'reference',args:{entity:id,field:fields[2][0]}},right:{op:'reference',args:{entity:id,field:fields[3][0]}}}}},
  {kind:'boolean',value:true},{kind:'text',value:'Team planning sample'},{kind:'text',value:notes}];
 await write(`entities/${shard}.jsonl`,JSON.stringify({id,key,schema,fields:Object.fromEntries(fields.map(([fid],i)=>[fid,values[i]]))})+'\n');
}
console.log(`Materialized 18 test fixture files at ${root}; real Rust admission is UNVERIFIED.`);
