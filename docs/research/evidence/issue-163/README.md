---
schema: research-evidence/v0
episode: issue-163-pptx-provenance
capture_mode: prospective
capture_status: active
captured_at: "2026-08-30T23:24:31Z"
repository: nurockplayer/tachiko-work
base_sha: db97ec88962bbbaa66cf042a90aa407f3b165ef6
issue: https://github.com/nurockplayer/tachiko-work/issues/163
authority_state: Hypothesis
agent:
  interface: Codex desktop
  provider: unknown
  model: unknown
  configuration: unknown
context_manifest_status: exact
intervention_classes: [none]
failure_classes: [implementation_assumption, tooling_failure]
links:
  issues:
    - https://github.com/nurockplayer/tachiko-work/issues/67
    - https://github.com/nurockplayer/tachiko-work/issues/163
  prs: []
  adrs:
    - docs/decisions/ADR-0015-stable-semantic-identity.md
    - docs/decisions/ADR-0020-first-class-headless-semantic-api.md
    - docs/decisions/ADR-0021-progressive-semantic-strengthening.md
    - docs/decisions/ADR-0022-resident-semantic-runtime-and-host-boundary.md
  specs: []
  tests:
    - docs/research/probes/issue-163-pptx-provenance/test/probe.test.mjs
    - docs/research/probes/issue-163-pptx-provenance/test/source-capture.test.mjs
  evidence:
    - docs/research/fixtures/issue-163-pptx-provenance/
    - docs/research/probes/issue-163-pptx-provenance/
    - docs/research/evidence/issue-163/experiment-summary.json
    - docs/research/evidence/issue-163/base/projection-manifest.json
    - docs/research/evidence/issue-163/base/moonfall-review.pptx
---

# #163 — Adapter-only PPTX projection with provenance manifest

This evidence record captures a bounded Strategy A+ prototype. The fixture,
adapter IDs, target metadata, generated manifest, and PPTX files are research
artifacts only. They do not add Tachiko presentation semantics, public export
behavior, or compatibility promises.

## Question / material decision

Can a one-way PPTX adapter plus a generated, non-authoritative provenance
manifest remove the tested research-to-presentation copy/update drift while
keeping Tachiko semantic/query/revision authority and PPTX renderer state on
their existing sides of the boundary?

Issue [#67](https://github.com/nurockplayer/tachiko-work/issues/67), not this
prototype, owns the later A / A+ / B / C interpretation.

## Hypothesis

For a fixed reviewed projection, existing semantic Analysis Query output,
research claim/evidence references, source revisions, and stable source IDs can
drive a one-way deck generator. A companion manifest can then identify which
generated fragments become stale without requiring durable `PresentationId`,
`SlideId`, generic `ViewId`, layout semantics, or round-trip reconciliation.

If the adapter cannot preserve a user-important renderer-independent narrative
intent, the prototype should record that concrete failure for #67 instead of
promoting a model locally.

## Baseline

The fixture models three manual copy/update steps: copy the reviewed metric,
copy the claim derived from it, and copy/format the evidence reference. The
generated path performs zero of those three target-copy steps after fixture
capture, eliminating three steps for the exercised deck.

The source metric is the existing formula-backed Moonfall Analysis Query
observation from `examples/game-balance/game-balance.ro`: Iron Sword DPS is
`40`, with source revision
`cli-semantic-sha256:a480657ffe42b83d82e148fbf8393bc528f54969a5ccd536699b1e4263c90894`.
The controlled metric variant captures the existing
`examples/game-balance/buffed-sword.ro` comparison value `50` and revision
`cli-semantic-sha256:e7253fae438d4a2487e5a4d5bfc9249b8a0a13a1dc829d78f268517a4cb64b19`.
The claim/citation and unrelated changes are explicit synthetic issue-local
variants.

## Alternatives

- **A:** keep presentation entirely at an export/adapter boundary.
- **A+:** add generated adapter provenance and stale-fragment detection without
  changing semantic authority. This is the only strategy implemented here.
- **B:** promote a minimal renderer-independent projection-intent concept only
  if a concrete continuity need defeats A+.
- **C:** adopt a richer first-class presentation/view model only if later,
  unexpectedly strong evidence justifies it.

No production presentation infrastructure or round-trip behavior was built for
any alternative.

## Governing authority

- [ADR-0015](../../../decisions/ADR-0015-stable-semantic-identity.md) keeps
  stable identity semantic rather than coordinate- or layout-derived.
- [ADR-0020](../../../decisions/ADR-0020-first-class-headless-semantic-api.md)
  makes Analysis Query output ephemeral and permits presentation projections to
  consume its structured result without becoming persisted Analysis truth.
- [ADR-0021](../../../decisions/ADR-0021-progressive-semantic-strengthening.md)
  requires new semantic structure to be earned progressively.
- [ADR-0022](../../../decisions/ADR-0022-resident-semantic-runtime-and-host-boundary.md)
  and the
  [frontend boundary](../../../architecture/frontend-backend-boundary.md) keep
  authoritative semantics in the shared runtime rather than a renderer.
- The [rendering-system hypothesis](../../../architecture/rendering-system.md)
  leaves any generic projection abstraction open and assigns target validation
  to #67.

The live #163 `agent-handoff:v1` supplied the execution contract and constrained
all implementation and evidence to issue-local research surfaces.

## Context / source manifest actually supplied

The experiment used repository commit
`db97ec88962bbbaa66cf042a90aa407f3b165ef6`; the later unrelated `main` movement
is not an input to the captured query results. Inputs were exactly:

- the live #163 body and its single `agent-handoff:v1`, plus parent #67;
- the governing files listed above and the game-development roadmap;
- `examples/game-balance/game-balance.ro`,
  `examples/game-balance/buffed-sword.ro`, and
  `examples/game-balance/README.md`;
- the issue-local base fixture, three source-change variants, and two renderer
  configurations under `docs/research/fixtures/issue-163-pptx-provenance/`;
- PptxGenJS `4.0.1` and JSZip `3.10.1`, installed from the committed pnpm
  lockfile with pnpm `11.25.0` on Node.js 24; and
- the official [PptxGenJS repository](https://github.com/gitbrent/PptxGenJS)
  as implementation comparison/documentation, not a Tachiko authority.

The adapter executes no evaluator. The optional source-capture test invokes the
existing Tachiko CLI and verifies that the frozen fixture reproduces its query
values, normalized definition, formula-use flag, and source revisions.

## Initial recommendation or result

The six preregistered cases produced the following bounded result:

| Case | Observed result |
| --- | --- |
| Metric change | Only the metric source ID changed; the metric and dependent claim fragments became stale. |
| Claim/citation change | The claim and evidence source IDs changed while `40 DPS` stayed fixed; the claim and citation fragments became stale. |
| Unrelated change | The unprojected localization note changed; no adapter fragment became stale. |
| Renderer-only layout change | Source/projection provenance stayed equal; only renderer configuration and artifact hashes changed. |
| Target limitation | Live evidence inspection/query refresh degraded to a static citation and source block; the loss is explicit and caused no semantic promotion. |
| Manual target edit | A simulated `40`→`41 DPS` PPTX edit changed only the target; regeneration restored `40`, with no round trip or conflict protocol. |

All three projected fragments carry source IDs and fingerprints. The citation
and repository revision are preserved. Repeated generation produced equal
semantic-projection and source-provenance fingerprints. PPTX bytes also happened
to match for the paired generations within that invocation. A later clean
invocation preserved semantic/provenance evidence but produced different PPTX
container bytes, confirming that byte identity is explicitly not a contract.

## Human intervention

No human intervention changed the experiment inputs, expected cases, or result.
The agent performed the bounded implementation, source capture, mutation tests,
manual-target-edit simulation, and visual inspection described here.

## Failures / incorrect assumptions / authority drift

The first CLI build exhausted the nearly full local filesystem. The build was
retried with a bounded temporary Cargo target and reduced debug/incremental
output; no repository data or experiment input changed.

Initial rendered-slide inspection also found an overly narrow heading gap and a
wrapped manual-edit label. Those were renderer-layout assumptions, not semantic
failures. No authority drift, second evaluator, presentation identity, or
round-trip behavior was introduced.

The prototype did **not** exercise user-curated narrative-intent continuity
across multiple renderers. That remains an explicit open question, not a hidden
success claim.

## Corrections

The slide-two columns were separated and the manual edit was narrowed to the
same-width `40`→`41 DPS` target mutation. Evidence was regenerated from a clean
output directory, all ten generated slides across five decks were rendered and
inspected, and the presentation overflow checker passed for every deck.

The CLI fixture-capture test was added so the research fixture cannot silently
diverge from the existing Analysis Query behavior it claims to capture.

## Final outcome

**A+ WAS SUFFICIENT FOR THE EXERCISED FIXED PROJECTION.** The adapter and
manifest correctly separated source-semantic, source-unrelated, and
renderer-only changes; preserved the tested citation and lineage; removed the
three baseline copy/update steps; disclosed PPTX fidelity loss; and demonstrated
that target edits are overwritten on regeneration.

No concrete renderer-independent intent failure was observed in these cases, so
this evidence does not recommend semantic promotion. It also does not decide
that A+ is universally sufficient: user-curated narrative continuity across
renderers remains unexercised, and #67 retains the A / A+ / B / C decision.

Regeneration requires the pinned Node dependencies and one command into a fresh
output directory. Target-only edits are intentionally discarded. This is
research evidence, not a production exporter, presentation model, or public
PPTX promise.

## Traceability

- [#163 research issue](https://github.com/nurockplayer/tachiko-work/issues/163)
- [#67 parent decision](https://github.com/nurockplayer/tachiko-work/issues/67)
- [`experiment-summary.json`](experiment-summary.json) — all six case outcomes,
  manual-step count, provenance coverage, repeatability, and decision signal
- [`base/projection-manifest.json`](base/projection-manifest.json) — source,
  projection, target metadata, generator, fidelity, and artifact provenance
- [`base/moonfall-review.pptx`](base/moonfall-review.pptx) — base generated deck,
  SHA-256
  `3351b64a5c7b387eb92ed7f9fa8ee47bed625717f5dffa952dd79eb8e130bdec`
- `renderer-layout-change/` and `repeatability/` — renderer-only and repeated
  generated outputs
- `manual-target-edit/` — manually edited target SHA-256
  `9318796a234bf6d72b4b47efdea9fe076b4a8d0f73c72d8b877b3c3b793dbde8`
  and regenerated source-derived artifact
- `docs/research/fixtures/issue-163-pptx-provenance/` — frozen source and
  renderer inputs
- `docs/research/probes/issue-163-pptx-provenance/` — executable adapter and
  black-box/source-capture tests

Reproduce the source capture and case checks from the probe directory:

```sh
corepack pnpm install --frozen-lockfile
CARGO_TARGET_DIR=/tmp/tachiko-issue163-target \
  CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_BUILD_JOBS=1 \
  cargo build -p tachiko-cli --locked
TACHIKO_BIN=/tmp/tachiko-issue163-target/debug/tachiko corepack pnpm test
```

Generate a fresh evidence bundle into an empty directory rather than
overwriting this record:

```sh
corepack pnpm run generate --output /tmp/issue-163-evidence
```

## Downstream observations

- 2026-08-31: the bounded prototype produced an A+-sufficient signal for the
  exercised fixed projection, with no recommendation to promote presentation
  semantics.
- 2026-08-31: the unexercised cross-renderer narrative-continuity question was
  returned to #67 rather than answered by inference.
