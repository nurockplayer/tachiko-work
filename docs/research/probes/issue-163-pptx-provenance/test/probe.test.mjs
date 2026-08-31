import assert from "node:assert/strict";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";
import JSZip from "jszip";

const PACKAGE_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const REPOSITORY_ROOT = resolve(PACKAGE_ROOT, "../../../..");
const FIXTURE_ROOT = join(
  REPOSITORY_ROOT,
  "docs/research/fixtures/issue-163-pptx-provenance",
);
const PROBE = join(PACKAGE_ROOT, "issue-163-pptx-provenance.mjs");

test("metric change identifies only metric-dependent adapter fragments", async () => {
  const output = await mkdtemp(join(tmpdir(), "tachiko-issue-163-test-"));
  try {
    const result = spawnSync(
      process.execPath,
      [PROBE, "--fixture-root", FIXTURE_ROOT, "--output", output],
      { encoding: "utf8" },
    );
    assert.equal(result.status, 0, result.stderr || result.stdout);

    const summary = JSON.parse(
      await readFile(join(output, "experiment-summary.json"), "utf8"),
    );
    assert.deepEqual(summary.cases.metric_change.changed_source_ids, [
      "semantic:field:89f0fd5e-dfc9-53bf-b008-85f78c403420",
    ]);
    assert.deepEqual(summary.cases.metric_change.stale_adapter_elements, [
      "adapter:claim-text",
      "adapter:metric-value",
    ]);
    assert.equal(summary.cases.metric_change.change_class, "source-semantic");
  } finally {
    await rm(output, { recursive: true, force: true });
  }
});

test("claim/citation and unrelated changes stay inside their dependency boundaries", async () => {
  const output = await mkdtemp(join(tmpdir(), "tachiko-issue-163-test-"));
  try {
    const result = spawnSync(
      process.execPath,
      [PROBE, "--fixture-root", FIXTURE_ROOT, "--output", output],
      { encoding: "utf8" },
    );
    assert.equal(result.status, 0, result.stderr || result.stdout);

    const summary = JSON.parse(
      await readFile(join(output, "experiment-summary.json"), "utf8"),
    );
    assert.deepEqual(summary.cases.claim_citation_change.changed_source_ids, [
      "fixture:claim:iron-sword-threshold",
      "fixture:evidence:moonfall-readme",
    ]);
    assert.deepEqual(
      summary.cases.claim_citation_change.stale_adapter_elements,
      ["adapter:citation-footer", "adapter:claim-text"],
    );
    assert.deepEqual(summary.cases.unrelated_change.changed_source_ids, [
      "fixture:unrelated:localization-note",
    ]);
    assert.deepEqual(summary.cases.unrelated_change.stale_adapter_elements, []);
  } finally {
    await rm(output, { recursive: true, force: true });
  }
});

test("renderer-only layout change preserves semantic and provenance identity", async () => {
  const output = await mkdtemp(join(tmpdir(), "tachiko-issue-163-test-"));
  try {
    const result = spawnSync(
      process.execPath,
      [PROBE, "--fixture-root", FIXTURE_ROOT, "--output", output],
      { encoding: "utf8" },
    );
    assert.equal(result.status, 0, result.stderr || result.stdout);

    const summary = JSON.parse(
      await readFile(join(output, "experiment-summary.json"), "utf8"),
    );
    assert.deepEqual(summary.cases.renderer_layout_change.changed_source_ids, []);
    assert.deepEqual(summary.cases.renderer_layout_change.stale_adapter_elements, []);
    assert.equal(
      summary.cases.renderer_layout_change.semantic_projection_fingerprint_unchanged,
      true,
    );
    assert.equal(
      summary.cases.renderer_layout_change.source_provenance_fingerprint_unchanged,
      true,
    );
    assert.equal(
      summary.cases.renderer_layout_change.renderer_configuration_fingerprint_changed,
      true,
    );
    assert.equal(summary.cases.renderer_layout_change.artifact_hash_changed, true);

    const baseManifest = JSON.parse(
      await readFile(join(output, "base/projection-manifest.json"), "utf8"),
    );
    const layoutManifest = JSON.parse(
      await readFile(
        join(output, "renderer-layout-change/projection-manifest.json"),
        "utf8",
      ),
    );
    assert.equal(
      baseManifest.semantic_projection_fingerprint,
      layoutManifest.semantic_projection_fingerprint,
    );
    assert.deepEqual(
      baseManifest.source_fingerprints,
      layoutManifest.source_fingerprints,
    );
    assert.equal(
      baseManifest.source_provenance_fingerprint,
      layoutManifest.source_provenance_fingerprint,
    );
    assert.notEqual(
      baseManifest.generator.configuration_sha256,
      layoutManifest.generator.configuration_sha256,
    );
  } finally {
    await rm(output, { recursive: true, force: true });
  }
});

test("PPTX target limitation is explicit in the manifest and rendered deck", async () => {
  const output = await mkdtemp(join(tmpdir(), "tachiko-issue-163-test-"));
  try {
    const result = spawnSync(
      process.execPath,
      [PROBE, "--fixture-root", FIXTURE_ROOT, "--output", output],
      { encoding: "utf8" },
    );
    assert.equal(result.status, 0, result.stderr || result.stdout);

    const summary = JSON.parse(
      await readFile(join(output, "experiment-summary.json"), "utf8"),
    );
    assert.equal(summary.cases.target_limitation.fidelity, "degraded");
    assert.equal(summary.cases.target_limitation.semantic_promotion, "none");

    const manifest = JSON.parse(
      await readFile(join(output, "base/projection-manifest.json"), "utf8"),
    );
    assert.deepEqual(manifest.fidelity_losses, [
      {
        intended_behavior_id: "adapter-behavior:live-evidence-hover",
        target: "PPTX",
        status: "degraded",
        realized_as: "static visible citation plus speaker-note source block",
        lost_behavior:
          "no live per-fragment evidence inspection or query refresh inside the PPTX",
        semantic_promotion: "none",
      },
    ]);

    const deck = await JSZip.loadAsync(
      await readFile(join(output, "base/moonfall-review.pptx")),
    );
    const slideTwo = await deck.file("ppt/slides/slide2.xml").async("string");
    assert.match(slideTwo, /PPTX freezes live evidence into a static snapshot/u);
    const sourceNotes = await deck
      .file("ppt/notesSlides/notesSlide1.xml")
      .async("string");
    assert.match(sourceNotes, /examples\/game-balance\/game-balance\.ro/u);
    assert.match(
      sourceNotes,
      /db97ec88962bbbaa66cf042a90aa407f3b165ef6/u,
    );
    assert.match(sourceNotes, /examples\/game-balance\/README\.md/u);
  } finally {
    await rm(output, { recursive: true, force: true });
  }
});

test("manual target edit is non-authoritative and disappears on regeneration", async () => {
  const output = await mkdtemp(join(tmpdir(), "tachiko-issue-163-test-"));
  try {
    const result = spawnSync(
      process.execPath,
      [PROBE, "--fixture-root", FIXTURE_ROOT, "--output", output],
      { encoding: "utf8" },
    );
    assert.equal(result.status, 0, result.stderr || result.stdout);

    const summary = JSON.parse(
      await readFile(join(output, "experiment-summary.json"), "utf8"),
    );
    assert.equal(
      summary.cases.manual_target_edit.contract,
      "target-edit-non-authoritative-overwritten-on-regeneration",
    );
    assert.equal(summary.cases.manual_target_edit.round_trip_supported, false);
    assert.equal(
      summary.cases.manual_target_edit.manual_target_value_removed,
      true,
    );
    assert.notEqual(
      summary.cases.manual_target_edit.original_artifact_sha256,
      summary.cases.manual_target_edit.manually_edited_artifact_sha256,
    );

    const manualDeck = await JSZip.loadAsync(
      await readFile(join(output, "manual-target-edit/manually-edited.pptx")),
    );
    const regeneratedDeck = await JSZip.loadAsync(
      await readFile(join(output, "manual-target-edit/regenerated.pptx")),
    );
    const manualSlide = await manualDeck
      .file("ppt/slides/slide1.xml")
      .async("string");
    const regeneratedSlide = await regeneratedDeck
      .file("ppt/slides/slide1.xml")
      .async("string");
    assert.match(manualSlide, />41 DPS</u);
    assert.doesNotMatch(regeneratedSlide, />41 DPS</u);
    assert.match(regeneratedSlide, />40 DPS</u);

    const manifest = JSON.parse(
      await readFile(join(output, "base/projection-manifest.json"), "utf8"),
    );
    assert.deepEqual(manifest.manual_target_edit_contract, {
      authoritative: false,
      regeneration: "overwrites target-only edits",
      round_trip_or_conflict_protocol: false,
    });
  } finally {
    await rm(output, { recursive: true, force: true });
  }
});

test("repeatability and provenance evidence are measured without a PPTX byte contract", async () => {
  const output = await mkdtemp(join(tmpdir(), "tachiko-issue-163-test-"));
  try {
    const result = spawnSync(
      process.execPath,
      [PROBE, "--fixture-root", FIXTURE_ROOT, "--output", output],
      { encoding: "utf8" },
    );
    assert.equal(result.status, 0, result.stderr || result.stdout);

    const summary = JSON.parse(
      await readFile(join(output, "experiment-summary.json"), "utf8"),
    );
    assert.deepEqual(summary.manual_workflow, {
      baseline_copy_or_update_steps: 3,
      generated_copy_or_update_steps: 0,
      manual_steps_eliminated: 3,
    });
    assert.deepEqual(summary.provenance_quality, {
      projected_fragments: 3,
      fragments_with_source_ids_and_fingerprints: 3,
      citation_fragment_preserved: true,
      repository_revision_preserved: true,
    });
    assert.equal(summary.repeatability.semantic_projection_equal, true);
    assert.equal(summary.repeatability.source_provenance_equal, true);
    assert.equal(summary.repeatability.pptx_byte_identity_required, false);
    assert.equal(
      summary.repeatability.pptx_bytes_equal_observation_scope,
      "paired-generations-same-invocation",
    );
    assert.equal(typeof summary.repeatability.pptx_bytes_equal_observation, "boolean");

    const baseManifest = JSON.parse(
      await readFile(join(output, "base/projection-manifest.json"), "utf8"),
    );
    const repeatedManifest = JSON.parse(
      await readFile(join(output, "repeatability/projection-manifest.json"), "utf8"),
    );
    assert.equal(
      baseManifest.semantic_projection_fingerprint,
      repeatedManifest.semantic_projection_fingerprint,
    );
    assert.deepEqual(
      baseManifest.source_fingerprints,
      repeatedManifest.source_fingerprints,
    );
    assert.equal(
      baseManifest.source_provenance_fingerprint,
      repeatedManifest.source_provenance_fingerprint,
    );
  } finally {
    await rm(output, { recursive: true, force: true });
  }
});

test("evidence remains adapter-local and returns the A+ signal to Issue #67", async () => {
  const output = await mkdtemp(join(tmpdir(), "tachiko-issue-163-test-"));
  try {
    const result = spawnSync(
      process.execPath,
      [PROBE, "--fixture-root", FIXTURE_ROOT, "--output", output],
      { encoding: "utf8" },
    );
    assert.equal(result.status, 0, result.stderr || result.stdout);

    const manifest = JSON.parse(
      await readFile(join(output, "base/projection-manifest.json"), "utf8"),
    );
    assert.equal(
      manifest.authority,
      "generated adapter provenance; not .roproj semantic truth",
    );
    assert(
      manifest.projection_elements.every(
        (element) => element.target_metadata_authority === "adapter-only",
      ),
    );
    assert.doesNotMatch(
      JSON.stringify(manifest),
      /PresentationId|SlideId|ViewId/u,
    );

    const summary = JSON.parse(
      await readFile(join(output, "experiment-summary.json"), "utf8"),
    );
    assert.deepEqual(summary.decision_signal, {
      strategy_under_test: "A+",
      result_for_exercised_fixed_projection:
        "adapter-plus-manifest-sufficient",
      concrete_renderer_independent_intent_failure_observed: false,
      semantic_promotion_recommended: false,
      unexercised_question:
        "user-curated narrative intent continuity across renderers",
      decision_owner: "Issue #67",
    });
  } finally {
    await rm(output, { recursive: true, force: true });
  }
});
