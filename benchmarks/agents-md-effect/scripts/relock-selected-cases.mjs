#!/usr/bin/env node

// Construction-only deterministic relocking for the three cases replaced or
// rebased after the root-AGENTS history contamination audit.

import {createHash} from "node:crypto";
import {readFile, stat, writeFile} from "node:fs/promises";
import {spawnSync} from "node:child_process";
import {dirname, resolve} from "node:path";
import {fileURLToPath} from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const benchmarkDir = resolve(scriptDir, "..");
const repositoryDir = resolve(benchmarkDir, "../..");

const paths = {
  cases: resolve(benchmarkDir, "evaluator/cases.json"),
  oracle: resolve(benchmarkDir, "evaluator/oracle-lock.json"),
  core: resolve(benchmarkDir, "evaluator/core-score-lock.json"),
  authority: resolve(benchmarkDir, "evaluator/authority-lock.json"),
  authoritySnapshots: resolve(benchmarkDir, "evaluator/authority-snapshots.json"),
  history: resolve(benchmarkDir, "evaluator/history-snapshots.json"),
};

function fail(message) {
  throw new Error(message);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function gitBytes(args) {
  const result = spawnSync("rtk", ["proxy", "git", ...args], {
    cwd: repositoryDir,
    encoding: null,
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.status !== 0) {
    fail(
      `rtk proxy git ${args.join(" ")} failed: ` +
        Buffer.from(result.stderr ?? result.stdout ?? []).toString("utf8"),
    );
  }
  return Buffer.from(result.stdout);
}

function git(args) {
  return gitBytes(args).toString("utf8").trim();
}

async function readJson(path) {
  return JSON.parse(await readFile(path, "utf8"));
}

async function writeJson(path, value) {
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`);
}

function replaceById(entries, replacement) {
  const index = entries.findIndex((entry) => entry.id === replacement.id);
  if (index === -1) fail(`missing ${replacement.id}`);
  entries[index] = replacement;
}

async function taskLock(id) {
  const taskPath = resolve(benchmarkDir, `tasks/${id}.md`);
  const bytes = await readFile(taskPath);
  return {
    task_file: `tasks/${id}.md`,
    task_sha256: sha256(bytes),
    task_bytes: (await stat(taskPath)).size,
  };
}

function sourceFile(commit, path) {
  const bytes = gitBytes(["show", `${commit}:${path}`]);
  return {path, sha256: sha256(bytes)};
}

function rustCommand(id, testFile, testName) {
  return {
    id,
    run:
      `cargo test -p tachiko-storage --test ${testFile} --locked ` +
      `${testName} -- --exact`,
  };
}

function rustAssertion(id, commandId, testName, group, points = 1) {
  return {
    id,
    command_id: commandId,
    selector: {
      kind: "rust_test_exact",
      test_name: testName,
      required_matching_tests: 1,
      adapter_allowed: true,
    },
    points,
    mandatory: true,
    contract_group_ids: [group],
    pass_rule:
      "exit 0 and exactly one locked test executes and passes; zero matches or nonzero exit awards zero; infrastructure error invalidates the run",
  };
}

function sourceText(history, resource, kind, nodeId = null) {
  const match = resource.match(/^(issue|pr)#(\d+)$/);
  if (!match) fail(`bad resource ${resource}`);
  const item = history.resources.find(
    (entry) => entry.kind === match[1] && entry.number === Number(match[2]),
  );
  if (!item) fail(`missing history resource ${resource}`);
  if (kind === "body") return item.captured_resource.body;
  if (kind === "body_version") {
    const version = item.body_edit_history.find((entry) => entry.id === nodeId);
    if (!version) fail(`missing body version ${nodeId}`);
    return version.diff;
  }
  if (kind === "comment") {
    const comment = item.captured_resource.comments.find(
      (entry) => (entry.id ?? entry.node_id) === nodeId,
    );
    if (!comment) fail(`missing comment ${nodeId}`);
    return comment.body;
  }
  fail(`unsupported history source kind ${kind}`);
}

function authoritySource(history, {id, resource, kind, node_id, effective_at}) {
  return {
    id,
    snapshot: "history",
    resource,
    kind,
    ...(node_id ? {node_id} : {}),
    effective_at,
    text_sha256: sha256(Buffer.from(sourceText(history, resource, kind, node_id), "utf8")),
    temporal_integrity: kind === "body_version" ? "exact_versioned" : "exact_unedited",
  };
}

const [casesDocument, oracleLock, coreLock, authorityLock, authoritySnapshots, history] =
  await Promise.all([
    readJson(paths.cases),
    readJson(paths.oracle),
    readJson(paths.core),
    readJson(paths.authority),
    readJson(paths.authoritySnapshots),
    readJson(paths.history),
  ]);

for (const entry of casesDocument.cases) {
  entry.replay_relation ??= "direct_parent";
}

const tw06Task = await taskLock("TW-06");
const tw08Task = await taskLock("TW-08");
const tw06StructuralPath = "scripts/validate-tw06-structural.mjs";
const tw06StructuralBytes = await readFile(resolve(benchmarkDir, tw06StructuralPath));

replaceById(casesDocument.cases, {
  id: "TW-06",
  title: "Align the public pre-alpha governance posture",
  capabilities: [
    "governance",
    "repository_authority",
    "scope_control",
    "documentation",
  ],
  historical_base_commit: "caf81e116c8f48c265fec40d7d12bd23a1fa4be0",
  historical_base_tree: "ff697610b5c34d96162e5736b143d47f2494503c",
  ground_truth_commit: "64410d6198296a4359053c5d2bb0912401b08056",
  replay_relation: "direct_parent",
  references: [
    "https://github.com/nurockplayer/tachiko-work/issues/15",
    "https://github.com/nurockplayer/tachiko-work/pull/22",
  ],
  ...tw06Task,
  expected_scope: [
    "Consistent public pre-alpha status and release-access guidance",
    "Accurate provisional licensing direction with historical grants preserved",
    "Temporary external-code contribution boundary with non-code participation open",
    "Discoverable cross-document governance without closing the licensing decision",
  ],
  forbidden_scope: [
    "Relicensing or changes to package/license metadata and license texts",
    "Runtime, format, release automation, or publication-state changes",
    "Presenting MPL-2.0 or any contributor/commercial model as final",
    "Closing unrelated governance questions or excluding non-code participation",
  ],
  validation: {
    base: [
      "cargo fmt --all -- --check",
      "cargo test --workspace --all-targets --locked",
    ],
    candidate: [
      "cargo fmt --all -- --check",
      "cargo test --workspace --all-targets --locked",
      "bash scripts/release-check.sh",
    ],
    ground_truth: [
      "cargo fmt --all -- --check",
      "cargo test --workspace --all-targets --locked",
      "bash scripts/release-check.sh",
    ],
    historical_oracle_files: [],
    machine_contract_groups: [
      {
        id: "public_prealpha_consistency",
        points: 6,
        mandatory: true,
        assessment: "blinded_semantic_review",
        criterion:
          "Public development, pre-alpha instability, and release authorization remain distinct and consistent.",
      },
      {
        id: "licensing_authority",
        points: 6,
        mandatory: true,
        assessment: "blinded_semantic_review",
        criterion:
          "Historical grants and the provisional protective-first direction are accurate without performing or implying a final relicense.",
      },
      {
        id: "contribution_boundary",
        points: 6,
        mandatory: true,
        assessment: "blinded_semantic_review",
        criterion:
          "The temporary code-contribution pause is explained while issues, discussion, review, and documentation participation remain open.",
      },
      {
        id: "governance_coherence",
        points: 6,
        mandatory: true,
        assessment: "blinded_semantic_review",
        criterion:
          "Active public, contributor, and release guidance agrees and keeps unresolved policy dimensions visibly open.",
      },
    ],
  },
  blinded_review_criteria: [
    "Provisional direction is kept distinct from a legal or relicensing decision.",
    "Public visibility, contribution eligibility, and release authority are not conflated.",
    "The change is discoverable and consistent without duplicating policy or expanding scope.",
  ],
  evidence_visibility: {
    agent_visible: [
      "Historical base repository",
      "tasks/TW-06.md",
      "selected AGENTS.md overlay",
    ],
    evaluator_only: [
      "Issue and PR snapshots/URLs",
      "PR #22 diff and solution topology",
      "historical target and CI evidence",
      "rubric and known failure modes",
    ],
  },
  historical_patch: {
    changed_files: 5,
    additions: 93,
    deletions: 20,
    similarity_scored: false,
  },
  difficulty: "medium",
  time_limit_minutes: 90,
  estimated_active_runtime_minutes: [30, 70],
  oracle_mode: "blinded_authority_and_scope_review",
  score_allocation: {machine: 19, blinded_review: 81},
  known_failure_modes: [
    {
      description: "A future license candidate is presented as a completed relicense",
      visibility: "prompt_explicit",
      provenance: "direct inverse of the frozen task and pre-work founder direction",
    },
    {
      description: "Previously published Apache/MIT grants are described as revocable",
      visibility: "prompt_explicit",
      provenance: "direct inverse of the frozen task and pre-work founder direction",
    },
    {
      description: "Stale private-repository or stable-release guidance remains active",
      visibility: "prompt_explicit",
      provenance: "direct inverse of the frozen task requirement",
    },
    {
      description: "The temporary code-contribution pause is broadened to non-code participation",
      visibility: "prompt_explicit",
      provenance: "direct inverse of the frozen task requirement",
    },
    {
      description: "Runtime, license files, package metadata, or release automation changes",
      visibility: "prompt_explicit",
      provenance: "direct inverse of the frozen forbidden scope",
    },
  ],
});

replaceById(casesDocument.cases, {
  id: "TW-08",
  title: "Implement the accepted legacy direct-RO v1 persistence boundary",
  capabilities: [
    "feature",
    "storage",
    "compatibility",
    "canonicalization",
    "architecture_authority",
  ],
  historical_base_commit: "c8528409dd327a9854ac030247ecbd8fcf765db7",
  historical_base_tree: "51f2d77c5200ace592f9a9fea50d0f33bb6c4fa8",
  ground_truth_commit: "1929dd758ed580f0ccd2bc70be11560f3e88b0da",
  replay_relation: "direct_parent",
  references: [
    "https://github.com/nurockplayer/tachiko-work/issues/74",
    "https://github.com/nurockplayer/tachiko-work/pull/80",
  ],
  ...tw08Task,
  expected_scope: [
    "Storage-owned complete legacy direct-RO/v1 DTO boundary",
    "Strict UTF-8, JSON, duplicate-member, and version-first reader pipeline",
    "Closed-world v1 conversion and exact historical canonical bytes",
    "Duplicate-safe migration-facing v1 seam and focused conformance evidence",
  ],
  forbidden_scope: [
    "A new representation version, permanent migration IR, or semantic model",
    "Numeric-policy changes or a claim of broad JCS compliance",
    "Automatic upgrade on read or interpretation of unsupported future bodies",
    ".roproj, package/container, diagnostics, runtime, plugin, collaboration, or cloud decisions",
  ],
  validation: {
    base: [
      "cargo test -p tachiko-storage --locked",
      "cargo test --workspace --all-targets --locked",
    ],
    candidate: [
      "cargo test -p tachiko-storage --locked",
      "cargo test --workspace --all-targets --locked",
      "bash scripts/release-check.sh",
    ],
    ground_truth: [
      "cargo test -p tachiko-storage --test strict_decoding --locked",
      "cargo test -p tachiko-storage --test canonical_v1 --locked",
      "cargo test -p tachiko-storage --locked",
      "cargo test --workspace --all-targets --locked",
      "bash scripts/release-check.sh",
    ],
    historical_oracle_files: [
      "crates/storage/tests/strict_decoding.rs",
      "crates/storage/tests/canonical_v1.rs",
      "crates/storage/tests/fixtures/all-v1-shapes.ro",
    ],
    machine_contract_groups: [
      {
        id: "strict_frontend_dispatch",
        points: 6,
        mandatory: true,
        assessment: "machine_with_blinded_adapter",
        criterion:
          "The byte reader distinguishes UTF-8/JSON failures, rejects decoded-name duplicates recursively, and classifies the version envelope before body decoding.",
      },
      {
        id: "closed_world_conversion",
        points: 6,
        mandatory: true,
        assessment: "machine_with_blinded_adapter",
        criterion:
          "Unsupported bodies remain uninterpreted and recognized v1 rejects recursive unknowns and incoherent representation relationships before semantic publication.",
      },
      {
        id: "canonical_compatibility",
        points: 6,
        mandatory: true,
        assessment: "machine_with_blinded_adapter",
        criterion:
          "Legacy v1 canonical bytes, discriminator coverage, ordering, escaping, Unicode preservation, and checked-in compatibility remain exact.",
      },
      {
        id: "migration_seam_and_scope",
        points: 6,
        mandatory: true,
        assessment: "blinded_semantic_review",
        criterion:
          "The migration-facing storage-owned seam shares the strict pipeline without duplicating semantic truth or deciding excluded representations/policies.",
      },
    ],
  },
  blinded_review_criteria: [
    "Durable v1 types are storage-owned rather than inherited from semantic Serde layout.",
    "One strict byte/version gate owns both public reading and migration-facing DTO access.",
    "The implementation freezes compatibility without importing unresolved or later contracts.",
  ],
  evidence_visibility: {
    agent_visible: [
      "Historical base with accepted ADR-0017 and storage specifications",
      "tasks/TW-08.md",
      "selected AGENTS.md overlay",
    ],
    evaluator_only: [
      "Issue completion material",
      "PR #80 implementation/diff and post-implementation review",
      "historical oracle tests and fixture",
      "rubric and hidden failure modes",
    ],
  },
  historical_patch: {
    changed_files: 11,
    additions: 2398,
    deletions: 79,
    similarity_scored: false,
  },
  difficulty: "very_hard",
  time_limit_minutes: 300,
  estimated_active_runtime_minutes: [180, 280],
  oracle_mode: "historical_black_box_oracle_plus_blinded_semantic_review",
  score_allocation: {machine: 37, blinded_review: 63},
  known_failure_modes: [
    {
      description: "The migration-facing DTO seam bypasses strict duplicate inspection",
      visibility: "evaluator_hidden",
      provenance: "post-implementation PR review and red-before-fix regression",
    },
    {
      description: "Nested or escaped-equivalent duplicate members collapse last-wins",
      visibility: "prompt_explicit",
      provenance: "direct inverse of the frozen task requirement",
    },
    {
      description: "Unsupported future bodies are decoded using v1 semantics",
      visibility: "prompt_explicit",
      provenance: "direct inverse of the frozen task requirement",
    },
    {
      description: "Semantic-core serialization types define the durable v1 decoder",
      visibility: "prompt_explicit",
      provenance: "direct inverse of the frozen task requirement",
    },
    {
      description: "Canonical output depends on insertion or mutable-name order",
      visibility: "prompt_explicit",
      provenance: "direct inverse of the frozen task requirement",
    },
    {
      description: "Later numeric or representation policy is retrofitted into legacy v1",
      visibility: "prompt_explicit",
      provenance: "direct inverse of the frozen task and forbidden scope",
    },
  ],
});

const tw09 = casesDocument.cases.find((entry) => entry.id === "TW-09");
tw09.historical_base_commit = "77821143e9847f62e129e553522556743c5032c1";
tw09.historical_base_tree = "db03a53542043581ab26e7f7dba8ba29f7194649";
tw09.replay_relation = "ancestor_with_independent_intervening_changes";
tw09.implementation_parent_commit = "c685fe72a126c6de26089461923991447c70ad8f";
tw09.intervening_outcome_only_commits = [
  "22fc8eb9d84c5bc13a7c9c64c6cb1f235974e5ba",
  "b5b097cde26952b60580f62b28dada10044d92dd",
  "c685fe72a126c6de26089461923991447c70ad8f",
];
tw09.evidence_visibility.agent_visible[0] =
  "Historical base including the completed formula prerequisite; the independent later storage work is absent";
tw09.oracle_mode = "rebased_behavioral_oracle_with_case_local_portable_contract";
tw09.historical_patch.basis =
  "implementation_parent_commit_to_ground_truth_commit";
tw09.replay_base_to_ground_truth_diff = {
  changed_files: 41,
  additions: 4883,
  deletions: 600,
  similarity_scored: false,
};

replaceById(oracleLock.cases, {
  id: "TW-06",
  source_commit: "64410d6198296a4359053c5d2bb0912401b08056",
  mode: "blinded_authority_and_scope_review",
  files: [],
  constructed_contracts: [
    {
      id: "TW-06-structural-scope-v1",
      path: tw06StructuralPath,
      sha256: sha256(tw06StructuralBytes),
    },
  ],
  commands: [
    "cargo fmt --all -- --check",
    "cargo test --workspace --all-targets --locked",
    "bash scripts/release-check.sh",
    "node <controller>/scripts/validate-tw06-structural.mjs --candidate-root <validation-workspace> --base caf81e116c8f48c265fec40d7d12bd23a1fa4be0 --candidate <candidate-commit>",
  ],
  group_mappings: [
    ["public_prealpha_consistency", "public/pre-alpha/release-state consistency"],
    ["licensing_authority", "provisional versus final licensing authority"],
    ["contribution_boundary", "code versus non-code contribution boundary"],
    ["governance_coherence", "cross-document coherence and discoverability"],
  ].map(([id, anchor]) => ({
    id,
    assessment: "blinded_semantic_review",
    assertion_ids: [],
    review_anchors: [anchor, "pre-work Issue #15 authority"],
  })),
  note:
    "Do not keyword-score governance prose or require the historical file topology. The target PR body and implementation remain outcome-only. tw06.structural_scope is zero-point candidate scope/link evidence; a nonzero exit is not an infrastructure invalidation.",
  command_specs: [
    {id: "tw06.fmt", run: "cargo fmt --all -- --check"},
    {id: "tw06.workspace", run: "cargo test --workspace --all-targets --locked"},
    {id: "tw06.release", run: "bash scripts/release-check.sh"},
    {
      id: "tw06.structural_scope",
      run: "node <controller>/scripts/validate-tw06-structural.mjs --candidate-root <validation-workspace> --base caf81e116c8f48c265fec40d7d12bd23a1fa4be0 --candidate <candidate-commit>",
    },
  ],
  assertions: [],
  integrity_gates: [
    "TW-06 structural contract byte hash",
  ],
  unscored_gates: [
    "tw06.fmt",
    "tw06.workspace",
    "tw06.release",
    "tw06.structural_scope",
  ],
});

const tw08Specs = [
  rustCommand("tw08.invalid_utf8", "strict_decoding", "invalid_utf8_is_distinct_from_invalid_json"),
  rustCommand("tw08.invalid_json", "strict_decoding", "invalid_json_is_reported_before_version_dispatch"),
  rustCommand("tw08.duplicates", "strict_decoding", "duplicate_members_are_rejected_at_every_depth_after_escape_decoding"),
  rustCommand("tw08.missing_version", "strict_decoding", "missing_version_is_distinct"),
  rustCommand("tw08.malformed_versions", "strict_decoding", "malformed_versions_are_distinct_from_missing_and_unsupported_versions"),
  rustCommand("tw08.unsupported_precedence", "strict_decoding", "unsupported_version_wins_before_v1_body_interpretation"),
  rustCommand("tw08.future_number", "strict_decoding", "unsupported_version_does_not_apply_v1_number_limits_to_the_future_body"),
  rustCommand("tw08.unknown_recursive", "strict_decoding", "supported_v1_rejects_unknown_members_recursively"),
  rustCommand("tw08.null_schema", "strict_decoding", "v1_basic_field_types_reject_a_present_null_schema_member"),
  rustCommand("tw08.map_id", "strict_decoding", "v1_rejects_schema_and_entity_map_key_id_mismatches"),
  rustCommand("tw08.relationships", "strict_decoding", "v1_rejects_unresolvable_schema_and_field_relationships"),
  rustCommand("tw08.minimal_bytes", "canonical_v1", "canonical_minimal_v1_has_exact_specified_bytes"),
  rustCommand("tw08.shapes_roundtrip", "canonical_v1", "every_v1_field_value_and_expression_discriminator_round_trips"),
  rustCommand("tw08.shapes_bytes", "canonical_v1", "every_v1_shape_has_exact_canonical_bytes"),
  rustCommand("tw08.unicode", "canonical_v1", "canonical_v1_preserves_unicode_scalar_sequences_without_normalization"),
  rustCommand("tw08.map_order", "canonical_v1", "every_legacy_id_map_uses_ascii_lexicographic_order"),
  rustCommand("tw08.examples", "canonical_v1", "checked_in_ro_examples_are_canonical_and_byte_stable"),
];
const tw08Assertions = [
  rustAssertion("tw-08.invalid-utf8", "tw08.invalid_utf8", "invalid_utf8_is_distinct_from_invalid_json", "strict_frontend_dispatch"),
  rustAssertion("tw-08.invalid-json", "tw08.invalid_json", "invalid_json_is_reported_before_version_dispatch", "strict_frontend_dispatch"),
  rustAssertion("tw-08.duplicates", "tw08.duplicates", "duplicate_members_are_rejected_at_every_depth_after_escape_decoding", "strict_frontend_dispatch", 2),
  rustAssertion("tw-08.missing-version", "tw08.missing_version", "missing_version_is_distinct", "strict_frontend_dispatch"),
  rustAssertion("tw-08.malformed-version", "tw08.malformed_versions", "malformed_versions_are_distinct_from_missing_and_unsupported_versions", "strict_frontend_dispatch"),
  rustAssertion("tw-08.unsupported-precedence", "tw08.unsupported_precedence", "unsupported_version_wins_before_v1_body_interpretation", "closed_world_conversion"),
  rustAssertion("tw-08.future-number", "tw08.future_number", "unsupported_version_does_not_apply_v1_number_limits_to_the_future_body", "closed_world_conversion"),
  rustAssertion("tw-08.unknown-recursive", "tw08.unknown_recursive", "supported_v1_rejects_unknown_members_recursively", "closed_world_conversion"),
  rustAssertion("tw-08.null-schema", "tw08.null_schema", "v1_basic_field_types_reject_a_present_null_schema_member", "closed_world_conversion"),
  rustAssertion("tw-08.map-id", "tw08.map_id", "v1_rejects_schema_and_entity_map_key_id_mismatches", "closed_world_conversion"),
  rustAssertion("tw-08.relationships", "tw08.relationships", "v1_rejects_unresolvable_schema_and_field_relationships", "closed_world_conversion"),
  rustAssertion("tw-08.minimal-bytes", "tw08.minimal_bytes", "canonical_minimal_v1_has_exact_specified_bytes", "canonical_compatibility"),
  rustAssertion("tw-08.shapes-roundtrip", "tw08.shapes_roundtrip", "every_v1_field_value_and_expression_discriminator_round_trips", "canonical_compatibility"),
  rustAssertion("tw-08.shapes-bytes", "tw08.shapes_bytes", "every_v1_shape_has_exact_canonical_bytes", "canonical_compatibility"),
  rustAssertion("tw-08.unicode", "tw08.unicode", "canonical_v1_preserves_unicode_scalar_sequences_without_normalization", "canonical_compatibility"),
  rustAssertion("tw-08.map-order", "tw08.map_order", "every_legacy_id_map_uses_ascii_lexicographic_order", "canonical_compatibility"),
  rustAssertion("tw-08.examples", "tw08.examples", "checked_in_ro_examples_are_canonical_and_byte_stable", "canonical_compatibility"),
];
replaceById(oracleLock.cases, {
  id: "TW-08",
  source_commit: "1929dd758ed580f0ccd2bc70be11560f3e88b0da",
  mode: "historical_black_box_oracle_plus_normalized_blinded_adapter",
  files: [
    sourceFile("1929dd758ed580f0ccd2bc70be11560f3e88b0da", "crates/storage/tests/strict_decoding.rs"),
    sourceFile("1929dd758ed580f0ccd2bc70be11560f3e88b0da", "crates/storage/tests/canonical_v1.rs"),
    sourceFile("1929dd758ed580f0ccd2bc70be11560f3e88b0da", "crates/storage/tests/fixtures/all-v1-shapes.ro"),
  ],
  commands: tw08Specs.map((entry) => entry.run),
  group_mappings: [
    {
      id: "strict_frontend_dispatch",
      assessment: "machine_with_blinded_adapter",
      assertion_ids: tw08Assertions
        .filter((entry) => entry.contract_group_ids[0] === "strict_frontend_dispatch")
        .map((entry) => entry.id),
    },
    {
      id: "closed_world_conversion",
      assessment: "machine_with_blinded_adapter",
      assertion_ids: tw08Assertions
        .filter((entry) => entry.contract_group_ids[0] === "closed_world_conversion")
        .map((entry) => entry.id),
    },
    {
      id: "canonical_compatibility",
      assessment: "machine_with_blinded_adapter",
      assertion_ids: tw08Assertions
        .filter((entry) => entry.contract_group_ids[0] === "canonical_compatibility")
        .map((entry) => entry.id),
    },
    {
      id: "migration_seam_and_scope",
      assessment: "blinded_semantic_review",
      assertion_ids: [],
      review_anchors: [
        "one strict migration-facing DTO seam",
        "storage-owned durable boundary",
        "forbidden-scope preservation",
      ],
    },
  ],
  command_specs: tw08Specs,
  assertions: tw08Assertions,
  integrity_gates: ["historical oracle hashes", "adapter behavior_implemented_by_adapter=false"],
  unscored_gates: [],
});

const tw09Oracle = oracleLock.cases.find((entry) => entry.id === "TW-09");
tw09Oracle.mode = "rebased_behavioral_oracle_plus_case_local_portable_contract";
tw09Oracle.files = [
  sourceFile(
    "156565a3d2dc7664088a24b7f6e38d02ad4e04fe",
    "crates/workspace-engine/tests/validation_report.rs",
  ),
];
const tw09PortableContractPath =
  "evaluator/contracts/TW-09-portable-validation-observations.json";
const tw09PortableContractBytes = await readFile(resolve(benchmarkDir, tw09PortableContractPath));
tw09Oracle.constructed_contracts = [
  ...(tw09Oracle.constructed_contracts ?? []).filter(
    (entry) => entry.id !== "TW-09-portable-validation-observations-v1",
  ),
  {
    id: "TW-09-portable-validation-observations-v1",
    path: tw09PortableContractPath,
    sha256: sha256(tw09PortableContractBytes),
  },
];
const tw09Portable = tw09Oracle.assertions.find(
  (entry) =>
    entry.id === "tw-09.portable.records-42-45" ||
    entry.id === "tw-09.portable.records-27-30",
);
if (!tw09Portable) fail("missing TW-09 portable assertion");
tw09Portable.id = "tw-09.portable.records-27-30";
tw09Portable.selector.indexes = [27, 28, 29, 30];
tw09Portable.selector.expected_records = tw09Portable.selector.expected_records.map(
  (record, index) => ({...record, index: 27 + index}),
);
tw09Oracle.group_mappings
  .find((entry) => entry.id === "agreement_portability")
  .assertion_ids = tw09Oracle.group_mappings
  .find((entry) => entry.id === "agreement_portability")
  .assertion_ids.map((id) =>
    id === "tw-09.portable.records-42-45" ? "tw-09.portable.records-27-30" : id,
  );
tw09Oracle.integrity_gates = [
  ...(tw09Oracle.integrity_gates ?? []).filter(
    (gate) =>
      gate !==
      "TW-09 case-local portable contract excludes independent intervening storage records",
  ),
  "TW-09 case-local portable contract excludes independent intervening storage records",
];

replaceById(coreLock.cases, {
  id: "TW-06",
  validation_checks: [
    {id: "core.tw06.fmt", dimension: "validation_tests", points: 3, command: "cargo fmt --all -- --check"},
    {id: "core.tw06.workspace", dimension: "validation_tests", points: 4, command: "cargo test --workspace --all-targets --locked"},
    {id: "core.tw06.release", dimension: "validation_tests", points: 3, command: "bash scripts/release-check.sh"},
  ],
});
replaceById(coreLock.cases, {
  id: "TW-08",
  validation_checks: [
    {id: "core.tw08.storage", dimension: "validation_tests", points: 4, command: "cargo test -p tachiko-storage --locked"},
    {id: "core.tw08.workspace", dimension: "validation_tests", points: 2, command: "cargo test --workspace --all-targets --locked"},
    {id: "core.tw08.release", dimension: "validation_tests", points: 4, command: "bash scripts/release-check.sh"},
  ],
});

replaceById(authorityLock.cases, {
  id: "TW-06",
  base_commit: "caf81e116c8f48c265fec40d7d12bd23a1fa4be0",
  assignment_cutoff: "2026-08-20T20:23:23Z",
  task_authority: [
    authoritySource(history, {
      id: "tw06.issue15.body",
      resource: "issue#15",
      kind: "body",
      effective_at: "2026-08-20T18:32:54Z",
    }),
    authoritySource(history, {
      id: "tw06.issue15.implementation_evidence",
      resource: "issue#15",
      kind: "comment",
      node_id: "IC_kwDOT-Sx7c8AAAABP4YjEQ",
      effective_at: "2026-08-20T19:31:49Z",
    }),
    authoritySource(history, {
      id: "tw06.issue15.founder_direction",
      resource: "issue#15",
      kind: "comment",
      node_id: "IC_kwDOT-Sx7c8AAAABP44UqA",
      effective_at: "2026-08-20T20:23:23Z",
    }),
  ],
  claims: [
    {
      id: "public_and_licensing_posture",
      contract_group_ids: [
        "public_prealpha_consistency",
        "licensing_authority",
        "contribution_boundary",
        "governance_coherence",
      ],
      authority_source_ids: [
        "tw06.issue15.body",
        "tw06.issue15.implementation_evidence",
        "tw06.issue15.founder_direction",
      ],
    },
    {
      id: "scope_and_exclusions",
      contract_group_ids: [],
      authority_source_ids: [
        "tw06.issue15.body",
        "tw06.issue15.founder_direction",
      ],
    },
  ],
  outcome_only: [
    "git:64410d6198296a4359053c5d2bb0912401b08056",
    "https://github.com/nurockplayer/tachiko-work/pull/22",
  ],
  excluded: [
    {
      source: "PR #22 body/diff/files/commits/comments and CodeRabbit configuration outcome",
      reason: "created during or after implementation; not task authority and not exposed to the agent",
    },
  ],
});

replaceById(authorityLock.cases, {
  id: "TW-08",
  base_commit: "c8528409dd327a9854ac030247ecbd8fcf765db7",
  assignment_cutoff: "2026-08-22T05:34:23Z",
  task_authority: [
    authoritySource(history, {
      id: "tw08.issue74.body",
      resource: "issue#74",
      kind: "body",
      effective_at: "2026-08-22T03:44:56Z",
    }),
    authoritySource(history, {
      id: "tw08.issue74.handoff",
      resource: "issue#74",
      kind: "comment",
      node_id: "IC_kwDOT-Sx7c8AAAABQIl0Kg",
      effective_at: "2026-08-22T03:53:02Z",
    }),
  ],
  claims: [
    {
      id: "legacy_v1_storage_boundary",
      contract_group_ids: [
        "strict_frontend_dispatch",
        "closed_world_conversion",
        "canonical_compatibility",
        "migration_seam_and_scope",
      ],
      authority_source_ids: ["tw08.issue74.body", "tw08.issue74.handoff"],
    },
    {
      id: "scope_and_exclusions",
      contract_group_ids: [],
      authority_source_ids: ["tw08.issue74.body", "tw08.issue74.handoff"],
    },
  ],
  outcome_only: [
    "git:1929dd758ed580f0ccd2bc70be11560f3e88b0da",
    "https://github.com/nurockplayer/tachiko-work/pull/80",
    "authority-review:PRRC_kwDOT-Sx7c7kmiFB",
    "authority-review:PRRC_kwDOT-Sx7c7km1sH",
  ],
  excluded: [
    {
      source: "PR #80 body, diff, review, resolution, and completion comments",
      reason: "historical outcome and evaluator-only evidence created after implementation began",
    },
  ],
});

const tw09Authority = authorityLock.cases.find((entry) => entry.id === "TW-09");
tw09Authority.base_commit = "77821143e9847f62e129e553522556743c5032c1";
tw09Authority.assignment_cutoff = "2026-08-23T18:02:36Z";
tw09Authority.outcome_only = [
  ...new Set([
    ...tw09Authority.outcome_only,
    "git:22fc8eb9d84c5bc13a7c9c64c6cb1f235974e5ba",
    "git:b5b097cde26952b60580f62b28dada10044d92dd",
    "git:c685fe72a126c6de26089461923991447c70ad8f",
  ]),
];
tw09Authority.excluded = [
  ...tw09Authority.excluded.filter(
    (entry) =>
      entry.source !== "intervening commits 22fc8eb, b5b097c, and c685fe7",
  ),
  {
    source: "intervening commits 22fc8eb, b5b097c, and c685fe7",
    reason:
      "integration history after the replay base; independent AGENTS/storage work is evaluator-only and absent from the agent checkout",
  },
];

const reviewResources = [
  {
    kind: "pull_request_review_comment",
    pull_request: 80,
    node_id: "PRRC_kwDOT-Sx7c7kmiFB",
    database_id: 3835306305,
    created_at: "2026-08-22T05:46:59Z",
    updated_at: "2026-08-22T05:46:59Z",
    html_url: "https://github.com/nurockplayer/tachiko-work/pull/80#discussion_r3835306305",
    body:
      "**P1: Make the migration DTO seam duplicate-safe.** `decode_dto` is `pub(crate)` and explicitly documented as the entry point future migrations will consume, but it calls `serde_json::from_str` directly. Duplicate members in map-shaped stores such as `schemas`, `entities`, or `fields` can therefore collapse before `DocumentV1::validate` sees them. The current public `from_bytes` path runs `strict_json::inspect` first, but #70 cannot obtain the raw `DocumentV1` through that safe path. Expose a version-dispatched DTO decode that requires or runs the strict UTF-8, duplicate, and version gate, then keep this raw deserializer private to that validated path, or require a validated-input marker. Add a regression using duplicate schema/entity/field map keys through the migration-facing API.",
  },
  {
    kind: "pull_request_review_comment",
    pull_request: 80,
    node_id: "PRRC_kwDOT-Sx7c7km1sH",
    database_id: 3835386631,
    created_at: "2026-08-22T06:30:46Z",
    updated_at: "2026-08-22T06:30:46Z",
    html_url: "https://github.com/nurockplayer/tachiko-work/pull/80#discussion_r3835386631",
    body:
      "Fixed in d202be7. The crate-visible raw decode_dto(&str) seam is removed. decode_v1_dto_for_migration(&[u8]) is now the sole DTO-returning path and performs UTF-8 validation, strict JSON and recursive duplicate inspection, format-version probe/dispatch, v1 Serde decoding, and DTO validation. from_bytes consumes the same seam, so parser logic is not duplicated.\n\nThe regression was observed red before the fix: duplicate schema keys, entity keys, schema-field keys, entity-field keys, and escaped-equivalent schema keys all collapsed last-wins. The migration-facing test now requires DuplicateMember for every case. Fresh results: 38 storage tests, 171 workspace tests, warning-denied Clippy, and the full release check passed. Focused standards/spec re-reviews reported no findings.",
  },
].map((entry) => ({...entry, body_sha256: sha256(Buffer.from(entry.body, "utf8"))}));
for (const resource of reviewResources) {
  const index = authoritySnapshots.resources.findIndex(
    (entry) => entry.node_id === resource.node_id,
  );
  if (index === -1) authoritySnapshots.resources.push(resource);
  else authoritySnapshots.resources[index] = resource;
}
authoritySnapshots.captured_at = history.captured_at;
authoritySnapshots.capture_command =
  "rtk gh api repos/nurockplayer/tachiko-work/pulls/{73,80}/comments --paginate (construction captures; exact resources frozen below)";

// The common offline-authority notice is part of every frozen task statement,
// so refresh all task byte locks even though only three historical cases are
// otherwise reconstructed by this script.
const refreshedTasks = {};
for (const entry of casesDocument.cases) {
  const lock = await taskLock(entry.id);
  Object.assign(entry, lock);
  refreshedTasks[entry.id] = lock;
}

await Promise.all([
  writeJson(paths.cases, casesDocument),
  writeJson(paths.oracle, oracleLock),
  writeJson(paths.core, coreLock),
  writeJson(paths.authority, authorityLock),
  writeJson(paths.authoritySnapshots, authoritySnapshots),
]);

console.log(
  JSON.stringify({
    relocked: ["TW-06", "TW-08", "TW-09"],
    tasks: refreshedTasks,
    tw09_portable_contract_sha256: sha256(tw09PortableContractBytes),
  }),
);
