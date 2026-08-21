# Tachiko Work Knowledge Base

This directory is the canonical knowledge base for Tachiko Work.

Use this page as the front door. Do not infer authority from file age, directory name, implementation status, or confident wording alone.

## Start here

Read these first when you need to understand or change the project:

1. [`vision/product-constitution.md`](vision/product-constitution.md) — highest-level product constraints.
2. [`vision/design-principles.md`](vision/design-principles.md) and [`vision/mission.md`](vision/mission.md) — durable product principles and mission.
3. [`governance/knowledge-authority.md`](governance/knowledge-authority.md) — how Principle, Accepted, Provisional, Hypothesis, Open Question, and Superseded states work.
4. [`governance/canonical-reconciliation-register.md`](governance/canonical-reconciliation-register.md) — current authority and supersession map across ADRs, architecture, specifications, and Issues.
5. [`decisions/README.md`](decisions/README.md) — ADR index and current decision status.
6. [`architecture/README.md`](architecture/README.md) — architecture map by subsystem and maturity.
7. [`specs/README.md`](specs/README.md) — specification map and which contract to read for each topic.

The current repository planning horizon is `02 · Core & Format Hardening`. GitHub Issues own unresolved decisions and implementation work; their existence does not by itself make a direction Accepted.

## Authority in one minute

When two artifacts appear to disagree, use the full policy in [`governance/knowledge-authority.md`](governance/knowledge-authority.md). The practical reading order is:

1. Product Constitution and foundational Principles.
2. Accepted ADRs and accepted governance policies.
3. Normative specifications, respecting their explicit decision state.
4. Architecture and product documents.
5. Shipped implementation, tests, and README as implementation evidence.
6. Decision Issues and implementation Issues.
7. Research and discussion history.

Implementation state and decision state are separate. An implemented behavior may still be Provisional; an Accepted direction may still be unimplemented.

## Find information by intent

| If you need to know... | Go to | Role |
| --- | --- | --- |
| Why Tachiko Work exists | [`vision/`](vision/) | Constitution, mission, durable principles |
| What has been explicitly decided | [`decisions/`](decisions/) | ADRs and decision history |
| How the system is structured | [`architecture/`](architecture/) | Current and target architecture |
| What an implementable contract says | [`specs/`](specs/) | Format, API, formula, collaboration, migration, and runtime contracts |
| What the product should do | [`product/`](product/) | Product strategy, wedge, user stories, and roadmap material |
| How decisions, releases, licensing, and contribution work | [`governance/`](governance/) | Authority, governance, release, licensing |
| Engineering practices and implementation guidance | [`engineering/`](engineering/) | Engineering workflow and technical guidance |
| Security model and security work | [`security/`](security/) | Security policy and design material |
| Market, competitor, standards, and technical evidence | [`research/`](research/) | Evidence and hypotheses, not automatic authority |
| Business and commercialization analysis | [`business/`](business/) | Business research and strategy |
| Why past discussions happened | [`discussions/`](discussions/) | Historical context only unless promoted elsewhere |
| Experimental workflows or helper material | [`superpowers/`](superpowers/) | Supporting project workflows; check authority before relying on them |

## AI-agent loading rule

ChatGPT, Deep Research, Codex, and other agents should load context in this order:

1. Product Constitution and Design Principles.
2. Knowledge Authority policy.
3. Relevant Accepted ADRs and policies.
4. Relevant specifications and architecture documents, including their decision state.
5. Target Issue and Decision Logs.
6. Current implementation and tests when shipped behavior matters.

Agents must not silently promote an Open Question or Provisional implementation detail into a durable public invariant.

## Documentation maintenance rules

- Preserve decision history. Superseded records stay available and point to their replacement when possible.
- Prefer indexes and explicit authority notes over duplicating the same contract in multiple documents.
- A file under `specs/` is not automatically Accepted.
- A README describes current behavior but does not silently supersede an Accepted ADR.
- When a document becomes a compatibility or navigation stub, say so explicitly rather than leaving stale normative wording in place.
- If a real contradiction cannot be resolved from existing authority, open focused decision work instead of improvising a winner in prose.
