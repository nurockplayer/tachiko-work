#!/usr/bin/env node

import {createHash} from "node:crypto";
import {mkdir, mkdtemp, readFile, realpath, rm, writeFile} from "node:fs/promises";
import {spawnSync} from "node:child_process";
import {dirname, isAbsolute, resolve} from "node:path";
import {tmpdir} from "node:os";
import {fileURLToPath} from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const benchmarkDir = resolve(scriptDir, "..");
const runOracles = resolve(scriptDir, "run-oracles.mjs");
const runTw05Offline = resolve(scriptDir, "run-tw05-offline.mjs");

function usage() {
  console.error(
    "usage: node qualify-oracles.mjs --source-repo /abs/repo --output /abs/oracles.json",
  );
  process.exit(2);
}

function parseArgs(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || value === undefined || values.has(key.slice(2))) usage();
    values.set(key.slice(2), value);
  }
  return values;
}

function fail(message) {
  throw new Error(message);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function git(sourceRepo, args, expected = 0) {
  const result = spawnSync("rtk", ["proxy", "git", ...args], {
    cwd: sourceRepo,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
    env: {
      ...process.env,
      GIT_CONFIG_NOSYSTEM: "1",
      GIT_CONFIG_GLOBAL: "/dev/null",
      GIT_ATTR_NOSYSTEM: "1",
    },
  });
  if (result.status !== expected) {
    fail(`rtk proxy git ${args[0]} failed (${result.status}): ${result.stderr}`);
  }
  return result.stdout.trim();
}

const fixtureCommand = `#!/usr/bin/env node
import {readFile, writeFile} from "node:fs/promises";
const config = JSON.parse(await readFile(new URL("fixture.json", import.meta.url), "utf8"));
if (config.kind === "rust") {
  for (let index = 0; index < config.matches; index += 1) console.log(\`test \${config.name} ... ok\`);
} else if (config.kind === "json") {
  console.log(JSON.stringify({assertions: {ready: config.value}}));
} else if (config.kind === "portable") {
  const outputIndex = process.argv.indexOf("--output") + 1;
  await writeFile(process.argv[outputIndex], JSON.stringify(config.observations));
} else if (config.kind !== "packet") {
  throw new Error("unknown fixture kind");
}
if (config.exit_code) process.exitCode = config.exit_code;
`;

async function executeRunner({root, id, command, selector, positiveConfig, negativeConfig, subjective = false}) {
  const familyRoot = resolve(root, id);
  const positive = resolve(familyRoot, "positive");
  const negative = resolve(familyRoot, "negative");
  await Promise.all([mkdir(positive, {recursive: true}), mkdir(negative, {recursive: true})]);
  const configBytes = [];
  for (const [directory, config] of [[positive, positiveConfig], [negative, negativeConfig]]) {
    const bytes = `${JSON.stringify(config)}\n`;
    configBytes.push(bytes);
    await Promise.all([
      writeFile(resolve(directory, "fixture-command.mjs"), fixtureCommand),
      writeFile(resolve(directory, "fixture.json"), bytes),
    ]);
  }
  const assertion = selector === null ? [] : [{
    id: `${id}.assertion`,
    command_id: `${id}.command`,
    selector,
  }];
  const manifest = {
    protocol_id: "tachiko-oracle-qualification-v1",
    classification: "construction_pilot_only",
    formal_result_eligible: false,
    cases: [{
      id: "QF",
      oracle_commands: [{
        id: `${id}.command`,
        command_template: command,
        assertion_ids: assertion.map((entry) => entry.id),
      }],
      assertions: assertion.map(({id: assertionId, command_id}) => ({
        id: assertionId,
        command_id,
        stage: "expectation_free_execution",
      })),
      subjective_groups: subjective ? [{id: "semantic", stage: "blinded_review_packet"}] : [],
    }],
  };
  const lock = {
    protocol_id: manifest.protocol_id,
    cases: [{id: "QF", assertions: assertion}],
  };
  const manifestPath = resolve(familyRoot, "manifest.json");
  const lockPath = resolve(familyRoot, "lock.json");
  await Promise.all([
    writeFile(manifestPath, `${JSON.stringify(manifest)}\n`),
    writeFile(lockPath, `${JSON.stringify(lock)}\n`),
  ]);

  const execute = (candidate, trusted) => spawnSync(
    process.execPath,
    [
      runOracles,
      "--case", "QF",
      "--candidate-root", candidate,
      "--trusted-dir", trusted,
      "--manifest", manifestPath,
      "--oracle-lock", lockPath,
      "--benchmark-dir", benchmarkDir,
    ],
    {encoding: "utf8", maxBuffer: 128 * 1024 * 1024},
  );
  const positiveResult = execute(positive, resolve(familyRoot, "positive-receipt"));
  const negativeResult = execute(negative, resolve(familyRoot, "negative-receipt"));
  if (positiveResult.status !== 0 || negativeResult.status !== 1) {
    fail(`${id} qualification failed: positive=${positiveResult.status}, negative=${negativeResult.status}\n${positiveResult.stderr}\n${negativeResult.stderr}`);
  }
  const positiveReceipt = JSON.parse(positiveResult.stdout.trim().split(/\r?\n/).at(-1));
  const negativeReceipt = JSON.parse(negativeResult.stdout.trim().split(/\r?\n/).at(-1));
  return {
    id,
    fixture_sha256: sha256(`${fixtureCommand}${configBytes[0]}${configBytes[1]}`),
    positive: {
      accepted: true,
      runner_status: positiveReceipt.overall_status,
      command_exit_code: positiveReceipt.commands[0].exit_code,
      assertion_pass: positiveReceipt.assertions[0]?.pass ?? null,
    },
    negative: {
      discriminated: true,
      runner_status: negativeReceipt.overall_status,
      command_exit_code: negativeReceipt.commands[0].exit_code,
      assertion_pass: negativeReceipt.assertions[0]?.pass ?? null,
    },
  };
}

async function qualifyTw05(root) {
  const familyRoot = resolve(root, "tw05-normalized-contract");
  const positive = resolve(familyRoot, "positive");
  const negative = resolve(familyRoot, "negative");
  await Promise.all([mkdir(positive, {recursive: true}), mkdir(negative, {recursive: true})]);
  const expected = [
    {step: "open", revision: 0},
    {step: "overview", entity_count: 2, formula_count: 2},
    {step: "calculate", first_product: 2, second_product: 4},
    {step: "set_first_base", revision: 1, first_product: 22},
    {step: "stale_set_first_base", typed_stale_revision_error: true, actual_revision: 1, state_unchanged: true},
    {step: "snapshot", revision: 1, first_base: 11, first_product: 22},
  ];
  const historicalMissing = [
    ...expected.slice(0, 4),
    {step: "stale_set_first_base", typed_stale_revision_error: false, actual_revision: 2, state_unchanged: false},
    {step: "snapshot", revision: 2, first_base: 12, first_product: 24},
  ];
  await Promise.all([
    writeFile(resolve(positive, "normalized-observations.json"), `${JSON.stringify(expected)}\n`),
    writeFile(resolve(negative, "normalized-observations.json"), `${JSON.stringify(historicalMissing)}\n`),
  ]);
  const adapterSource = `#!/usr/bin/env node
import {createHash} from "node:crypto";
import {readFile, writeFile} from "node:fs/promises";
import {fileURLToPath} from "node:url";
const args = new Map();
for (let index = 2; index < process.argv.length; index += 2) args.set(process.argv[index], process.argv[index + 1]);
const observations = JSON.parse(await readFile(args.get("--candidate-root") + "/normalized-observations.json", "utf8"));
const contract = await readFile(args.get("--contract"));
const self = await readFile(fileURLToPath(import.meta.url));
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
await writeFile(args.get("--output"), JSON.stringify({
  contract_sha256: hash(contract),
  adapter: {sha256: hash(self), behavior_implemented_by_adapter: false},
  native: {execution: "native_process", observations},
  wasm: {execution: "real_wasm32", worker_boundary: "typescript_worker", observations},
}));
`;
  const adapter = resolve(familyRoot, "relay-adapter.mjs");
  await writeFile(adapter, adapterSource);
  const execute = (candidate, trusted) => spawnSync(
    process.execPath,
    [
      runOracles,
      "--case", "TW-05",
      "--candidate-root", candidate,
      "--trusted-dir", trusted,
      "--adapter-file", adapter,
    ],
    {encoding: "utf8", maxBuffer: 128 * 1024 * 1024},
  );
  const positiveResult = execute(positive, resolve(familyRoot, "positive-receipt"));
  const negativeResult = execute(negative, resolve(familyRoot, "negative-receipt"));
  if (positiveResult.status !== 0 || negativeResult.status !== 1) {
    fail(`TW-05 normalized qualification failed: positive=${positiveResult.status}, negative=${negativeResult.status}\n${positiveResult.stderr}\n${negativeResult.stderr}`);
  }
  const positiveReceipt = JSON.parse(positiveResult.stdout.trim().split(/\r?\n/).at(-1));
  const negativeReceipt = JSON.parse(negativeResult.stdout.trim().split(/\r?\n/).at(-1));
  return {
    id: "tw05_normalized_contract",
    fixture_sha256: sha256(`${adapterSource}${JSON.stringify(expected)}${JSON.stringify(historicalMissing)}`),
    positive: {
      accepted: positiveReceipt.assertions.every((entry) => entry.pass),
      runner_status: positiveReceipt.overall_status,
      assertion_pass_count: positiveReceipt.assertions.filter((entry) => entry.pass).length,
    },
    negative: {
      discriminated: negativeReceipt.assertions.some((entry) => !entry.pass),
      runner_status: negativeReceipt.overall_status,
      failed_assertion_ids: negativeReceipt.assertions.filter((entry) => !entry.pass).map((entry) => entry.id),
    },
  };
}

async function qualifyTw09(root) {
  const familyRoot = resolve(root, "tw09-normalized-contract");
  const positive = resolve(familyRoot, "positive");
  const negative = resolve(familyRoot, "negative");
  await Promise.all([mkdir(positive, {recursive: true}), mkdir(negative, {recursive: true})]);
  const contractPath = resolve(
    benchmarkDir,
    "evaluator/contracts/TW-09-stable-diagnostic-facts.json",
  );
  const contract = JSON.parse(await readFile(contractPath, "utf8"));
  const positiveObservations = contract.expected_observations;
  const negativeObservations = structuredClone(positiveObservations);
  negativeObservations.machine_fact.code = "presentation.only";
  await Promise.all([
    writeFile(
      resolve(positive, "normalized-observations.json"),
      `${JSON.stringify(positiveObservations)}\n`,
    ),
    writeFile(
      resolve(negative, "normalized-observations.json"),
      `${JSON.stringify(negativeObservations)}\n`,
    ),
  ]);
  const adapterSource = `#!/usr/bin/env node
import {createHash} from "node:crypto";
import {readFile, writeFile} from "node:fs/promises";
import {fileURLToPath} from "node:url";
const args = new Map();
for (let index = 2; index < process.argv.length; index += 2) args.set(process.argv[index], process.argv[index + 1]);
const observations = JSON.parse(await readFile(args.get("--candidate-root") + "/normalized-observations.json", "utf8"));
const contract = await readFile(args.get("--contract"));
const self = await readFile(fileURLToPath(import.meta.url));
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
await writeFile(args.get("--output"), JSON.stringify({
  contract_sha256: hash(contract),
  adapter: {sha256: hash(self), behavior_implemented_by_adapter: false},
  observations,
}));
`;
  const adapter = resolve(familyRoot, "relay-adapter.mjs");
  await writeFile(adapter, adapterSource);

  const frozenLock = JSON.parse(
    await readFile(resolve(benchmarkDir, "evaluator/oracle-lock.json"), "utf8"),
  );
  const assertions = frozenLock.cases
    .find((entry) => entry.id === "TW-09")
    .assertions.filter((entry) => entry.command_id === "tw09.validate");
  const caseId = "QF-TW09";
  const protocolId = "tachiko-tw09-qualification-v1";
  const manifest = {
    protocol_id: protocolId,
    classification: "construction_pilot_only",
    formal_result_eligible: false,
    cases: [{
      id: caseId,
      oracle_commands: [
        {
          id: "tw09.adapter",
          command_template: "node <trusted-adapter-file> --candidate-root <validation-workspace> --contract <trusted-contract-file> --output <trusted-observations-file>",
          assertion_ids: [],
        },
        {
          id: "tw09.validate",
          command_template: "node <benchmark>/scripts/validate-tw09-stable-facts.mjs --contract <trusted-contract-file> --observations <trusted-observations-file> --adapter-file <trusted-adapter-file>",
          assertion_ids: assertions.map((entry) => entry.id),
        },
      ],
      assertions: assertions.map((entry) => ({
        id: entry.id,
        command_id: entry.command_id,
        stage: "expectation_free_execution",
      })),
      subjective_groups: [],
    }],
  };
  const lock = {protocol_id: protocolId, cases: [{id: caseId, assertions}]};
  const manifestPath = resolve(familyRoot, "manifest.json");
  const lockPath = resolve(familyRoot, "lock.json");
  await Promise.all([
    writeFile(manifestPath, `${JSON.stringify(manifest)}\n`),
    writeFile(lockPath, `${JSON.stringify(lock)}\n`),
  ]);
  const execute = (candidate, trusted) => spawnSync(
    process.execPath,
    [
      runOracles,
      "--case", caseId,
      "--candidate-root", candidate,
      "--trusted-dir", trusted,
      "--manifest", manifestPath,
      "--oracle-lock", lockPath,
      "--benchmark-dir", benchmarkDir,
      "--adapter-file", adapter,
      "--contract-file", contractPath,
    ],
    {encoding: "utf8", maxBuffer: 128 * 1024 * 1024},
  );
  const positiveResult = execute(positive, resolve(familyRoot, "positive-receipt"));
  const negativeResult = execute(negative, resolve(familyRoot, "negative-receipt"));
  if (positiveResult.status !== 0 || negativeResult.status !== 1) {
    fail(`TW-09 normalized qualification failed: positive=${positiveResult.status}, negative=${negativeResult.status}\n${positiveResult.stderr}\n${negativeResult.stderr}`);
  }
  const positiveReceipt = JSON.parse(positiveResult.stdout.trim().split(/\r?\n/).at(-1));
  const negativeReceipt = JSON.parse(negativeResult.stdout.trim().split(/\r?\n/).at(-1));
  return {
    id: "tw09_normalized_contract",
    fixture_sha256: sha256(
      `${adapterSource}${JSON.stringify(positiveObservations)}${JSON.stringify(negativeObservations)}`,
    ),
    positive: {
      accepted: positiveReceipt.assertions.every((entry) => entry.pass),
      runner_status: positiveReceipt.overall_status,
      assertion_pass_count: positiveReceipt.assertions.filter((entry) => entry.pass).length,
    },
    negative: {
      discriminated: negativeReceipt.assertions.some((entry) => !entry.pass),
      runner_status: negativeReceipt.overall_status,
      failed_assertion_ids: negativeReceipt.assertions
        .filter((entry) => !entry.pass)
        .map((entry) => entry.id),
    },
  };
}

async function qualifyTw05Offline(root) {
  const familyRoot = resolve(root, "tw05-offline-direct");
  const positive = resolve(familyRoot, "positive");
  const negative = resolve(familyRoot, "negative");
  await Promise.all([mkdir(positive, {recursive: true}), mkdir(negative, {recursive: true})]);
  const worker = `import test from "node:test"; test("offline fixture", () => {});\n`;
  const benchmark = `process.stdout.write("offline fixture benchmark\\n");\n`;
  for (const candidate of [positive, negative]) {
    await Promise.all([
      writeFile(resolve(candidate, "worker.test.mjs"), worker),
      writeFile(resolve(candidate, "bench.mjs"), benchmark),
    ]);
  }
  const execute = (candidate, output, cargoCommand) => spawnSync(
    process.execPath,
    [
      runTw05Offline,
      "--candidate-root", candidate,
      "--output", output,
      "--cargo-command", cargoCommand,
      "--node-test-file", "worker.test.mjs",
      "--node-benchmark-file", "bench.mjs",
    ],
    {encoding: "utf8", maxBuffer: 128 * 1024 * 1024},
  );
  const positiveOutput = resolve(familyRoot, "positive.json");
  const negativeOutput = resolve(familyRoot, "negative.json");
  const positiveResult = execute(positive, positiveOutput, "cargo --version");
  const negativeResult = execute(negative, negativeOutput, "cargo metadata --offline");
  if (positiveResult.status !== 0 || negativeResult.status !== 1) {
    fail(`TW-05 offline qualification failed: positive=${positiveResult.status}, negative=${negativeResult.status}`);
  }
  const [positiveReceipt, negativeReceipt] = await Promise.all([
    readFile(positiveOutput, "utf8").then(JSON.parse),
    readFile(negativeOutput, "utf8").then(JSON.parse),
  ]);
  return {
    id: "tw05_offline_direct_execution",
    fixture_sha256: sha256(`${worker}${benchmark}`),
    positive: {
      accepted: positiveReceipt.pass && positiveReceipt.package_manager_dependency === false,
      direct_executables: positiveReceipt.executions.map((entry) => entry.name),
      offline: positiveReceipt.offline,
    },
    negative: {
      discriminated: !negativeReceipt.pass,
      failing_purpose: negativeReceipt.executions.find((entry) => entry.exit_code !== 0)?.purpose,
    },
  };
}

const args = parseArgs(process.argv.slice(2));
for (const key of ["source-repo", "output"]) {
  if (!args.has(key)) usage();
}
if (!isAbsolute(args.get("source-repo")) || !isAbsolute(args.get("output"))) {
  fail("source-repo and output must be absolute");
}
const sourceRepo = await realpath(resolve(args.get("source-repo")));
const output = resolve(args.get("output"));
const fixtureRoot = await mkdtemp(resolve(tmpdir(), "tachiko-oracle-qualification-"));

try {
  const expectedRecord = {index: 3, class: 0, bits: "good", auxiliary: "9"};
  const families = [];
  families.push(await executeRunner({
    root: fixtureRoot,
    id: "rust_test_exact",
    command: "node fixture-command.mjs",
    selector: {kind: "rust_test_exact", test_name: "locked_name", required_matching_tests: 1},
    positiveConfig: {kind: "rust", name: "locked_name", matches: 1},
    negativeConfig: {kind: "rust", name: "locked_name", matches: 0},
  }));
  families.push(await executeRunner({
    root: fixtureRoot,
    id: "json_pointer",
    command: "node fixture-command.mjs",
    selector: {kind: "json_pointer", json_pointer: "/assertions/ready", expected: true},
    positiveConfig: {kind: "json", value: true},
    negativeConfig: {kind: "json", value: false},
  }));
  families.push(await executeRunner({
    root: fixtureRoot,
    id: "portable_record_set",
    command: "node fixture-command.mjs --output <trusted-portable-observations-file>",
    selector: {
      kind: "portable_record_set",
      indexes: [3],
      expected_records: [expectedRecord],
      require_selected_native_wasm_equal: true,
      reject_class: 255,
    },
    positiveConfig: {
      kind: "portable",
      observations: {contract_id: "tachiko-portable-observations-v1", native: [expectedRecord], wasm: [expectedRecord]},
    },
    negativeConfig: {
      kind: "portable",
      observations: {
        contract_id: "tachiko-portable-observations-v1",
        native: [{...expectedRecord, bits: "bad"}],
        wasm: [expectedRecord],
      },
    },
  }));
  families.push(await executeRunner({
    root: fixtureRoot,
    id: "subjective_packet_gate",
    command: "node fixture-command.mjs",
    selector: null,
    positiveConfig: {kind: "packet", exit_code: 0},
    negativeConfig: {kind: "packet", exit_code: 3},
    subjective: true,
  }));
  families.push(await qualifyTw05(fixtureRoot));
  families.push(await qualifyTw05Offline(fixtureRoot));
  families.push(await qualifyTw09(fixtureRoot));

  const [casesBytes, manifestBytes, lockBytes] = await Promise.all([
    readFile(resolve(benchmarkDir, "evaluator/cases.json")),
    readFile(resolve(benchmarkDir, "evaluator/production-oracles.json")),
    readFile(resolve(benchmarkDir, "evaluator/oracle-lock.json")),
  ]);
  const casesDocument = JSON.parse(casesBytes.toString("utf8"));
  const familyMap = {
    "TW-01": ["subjective_packet_gate"],
    "TW-02": ["subjective_packet_gate"],
    "TW-03": ["rust_test_exact", "portable_record_set"],
    "TW-04": ["rust_test_exact", "json_pointer", "portable_record_set"],
    "TW-05": ["tw05_normalized_contract", "tw05_offline_direct_execution"],
    "TW-06": ["subjective_packet_gate"],
    "TW-07": ["rust_test_exact", "portable_record_set"],
    "TW-08": ["rust_test_exact"],
    "TW-09": [
      "rust_test_exact",
      "json_pointer",
      "portable_record_set",
      "tw09_normalized_contract",
    ],
  };
  const caseQualifications = [];
  for (const caseEntry of casesDocument.cases) {
    const targetTree = git(sourceRepo, ["rev-parse", `${caseEntry.ground_truth_commit}^{tree}`]);
    const baseTree = git(sourceRepo, ["rev-parse", `${caseEntry.historical_base_commit}^{tree}`]);
    if (baseTree !== caseEntry.historical_base_tree) {
      fail(`${caseEntry.id} historical base tree mismatch`);
    }
    const entry = {
      case_id: caseEntry.id,
      historical_target: {commit: caseEntry.ground_truth_commit, tree: targetTree},
      historical_base: {commit: caseEntry.historical_base_commit, tree: baseTree},
      qualified_families: familyMap[caseEntry.id],
      positive: {accepted: true, source: "controlled_reference_and_frozen_target_identity"},
      negative: {discriminated: true, source: "behavior_missing_reference_and_frozen_base_identity"},
    };
    if (caseEntry.id === "TW-05") {
      const expectedRevision = spawnSync(
        "rtk",
        ["proxy", "git", "grep", "-n", "expected_revision", caseEntry.ground_truth_commit, "--", "spikes/issue-26-runtime"],
        {cwd: sourceRepo, encoding: "utf8"},
      );
      const typedStale = spawnSync(
        "rtk",
        ["proxy", "git", "grep", "-n", "StaleRevision", caseEntry.ground_truth_commit, "--", "spikes/issue-26-runtime"],
        {cwd: sourceRepo, encoding: "utf8"},
      );
      if (expectedRevision.status !== 1 || typedStale.status !== 1) {
        fail("TW-05 historical target calibration unexpectedly contains stale-revision support");
      }
      entry.historical_target_calibration = {
        outcome: "expected_contract_miss",
        stale_revision_rejection_present: false,
        required_behavior_preserved: true,
        qualification_positive_source: "controlled_reference_positive",
        limitation: "the frozen historical target predates the required expected-revision input and is not claimed as a contract-positive",
      };
    }
    caseQualifications.push(entry);
  }

  const controlPaths = [
    "evaluator/cases.json",
    "evaluator/oracle-lock.json",
    "evaluator/production-oracles.json",
    "evaluator/contracts/TW-05-resident-parity.json",
    "evaluator/contracts/TW-09-stable-diagnostic-facts.json",
    "scripts/qualify-oracles.mjs",
    "scripts/run-oracles.mjs",
    "scripts/run-tw05-offline.mjs",
    "scripts/validate-tw05-observations.mjs",
    "scripts/validate-tw09-stable-facts.mjs",
  ];
  const controls = [];
  for (const path of controlPaths) {
    const bytes = await readFile(resolve(benchmarkDir, path));
    controls.push({path, bytes: bytes.length, sha256: sha256(bytes)});
  }
  const payload = {
    schema: "tachiko-oracle-qualification-v1",
    protocol_id: casesDocument.protocol_id,
    classification: "construction_pilot_only",
    formal_result_eligible: false,
    execution_standard: "practical_internal_v1",
    no_codex_launched: true,
    controls,
    frozen_manifest_sha256: sha256(manifestBytes),
    frozen_oracle_lock_sha256: sha256(lockBytes),
    families,
    cases: caseQualifications,
    limitations: [
      "Controlled references qualify selector, validator, packet-gate, and offline-execution families; historical target identity is bound but historical targets are not all recompiled in this compact qualification.",
      "TW-05 historical target 16289f8 lacks stale-revision rejection, so its expected miss is calibration evidence and the controlled reference-positive qualifies the frozen requirement.",
      "Subjective criteria are qualified as deterministic packet gates; no machine score or reviewer judgment is fabricated.",
    ],
  };
  const receipt = {
    payload_sha256: sha256(`${JSON.stringify(payload)}\n`),
    payload,
  };
  await mkdir(dirname(output), {recursive: true});
  await writeFile(output, `${JSON.stringify(receipt, null, 2)}\n`, {mode: 0o600});
  console.log(JSON.stringify({output, payload_sha256: receipt.payload_sha256}));
} finally {
  await rm(fixtureRoot, {recursive: true, force: true});
}
