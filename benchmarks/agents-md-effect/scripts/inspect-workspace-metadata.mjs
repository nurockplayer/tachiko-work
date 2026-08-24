#!/usr/bin/env node

import {createHash} from "node:crypto";
import {readFile, writeFile} from "node:fs/promises";
import {spawnSync} from "node:child_process";
import {resolve} from "node:path";

function usage() {
  console.error(
    "usage: node inspect-workspace-metadata.mjs --candidate-root /abs/repo " +
      "--output /abs/metadata-observations.json",
  );
  process.exit(2);
}

function parseArgs(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || value === undefined) usage();
    values.set(key.slice(2), value);
  }
  return values;
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

const args = parseArgs(process.argv.slice(2));
for (const key of ["candidate-root", "output"]) {
  if (!args.has(key)) usage();
}

const candidateRoot = resolve(args.get("candidate-root"));
const output = resolve(args.get("output"));
const result = spawnSync(
  "cargo",
  ["metadata", "--no-deps", "--format-version", "1", "--locked"],
  {cwd: candidateRoot, encoding: "utf8", maxBuffer: 32 * 1024 * 1024},
);

const assertions = {
  workspace_engine_present: false,
  workflow_absent: false,
  workspace_engine_dependencies: false,
  semantic_has_no_local_dependencies: false,
  formula_depends_on_semantic: false,
  diff_and_merge_depend_on_formula_and_semantic: false,
  storage_depends_on_semantic: false,
  ai_depends_on_workspace: false,
  cli_depends_on_storage_and_workspace: false,
};

let metadataUsable = false;
if (result.status === 0) {
  try {
    const metadata = JSON.parse(result.stdout);
    const workspaceIds = new Set(metadata.workspace_members);
    const packages = metadata.packages.filter((candidate) => workspaceIds.has(candidate.id));
    const names = new Set(packages.map((candidate) => candidate.name));
    const localDependencies = new Map(
      packages.map((candidate) => [
        candidate.name,
        [
          ...new Set(
            candidate.dependencies
              .filter((dependency) => names.has(dependency.name))
              .map((dependency) => dependency.name),
          ),
        ].sort(),
      ]),
    );
    const isExact = (name, expected) =>
      JSON.stringify(localDependencies.get(name)) === JSON.stringify([...expected].sort());

    metadataUsable = true;
    assertions.workspace_engine_present = names.has("tachiko-workspace-engine");
    assertions.workflow_absent = !names.has("tachiko-workflow");
    assertions.workspace_engine_dependencies = isExact("tachiko-workspace-engine", [
      "tachiko-diff-engine",
      "tachiko-formula-engine",
      "tachiko-merge-engine",
      "tachiko-semantic-core",
    ]);
    assertions.semantic_has_no_local_dependencies = isExact("tachiko-semantic-core", []);
    assertions.formula_depends_on_semantic = isExact("tachiko-formula-engine", [
      "tachiko-semantic-core",
    ]);
    assertions.diff_and_merge_depend_on_formula_and_semantic =
      isExact("tachiko-diff-engine", [
        "tachiko-formula-engine",
        "tachiko-semantic-core",
      ]) &&
      isExact("tachiko-merge-engine", [
        "tachiko-formula-engine",
        "tachiko-semantic-core",
      ]);
    assertions.storage_depends_on_semantic = isExact("tachiko-storage", [
      "tachiko-semantic-core",
    ]);
    assertions.ai_depends_on_workspace = isExact("tachiko-ai-api", [
      "tachiko-workspace-engine",
    ]);
    assertions.cli_depends_on_storage_and_workspace = isExact("tachiko-cli", [
      "tachiko-storage",
      "tachiko-workspace-engine",
    ]);
  } catch {
    metadataUsable = false;
  }
}

const observation = {
  contract_id: "TW-04-workspace-metadata-v1",
  metadata_usable: metadataUsable,
  cargo_exit_code: result.status,
  cargo_stdout_sha256: sha256(result.stdout ?? ""),
  cargo_stderr_sha256: sha256(result.stderr ?? ""),
  assertions,
};
await writeFile(output, `${JSON.stringify(observation, null, 2)}\n`, {mode: 0o600});
console.log(JSON.stringify(observation));
