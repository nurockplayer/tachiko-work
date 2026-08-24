#!/usr/bin/env node

import {readFile, writeFile} from "node:fs/promises";
import {spawnSync} from "node:child_process";
import {dirname, resolve} from "node:path";
import {fileURLToPath} from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const benchmarkDir = resolve(scriptDir, "..");
const snapshotPath = resolve(benchmarkDir, "evaluator/history-snapshots.json");

function usage() {
  console.error(
    "usage: node capture-history-snapshot.mjs " +
      "--kind <issue|pr> --number <n> [--kind <issue|pr> --number <n> ...]",
  );
  process.exit(2);
}

function fail(message) {
  throw new Error(message);
}

function rtkGh(args) {
  const result = spawnSync("rtk", ["proxy", "gh", ...args], {
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.status !== 0) {
    fail(`rtk proxy gh ${args.join(" ")} failed: ${result.stderr || result.stdout}`);
  }
  return JSON.parse(result.stdout);
}

const requests = [];
const cliArgs = process.argv.slice(2);
for (let index = 0; index < cliArgs.length; index += 4) {
  const [kindFlag, kind, numberFlag, numberText] = cliArgs.slice(index, index + 4);
  if (kindFlag !== "--kind" || !["issue", "pr"].includes(kind) || numberFlag !== "--number") {
    usage();
  }
  const number = Number(numberText);
  if (!Number.isSafeInteger(number) || number <= 0) usage();
  requests.push({kind, number});
}
if (requests.length === 0 || cliArgs.length !== requests.length * 4) usage();

const document = JSON.parse(await readFile(snapshotPath, "utf8"));
if (document.repository !== "nurockplayer/tachiko-work") {
  fail("unexpected snapshot repository");
}

const viewFields = {
  issue: "author,body,closedAt,comments,createdAt,number,state,title,updatedAt,url",
  pr: "author,baseRefOid,body,closedAt,comments,commits,createdAt,files,headRefOid,mergedAt,number,reviews,state,statusCheckRollup,title,updatedAt,url",
};
const graphField = {issue: "issue", pr: "pullRequest"};

for (const request of requests) {
  const capturedResource = rtkGh([
    request.kind === "issue" ? "issue" : "pr",
    "view",
    String(request.number),
    "--repo",
    document.repository,
    "--json",
    viewFields[request.kind],
  ]);
  const query =
    `query($owner:String!,$name:String!,$number:Int!){` +
    `repository(owner:$owner,name:$name){${graphField[request.kind]}(number:$number){` +
    "userContentEdits(first:100){nodes{id editedAt editor{login} diff}}}}}";
  const editResult = rtkGh([
    "api",
    "graphql",
    "-F",
    "owner=nurockplayer",
    "-F",
    "name=tachiko-work",
    "-F",
    `number=${request.number}`,
    "-f",
    `query=${query}`,
  ]);
  const bodyEditHistory =
    editResult.data.repository[graphField[request.kind]].userContentEdits.nodes;
  const replacement = {
    kind: request.kind,
    number: request.number,
    captured_resource: capturedResource,
    body_edit_history: bodyEditHistory,
  };
  const existingIndex = document.resources.findIndex(
    (entry) => entry.kind === request.kind && entry.number === request.number,
  );
  if (existingIndex === -1) document.resources.push(replacement);
  else document.resources[existingIndex] = replacement;
}

document.captured_at = new Date().toISOString();
await writeFile(snapshotPath, `${JSON.stringify(document, null, 2)}\n`);
console.log(
  JSON.stringify({
    repository: document.repository,
    captured_at: document.captured_at,
    captured: requests,
  }),
);
