// Adapter for the unchanged Sheet C1 storage acceptance. It uses only the
// public experimental client kit in a real browser Worker. It does not import
// the private bridge, call a raw WASM export, or decode either storage format.
import http from 'node:http';
import assert from 'node:assert/strict';
import {readFile,stat} from 'node:fs/promises';
import path from 'node:path';

const kitRoot=process.env.WORK_CLIENT_KIT;
if(!kitRoot) throw new Error('WORK_CLIENT_KIT is required for the public-kit storage driver.');
const playwrightModule=process.env.WORK_PLAYWRIGHT_MODULE ?? 'playwright-core';
const chromiumPath=process.env.WORK_CHROMIUM;
let chromium;
try { ({chromium}=await import(playwrightModule)); }
catch(error) { throw new Error('Playwright is required for the public-kit storage driver.',{cause:error}); }

const mime={'.html':'text/html','.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm','.json':'application/json'};
const pageHtml='<!doctype html><meta charset="utf-8"><input id="fixture" type="file" webkitdirectory multiple><script type="module">\
let mod; let client; let revision; let fields; let priorPresentation;\
const input=document.querySelector("#fixture");\
const bytesFromBase64=value=>Uint8Array.from(atob(value),letter=>letter.charCodeAt(0));\
const base64FromBytes=value=>{let output="";for(const byte of new Uint8Array(value))output+=String.fromCharCode(byte);return btoa(output);};\
const required=(object,name)=>{if(typeof object[name]!=="function")throw new Error("Public kit API unavailable: "+name);return object[name].bind(object);};\
const target=(field)=>({entity:fields.entity,field:fields[field]});\
const currentFields=async()=>{const batch=await client.queryFields(revision,[target("impact"),target("priority"),target("notes")]);const byId=new Map(batch.fields.map(field=>[field.target.field,field]));const impact=byId.get(fields.impact);const priority=byId.get(fields.priority);if(impact?.stored?.kind!=="number"||priority?.calculated?.status!=="value")throw new Error("Public query did not produce required scalar/calculation projections.");const occurrence=await required(client,"observeOccurrence")();if(occurrence.revision!==revision)throw new Error("Occurrence revision differs from active public projection.");const canonical=await required(client,"exportCanonicalTree")(revision);return {scope:occurrence.scope,revision:occurrence.revision,impact:impact.stored.value,priority:priority.calculated.value,notes:byId.get(fields.notes)?.stored?.value,canonicalFiles:encodeFiles(canonical.files)};};\
const canonicalEntries=entries=>entries.map(entry=>({path:entry.path,bytes:bytesFromBase64(entry.base64).buffer}));\
const closeClient=async instance=>{if(instance){await instance.closeProject().catch(()=>{});await instance.close().catch(()=>{});}};\
const newClient=()=>mod.createExperimentalDesignerClient();\
const openBytes=async bytes=>{await closeClient(client);client=newClient();const opened=await client.openProject(bytes);revision=opened.bootstrap.revision;return opened;};\
const encodeFiles=files=>files.map(file=>({path:file.path,base64:base64FromBytes(file.bytes)}));\
window.__tachikoStorageDriver={\
 async initialize(config){fields=config.fields;mod=await import("/kit/experimental-client.js");const bytes=await mod.projectTransferFromFiles(input.files);await openBytes(bytes);return currentFields();},\
 async editImpact(value){const publication=await client.editNumber(revision,target("impact"),String(value));revision=publication.resulting_revision;},\
 async editNotes(value){const publication=await client.editText(revision,target("notes"),value);revision=publication.resulting_revision;},\
 async exportCanonicalTree(){const result=await required(client,"exportCanonicalTree")(revision);if(result.revision!==revision)throw new Error("Canonical export revision differs from active public projection.");return {revision:result.revision,files:encodeFiles(result.files)};},\
 async exportPortableRo(){const result=await required(client,"exportPortableRo")(revision);if(result.revision!==revision)throw new Error("Portable .ro export revision differs from active public projection.");return {revision:result.revision,base64:base64FromBytes(result.bytes)};},\
 async verifyPortableRo(base64){const bytes=bytesFromBase64(base64).buffer;const verifier=newClient();const reader=newClient();try{await required(verifier,"verifyPortableRo")(bytes.slice(0));const opened=await required(reader,"openPortableRo")(bytes.slice(0));const batch=await reader.queryFields(opened.bootstrap.revision,[target("impact"),target("priority")]);const byId=new Map(batch.fields.map(field=>[field.target.field,field]));const impact=byId.get(fields.impact);const priority=byId.get(fields.priority);if(impact?.stored?.kind!=="number"||priority?.calculated?.status!=="value")throw new Error("Fresh public .ro open did not expose required projections.");return {accepted:true,impact:impact.stored.value,priority:priority.calculated.value};}finally{await closeClient(verifier);await closeClient(reader);}},\
 async reopenCanonical(entries){priorPresentation=await currentFields();await closeClient(client);client=newClient();const opened=await required(client,"openCanonicalTree")(canonicalEntries(entries));revision=opened.bootstrap.revision;return currentFields();},\
 async tryOpenCanonical(entries){const priorRevision=revision;try{const opened=await required(client,"openCanonicalTree")(canonicalEntries(entries));revision=opened.bootstrap.revision;return {outcome:"opened"};}catch(error){const code=error instanceof mod.DesignerRuntimeError?error.failure.code:null;if(["invalid_project","project_too_large","unsupported_project"].includes(code)){if(revision!==priorRevision)throw new Error("Rejected canonical admission changed the active driver revision.");return {outcome:"rejected",code};}throw error;}},\
 async observe(){return currentFields();},\
 async priorPresentation(){return priorPresentation;},\
 async close(){await closeClient(client);client=undefined;}\
};\
</script>';

function safeAssetPath(requestPath) {
  const decoded=decodeURIComponent(requestPath);
  if(decoded==='/') return null;
  if(!decoded.startsWith('/kit/')) return undefined;
  const resolved=path.resolve(kitRoot,decoded.slice('/kit/'.length));
  if(!resolved.startsWith(path.resolve(kitRoot)+path.sep)) return undefined;
  return resolved;
}

async function createServer() {
  await stat(path.join(kitRoot,'experimental-client.js'));
  await stat(path.join(kitRoot,'experimental-client.worker.js'));
  await stat(path.join(kitRoot,'designer_runtime.wasm'));
  const server=http.createServer(async (request,response)=>{
    try {
      if(request.method!=='GET') { response.writeHead(405); response.end(); return; }
      const requested=new URL(request.url,'http://127.0.0.1').pathname;
      const file=safeAssetPath(requested);
      if(file===null) { response.writeHead(200,{'Content-Type':'text/html','Cache-Control':'no-store'}); response.end(pageHtml); return; }
      if(!file) { response.writeHead(404); response.end(); return; }
      const bytes=await readFile(file);
      response.writeHead(200,{'Content-Type':mime[path.extname(file)] ?? 'application/octet-stream','Cache-Control':'no-store'});
      response.end(bytes);
    } catch {
      response.writeHead(400); response.end('Invalid or unavailable local kit asset');
    }
  });
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(0,'127.0.0.1',resolve);});
  const address=server.address();
  assert.ok(address&&typeof address==='object');
  return {server,url:'http://127.0.0.1:'+address.port+'/'};
}

async function decodeTree(encoded) {
  return encoded.files.map(file=>({path:file.path,bytes:Uint8Array.from(Buffer.from(file.base64,'base64'))}));
}

export async function createDriver({fixture}) {
  const expected=JSON.parse(await readFile(path.join(path.dirname(fixture),'expected.json'),'utf8'));
  const {server,url}=await createServer();
  let browser;
  let page;
  try {
    browser=await chromium.launch({headless:true,...(chromiumPath?{executablePath:chromiumPath}:{})});
    page=await browser.newPage();
    await page.goto(url);
    await page.locator('#fixture').setInputFiles(fixture);
    await page.evaluate(config=>window.__tachikoStorageDriver.initialize(config),{fields:expected});
  } catch(error) {
    await browser?.close().catch(()=>{});
    await new Promise(resolve=>server.close(resolve));
    throw error;
  }
  const call=(method,arg)=>page.evaluate(async ({method,arg})=>window.__tachikoStorageDriver[method](arg),{method,arg});
  return {
    editImpact:value=>call('editImpact',value),
    editNotes:value=>call('editNotes',value),
    async exportCanonicalTree(){return decodeTree(await call('exportCanonicalTree'));},
    async exportRo(){return Uint8Array.from(Buffer.from((await call('exportPortableRo')).base64,'base64'));},
    verifyRoUsingCore:async bytes=>call('verifyPortableRo',Buffer.from(bytes).toString('base64')),
    async reopenCanonicalWithOldPresentationCache(files) {
      const entries=files.map(file=>({path:file.path,base64:Buffer.from(file.bytes).toString('base64')}));
      await call('reopenCanonical',entries);
    },
    async tryOpenCanonical(files) {
      const entries=files.map(file=>({path:file.path,base64:Buffer.from(file.bytes).toString('base64')}));
      return call('tryOpenCanonical',entries);
    },
    observe:()=>call('observe'),
    close:async()=>{try{await call('close');}finally{try{await browser.close();}finally{await new Promise(resolve=>server.close(resolve));}}},
  };
}
