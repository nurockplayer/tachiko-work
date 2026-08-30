---
schema: research-evidence/v0
episode: issue-185-git-share-use
capture_mode: prospective
capture_status: active
captured_at: "2026-08-30T22:48:51Z"
repository: nurockplayer/tachiko-work
base_sha: c3b5ad2aad04e6b79594dbc7f79199591997bdc4
issue: https://github.com/nurockplayer/tachiko-work/issues/185
authority_state: Hypothesis
agent:
  interface: Codex desktop
  provider: OpenAI
  model: unknown
  configuration: unknown
context_manifest_status: partial
intervention_classes: [constraint_addition, correction]
failure_classes: [implementation_assumption, tooling_failure]
links:
  issues:
    - https://github.com/nurockplayer/tachiko-work/issues/176
    - https://github.com/nurockplayer/tachiko-work/issues/177
    - https://github.com/nurockplayer/tachiko-work/issues/178
    - https://github.com/nurockplayer/tachiko-work/issues/179
    - https://github.com/nurockplayer/tachiko-work/issues/181
    - https://github.com/nurockplayer/tachiko-work/issues/184
    - https://github.com/nurockplayer/tachiko-work/issues/185
    - https://github.com/nurockplayer/tachiko-work/issues/135
  prs:
    - https://github.com/nurockplayer/tachiko-work/pull/201
  adrs: []
  specs:
    - https://github.com/nurockplayer/tachiko-work/blob/c3b5ad2aad04e6b79594dbc7f79199591997bdc4/docs/research/evidence/schema-v0.md
  tests:
    - docs/research/probes/issue-185-git-share-use.mjs
  evidence:
    - https://github.com/nurockplayer/tachiko-work/issues/185#issuecomment-5464202095
    - https://github.com/nurockplayer/tachiko-work/issues/185#issuecomment-5465361161
    - https://github.com/nurockplayer/tachiko-work/issues/185#issuecomment-5471698038
    - https://github.com/tt-a1i/archify/commit/b36d79fdbc3aec3728744341485a7e79f03c0071
---

# #185 — Git-hosted Share → Copy/Vendor/Fork experiment

## Question / material decision

Can an ordinary Git repository plus immutable revision pinning and Tachiko's
existing semantic CLI prove one safe early creator-to-consumer loop without a
registry, Marketplace, package-manager runtime, final manifest, background
updater, or new semantic authority?

Issue #185 owns the Research / Hypothesis interpretation. This episode records
the bounded executable result so the Project Steward can independently choose
among the Issue's A/B/C/D candidate outcomes.

## Hypothesis

A whole-project/template-style Moonfall example can complete this loop using
only an explicit publication allowlist, an exact Git commit and payload digest,
pre-use inspection, independent Copy/Vendor and Fork/Remix relationships, and
manual semantic diff/three-way merge:

```text
publish immutable A → inspect exact A → Copy/Vendor → Fork/Remix
                    → verify immutable B → inspect diffs → explicit merge candidate
```

Git identity is transport provenance only. It must not grant trust, support,
compatibility, signing, approval, capabilities, or semantic authority.

## Baseline

The probe started from live `main` at
`c3b5ad2aad04e6b79594dbc7f79199591997bdc4`. The checked-in
`examples/game-balance/game-balance.ro` fixture was clean and used as read-only
input. The current `tachiko` CLI already exposed `validate`, `set`, semantic
`diff`, and three-way `merge`.

At dispatch, open PRs #188, #195, and #198 did not touch either E9 target file.
Both target files were absent on live `main`. The experiment therefore began
inside the exact two-file research boundary authorized by #185.

## Alternatives

The Issue retains four interpretations for the Project Steward:

1. ordinary Git convention only;
2. a small Git-hosted reusable-asset flow;
3. Git storage plus an early lightweight catalog; or
4. rejection of Git-hosted bootstrap.

The probe tests whether the bounded loop is technically credible. It does not
select an outcome, define production architecture, or prove ecosystem demand.

## Governing authority

The executable boundary is the current canonical
[#185 handoff](https://github.com/nurockplayer/tachiko-work/issues/185#issuecomment-5464202095),
its [Archify evidence addendum](https://github.com/nurockplayer/tachiko-work/issues/185#issuecomment-5465361161),
and its [execution-coordination addendum](https://github.com/nurockplayer/tachiko-work/issues/185#issuecomment-5471698038).

Parent #176 keeps the ecosystem contribution ladder in Research state. #184
selects a game-balance-first, Git/static, manually curated cold start. Existing
repository knowledge-authority, delivery, semantic validation, approval, and
no-overwrite rules remain in force. No Research result here amends an Accepted
ADR, specification, `.ro`/`.roproj` meaning, or public distribution policy.

## Context / source manifest actually supplied

Confirmed supplied context included:

- complete live Issue #185 body and all three comments above;
- parent #176 only to confirm ownership, sequencing, and durable guardrails;
- live Product Roadmap and current open-PR overlap;
- `CONTRIBUTING.md`, the repository delivery workflow, knowledge-authority
  policy, Research Evidence Capture v0 instructions, and schema;
- the Moonfall fixture and example workflow documentation;
- current CLI command definitions and existing Git/semantic smoke journeys;
- the exact live `main` commit and both repository license texts.

The durable context manifest is `partial`: it does not preserve private chat or
built-in system-prompt content, and provider deployment/configuration details
are unavailable. Those values are not inferred.

## Initial recommendation or result

The first complete probe execution passed without production changes.

Exact immutable evidence from that run:

| Evidence | Exact result |
| --- | --- |
| Publisher commit A | `030a82e199d7dde4ba057c7365573a50b9de6998` |
| Payload A SHA-256 | `107db1e39175855e9fdb15d7f30e657efc3906b54d65917b8a1c84aa9dba8365` |
| Local Remix SHA-256 | `e7253fae438d4a2487e5a4d5bfc9249b8a0a13a1dc829d78f268517a4cb64b19` |
| Publisher commit B | `2461d06995cb5a141c027797a798b92cf62d6262` |
| Payload B SHA-256 | `a95780adbad5a781eec6946d168ddfa5c20676e6cdd96498381892ae5e4268b5` |

The publisher repository contained exactly five allowlisted paths: the Moonfall
payload, an experiment-only README descriptor, a source/license note, and both
upstream license texts. `.env`, local/cache/private-history sentinels, and the
share preview itself did not enter the Git tree. This proves an allowlist-first
export/preview requirement; it does not claim a production secret scanner.

Before creating the consumer asset area, the probe read A from its exact commit,
verified its digest, validated it twice deterministically, and produced a
human-readable inspection summary covering:

- artifact class and contributed schemas/entities/formulas/references;
- source reference, exact SHA, payload path, and digest;
- source/license facts with future legal interpretation explicit as unresolved;
- current Tachiko assumptions and semantic requirements as requirements, not
  grants;
- no executable/external effects and zero reusable-asset dependencies;
- validation evidence and the Git-does-not-imply-trust boundary.

Copy/Vendor created an independent exact copy and kept origin facts outside
canonical Tachiko semantic state. Fork/Remix created a separate Git repository,
retained human-readable ancestry, and used `tachiko set` to change sword damage
from 36 to 45 without inheriting trust or an update relationship. Use/reference
remained deliberately unimplemented.

For the update pressure test, a missing immutable B and a resolvable B with a
mismatched digest were both withheld: no discovery output appeared, pinned A
remained reproducible, and the local Remix remained byte-identical. Only after
the exact B tuple resolved and its digest matched did the probe surface a
non-authoritative discovery hint.

The consumer then inspected A→B and local-Remix→B semantic divergence without
changing the Remix. A three-way semantic merge used A as base, Remix as ours,
and B as theirs, wrote only a new candidate, validated deterministically, and
preserved both disjoint changes:

- `damage: 36 → 45`;
- `attack_interval: 0.9 → 0.8`;
- derived `dps: 40 → 56.25`.

The pre-merge local Remix and checked-in Moonfall fixture were byte-identical
before and after the update exercise.

## Human intervention

Before execution, the Project Steward added the Archify-derived publication
ordering pressure test: an immutable payload must resolve and byte-match before
a moving discovery pointer can surface it. The execution-coordination addendum
also required exactly one PR handoff comment and periodic Steward-watch checks.
These were constraint additions, not a result override.

No post-PR `project-steward-watch:v1` guidance existed at initial capture. Any
later correction is appended rather than rewriting this result.

## Failures / incorrect assumptions / authority drift

No probe assertion, semantic validation, or overlap gate failed in the initial
run. The experiment did not require a registry, resolver, hidden live
dependency, executable capability, semantic-origin field, shared runtime seam,
or overwrite of local work.

The pass does **not** establish that a production secret scanner, license
policy, catalog, signing system, dependency solver, updater, or compatibility
contract exists. Treating the disposable README, origin note, tuple, or
discovery hint as such a contract would be authority drift.

## Corrections

No corrective probe iteration was required before the initial passing result.
The Archify missing/mismatched-B assertions were present from the first run.

Post-PR independent and hosted review then found four evidence-integrity
assumptions that the initial clean-machine pass had not exercised:

1. the human-visible source reference was synthetic while an unreported
   temporary path performed Git resolution;
2. the probe compared Moonfall before/after but could publish valid dirty
   working-tree bytes;
3. disposable Git commits could inherit user/system configuration, signing,
   and hooks; and
4. the two published license texts still came from working-tree bytes after
   Moonfall itself had been pinned.

The probe now uses the advertised runtime repository path as the actual Git
resolver input and rejects a wrong reference. It requires a clean Moonfall path,
reads Moonfall and both license payloads from the exact checked-in repository
HEAD, strips inherited `GIT_*` inputs, disables system/global Git configuration,
and explicitly disables signing and hooks. Normal and hostile-environment runs
with injected `GIT_DIR`, `commit.gpgSign`, and `core.hooksPath` settings produced
byte-identical A/B commit and digest evidence.

The 2026-08-31 08:00 JST
[`project-steward-watch:v1`](https://github.com/nurockplayer/tachiko-work/pull/201#issuecomment-5471799481)
independently required the exact-HEAD license correction and this appended
history. The correction changed no experiment relationship, semantic outcome,
or production boundary. A transient release-gate retry also encountered a
stale local preview listener and then exhausted disk space in generated Cargo
artifacts; both environmental conditions were removed without source changes.

## Final outcome

**BOUNDED PROBE PASS; HYPOTHESIS NOT FALSIFIED.** Ordinary Git plus exact
revision/digest pinning and existing Tachiko semantic authority completed the
full authorized loop inside two isolated research files. That is evidence that
Git-hosted bootstrap remains technically credible; it is not a production
reusable-asset contract and is not an A/B/C/D decision.

The smallest creator UX supported by the evidence is: preview an explicit
allowlist, inspect exclusions, publish the immutable payload, verify its tuple,
then expose a moving discovery hint. The smallest consumer UX is: inspect the
exact tuple and trust boundary, explicitly Copy/Vendor or Fork/Remix, validate,
and manually inspect/diff/merge an upstream revision into a new candidate.

Git-only distribution stops being sufficient for this proven slice when a
required workflow cannot retain its tested constraints: zero transitive asset
dependencies, exact immutable pins, no executable effects, manual discovery,
explicit local operations, and human-inspectable provenance. In particular,
live Use/reference dependencies hand back to #177/#178, reusable procedures to
#179, catalog/discovery pressure to #181, and signing/trust/compatibility,
executable distribution, or lifecycle automation to #135. The experiment does
not justify building any of those surfaces before their own evidence and
authority gates are met.

## Traceability

- [#185 Research Issue](https://github.com/nurockplayer/tachiko-work/issues/185)
- [PR #201 executable evidence](https://github.com/nurockplayer/tachiko-work/pull/201)
- [#176 ecosystem Epic](https://github.com/nurockplayer/tachiko-work/issues/176)
- [#184 cold-start decision](https://github.com/nurockplayer/tachiko-work/issues/184)
- [Executable E9 probe](../../../probes/issue-185-git-share-use.mjs)
- [Archify publication-ordering source](https://github.com/tt-a1i/archify/commit/b36d79fdbc3aec3728744341485a7e79f03c0071)
- Research handoffs: [#177](https://github.com/nurockplayer/tachiko-work/issues/177),
  [#178](https://github.com/nurockplayer/tachiko-work/issues/178),
  [#179](https://github.com/nurockplayer/tachiko-work/issues/179),
  [#181](https://github.com/nurockplayer/tachiko-work/issues/181), and
  [#135](https://github.com/nurockplayer/tachiko-work/issues/135)

Reproduction from a repository checkout with the current CLI built:

```sh
cargo build -p tachiko-cli --locked
TACHIKO_BIN="$PWD/target/debug/tachiko" \
  node docs/research/probes/issue-185-git-share-use.mjs
bash scripts/docs-consistency-check.sh
git diff --check
```

## Downstream observations

- 2026-08-31: initial probe passed on `main@c3b5ad2a`; Project Steward A/B/C/D
  interpretation and any follow-up authority remain pending.
- 2026-08-31: post-PR review corrections made the source tuple, repository
  inputs, and Git commit environment exact and reproducible; the 08:00 JST
  Steward watch confirmed the bounded hypothesis remained plausible and kept
  final interpretation pending exact-head gates.
