#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFile, stat } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const benchmarkDir = resolve(scriptDir, "..");
const repositoryDir = resolve(benchmarkDir, "../..");

function fail(message) {
  throw new Error(message);
}

function runRtkGit(args) {
  const result = spawnSync("rtk", ["proxy", "git", ...args], {
    cwd: repositoryDir,
    encoding: "utf8",
  });
  if (result.status !== 0) {
    fail(`rtk proxy git ${args.join(" ")} failed: ${result.stderr || result.stdout}`);
  }
  return result.stdout.trim();
}

function runRtkGitBytes(args) {
  const result = spawnSync("rtk", ["proxy", "git", ...args], {
    cwd: repositoryDir,
    encoding: null,
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    fail(
      `rtk proxy git ${args.join(" ")} failed: ` +
        Buffer.from(result.stderr ?? result.stdout ?? []).toString("utf8"),
    );
  }
  return Buffer.from(result.stdout);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

const casesPath = resolve(benchmarkDir, "evaluator/cases.json");
const oraclePath = resolve(benchmarkDir, "evaluator/oracle-lock.json");
const coreScorePath = resolve(benchmarkDir, "evaluator/core-score-lock.json");
const productionOraclePath = resolve(
  benchmarkDir,
  "evaluator/production-oracles.json",
);
const authorityPath = resolve(benchmarkDir, "evaluator/authority-lock.json");
const authoritySnapshotsPath = resolve(
  benchmarkDir,
  "evaluator/authority-snapshots.json",
);
const historyPath = resolve(benchmarkDir, "evaluator/history-snapshots.json");
const environmentPath = resolve(benchmarkDir, "environment-lock.json");
const schemaPath = resolve(benchmarkDir, "schemas/result-record.schema.json");
const reviewSchemaPath = resolve(
  benchmarkDir,
  "schemas/blinded-review-score.schema.json",
);
const constructionPilotIndexPath = resolve(
  benchmarkDir,
  "evaluator/construction-pilot-index.json",
);
const rubricPath = resolve(benchmarkDir, "SCORING.md");

const casesDocument = JSON.parse(await readFile(casesPath, "utf8"));
const oracleBytes = await readFile(oraclePath);
const oracleLock = JSON.parse(oracleBytes.toString("utf8"));
const coreScoreBytes = await readFile(coreScorePath);
const coreScoreLock = JSON.parse(coreScoreBytes.toString("utf8"));
const productionOracleBytes = await readFile(productionOraclePath);
const productionOracles = JSON.parse(productionOracleBytes.toString("utf8"));
const authorityBytes = await readFile(authorityPath);
const authorityLock = JSON.parse(authorityBytes.toString("utf8"));
const authoritySnapshotBytes = await readFile(authoritySnapshotsPath);
const authoritySnapshots = JSON.parse(authoritySnapshotBytes.toString("utf8"));
const historyBytes = await readFile(historyPath);
const historySnapshots = JSON.parse(historyBytes.toString("utf8"));
const environment = JSON.parse(await readFile(environmentPath, "utf8"));
const resultSchema = JSON.parse(await readFile(schemaPath, "utf8"));
const reviewSchema = JSON.parse(await readFile(reviewSchemaPath, "utf8"));
const constructionPilotIndexBytes = await readFile(constructionPilotIndexPath);
const constructionPilotIndex = JSON.parse(constructionPilotIndexBytes.toString("utf8"));

const globalContractBindings = [
  ["pre_unblind_projection_contract", "pre_unblind_projection_contract_sha256"],
  ["semantic_result_projection_contract", "semantic_result_projection_contract_sha256"],
  ["wave_registration_contract", "wave_registration_contract_sha256"],
  ["reviewer_allocation_contract", "reviewer_allocator_contract_sha256"],
  ["review_packet_blinding_contract", "review_packet_blinding_contract_sha256"],
  ["denied_access_attribution_contract", "denied_access_contract_sha256"],
];
for (const [environmentKey, schemaKey] of globalContractBindings) {
  const lock = environment.result_recording?.[environmentKey];
  if (!lock) fail(`missing environment lock for ${environmentKey}`);
  const contractPath = resolve(benchmarkDir, lock.path);
  if (!contractPath.startsWith(`${benchmarkDir}/`)) {
    fail(`global contract escapes benchmark root: ${lock.path}`);
  }
  const bytes = await readFile(contractPath);
  const document = JSON.parse(bytes.toString("utf8"));
  if (
    bytes.length !== lock.bytes ||
    sha256(bytes) !== lock.sha256 ||
    document.contract_id !== lock.id
  ) {
    fail(`global contract hash/size/identity mismatch: ${lock.path}`);
  }
  const schemaConstant =
    resultSchema.properties?.registration?.properties?.[schemaKey]?.const;
  if (schemaConstant !== lock.sha256) {
    fail(`result schema does not bind ${lock.path}`);
  }
}

function verifyPatternTypes(node, path = "$") {
  if (!node || typeof node !== "object") return;
  if (Object.hasOwn(node, "pattern")) {
    const types = Array.isArray(node.type) ? node.type : [node.type];
    if (!types.includes("string")) {
      fail(`${path} declares pattern without string type`);
    }
  }
  for (const [key, value] of Object.entries(node)) {
    verifyPatternTypes(value, `${path}.${key}`);
  }
}
verifyPatternTypes(resultSchema, "result-record.schema.json");
verifyPatternTypes(reviewSchema, "blinded-review-score.schema.json");

if (
  constructionPilotIndex.protocol_id !== casesDocument.protocol_id ||
  constructionPilotIndex.classification !== "construction_pilot_only" ||
  constructionPilotIndex.formal_result_eligible !== false
) {
  fail("construction pilot index is not permanently excluded from formal results");
}
for (const receipt of constructionPilotIndex.receipts) {
  const receiptPath = resolve(benchmarkDir, receipt.path);
  if (!receiptPath.startsWith(`${benchmarkDir}/`)) {
    fail(`construction receipt escapes benchmark root: ${receipt.path}`);
  }
  const bytes = await readFile(receiptPath);
  if (bytes.length !== receipt.bytes || sha256(bytes) !== receipt.sha256) {
    fail(`construction receipt hash/size mismatch: ${receipt.path}`);
  }
  if (receipt.path.endsWith(".json")) {
    const document = JSON.parse(bytes.toString("utf8"));
    if (
      document.classification !== "construction_pilot_only" ||
      document.formal_result_eligible !== false
    ) {
      fail(`construction receipt is not formally excluded: ${receipt.path}`);
    }
    for (const command of document.commands ?? []) {
      if (
        sha256(Buffer.from(command.stdout, "utf8")) !== command.stdout_sha256 ||
        sha256(Buffer.from(command.stderr, "utf8")) !== command.stderr_sha256
      ) {
        fail(`construction command output hash mismatch: ${receipt.path}`);
      }
    }
  }
}

const tw09PortablePilot = JSON.parse(
  await readFile(
    resolve(
      benchmarkDir,
      "evaluator/construction-pilots/TW-09-portable-selected.json",
    ),
    "utf8",
  ),
);
const tw09PortableContract = JSON.parse(
  await readFile(
    resolve(
      benchmarkDir,
      "evaluator/contracts/TW-09-portable-validation-observations.json",
    ),
    "utf8",
  ),
);
const expectedTw09Records = tw09PortableContract.records.map((record) => ({
  index: record.index,
  class: record.expected_class,
  bits: record.expected_count.toString(16).padStart(16, "0"),
  auxiliary: record.expected_fingerprint,
}));
if (
  JSON.stringify(tw09PortablePilot.native) !== JSON.stringify(expectedTw09Records) ||
  JSON.stringify(tw09PortablePilot.wasm) !== JSON.stringify(expectedTw09Records) ||
  tw09PortablePilot.native_wasm_byte_equal !== true ||
  tw09PortablePilot.matches_case_local_contract !== true
) {
  fail("TW-09 construction portable receipt differs from its case-local contract");
}

if (casesDocument.protocol_id !== "tachiko-agents-effect-v1") {
  fail("unexpected protocol_id in evaluator/cases.json");
}
if (casesDocument.case_count !== 9 || casesDocument.cases.length !== 9) {
  fail("the manifest must contain exactly nine cases");
}
if (oracleLock.protocol_id !== casesDocument.protocol_id || oracleLock.cases.length !== 9) {
  fail("oracle lock must contain the same protocol and nine cases");
}
if (
  oracleLock.oracle_lock_version !== 2 ||
  oracleLock.physical_command_status !==
    "construction_only_unqualified_for_controlled_run"
) {
  fail("oracle lock must use assertion-level v2 and fail closed on physical commands");
}
if (
  coreScoreLock.protocol_id !== casesDocument.protocol_id ||
  coreScoreLock.machine_points !== 19 ||
  coreScoreLock.cases.length !== 9
) {
  fail("core score lock must contain the same protocol, nine cases, and 19 machine points");
}
const globalCorePoints = coreScoreLock.global_checks.reduce(
  (sum, check) => sum + check.points,
  0,
);
if (globalCorePoints !== 9) fail("global core checks must total nine points");
const reproducibilityCheck = coreScoreLock.global_checks.find(
  (check) => check.id === "core.tooling.reproducible_dependencies",
);
if (
  !reproducibilityCheck ||
  reproducibilityCheck.dimension !== "tooling_workflow" ||
  reproducibilityCheck.selector !==
    "registered_command_policy_and_environment_receipts" ||
  !reproducibilityCheck.pass_rule.includes(
    "does not affect this tooling check",
  ) ||
  reproducibilityCheck.evidence.some((item) => /exit status/i.test(item))
) {
  fail("dependency tooling points must not duplicate candidate validation status");
}
if (
  !resultSchema.required.includes("blinded_candidate_id") ||
  !resultSchema.required.includes("registration") ||
  !resultSchema.required.includes("record_validation_receipt_sha256") ||
  resultSchema.properties?.timing?.properties?.process_group_extinct?.type !==
    "boolean" ||
  resultSchema.properties?.environment?.properties?.probe_builder_account?.const !==
    "probe-builder" ||
  resultSchema.properties?.environment?.properties?.probe_execution_account?.const !==
    "probe-runner" ||
  !resultSchema.properties?.environment?.required?.includes(
    "agent_visible_identity_receipt_sha256",
  ) ||
  !resultSchema.properties?.environment?.required?.includes(
    "home_isolation_receipt_sha256",
  ) ||
  !resultSchema.properties?.registration?.required?.includes(
    "production_oracle_command_manifest_sha256",
  ) ||
  !resultSchema.properties?.registration?.required?.includes(
    "intrinsic_neutrality_audit_sha256",
  ) ||
  !resultSchema.properties?.registration?.required?.includes(
    "variant_comparison_audit_sha256",
  )
) {
  fail("result schema lacks the locked identity/termination/oracle-isolation contract");
}
if (historySnapshots.repository !== casesDocument.repository) {
  fail("history snapshot repository mismatch");
}
if (
  authorityLock.protocol_id !== casesDocument.protocol_id ||
  authorityLock.cases.length !== 9 ||
  authoritySnapshots.repository !== casesDocument.repository
) {
  fail("authority lock/snapshot protocol or repository mismatch");
}

function resolveAuthorityText(source) {
  if (source.snapshot === "authority") {
    const item = authoritySnapshots.resources.find(
      (candidate) => candidate.node_id === source.node_id,
    );
    if (!item || item.created_at !== source.effective_at) {
      fail(`${source.id} does not resolve in authority-snapshots.json`);
    }
    if (item.updated_at !== item.created_at) {
      fail(`${source.id} review comment was edited`);
    }
    return item.body;
  }

  const match = source.resource.match(/^(issue|pr)#(\d+)$/);
  if (!match) fail(`${source.id} has an invalid history resource locator`);
  const resource = historySnapshots.resources.find(
    (candidate) => candidate.kind === match[1] && candidate.number === Number(match[2]),
  );
  if (!resource) fail(`${source.id} history resource is absent`);
  if (source.kind === "body") {
    if (
      source.temporal_integrity !== "exact_unedited" ||
      resource.body_edit_history.length !== 0 ||
      resource.captured_resource.createdAt !== source.effective_at
    ) {
      fail(`${source.id} body lacks exact unedited temporal integrity`);
    }
    return resource.captured_resource.body;
  }
  if (source.kind === "body_version") {
    const version = resource.body_edit_history.find(
      (candidate) => candidate.id === source.node_id,
    );
    if (!version || version.editedAt !== source.effective_at || !version.diff) {
      fail(`${source.id} exact body version is absent`);
    }
    return version.diff;
  }
  if (source.kind === "comment") {
    const comment = resource.captured_resource.comments.find(
      (candidate) => (candidate.id ?? candidate.node_id) === source.node_id,
    );
    if (!comment || comment.createdAt !== source.effective_at) {
      fail(`${source.id} exact comment is absent`);
    }
    if (comment.updatedAt && comment.updatedAt !== comment.createdAt) {
      fail(`${source.id} comment was edited without a frozen version`);
    }
    if (comment.includesCreatedEdit !== false) {
      fail(`${source.id} comment does not prove an unedited creation body`);
    }
    return comment.body;
  }
  fail(`${source.id} uses an unsupported authority source kind`);
}

const ids = new Set();
for (const entry of casesDocument.cases) {
  if (ids.has(entry.id)) fail(`duplicate case ID ${entry.id}`);
  ids.add(entry.id);

  const expectedId = `TW-${String(ids.size).padStart(2, "0")}`;
  if (entry.id !== expectedId) fail(`expected ${expectedId}, found ${entry.id}`);

  const taskPath = resolve(benchmarkDir, entry.task_file);
  const taskBytes = await readFile(taskPath);
  const taskStat = await stat(taskPath);
  if (sha256(taskBytes) !== entry.task_sha256) {
    fail(`${entry.id} task SHA-256 mismatch`);
  }
  if (taskStat.size !== entry.task_bytes) {
    fail(`${entry.id} task byte count mismatch`);
  }

  const taskText = taskBytes.toString("utf8");
  const offlineNotice =
    "Work only from the supplied checkout and task context; external services,\n" +
    "current remote state, and descendant history are unavailable.";
  if (taskText.split(offlineNotice).length !== 2) {
    fail(`${entry.id} must contain exactly one common offline-authority notice`);
  }
  const leakPatterns = [
    /github\.com\//i,
    /pull\/\d+/i,
    /issues\/\d+/i,
    /AGENTS\.md.{0,40}benchmark/i,
    /benchmark (case|score|evaluation|experiment)/i,
    /\bBaseline A\b/i,
    /\bVariant B\b/i,
    /\bscoring rubric\b/i,
    /\bhidden failure/i,
    /\b[0-9a-f]{40}\b/i,
  ];
  for (const pattern of leakPatterns) {
    if (pattern.test(taskText)) fail(`${entry.id} task leaks ${pattern}`);
  }

  const base = runRtkGit(["rev-parse", `${entry.historical_base_commit}^{commit}`]);
  const target = runRtkGit(["rev-parse", `${entry.ground_truth_commit}^{commit}`]);
  const tree = runRtkGit(["show", "-s", "--format=%T", base]);
  const parent = runRtkGit(["rev-parse", `${target}^1`]);
  if (base !== entry.historical_base_commit) fail(`${entry.id} base is not exact`);
  if (target !== entry.ground_truth_commit) fail(`${entry.id} target is not exact`);
  if (tree !== entry.historical_base_tree) fail(`${entry.id} base tree mismatch`);
  const baseAncestry = spawnSync(
    "rtk",
    ["proxy", "git", "merge-base", "--is-ancestor", base, target],
    {cwd: repositoryDir, encoding: "utf8"},
  );
  if (baseAncestry.status !== 0) fail(`${entry.id} base is not an ancestor of target`);
  if (entry.replay_relation === "direct_parent") {
    if (parent !== base) fail(`${entry.id} direct replay target does not have the base as parent`);
    if (entry.implementation_parent_commit || entry.intervening_outcome_only_commits) {
      fail(`${entry.id} direct replay contains an intervening-history exception`);
    }
  } else if (entry.replay_relation === "ancestor_with_independent_intervening_changes") {
    if (
      entry.implementation_parent_commit !== parent ||
      !Array.isArray(entry.intervening_outcome_only_commits) ||
      entry.intervening_outcome_only_commits.length === 0
    ) {
      fail(`${entry.id} ancestor replay lacks an exact implementation-parent lock`);
    }
    const intervening = runRtkGit([
      "rev-list",
      "--first-parent",
      "--reverse",
      `${base}..${parent}`,
    ])
      .split("\n")
      .filter(Boolean);
    if (JSON.stringify(intervening) !== JSON.stringify(entry.intervening_outcome_only_commits)) {
      fail(`${entry.id} intervening first-parent history differs from its lock`);
    }
  } else {
    fail(`${entry.id} has an unsupported replay relation`);
  }

  const historicalAgentsObjects = runRtkGit([
    "rev-list",
    "--objects",
    base,
    "--",
    "AGENTS.md",
  ])
    .split("\n")
    .filter((line) => line.endsWith(" AGENTS.md"));
  if (historicalAgentsObjects.length !== 0) {
    fail(`${entry.id} base ancestry exposes a historical root AGENTS.md blob`);
  }

  if (!Array.isArray(entry.expected_scope) || entry.expected_scope.length === 0) {
    fail(`${entry.id} has no expected scope`);
  }
  if (!Array.isArray(entry.forbidden_scope) || entry.forbidden_scope.length === 0) {
    fail(`${entry.id} has no forbidden scope`);
  }
  const groups = entry.validation?.machine_contract_groups;
  if (!Array.isArray(groups) || groups.length === 0) fail(`${entry.id} has no machine groups`);
  const functionalPoints = groups.reduce((sum, group) => sum + group.points, 0);
  if (functionalPoints !== 24) fail(`${entry.id} machine contract groups must total 24`);
  if (!groups.every((group) => group.mandatory === true)) {
    fail(`${entry.id} contains a non-mandatory primary contract group`);
  }
  const assessmentModes = new Set([
    "machine_oracle",
    "machine_with_blinded_adapter",
    "blinded_semantic_review",
  ]);
  if (!groups.every((group) => assessmentModes.has(group.assessment))) {
    fail(`${entry.id} contains an invalid contract-group assessment mode`);
  }
  const blindedFunctionalPoints = groups
    .filter((group) => group.assessment === "blinded_semantic_review")
    .reduce((sum, group) => sum + group.points, 0);
  const expectedMachinePoints = 43 - blindedFunctionalPoints;
  const expectedReviewPoints = 57 + blindedFunctionalPoints;
  if (
    entry.score_allocation?.machine !== expectedMachinePoints ||
    entry.score_allocation?.blinded_review !== expectedReviewPoints
  ) {
    fail(`${entry.id} score allocation does not match its assessment modes`);
  }
  if (!Array.isArray(entry.known_failure_modes) || entry.known_failure_modes.length === 0) {
    fail(`${entry.id} has no known failure modes`);
  }
  if (
    !entry.known_failure_modes.every(
      (mode) =>
        mode.description &&
        ["prompt_explicit", "base_discoverable", "evaluator_hidden"].includes(
          mode.visibility,
        ) &&
        mode.provenance,
    )
  ) {
    fail(`${entry.id} has an invalid failure-mode visibility record`);
  }
  for (const reference of entry.references) {
    const match = reference.match(/\/(issues|pull)\/(\d+)$/);
    if (!match) fail(`${entry.id} has an invalid historical reference ${reference}`);
    const kind = match[1] === "issues" ? "issue" : "pr";
    const number = Number(match[2]);
    const snapshot = historySnapshots.resources.find(
      (resource) => resource.kind === kind && resource.number === number,
    );
    if (!snapshot || snapshot.captured_resource.url !== reference) {
      fail(`${entry.id} has no matching frozen snapshot for ${reference}`);
    }
  }

  const oracle = oracleLock.cases.find((candidate) => candidate.id === entry.id);
  if (!oracle) fail(`${entry.id} has no oracle lock entry`);
  if (oracle.source_commit !== entry.ground_truth_commit) {
    fail(`${entry.id} oracle source does not equal the ground-truth commit`);
  }
  if (!Array.isArray(oracle.commands) || oracle.commands.length === 0) {
    fail(`${entry.id} has no evaluator command`);
  }
  if (
    !Array.isArray(oracle.command_specs) ||
    oracle.command_specs.length !== oracle.commands.length ||
    JSON.stringify(oracle.command_specs.map((command) => command.run)) !==
      JSON.stringify(oracle.commands)
  ) {
    fail(`${entry.id} command specs do not match its command list`);
  }
  const commandIds = new Set(oracle.command_specs.map((command) => command.id));
  if (commandIds.size !== oracle.command_specs.length) {
    fail(`${entry.id} has duplicate evaluator command IDs`);
  }
  if (!Array.isArray(oracle.group_mappings) || oracle.group_mappings.length !== 4) {
    fail(`${entry.id} must map exactly four contract groups to evaluator evidence`);
  }
  const mappedIds = oracle.group_mappings.map((mapping) => mapping.id).sort();
  const groupIds = groups.map((group) => group.id).sort();
  if (JSON.stringify(mappedIds) !== JSON.stringify(groupIds)) {
    fail(`${entry.id} oracle mappings do not match its contract groups`);
  }
  if (!Array.isArray(oracle.assertions)) {
    fail(`${entry.id} has no assertion registry`);
  }
  const assertionIds = new Set();
  const commandSelectors = new Map();
  for (const assertion of oracle.assertions) {
    if (assertionIds.has(assertion.id)) {
      fail(`${entry.id} has duplicate evaluator assertion ${assertion.id}`);
    }
    assertionIds.add(assertion.id);
    if (
      assertion.mandatory !== true ||
      !commandIds.has(assertion.command_id) ||
      !Number.isFinite(assertion.points) ||
      assertion.points <= 0 ||
      !Array.isArray(assertion.contract_group_ids) ||
      assertion.contract_group_ids.length !== 1 ||
      typeof assertion.pass_rule !== "string"
    ) {
      fail(`${entry.id} assertion ${assertion.id} has invalid common fields`);
    }

    const selector = assertion.selector;
    if (!selector || typeof selector.adapter_allowed !== "boolean") {
      fail(`${entry.id} assertion ${assertion.id} has no adapter policy`);
    }
    let selectorIdentity;
    if (selector.kind === "rust_test_exact") {
      if (!selector.test_name || selector.required_matching_tests !== 1) {
        fail(`${entry.id} assertion ${assertion.id} has an invalid exact-test selector`);
      }
      selectorIdentity = `rust:${selector.test_name}`;
    } else if (selector.kind === "json_pointer") {
      if (!selector.json_pointer?.startsWith("/") || selector.expected !== true) {
        fail(`${entry.id} assertion ${assertion.id} has an invalid JSON pointer selector`);
      }
      selectorIdentity = `json:${selector.json_pointer}`;
    } else if (selector.kind === "portable_record_set") {
      if (
        !Array.isArray(selector.indexes) ||
        selector.indexes.length === 0 ||
        new Set(selector.indexes).size !== selector.indexes.length ||
        !Array.isArray(selector.expected_records) ||
        selector.expected_records.length !== selector.indexes.length ||
        selector.require_selected_native_wasm_equal !== true ||
        selector.reject_class !== 255 ||
        !["core", "full"].includes(selector.crate_set)
      ) {
        fail(`${entry.id} assertion ${assertion.id} has an invalid portable selector`);
      }
      const expectedIndexes = selector.expected_records.map((record) => record.index);
      if (JSON.stringify(expectedIndexes) !== JSON.stringify(selector.indexes)) {
        fail(`${entry.id} assertion ${assertion.id} portable indexes do not match records`);
      }
      if (
        !selector.expected_records.every(
          (record) =>
            Number.isInteger(record.index) &&
            Number.isInteger(record.class) &&
            record.class !== 255 &&
            /^[0-9a-f]{16}$/.test(record.bits) &&
            /^\d+$/.test(record.auxiliary),
        )
      ) {
        fail(`${entry.id} assertion ${assertion.id} has an invalid portable record`);
      }
      selectorIdentity = `portable:${selector.indexes.join(",")}`;
    } else {
      fail(`${entry.id} assertion ${assertion.id} has unsupported selector kind`);
    }
    const selectors = commandSelectors.get(assertion.command_id) ?? [];
    selectors.push({kind: selector.kind, identity: selectorIdentity});
    commandSelectors.set(assertion.command_id, selectors);
  }

  for (const [commandId, selectors] of commandSelectors) {
    if (selectors.length <= 1) continue;
    if (
      !selectors.every((selector) => selector.kind === "json_pointer") ||
      new Set(selectors.map((selector) => selector.identity)).size !== selectors.length
    ) {
      fail(`${entry.id} command ${commandId} reuses non-disjoint point evidence`);
    }
  }

  const mappedAssertions = [];
  for (const mapping of oracle.group_mappings) {
    const manifestGroup = groups.find((candidate) => candidate.id === mapping.id);
    if (
      mapping.assessment !== manifestGroup.assessment ||
      !Array.isArray(mapping.assertion_ids)
    ) {
      fail(`${entry.id} group ${mapping.id} has an invalid assessment mapping`);
    }
    if (mapping.assessment === "blinded_semantic_review") {
      if (
        mapping.assertion_ids.length !== 0 ||
        !Array.isArray(mapping.review_anchors) ||
        mapping.review_anchors.length === 0
      ) {
        fail(`${entry.id} blinded group ${mapping.id} has invalid review anchors`);
      }
      continue;
    }
    if (mapping.assertion_ids.length === 0) {
      fail(`${entry.id} machine group ${mapping.id} has no assertions`);
    }
    let groupPoints = 0;
    for (const assertionId of mapping.assertion_ids) {
      const assertion = oracle.assertions.find((candidate) => candidate.id === assertionId);
      if (
        !assertion ||
        assertion.contract_group_ids[0] !== mapping.id ||
        assertion.selector.adapter_allowed !==
          (mapping.assessment === "machine_with_blinded_adapter")
      ) {
        fail(`${entry.id} group ${mapping.id} has an invalid assertion ${assertionId}`);
      }
      mappedAssertions.push(assertionId);
      groupPoints += assertion.points;
    }
    if (groupPoints !== manifestGroup.points) {
      fail(`${entry.id} group ${mapping.id} assertion points do not match manifest`);
    }
  }
  if (
    mappedAssertions.length !== oracle.assertions.length ||
    new Set(mappedAssertions).size !== mappedAssertions.length ||
    !oracle.assertions.every((assertion) => mappedAssertions.includes(assertion.id))
  ) {
    fail(`${entry.id} assertions are not assigned exactly once`);
  }
  for (const command of oracle.command_specs) {
    if (/\b(?:npm|yarn|bun)\b/.test(command.run)) {
      fail(`${entry.id} evaluator command violates the pinned package workflow`);
    }
  }
  for (const file of oracle.files) {
    if (!file.path || !/^[0-9a-f]{64}$/.test(file.sha256)) {
      fail(`${entry.id} has an invalid oracle file record`);
    }
    const sourceBytes = runRtkGitBytes(["show", `${oracle.source_commit}:${file.path}`]);
    if (sha256(sourceBytes) !== file.sha256) {
      fail(`${entry.id} oracle SHA-256 mismatch for ${file.path}`);
    }
  }
  for (const contract of oracle.constructed_contracts ?? []) {
    const contractPath = resolve(benchmarkDir, contract.path);
    if (!contractPath.startsWith(`${benchmarkDir}/`)) {
      fail(`${entry.id} constructed contract escapes the benchmark directory`);
    }
    if (sha256(await readFile(contractPath)) !== contract.sha256) {
      fail(`${entry.id} constructed contract SHA-256 mismatch for ${contract.path}`);
    }
  }

  const coreScore = coreScoreLock.cases.find((candidate) => candidate.id === entry.id);
  if (!coreScore || !Array.isArray(coreScore.validation_checks)) {
    fail(`${entry.id} has no core-score validation lock`);
  }
  if (
    coreScore.validation_checks.reduce((sum, check) => sum + check.points, 0) !== 10
  ) {
    fail(`${entry.id} core-score validation checks must total ten points`);
  }
  const candidateCommands = entry.validation?.candidate ?? [];
  for (const check of coreScore.validation_checks) {
    if (
      check.dimension !== "validation_tests" ||
      !check.id?.startsWith(`core.${entry.id.toLowerCase().replace("-", "")}.`) ||
      !candidateCommands.includes(check.command)
    ) {
      fail(`${entry.id} has an invalid core-score validation check`);
    }
  }

  const authority = authorityLock.cases.find((candidate) => candidate.id === entry.id);
  if (!authority || authority.base_commit !== entry.historical_base_commit) {
    fail(`${entry.id} authority lock base mismatch`);
  }
  const cutoff = Date.parse(authority.assignment_cutoff);
  if (!Number.isFinite(cutoff)) fail(`${entry.id} authority cutoff is invalid`);
  const baseCommittedAt = Date.parse(
    runRtkGit(["show", "-s", "--format=%cI", entry.historical_base_commit]),
  );
  if (baseCommittedAt > cutoff) fail(`${entry.id} authority cutoff predates its base`);
  const groundTruthCommittedAt = Date.parse(
    runRtkGit(["show", "-s", "--format=%cI", entry.ground_truth_commit]),
  );
  if (cutoff >= groundTruthCommittedAt) {
    fail(`${entry.id} authority cutoff is not earlier than its ground truth`);
  }
  const sourceIds = new Set();
  for (const source of authority.task_authority) {
    if (sourceIds.has(source.id)) fail(`${entry.id} duplicate authority source ${source.id}`);
    sourceIds.add(source.id);
    if (Date.parse(source.effective_at) > cutoff) {
      fail(`${entry.id} authority source ${source.id} is after assignment cutoff`);
    }
    if (sha256(Buffer.from(resolveAuthorityText(source), "utf8")) !== source.text_sha256) {
      fail(`${entry.id} authority source ${source.id} text hash mismatch`);
    }
  }
  const claimedGroups = new Set();
  let hasScopeClaim = false;
  for (const claim of authority.claims) {
    if (!claim.authority_source_ids.every((id) => sourceIds.has(id))) {
      fail(`${entry.id} claim ${claim.id} cites a non-authority source`);
    }
    if (claim.id === "scope_and_exclusions") hasScopeClaim = true;
    for (const groupId of claim.contract_group_ids) {
      if (!groupIds.includes(groupId)) {
        fail(`${entry.id} claim ${claim.id} maps an unknown contract group`);
      }
      claimedGroups.add(groupId);
    }
  }
  if (!hasScopeClaim || claimedGroups.size !== groupIds.length) {
    fail(`${entry.id} authority claims do not cover scope and every contract group`);
  }
  if (!authority.outcome_only.includes(`git:${entry.ground_truth_commit}`)) {
    fail(`${entry.id} ground-truth commit is not explicitly outcome-only`);
  }
}

const coreIds = [
  ...coreScoreLock.global_checks.map((check) => check.id),
  ...coreScoreLock.cases.flatMap((entry) =>
    entry.validation_checks.map((check) => check.id),
  ),
];
if (new Set(coreIds).size !== coreIds.length) fail("core-score check IDs must be unique");

const allAssertionIds = oracleLock.cases.flatMap((entry) =>
  entry.assertions.map((assertion) => assertion.id),
);
if (new Set(allAssertionIds).size !== allAssertionIds.length) {
  fail("oracle assertion IDs must be globally unique");
}

function productionIds(entries, label) {
  if (!Array.isArray(entries)) fail(`${label} must be an array`);
  const values = entries.map((entry) => entry.id);
  if (values.some((id) => typeof id !== "string") || new Set(values).size !== values.length) {
    fail(`${label} IDs must be unique strings`);
  }
  return values.sort();
}

function sameProductionIds(actual, expected, label) {
  if (JSON.stringify(productionIds(actual, label)) !== JSON.stringify([...expected].sort())) {
    fail(`${label} does not represent the frozen IDs exactly once`);
  }
}

function rejectFrozenScoringCopies(value, path = "production-oracles.json") {
  if (!value || typeof value !== "object") return;
  if (Object.hasOwn(value, "points") || Object.hasOwn(value, "selector")) {
    fail(`${path} must reference frozen points and selectors instead of copying them`);
  }
  for (const [key, child] of Object.entries(value)) {
    rejectFrozenScoringCopies(child, `${path}.${key}`);
  }
}

if (
  productionOracles.protocol_id !== casesDocument.protocol_id ||
  productionOracles.manifest_version !== 1 ||
  productionOracles.classification !== "construction_pilot_only" ||
  productionOracles.formal_result_eligible !== false ||
  productionOracles.execution_standard !== "practical_internal_v1" ||
  productionOracles.qualification_requirement !==
    "construction_pilot_only_qualification_required" ||
  productionOracles.node_test_entry_point !==
    "node --test benchmarks/agents-md-effect/tests/operational.test.mjs"
) {
  fail("production oracle manifest has an invalid operational contract");
}
sameProductionIds(
  productionOracles.cases,
  casesDocument.cases.map((entry) => entry.id),
  "production oracle cases",
);
rejectFrozenScoringCopies(productionOracles);

for (const caseEntry of casesDocument.cases) {
  const productionCase = productionOracles.cases.find(
    (entry) => entry.id === caseEntry.id,
  );
  const oracleCase = oracleLock.cases.find((entry) => entry.id === caseEntry.id);
  const coreScoreCase = coreScoreLock.cases.find((entry) => entry.id === caseEntry.id);
  if (!productionCase || !oracleCase || !coreScoreCase) {
    fail(`${caseEntry.id} lacks a production oracle input`);
  }

  sameProductionIds(
    productionCase.core_commands,
    coreScoreCase.validation_checks.map((entry) => entry.id),
    `${caseEntry.id} production core commands`,
  );
  for (const command of productionCase.core_commands) {
    const lockedCommand = coreScoreCase.validation_checks.find(
      (entry) => entry.id === command.id,
    );
    if (
      command.command_template !== lockedCommand.command ||
      command.stage !== "candidate_core_validation"
    ) {
      fail(`${caseEntry.id} core command ${command.id} has an invalid production mapping`);
    }
  }

  sameProductionIds(
    productionCase.oracle_commands,
    oracleCase.command_specs.map((entry) => entry.id),
    `${caseEntry.id} production oracle commands`,
  );
  for (const command of productionCase.oracle_commands) {
    const lockedCommand = oracleCase.command_specs.find(
      (entry) => entry.id === command.id,
    );
    const expectedAssertionIds = oracleCase.assertions
      .filter((assertion) => assertion.command_id === command.id)
      .map((assertion) => assertion.id)
      .sort();
    if (
      command.command_template !== lockedCommand.run ||
      command.stage !== "isolated_oracle_pipeline" ||
      JSON.stringify(command.stages) !==
        JSON.stringify([
          "candidate_artifact_build",
          "trusted_probe_build",
          "expectation_free_execution",
        ]) ||
      JSON.stringify([...command.assertion_ids].sort()) !== JSON.stringify(expectedAssertionIds)
    ) {
      fail(`${caseEntry.id} oracle command ${command.id} has an invalid production mapping`);
    }
  }

  sameProductionIds(
    productionCase.assertions,
    oracleCase.assertions.map((entry) => entry.id),
    `${caseEntry.id} production assertions`,
  );
  for (const assertion of productionCase.assertions) {
    const lockedAssertion = oracleCase.assertions.find((entry) => entry.id === assertion.id);
    if (
      assertion.command_id !== lockedAssertion.command_id ||
      assertion.stage !== "expectation_free_execution"
    ) {
      fail(`${caseEntry.id} assertion ${assertion.id} has an invalid production mapping`);
    }
  }

  const subjectiveGroupIds = caseEntry.validation.machine_contract_groups
    .filter((group) => group.assessment === "blinded_semantic_review")
    .map((group) => group.id);
  sameProductionIds(
    productionCase.subjective_groups,
    subjectiveGroupIds,
    `${caseEntry.id} production subjective groups`,
  );
  if (
    !productionCase.subjective_groups.every(
      (group) => group.stage === "blinded_review_packet",
    )
  ) {
    fail(`${caseEntry.id} subjective groups do not enter blinded review`);
  }
}

if (environment.controlled_agent.multi_agent !== false) {
  fail("controlled runs must disable multi-agent execution");
}
if (environment.controlled_agent.shell_network_access !== false) {
  fail("controlled shell network access must be false");
}
if (
  environment.toolchain?.git?.version !== "2.55.0" ||
  environment.toolchain?.git?.binary_sha256 !==
    "80045299a1ae4309b716fd02f076c677d74c5dac2d5f065ca6d6364afab198ad"
) {
  fail("trusted Git binary/version is not locked");
}

const agentsBytes = await readFile(resolve(repositoryDir, "AGENTS.md"));
if (sha256(agentsBytes) !== environment.baseline_a_agents.sha256) {
  fail("current AGENTS.md differs from the frozen Baseline A bytes");
}
if (agentsBytes.length !== environment.baseline_a_agents.bytes) {
  fail("current AGENTS.md byte count differs from the Baseline A lock");
}

console.log(`verified ${casesDocument.cases.length} cases for ${casesDocument.protocol_id}`);
console.log(`manifest_sha256=${sha256(await readFile(casesPath))}`);
console.log(`oracle_lock_sha256=${sha256(oracleBytes)}`);
console.log(`core_score_lock_sha256=${sha256(coreScoreBytes)}`);
console.log(`authority_lock_sha256=${sha256(authorityBytes)}`);
console.log(`authority_snapshots_sha256=${sha256(authoritySnapshotBytes)}`);
console.log(`history_snapshots_sha256=${sha256(historyBytes)}`);
console.log(`rubric_sha256=${sha256(await readFile(rubricPath))}`);
console.log(`environment_lock_sha256=${sha256(await readFile(environmentPath))}`);
console.log(`result_schema_sha256=${sha256(await readFile(schemaPath))}`);
console.log(`review_schema_sha256=${sha256(await readFile(reviewSchemaPath))}`);
console.log(`construction_pilot_index_sha256=${sha256(constructionPilotIndexBytes)}`);
