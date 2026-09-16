// Adapter for the retained Sheet M2 acceptance. It uses the packaged public
// experimental client in a real browser Worker; it never imports Rust layouts
// or supplies trusted principal, grant, approval, clock, or publication data.
import http from "node:http";
import assert from "node:assert/strict";
import { readFile, stat } from "node:fs/promises";
import path from "node:path";

const kitRoot = process.env.WORK_CLIENT_KIT;
if (!kitRoot) throw new Error("WORK_CLIENT_KIT is required for the delegated proposal driver.");
const playwright = await import(process.env.WORK_PLAYWRIGHT_MODULE ?? "playwright-core");
const { chromium } = playwright.default ?? playwright;
const mime = { ".js": "text/javascript", ".wasm": "application/wasm" };
const pageHtml = `<!doctype html><meta charset="utf-8"><input id="fixture" type="file" webkitdirectory multiple><script type="module">
let client, mod, revision, fixtureBytes, fields;
const input=document.querySelector('#fixture');
const target=intent=>({entity:intent.target.entity,field:intent.target.field});
const close=async()=>{if(client){for(const action of [()=>client.closeProject(),()=>client.close()])try{await action()}catch{}}};
const open=async bytes=>{await close();client=mod.createExperimentalDesignerClient();const opened=await client.openProject(bytes.slice(0));revision=opened.bootstrap.revision;};
const observe=async()=>{const batch=await client.queryFields(revision,[{entity:fields.entity,field:fields.impact},{entity:fields.entity,field:fields.priority}]);const byId=new Map(batch.fields.map(field=>[field.target.field,field]));return {impact:byId.get(fields.impact)?.stored?.value,priority:byId.get(fields.priority)?.calculated?.value,revision};};
window.__tachikoDelegatedDriver={
 async initialize(config){fields=config.fields;mod=await import('/kit/experimental-client.js');fixtureBytes=await mod.projectTransferFromFiles(input.files);await open(fixtureBytes);},
 observe,
 async propose(intent){const proposal=await client.proposeDelegatedScalar(revision,target(intent),String(intent.value));return {id:proposal.proposal_id};},
 async preview(proposal){const review=await client.previewDelegatedProposal(proposal.id);return {outcome:review.outcome,disclosedSubjects:review.disclosed_subjects,disclosedValues:review.disclosed_values};},
 async approveAsHuman(proposal){return client.approveDelegatedProposal(proposal.id);},
 async execute(proposal){const result=await client.executeDelegatedProposal(proposal.id);if(result.publication)revision=result.publication.resulting_revision;return {outcome:result.outcome};},
 async editAsHuman(intent){const result=await client.editNumber(revision,target(intent),String(intent.value));revision=result.resulting_revision;},
 async revokeDelegatedAuthority(){return client.testRevokeDelegatedAuthority();},
 async revokeQueryAuthority(){return client.testRevokeDelegatedQuery();},
 async executeAltered(proposal,intent){const result=await client.testExecuteDelegatedAltered(proposal.id,target(intent),String(intent.value));return {outcome:result.outcome};},
 async reopenFixture(){await open(fixtureBytes);},
 async externalEffectCount(){return 0;},
 async close(){await close();client=undefined;}
};
</script>`;

function asset(requestPath) {
  const decoded = decodeURIComponent(requestPath);
  if (decoded === "/") return null;
  if (!decoded.startsWith("/kit/")) return undefined;
  const resolved = path.resolve(kitRoot, decoded.slice("/kit/".length));
  return resolved.startsWith(`${path.resolve(kitRoot)}${path.sep}`) ? resolved : undefined;
}

async function createServer() {
  for (const file of ["experimental-client.js", "experimental-client.worker.js", "designer_runtime.wasm"]) {
    await stat(path.join(kitRoot, file));
  }
  const server = http.createServer(async (request, response) => {
    try {
      if (request.method !== "GET") { response.writeHead(405).end(); return; }
      const file = asset(new URL(request.url, "http://127.0.0.1").pathname);
      if (file === null) { response.writeHead(200, { "Content-Type": "text/html" }).end(pageHtml); return; }
      if (!file) { response.writeHead(404).end(); return; }
      response.writeHead(200, { "Content-Type": mime[path.extname(file)] ?? "application/octet-stream" });
      response.end(await readFile(file));
    } catch { response.writeHead(400).end("Invalid or unavailable kit asset"); }
  });
  await new Promise((resolve, reject) => { server.once("error", reject); server.listen(0, "127.0.0.1", resolve); });
  const address = server.address();
  assert.ok(address && typeof address === "object");
  return { server, url: `http://127.0.0.1:${address.port}/` };
}

export async function createDriver({ fixture }) {
  const expected = JSON.parse(await readFile(path.join(path.dirname(fixture), "expected.json"), "utf8"));
  const { server, url } = await createServer();
  let browser;
  try {
    browser = await chromium.launch({ headless: true, ...(process.env.WORK_CHROMIUM ? { executablePath: process.env.WORK_CHROMIUM } : {}) });
    const page = await browser.newPage();
    await page.goto(url);
    await page.locator("#fixture").setInputFiles(fixture);
    await page.evaluate(config => window.__tachikoDelegatedDriver.initialize(config), { fields: expected });
    const call = (method, ...args) => page.evaluate(async ({ method, args }) => window.__tachikoDelegatedDriver[method](...args), { method, args });
    return {
      observe: () => call("observe"),
      propose: intent => call("propose", intent),
      preview: proposal => call("preview", proposal),
      approveAsHuman: proposal => call("approveAsHuman", proposal),
      execute: proposal => call("execute", proposal),
      editAsHuman: intent => call("editAsHuman", intent),
      revokeDelegatedAuthority: () => call("revokeDelegatedAuthority"),
      revokeQueryAuthority: () => call("revokeQueryAuthority"),
      executeAltered: (proposal, intent) => call("executeAltered", proposal, intent),
      reopenFixture: () => call("reopenFixture"),
      externalEffectCount: () => call("externalEffectCount"),
      close: async () => { try { await call("close"); } finally { await browser.close(); await new Promise(resolve => server.close(resolve)); } },
    };
  } catch (error) {
    await browser?.close().catch(() => {});
    await new Promise(resolve => server.close(resolve));
    throw error;
  }
}
