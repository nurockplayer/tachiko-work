import {
  createExperimentalDesignerClient, DesignerRuntimeError, projectTransferFromFiles,
} from "/vendor/tachiko/experimental-client.js";

type Target = { entity: string; field: string };
type Field = { target: Target; stored: { kind: string; value?: string | number } | null; calculated: { status: string; value?: number } | null };
type Table = { revision: string; rows: Array<{ id: string; fields: Field[] }> };

const client = createExperimentalDesignerClient();
const byId = <T extends HTMLElement>(id: string): T => {
  const element = document.getElementById(id);
  if (!element) throw new Error(`Missing #${id}`);
  return element as T;
};
const prepare = byId<HTMLButtonElement>("prepare");
const proposal = byId<HTMLElement>("proposal");
const cancel = byId<HTMLButtonElement>("cancel");
const accept = byId<HTMLButtonElement>("accept");
const work = byId<HTMLElement>("work");
const revision = byId<HTMLOutputElement>("revision");
const taxInput = byId<HTMLInputElement>("tax-rate");
const earlierInput = byId<HTMLInputElement>("earlier-tax");
const applyTax = byId<HTMLButtonElement>("apply-tax");
const freezeEarlier = byId<HTMLButtonElement>("freeze-earlier");
const applyEarlier = byId<HTMLButtonElement>("apply-earlier");
const plan = byId<HTMLElement>("plan");
const impact = byId<HTMLElement>("impact");
const error = byId<HTMLElement>("operation-error");
const save = byId<HTMLButtonElement>("save");
const close = byId<HTMLButtonElement>("close");
const saved = byId<HTMLInputElement>("saved-work");
const openSaved = byId<HTMLButtonElement>("open-saved");

let candidate: ArrayBuffer | null = null;
let currentRevision: string | null = null;
let taxTarget: Target | null = null;
let taxValue = "";
let earlierRevision: string | null = null;
let labels = new Map<string, string>();

const key = (target: Target) => `${target.entity}.${target.field}`;
const netTargets: Target[] = ["e-jan", "e-feb", "e-mar"].map(entity => ({ entity, field: "f-net" }));
const text = (field: Field) => field.stored?.kind === "text" ? String(field.stored.value) : "";
const number = (field: Field) => field.calculated?.status === "value" ? String(field.calculated.value) : "—";
const clearError = () => { error.textContent = ""; };
const report = (reason: unknown) => {
  error.textContent = reason instanceof DesignerRuntimeError ? reason.failure.code : String(reason);
};

async function candidateBytes(): Promise<ArrayBuffer> {
  if (candidate) return candidate;
  const paths: string[] = await fetch("/candidate-files.json").then(response => response.json());
  const files = await Promise.all(paths.map(async (path) => {
    const bytes = await fetch(`/candidate.roproj/${path}`).then(response => {
      if (!response.ok) throw new Error(`Unable to read frozen candidate file: ${path}`);
      return response.blob();
    });
    const file = new File([bytes], path.split("/").at(-1) ?? path);
    Object.defineProperty(file, "webkitRelativePath", { value: `candidate.roproj/${path}` });
    return file;
  }));
  const fileList = Object.assign(files, { item: (index: number) => files[index] ?? null }) as unknown as FileList;
  candidate = await projectTransferFromFiles(fileList);
  return candidate;
}

function proposalPreview(ledger: Array<{ classification: string; code: string }>): void {
  const words = new Set(ledger.map(entry => entry.code));
  proposal.hidden = false;
  proposal.textContent = [
    words.has("new_identity") ? "New identities" : "",
    words.has("explicit_schema") ? "Explicit schema" : "",
    words.has("presentation_not_semantics") ? "Presentation only" : "",
  ].filter(Boolean).join(" · ");
}

function field(table: Table, target: Target): Field | undefined {
  return table.rows.flatMap(row => row.fields).find(item => key(item.target) === key(target));
}

async function install(opened: { bootstrap: { revision: string; collections: Array<{ key: string }> } }): Promise<void> {
  currentRevision = opened.bootstrap.revision;
  revision.textContent = currentRevision;
  const tables = new Map<string, Table>();
  for (const collection of opened.bootstrap.collections) tables.set(collection.key, await client.queryTable(collection.key) as Table);
  const assumptions = tables.get("assumptions");
  const quarterlyPlan = tables.get("plan");
  if (!assumptions || !quarterlyPlan) throw new Error("Frozen candidate lacks the required plan collections.");
  taxTarget = field(assumptions, { entity: "e-tax", field: "f-rate" })?.target ?? null;
  const tax = taxTarget && field(assumptions, taxTarget);
  if (!taxTarget || !tax || tax.stored?.kind !== "number") throw new Error("Frozen candidate lacks its editable tax field.");
  taxValue = String(tax.stored.value);
  taxInput.value = taxValue;
  labels = new Map(quarterlyPlan.rows.map(row => [row.id, text(field(quarterlyPlan, { entity: row.id, field: "f-label" })!)]));
  renderPlan(quarterlyPlan);
  work.hidden = false;
  taxInput.disabled = false;
  save.disabled = false;
  prepare.disabled = true;
  accept.disabled = true;
  saved.disabled = true;
  openSaved.disabled = true;
}

function renderPlan(table: Table): void {
  const rows = table.rows.flatMap(row => {
    const gross = field(table, { entity: row.id, field: "f-gross" });
    const net = field(table, { entity: row.id, field: "f-net" });
    const grossLine = document.createElement("p");
    grossLine.dataset.testid = `gross-${row.id}`;
    grossLine.textContent = gross ? number(gross) : "—";
    const netLine = document.createElement("p");
    netLine.dataset.testid = `net-${row.id}`;
    netLine.textContent = net ? number(net) : "—";
    return [grossLine, netLine];
  });
  plan.replaceChildren(...rows);
}

async function refreshPlan(): Promise<void> {
  const table = await client.queryTable("plan") as Table;
  renderPlan(table);
}

prepare.addEventListener("click", async () => {
  clearError();
  try {
    const [bytes, ledger] = await Promise.all([candidateBytes(), fetch("/handoff-ledger.json").then(response => response.json())]);
    await client.inspectProject?.(bytes.slice(0));
    proposalPreview(ledger);
  } catch (reason) { report(reason); }
});
cancel.addEventListener("click", () => { proposal.hidden = true; clearError(); });
accept.addEventListener("click", async () => {
  clearError();
  try {
    if (currentRevision) throw new Error("Close accepted work before accepting another handoff.");
    if (!candidate) throw new Error("Prepare the frozen handoff first.");
    await install(await client.openProject(candidate.slice(0)));
    proposal.hidden = true;
  } catch (reason) { report(reason); }
});
freezeEarlier.addEventListener("click", () => { earlierRevision = currentRevision; clearError(); });
applyTax.addEventListener("click", async () => {
  clearError();
  try {
    if (!currentRevision || !taxTarget) throw new Error("Accept the handoff first.");
    const priorTaxInput = taxValue;
    const before = await client.queryFields(currentRevision, [taxTarget, ...netTargets]);
    const published = await client.editNumber(currentRevision, taxTarget, taxInput.value);
    const after = await client.queryFields(published.resulting_revision, [taxTarget, ...published.affected_calculations]);
    currentRevision = published.resulting_revision;
    taxValue = taxInput.value;
    revision.textContent = currentRevision;
    const prior = new Map((before.fields as Field[]).map(item => [key(item.target), item]));
    impact.replaceChildren(...(after.fields as Field[]).filter(item => item.target.field === "f-net").map(item => {
      const line = document.createElement("li");
      line.dataset.testid = `impact-${item.target.entity}`;
      line.textContent = `${labels.get(item.target.entity)} net: ${number(prior.get(key(item.target)) ?? item)} → ${number(item)}`;
      return line;
    }));
    const changed = document.createElement("p");
    changed.dataset.testid = "changed-field";
    changed.textContent = `Tax rate: ${priorTaxInput} → ${taxInput.value}`;
    impact.prepend(changed);
    await refreshPlan();
  } catch (reason) { report(reason); }
});
applyEarlier.addEventListener("click", async () => {
  clearError();
  try {
    if (!earlierRevision || !taxTarget) throw new Error("Freeze an earlier draft first.");
    await client.editNumber(earlierRevision, taxTarget, earlierInput.value);
  } catch (reason) { report(reason); }
});
save.addEventListener("click", async () => {
  clearError();
  try {
    if (!currentRevision) throw new Error("No accepted work to save.");
    const exported = await client.exportProject(currentRevision);
    const anchor = document.createElement("a");
    anchor.href = URL.createObjectURL(new Blob([exported.bytes]));
    anchor.download = "quarterly-plan.twdproj";
    anchor.click();
    setTimeout(() => URL.revokeObjectURL(anchor.href), 0);
  } catch (reason) { report(reason); }
});
close.addEventListener("click", async () => {
  clearError();
  try { await client.closeProject(); } catch (reason) { report(reason); return; }
  currentRevision = null; taxTarget = null; taxValue = ""; earlierRevision = null;
  candidate = null;
  work.hidden = true; taxInput.disabled = true; save.disabled = true;
  prepare.disabled = false; accept.disabled = true; saved.disabled = false; openSaved.disabled = false;
});
openSaved.addEventListener("click", async () => {
  clearError();
  try {
    if (currentRevision) throw new Error("Close accepted work before opening saved work.");
    const file = saved.files?.item(0);
    if (!file) throw new Error("Select a saved work file first.");
    await install(await client.openProject(await file.arrayBuffer()));
  } catch (reason) { report(reason); }
});
