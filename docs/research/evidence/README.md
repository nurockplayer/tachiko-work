# Research Evidence Capture v0

Research Evidence Capture preserves a small, durable reconstruction record for
material AI-assisted research and decision episodes. It is for evidence that
would be hard to recover later: the snapshot, question, source context that
was actually available, recommendation or result, human intervention,
corrections, disposition, and observed consequences.

This directory complements the repository's
[knowledge-authority policy](../../governance/knowledge-authority.md) and
[decision-traceability protocol](../../governance/decision-traceability.md).
An episode is research/history evidence, not a source of product authority. It
does not outrank the Product Constitution, Accepted ADRs or policies, or
normative specifications; linking to an artifact records provenance, not
promotion.

## When to capture

Create an episode only when the event is materially useful for studying
cognition, authority, or research quality. Typical triggers are durable
architecture or product decisions, expensive-to-reverse contracts materially
shaped by AI, human corrections or overrides, authority-interpretation
failures, conceptual failures discovered during implementation, and formal
experiments or benchmarks.

Do not create episodes for routine implementation, refactors, linting or
formatting, ordinary bug fixes, routine CI/test work, or low-value agent
activity.

## Workflow

The material Research or Decision Issue remains the durable owner. Near the
research or decision point, create one episode using
[schema-v0.md](schema-v0.md), link rather than copy durable Issue, PR, ADR,
specification, test, fixture, and evidence-bundle records, and keep the
episode readable without private chat history. Prospective captures should be
made before a recommendation is promoted into Accepted authority when that is
practical.

Every v0 episode, including a formal experiment or benchmark, must reference
one material Research or Decision Issue as that durable owner; v0 defines no
issue-less episode representation.

Record only context that durable evidence proves was supplied. Use the literal
value `unknown` for unavailable metadata; never infer a provider, model,
deployment, prompt, configuration, or historical context. Append downstream
observations chronologically when they are actually observed. Do not revise a
historical snapshot or recommendation to make later history look cleaner.

## Privacy and safety

Do not store raw private chat transcripts, secrets, credentials, hidden
prompts, or irrelevant conversation history. For formal experiments, preserve
or durably link reproducibility-critical prompts, context, configuration,
results, scoring, hashes, and fixtures where legally and practically
available.

## v0 evolution

This is a small repository documentation convention, not a Tachiko product
schema or compatibility promise. It has no CI merge gate and adds no mandatory
Issue or PR-template fields. After roughly 10–20 material episodes, review the
observed records before changing fields or taxonomy; do not pre-freeze a larger
ontology.

## Episodes

- [#104 — context dogfood benchmark](episodes/2026-08/issue-104-context-dogfood.md)
- [#33 — semantic analysis query](episodes/2026-08/issue-33-semantic-analysis-query.md)
