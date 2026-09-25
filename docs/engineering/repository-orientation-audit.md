# Repository orientation audit — 2026-09-26

Status: dated explanatory evidence, not an ADR, live queue, or qualification
receipt. Owning work: [#476](https://github.com/nurockplayer/tachiko-work/issues/476).
Source baseline: [`97fcff83044ac53bb70952bbf6164d9662c52a4d`](https://github.com/nurockplayer/tachiko-work/commit/97fcff83044ac53bb70952bbf6164d9662c52a4d).
Recheck current source and handoffs before relying on delivery state below.

## Navigation decision

Use three existing entry points rather than introduce another mandatory agent
manual: [README](../../README.md) explains the value and current product boundary;
[root ARCHITECTURE](../../ARCHITECTURE.md) supplies the compact mental model and
source map; [docs/README](../README.md) routes questions to canonical detail.
This changes explanatory/navigation structure only. AGENTS and its SCD Skill
extraction in [PR #473](https://github.com/nurockplayer/tachiko-work/pull/473) are
left untouched, as is held [PR #413](https://github.com/nurockplayer/tachiko-work/pull/413).
Concept artwork remains in the repository but no longer precedes the README's
meaning or appears to be a product screenshot.

## Evidence and reconciliation

| Observation at the baseline | Treatment and evidence |
| --- | --- |
| Root ARCHITECTURE described `.roproj/v3` as accepted but unimplemented. | Correct the entry point: [storage exports](../../crates/storage/src/lib.rs) include v3 codecs and explicit migration. [ADR-0041](../decisions/ADR-0041-bounded-durable-field-constraints.md) owns the contract; [#449](https://github.com/nurockplayer/tachiko-work/issues/449) owns that delivery. Do not infer client support. |
| The register still labels #450 and #454 queued and describes earlier diff/lifecycle slices. | Treat those annotations as stale implementation snapshots, not normative reversals. [diff-engine](../../crates/diff-engine/src/lib.rs), [merge-engine](../../crates/merge-engine/src/lib.rs) and [patch lifecycle](../../crates/workspace-engine/src/patch_lifecycle.rs) contain constraint-aware v2 work; [#374](https://github.com/nurockplayer/tachiko-work/issues/374) records the later delivery. Broader register/spec annotation cleanup remains separate; their accepted meaning is unchanged. |
| Historical Designer save/UI language remains in subsystem docs and runtime symbols. | [#354](https://github.com/nurockplayer/tachiko-work/issues/354)'s founder cutover, [#459](https://github.com/nurockplayer/tachiko-work/issues/459) and the baseline retirement merge establish current ownership. Work keeps the [experimental producer](../../packages/browser-client/README.md); Sheet owns spreadsheet UX. Retained ABI names do not reopen a retired product lane. |
| The browser adapter imports v1/v2 storage codecs, while storage exports v3. | This is a real delivery/qualification boundary, not an accepted-authority contradiction. [Runtime source](../../packages/browser-client/runtime/src/lib.rs) and [#452](https://github.com/nurockplayer/tachiko-work/issues/452) must be read together. Work main is not automatically the Sheet-qualified producer pin. |
| Roadmap uses “Migration & Enterprise Beta” and completed horizons. | The [roadmap](../product/product-roadmap.md) explicitly defines these as planning classifications, not proof that all aspirational capabilities shipped. README retains the pre-alpha boundary. |

ADR-0041's statement that the ADR itself authorizes no production implementation
is not evidence that subsequent separately owned implementation never happened.
Likewise, explanatory shorthand cannot amend an Accepted contract. No new
product, storage, API, runtime-ownership or licensing decision is made here.

## Boundaries still requiring their existing owners

**Native v3 consumer:** [#452](https://github.com/nurockplayer/tachiko-work/issues/452)
still needs the selected occurrence, complete saved metadata, explicit conversion
and host receipt/consumer agreement. Existing codecs, delta and lifecycle work
are building blocks, not that end-to-end acceptance result.

**Delegated desktop approval:** [ADR-0039](../decisions/ADR-0039-native-trusted-host-delegated-human-approval.md)
selects one native owner for the whole occurrence. It does not make a browser
consent UI trusted. [#361](https://github.com/nurockplayer/tachiko-work/issues/361)
and the current campaign handoff own implementation readiness; held PR #413 is
not a reusable merged feature.

**Future platform promises:** stable transport/SDK mechanics, broader hosted
storage/collaboration, large-artifact incremental persistence and enterprise
product claims are not established by the short introduction. See the
[authority register](../governance/canonical-reconciliation-register.md),
[#313](https://github.com/nurockplayer/tachiko-work/issues/313),
[#464](https://github.com/nurockplayer/tachiko-work/issues/464), and deferred
[#470](https://github.com/nurockplayer/tachiko-work/issues/470)/[#471](https://github.com/nurockplayer/tachiko-work/issues/471).
Do not turn these research or held paths into implementation permission.

## Verification and limits

The investigation compared the Constitution/design principles, governance,
ADR/register/specification routes, current campaign and retirement context,
eight-crate ownership and source entry points, resident/patch lifecycle,
versioned storage, experimental runtime composition, and CI/smoke sources.
It was an orientation audit, not a complete code/security review or a claim that
every historical document is now reconciled.

The README example uses the checked-in Moonfall values and command shapes from
[the walkthrough](../../examples/game-balance/README.md) and
[first-user smoke](../../scripts/first-user-smoke.sh). No benchmark, unlimited
capacity, full Excel fidelity or timed usability result is inferred.

The approximate 30-second/3-minute targets are reading-budget goals. Word counts
and navigation checks are proxies, not a measured user study. Actual patch
checks, exact commit identity, hosted CI and outstanding independent-review
requirements belong in the linked PR's single canonical handoff. Do not copy a
validation verdict from this dated document onto a later commit.
