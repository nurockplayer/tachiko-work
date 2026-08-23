#!/usr/bin/env node

import { spawnSync } from "node:child_process";

const expected = new Map([
  ["tachiko-semantic-core", []],
  ["tachiko-formula-engine", ["tachiko-semantic-core"]],
  [
    "tachiko-diff-engine",
    ["tachiko-formula-engine", "tachiko-semantic-core"],
  ],
  [
    "tachiko-merge-engine",
    ["tachiko-formula-engine", "tachiko-semantic-core"],
  ],
  ["tachiko-storage", ["tachiko-semantic-core"]],
  [
    "tachiko-workspace-engine",
    [
      "tachiko-diff-engine",
      "tachiko-formula-engine",
      "tachiko-merge-engine",
      "tachiko-semantic-core",
    ],
  ],
  ["tachiko-ai-api", ["tachiko-workspace-engine"]],
  ["tachiko-cli", ["tachiko-storage", "tachiko-workspace-engine"]],
]);

const metadataResult = spawnSync(
  "cargo",
  ["metadata", "--no-deps", "--format-version", "1", "--locked"],
  { encoding: "utf8" },
);

if (metadataResult.status !== 0) {
  process.stderr.write(metadataResult.stderr);
  process.exit(metadataResult.status ?? 1);
}

const metadata = JSON.parse(metadataResult.stdout);
const workspaceIds = new Set(metadata.workspace_members);
const packages = metadata.packages.filter((candidate) =>
  workspaceIds.has(candidate.id),
);
const actualNames = new Set(packages.map((candidate) => candidate.name));
const failures = [];

for (const name of expected.keys()) {
  if (!actualNames.has(name)) {
    failures.push(`missing workspace package: ${name}`);
  }
}
for (const name of actualNames) {
  if (!expected.has(name)) {
    failures.push(`unexpected workspace package: ${name}`);
  }
}

for (const candidate of packages) {
  const wanted = expected.get(candidate.name);
  if (!wanted) {
    continue;
  }
  const localDependencies = [
    ...new Set(
      candidate.dependencies
        .filter((dependency) => actualNames.has(dependency.name))
        .map((dependency) => dependency.name),
    ),
  ].sort();
  const expectedDependencies = [...wanted].sort();
  if (JSON.stringify(localDependencies) !== JSON.stringify(expectedDependencies)) {
    failures.push(
      `${candidate.name}: expected [${expectedDependencies.join(", ")}], found [${localDependencies.join(", ")}]`,
    );
  }
}

if (failures.length > 0) {
  for (const failure of failures) {
    console.error(`workspace dependency check: ${failure}`);
  }
  process.exit(1);
}

console.log("workspace dependency check passed: Cargo graph matches ADR-0016");
