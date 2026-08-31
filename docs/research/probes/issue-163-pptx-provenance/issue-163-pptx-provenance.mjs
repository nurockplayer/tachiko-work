#!/usr/bin/env node
// DISPOSABLE, NON-PRODUCTION research probe for Issue #163.
// PPTX objects and this manifest profile are adapter evidence, not Tachiko
// semantic/storage/public API authority.

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import JSZip from "jszip";
import pptxgen from "pptxgenjs";

const PACKAGE_ROOT = dirname(fileURLToPath(import.meta.url));
const DEFAULT_FIXTURE_ROOT = resolve(
  PACKAGE_ROOT,
  "../../fixtures/issue-163-pptx-provenance",
);

function parseArguments(arguments_) {
  const parsed = { fixtureRoot: DEFAULT_FIXTURE_ROOT, output: null };
  for (let index = 0; index < arguments_.length; index += 2) {
    const option = arguments_[index];
    const value = arguments_[index + 1];
    if (!value) throw new Error(`missing value for ${option}`);
    if (option === "--fixture-root") parsed.fixtureRoot = resolve(value);
    else if (option === "--output") parsed.output = resolve(value);
    else throw new Error(`unknown option: ${option}`);
  }
  if (parsed.output === null) {
    throw new Error(
      "usage: node issue-163-pptx-provenance.mjs [--fixture-root PATH] --output EMPTY_DIRECTORY",
    );
  }
  return parsed;
}

async function readJson(path) {
  return JSON.parse(await readFile(path, "utf8"));
}

function canonicalize(value) {
  if (Array.isArray(value)) return value.map(canonicalize);
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.keys(value)
        .sort((left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right)))
        .map((key) => [key, canonicalize(value[key])]),
    );
  }
  return value;
}

function canonicalJson(value) {
  return JSON.stringify(canonicalize(value));
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function sourceMap(fixture) {
  return new Map(fixture.sources.map((source) => [source.id, source]));
}

function applyVariant(base, variant) {
  const next = structuredClone(base);
  next.snapshot = variant.id;
  const sources = sourceMap(next);
  for (const change of variant.changes) {
    const source = sources.get(change.source_id);
    assert(source, `variant ${variant.id} names unknown source ${change.source_id}`);
    Object.assign(source, change.set);
  }
  if (variant.lineage_changes?.source_revision) {
    next.analysis_lineage.source_revision =
      variant.lineage_changes.source_revision;
  }
  if (variant.lineage_changes?.semantic_source_path) {
    next.repository.semantic_source_path =
      variant.lineage_changes.semantic_source_path;
  }
  return next;
}

function sourceFingerprints(fixture) {
  return Object.fromEntries(
    [...sourceMap(fixture)]
      .map(([id, source]) => [id, `sha256:${sha256(canonicalJson(source))}`])
      .sort(([left], [right]) => Buffer.compare(Buffer.from(left), Buffer.from(right))),
  );
}

function compareSources(base, candidate) {
  const before = sourceFingerprints(base);
  const after = sourceFingerprints(candidate);
  const changedSourceIds = Object.keys(before)
    .filter((id) => before[id] !== after[id])
    .sort((left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right)));
  const staleAdapterElements = base.projection.elements
    .filter((element) =>
      element.source_ids.some((sourceId) => changedSourceIds.includes(sourceId)),
    )
    .map((element) => element.adapter_element_id)
    .sort((left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right)));
  return { changedSourceIds, staleAdapterElements };
}

function findSource(fixture, id) {
  const source = sourceMap(fixture).get(id);
  assert(source, `fixture source ${id} is missing`);
  return source;
}

async function createDeck(fixture, rendererConfiguration) {
  const metric = findSource(
    fixture,
    "semantic:field:89f0fd5e-dfc9-53bf-b008-85f78c403420",
  );
  const claim = findSource(fixture, "fixture:claim:iron-sword-threshold");
  const evidence = findSource(fixture, "fixture:evidence:moonfall-readme");

  const pptx = new pptxgen();
  pptx.layout = rendererConfiguration.layout;
  pptx.author = "Tachiko Work Issue #163 research probe";
  pptx.company = "Tachiko Work";
  pptx.subject = "Non-authoritative PPTX projection provenance experiment";
  pptx.title = "Moonfall reviewed balance projection";
  pptx.theme = {
    headFontFace: "Arial",
    bodyFontFace: "Arial",
    lang: "en-US",
  };

  const slide = pptx.addSlide();
  slide.background = { color: rendererConfiguration.background };
  slide.addText("Reviewed balance facts stay linked", {
    x: 0.75,
    y: 0.55,
    w: 11.8,
    h: 0.7,
    margin: 0,
    fontFace: "Arial",
    fontSize: 35,
    bold: true,
    color: rendererConfiguration.ink,
    objectName: "adapter:title",
  });
  slide.addShape(pptx.ShapeType.line, {
    x: 0.75,
    y: 1.42,
    w: 11.8,
    h: 0,
    line: { color: rendererConfiguration.rule, width: 1 },
    objectName: "adapter:title-rule",
  });
  slide.addText(`${metric.value} DPS`, {
    ...rendererConfiguration.metric,
    margin: 0,
    fontFace: "Arial",
    fontSize: 54,
    bold: true,
    color: rendererConfiguration.accent,
    objectName: "adapter:metric-value",
  });
  slide.addText(metric.label, {
    ...rendererConfiguration.metric_label,
    margin: 0,
    fontFace: "Arial",
    fontSize: 24,
    bold: true,
    color: rendererConfiguration.ink,
    objectName: "adapter:metric-label",
  });
  slide.addText(claim.text, {
    ...rendererConfiguration.claim,
    margin: 0,
    fontFace: "Arial",
    fontSize: 24,
    breakLine: false,
    color: rendererConfiguration.ink,
    objectName: "adapter:claim-text",
  });
  slide.addText(
    `Source: ${evidence.repository_path} — ${evidence.locator}`,
    {
      x: 0.75,
      y: 6.55,
      w: 11.8,
      h: 0.28,
      margin: 0,
      fontFace: "Arial",
      fontSize: 16,
      color: rendererConfiguration.muted_ink,
      objectName: "adapter:citation-footer",
    },
  );
  slide.addNotes(
    `[Sources]\n- ${fixture.repository.semantic_source_path} @ ${fixture.repository.base_sha}\n- ${evidence.repository_path} — ${evidence.locator}\n[/Sources]`,
  );

  const limitation = fixture.projection.desired_behavior;
  const limitationSlide = pptx.addSlide();
  limitationSlide.background = { color: rendererConfiguration.background };
  limitationSlide.addText(
    "PPTX freezes live evidence into a static snapshot",
    {
      x: 0.75,
      y: 0.55,
      w: 11.8,
      h: 0.7,
      margin: 0,
      fontFace: "Arial",
      fontSize: 35,
      bold: true,
      color: rendererConfiguration.ink,
      objectName: "adapter:limitation-title",
    },
  );
  limitationSlide.addShape(pptx.ShapeType.line, {
    x: 0.75,
    y: 1.42,
    w: 11.8,
    h: 0,
    line: { color: rendererConfiguration.rule, width: 1 },
    objectName: "adapter:limitation-rule",
  });
  limitationSlide.addText("Desired behavior", {
    x: 0.75,
    y: 2,
    w: 5.35,
    h: 0.45,
    margin: 0,
    fontFace: "Arial",
    fontSize: 24,
    bold: true,
    color: rendererConfiguration.accent,
    objectName: "adapter:limitation-desired-heading",
  });
  limitationSlide.addText(limitation.description, {
    x: 0.75,
    y: 2.6,
    w: 5.35,
    h: 1.5,
    margin: 0,
    fontFace: "Arial",
    fontSize: 20,
    color: rendererConfiguration.ink,
    objectName: "adapter:limitation-desired-body",
  });
  limitationSlide.addText("PPTX realization", {
    x: 7.2,
    y: 2,
    w: 5.35,
    h: 0.45,
    margin: 0,
    fontFace: "Arial",
    fontSize: 24,
    bold: true,
    color: rendererConfiguration.accent,
    objectName: "adapter:limitation-realized-heading",
  });
  limitationSlide.addText(
    "Static visible citation plus speaker-note sources. Live evidence inspection and query refresh are unavailable.",
    {
      x: 7.2,
      y: 2.6,
      w: 5.35,
      h: 1.5,
      margin: 0,
      fontFace: "Arial",
      fontSize: 20,
      color: rendererConfiguration.ink,
      objectName: "adapter:limitation-realized-body",
    },
  );
  limitationSlide.addText(
    "The loss stays in adapter evidence; it does not create presentation semantics.",
    {
      x: 0.75,
      y: 5.45,
      w: 11.8,
      h: 0.55,
      margin: 0,
      fontFace: "Arial",
      fontSize: 24,
      bold: true,
      color: rendererConfiguration.ink,
      objectName: "adapter:limitation-boundary",
    },
  );
  limitationSlide.addNotes(
    `[Sources]\n- ${fixture.repository.semantic_source_path} @ ${fixture.repository.base_sha}\n- Issue #163 research fixture desired behavior: ${limitation.id}\n[/Sources]`,
  );

  return {
    bytes: Buffer.from(
      await pptx.write({ outputType: "nodebuffer", compression: true }),
    ),
    generatorVersion: pptx.version,
  };
}

function projectionElements(fixture, fingerprints) {
  const targets = {
    "adapter:metric-value": { slide_number: 1, object_name: "adapter:metric-value" },
    "adapter:claim-text": { slide_number: 1, object_name: "adapter:claim-text" },
    "adapter:citation-footer": {
      slide_number: 1,
      object_name: "adapter:citation-footer",
    },
  };
  return fixture.projection.elements.map((element) => ({
    adapter_element_id: element.adapter_element_id,
    source_ids: element.source_ids,
    source_fingerprints: Object.fromEntries(
      element.source_ids.map((id) => [id, fingerprints[id]]),
    ),
    target: targets[element.adapter_element_id],
    target_metadata_authority: "adapter-only",
  }));
}

async function writeBundle(
  fixture,
  rendererConfiguration,
  output,
  directoryName,
) {
  const directory = join(output, directoryName);
  await mkdir(directory);
  const deck = await createDeck(fixture, rendererConfiguration);
  const deckPath = join(directory, "moonfall-review.pptx");
  await writeFile(deckPath, deck.bytes);
  const fingerprints = sourceFingerprints(fixture);
  const manifest = {
    profile: "tachiko.issue-163.projection-manifest/probe-v1",
    authority: "generated adapter provenance; not .roproj semantic truth",
    repository: fixture.repository,
    fixture_snapshot: fixture.snapshot,
    analysis_lineage: fixture.analysis_lineage,
    generator: {
      name: "PptxGenJS",
      version: deck.generatorVersion,
      package_manager: "pnpm@11.25.0",
      configuration: rendererConfiguration.id,
      configuration_sha256: sha256(canonicalJson(rendererConfiguration)),
    },
    source_fingerprints: fingerprints,
    source_provenance_fingerprint: `sha256:${sha256(
      canonicalJson({
        analysis_lineage: fixture.analysis_lineage,
        fixture_snapshot: fixture.snapshot,
        repository: fixture.repository,
        source_fingerprints: fingerprints,
      }),
    )}`,
    projection_elements: projectionElements(fixture, fingerprints),
    semantic_projection_fingerprint: `sha256:${sha256(
      canonicalJson({
        analysis_lineage: fixture.analysis_lineage,
        projection: fixture.projection.elements,
        sources: fixture.sources.filter((source) =>
          fixture.projection.elements.some((element) =>
            element.source_ids.includes(source.id),
          ),
        ),
      }),
    )}`,
    fidelity_losses: [
      {
        intended_behavior_id: fixture.projection.desired_behavior.id,
        target: "PPTX",
        status: "degraded",
        realized_as: "static visible citation plus speaker-note source block",
        lost_behavior:
          "no live per-fragment evidence inspection or query refresh inside the PPTX",
        semantic_promotion: "none",
      },
    ],
    manual_target_edit_contract: {
      authoritative: false,
      regeneration: "overwrites target-only edits",
      round_trip_or_conflict_protocol: false,
    },
    artifact: {
      path: "moonfall-review.pptx",
      sha256: sha256(deck.bytes),
      bytes: deck.bytes.length,
      byte_identity_is_contract: false,
    },
  };
  await writeFile(
    join(directory, "projection-manifest.json"),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );
  return manifest;
}

async function simulateManualTargetEdit(deckBytes) {
  const archive = await JSZip.loadAsync(deckBytes);
  const path = "ppt/slides/slide1.xml";
  const slide = await archive.file(path).async("string");
  assert.match(slide, />40 DPS</u, "generated metric text is missing");
  archive.file(path, slide.replace(">40 DPS<", ">41 DPS<"));
  return Buffer.from(
    await archive.generateAsync({ type: "nodebuffer", compression: "DEFLATE" }),
  );
}

async function writeManualTargetEditEvidence(
  fixture,
  rendererConfiguration,
  output,
) {
  const directory = join(output, "manual-target-edit");
  await mkdir(directory);
  const original = await readFile(join(output, "base/moonfall-review.pptx"));
  const manuallyEdited = await simulateManualTargetEdit(original);
  const regenerated = (await createDeck(fixture, rendererConfiguration)).bytes;
  await writeFile(join(directory, "manually-edited.pptx"), manuallyEdited);
  await writeFile(join(directory, "regenerated.pptx"), regenerated);

  const regeneratedArchive = await JSZip.loadAsync(regenerated);
  const regeneratedSlide = await regeneratedArchive
    .file("ppt/slides/slide1.xml")
    .async("string");
  return {
    contract: "target-edit-non-authoritative-overwritten-on-regeneration",
    round_trip_supported: false,
    original_artifact_sha256: sha256(original),
    manually_edited_artifact_sha256: sha256(manuallyEdited),
    regenerated_artifact_sha256: sha256(regenerated),
    manual_target_value_removed: !regeneratedSlide.includes(">41 DPS<"),
  };
}

async function ensureEmptyDirectory(path) {
  await mkdir(path, { recursive: true });
  assert.deepEqual(
    await readdir(path),
    [],
    `output directory must be empty: ${path}`,
  );
}

async function main() {
  const arguments_ = parseArguments(process.argv.slice(2));
  await ensureEmptyDirectory(arguments_.output);
  const base = await readJson(join(arguments_.fixtureRoot, "base.json"));
  const metricVariant = await readJson(
    join(arguments_.fixtureRoot, "variants/metric-change.json"),
  );
  const claimCitationVariant = await readJson(
    join(arguments_.fixtureRoot, "variants/claim-citation-change.json"),
  );
  const unrelatedVariant = await readJson(
    join(arguments_.fixtureRoot, "variants/unrelated-change.json"),
  );
  const defaultRenderer = await readJson(
    join(arguments_.fixtureRoot, "renderer/default.json"),
  );
  const layoutRenderer = await readJson(
    join(arguments_.fixtureRoot, "renderer/layout-change.json"),
  );
  const metricCandidate = applyVariant(base, metricVariant);
  const metricComparison = compareSources(base, metricCandidate);
  assert.deepEqual(
    metricComparison.changedSourceIds,
    metricVariant.expected.changed_source_ids,
  );
  assert.deepEqual(
    metricComparison.staleAdapterElements,
    metricVariant.expected.stale_adapter_elements,
  );
  const claimCitationComparison = compareSources(
    base,
    applyVariant(base, claimCitationVariant),
  );
  const claimCitationCandidate = applyVariant(base, claimCitationVariant);
  assert.deepEqual(
    claimCitationComparison.changedSourceIds,
    claimCitationVariant.expected.changed_source_ids,
  );
  assert.deepEqual(
    claimCitationComparison.staleAdapterElements,
    claimCitationVariant.expected.stale_adapter_elements,
  );
  const unrelatedComparison = compareSources(
    base,
    applyVariant(base, unrelatedVariant),
  );
  assert.deepEqual(
    unrelatedComparison.changedSourceIds,
    unrelatedVariant.expected.changed_source_ids,
  );
  assert.deepEqual(
    unrelatedComparison.staleAdapterElements,
    unrelatedVariant.expected.stale_adapter_elements,
  );

  const manifest = await writeBundle(
    base,
    defaultRenderer,
    arguments_.output,
    "base",
  );
  const layoutManifest = await writeBundle(
    base,
    layoutRenderer,
    arguments_.output,
    "renderer-layout-change",
  );
  const repeatedManifest = await writeBundle(
    base,
    defaultRenderer,
    arguments_.output,
    "repeatability",
  );
  const rendererComparison = compareSources(base, base);
  const manualTargetEdit = await writeManualTargetEditEvidence(
    base,
    defaultRenderer,
    arguments_.output,
  );
  const summary = {
    profile: "tachiko.issue-163.experiment-summary/probe-v1",
    repository_base_sha: base.repository.base_sha,
    base_artifact_sha256: manifest.artifact.sha256,
    manual_workflow: {
      baseline_copy_or_update_steps: base.manual_baseline.copy_or_update_steps,
      generated_copy_or_update_steps: 0,
      manual_steps_eliminated: base.manual_baseline.copy_or_update_steps,
    },
    provenance_quality: {
      projected_fragments: manifest.projection_elements.length,
      fragments_with_source_ids_and_fingerprints:
        manifest.projection_elements.filter(
          (element) =>
            element.source_ids.length > 0 &&
            Object.keys(element.source_fingerprints).length ===
              element.source_ids.length,
        ).length,
      citation_fragment_preserved: manifest.projection_elements.some(
        (element) =>
          element.adapter_element_id === "adapter:citation-footer" &&
          element.source_ids.includes("fixture:evidence:moonfall-readme"),
      ),
      repository_revision_preserved:
        manifest.repository.base_sha === base.repository.base_sha,
    },
    repeatability: {
      semantic_projection_equal:
        manifest.semantic_projection_fingerprint ===
        repeatedManifest.semantic_projection_fingerprint,
      source_provenance_equal:
        manifest.source_provenance_fingerprint ===
        repeatedManifest.source_provenance_fingerprint,
      pptx_byte_identity_required: false,
      pptx_bytes_equal_observation_scope: "paired-generations-same-invocation",
      pptx_bytes_equal_observation:
        manifest.artifact.sha256 === repeatedManifest.artifact.sha256,
    },
    decision_signal: {
      strategy_under_test: "A+",
      result_for_exercised_fixed_projection:
        "adapter-plus-manifest-sufficient",
      concrete_renderer_independent_intent_failure_observed: false,
      semantic_promotion_recommended: false,
      unexercised_question:
        "user-curated narrative intent continuity across renderers",
      decision_owner: "Issue #67",
    },
    cases: {
      metric_change: {
        change_class: metricVariant.expected.change_class,
        changed_source_ids: metricComparison.changedSourceIds,
        stale_adapter_elements: metricComparison.staleAdapterElements,
      },
      claim_citation_change: {
        change_class: claimCitationVariant.expected.change_class,
        changed_source_ids: claimCitationComparison.changedSourceIds,
        stale_adapter_elements: claimCitationComparison.staleAdapterElements,
        metric_unchanged:
          sourceFingerprints(base)[
            "semantic:field:89f0fd5e-dfc9-53bf-b008-85f78c403420"
          ] ===
          sourceFingerprints(claimCitationCandidate)[
            "semantic:field:89f0fd5e-dfc9-53bf-b008-85f78c403420"
          ],
      },
      unrelated_change: {
        change_class: unrelatedVariant.expected.change_class,
        changed_source_ids: unrelatedComparison.changedSourceIds,
        stale_adapter_elements: unrelatedComparison.staleAdapterElements,
      },
      renderer_layout_change: {
        change_class: "renderer-only",
        changed_source_ids: rendererComparison.changedSourceIds,
        stale_adapter_elements: rendererComparison.staleAdapterElements,
        semantic_projection_fingerprint_unchanged:
          manifest.semantic_projection_fingerprint ===
          layoutManifest.semantic_projection_fingerprint,
        source_provenance_fingerprint_unchanged:
          manifest.source_provenance_fingerprint ===
          layoutManifest.source_provenance_fingerprint,
        renderer_configuration_fingerprint_changed:
          manifest.generator.configuration_sha256 !==
          layoutManifest.generator.configuration_sha256,
        artifact_hash_changed:
          manifest.artifact.sha256 !== layoutManifest.artifact.sha256,
      },
      target_limitation: {
        fidelity: manifest.fidelity_losses[0].status,
        semantic_promotion: manifest.fidelity_losses[0].semantic_promotion,
      },
      manual_target_edit: manualTargetEdit,
    },
  };
  await writeFile(
    join(arguments_.output, "experiment-summary.json"),
    `${JSON.stringify(summary, null, 2)}\n`,
  );
  process.stdout.write(`${JSON.stringify(summary)}\n`);
}

await main();
