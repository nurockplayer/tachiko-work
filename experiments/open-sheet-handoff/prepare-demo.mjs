// Trusted staging command for the fixed #341 model. It is deliberately not a
// general open-sheet loader: both the source module and mapping are local,
// reviewed inputs, and Rust performs the only semantic admission.
import { execFileSync } from "node:child_process";
import {
  cpSync, existsSync, mkdtempSync, mkdirSync, readdirSync, renameSync,
  rmSync, writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import { basename, dirname, isAbsolute, join, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { compileModel } from "./model.mjs";
import { proposeHandoff } from "./handoff.mjs";

function fail(message) {
  throw new Error(`prepare-demo: ${message}`);
}

function outputArgument() {
  const args = process.argv.slice(2);
  if (args.length !== 2 || args[0] !== "--output") {
    fail("usage: node prepare-demo.mjs --output <new absolute directory>");
  }
  if (!isAbsolute(args[1])) fail("output must be an absolute path");
  return resolve(args[1]);
}

function command(file, args) {
  execFileSync(file, args, { stdio: "inherit" });
}

function paths(root, base = root) {
  const result = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const location = join(root, entry.name);
    if (entry.isDirectory()) result.push(...paths(location, base));
    else if (entry.isFile()) result.push(relative(base, location));
    else fail(`materialized project contains an unsupported entry: ${location}`);
  }
  return result.sort();
}

const output = outputArgument();
if (existsSync(output)) fail(`output already exists: ${output}`);
const cli = process.env.TACHIKO_BIN;
if (!cli) fail("TACHIKO_BIN must name the built Tachiko CLI");
const root = resolve(new URL("../..", import.meta.url).pathname);
const mapping = JSON.parse(
  await import("node:fs/promises").then(({ readFile }) => readFile(new URL("./mapping.json", import.meta.url), "utf8")),
);
const require = createRequire(new URL("../open-sheet-export/package.json", import.meta.url));
const core = await import(pathToFileURL(require.resolve("@open-sheet/core")).href);
const proposal = await proposeHandoff(compileModel(core), mapping);
if (proposal.status !== "proposal") fail("trusted converter did not produce a proposal");

mkdirSync(dirname(output), { recursive: true });
const stage = mkdtempSync(join(dirname(output), ".issue341-stage-"));
try {
  const candidate = join(stage, "candidate.ro");
  const project = join(stage, "candidate.roproj");
  writeFileSync(candidate, `${JSON.stringify(proposal.document, null, 2)}\n`, { flag: "wx" });
  command(cli, ["validate", candidate]);
  command(cli, ["roproj", "materialize", candidate, project]);
  command(cli, ["roproj", "validate", project]);
  rmSync(candidate);
  cpSync(new URL("./demo.html", import.meta.url), join(stage, "index.html"));
  cpSync(new URL("./demo.ts", import.meta.url), join(stage, "demo.ts"));
  writeFileSync(join(stage, "handoff-ledger.json"), `${JSON.stringify(proposal.ledger, null, 2)}\n`, { flag: "wx" });
  writeFileSync(join(stage, "candidate-files.json"), `${JSON.stringify(paths(project), null, 2)}\n`, { flag: "wx" });
  mkdirSync(join(stage, "vendor"));
  command("bash", [join(root, "scripts/export-experimental-designer-client.sh"), join(stage, "vendor/tachiko")]);
  renameSync(stage, output);
  process.stdout.write(`prepared #341 handoff demo at ${output}\n`);
} catch (error) {
  rmSync(stage, { recursive: true, force: true });
  throw error;
}
