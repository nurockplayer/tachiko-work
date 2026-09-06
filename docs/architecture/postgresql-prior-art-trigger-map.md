# PostgreSQL Prior-Art Lessons and Reopening Triggers

Authority: Explanatory architecture synthesis

Decision state: Derived from completed research in GitHub Issue [#220](https://github.com/nurockplayer/tachiko-work/issues/220) and the founder framing in [#234](https://github.com/nurockplayer/tachiko-work/issues/234). This document does not independently create Accepted architecture authority.

Recorded: 2026-09-06 JST

## Purpose

Issue #220 completed the broad PostgreSQL prior-art study. The project should not keep researching PostgreSQL simply because more PostgreSQL mechanisms exist.

This note preserves the small set of lessons that survived that study and records the concrete conditions that should justify reopening a PostgreSQL comparison later. The goal is to avoid both outcomes:

```text
forget the research
→ rediscover the same conclusions later
```

and:

```text
admire PostgreSQL
→ copy database machinery before Tachiko needs it
```

Use PostgreSQL again only when a real Tachiko pressure matches a problem family PostgreSQL has already pressure-tested.

## Durable lessons

### 1. Prefer small typed owner-specific contracts over one universal registry

PostgreSQL does not represent every capability in one generic `everything` table. Types, functions, operators, casts, aggregates, dependencies, extensions, and access methods have different typed ownership and lifecycle rules.

The transferable Tachiko principle is:

> Independently evolving semantic facts should have explicit identity and a narrow owner. Do not centralize them merely because a registry can technically describe them.

Current implications:

- keep canonical semantic types closed until product pressure earns another type;
- keep type meaning, operations, conversions, validation, parsing, presentation, and extension lifecycle separately owned;
- prefer compiler-checked closed contracts when the vocabulary is intentionally closed;
- introduce discovery metadata only when independent evolution actually requires discovery.

The canonical `Date` work in #265–#267 is a useful example: Tachiko added one real semantic scalar and its laws without introducing `SemanticTypeId`, `SemanticCatalog`, `CREATE TYPE`, dynamic values, or generic operator overloading.

### 2. Lifecycle becomes first-class only when something truly installs and evolves independently

PostgreSQL extensions make identity, version, dependency, installation, upgrade, ownership, and removal agree as one lifecycle.

The transferable Tachiko principle is:

> If an asset can be installed, upgraded, removed, or supplied by an independently versioned provider, its lifecycle and dependency meaning must be explicit rather than inferred from files or load order.

This does **not** imply that templates, semantic assets, Skills, connectors, and executable extensions should share one package model. Their trust and execution classes may require different contracts.

### 3. Separate semantic intent from replaceable execution strategy

PostgreSQL keeps a declarative request separate from the physical strategy selected to execute it.

The transferable Tachiko principle is:

> Public semantic meaning should not depend on which internal execution strategy happens to be fastest today.

Tachiko should introduce a stronger planning boundary only when at least two semantically equivalent execution strategies exist and measured pressure makes choosing among them valuable. Until then, direct explicit Rust code is preferable to a speculative planner framework.

### 4. Explanations should be projections over deterministic evidence

PostgreSQL `EXPLAIN` is useful because internal decisions can be inspected as structured facts rather than only described in prose.

The transferable Tachiko principle is already aligned with Design Principle 10:

> Important semantic decisions and effects should expose deterministic, machine-readable evidence. Human-readable or AI-generated explanation is a projection over those facts, not the authority for them.

Examples include:

- why validation rejected a publication;
- which stable subjects a semantic change affected;
- why a formula or analysis produced a result;
- why a capability is or is not applicable;
- which dependency caused an invalidation;
- which authorization scope or gate blocked an operation.

Do not create a universal `Explain` subsystem merely to make all explanations look alike. Reuse the authoritative facts owned by each workflow.

## Reopening trigger map

The broad PostgreSQL study stays closed. Reopen only the relevant mechanism when one of these conditions becomes concrete.

| Pressure observed in Tachiko | PostgreSQL mechanism worth revisiting | Reopening trigger | Existing owner / route |
| --- | --- | --- | --- |
| A client encounters semantic capability it does not implement | Catalog / capability discovery | Independent capabilities must be discovered or safely preserved without rebuilding every client | #54 and current protocol authority |
| Third-party executable behavior has install/update/remove semantics | Extension lifecycle / dependencies | At least one real independently versioned provider must own members, migrations, dependency edges, or trust state | #134 / #135 and extension authority |
| Reusable declarative assets evolve with dependencies | Dependency/lifecycle metadata | Real asset upgrades or composition require version/provenance/dependency decisions beyond copy/vendor workflows | #176 / #178 / #181 or their successors |
| One semantic operation has multiple equivalent execution strategies | Planner / executor separation | Two or more strategies are semantically equivalent and measured performance evidence makes strategy selection material | owning runtime/performance issue; do not open a planner program by analogy |
| Users/agents cannot understand a deterministic internal decision | `EXPLAIN` / observability | Existing structured evidence is insufficient to explain an actual product workflow without reconstructing rules in the client or LLM | owning semantic workflow/spec; extend its evidence first |
| Current snapshots/revisions cannot express a real concurrency requirement | MVCC/snapshot concepts | A concrete multi-client/offline scenario cannot be solved safely by current revision/merge/collaboration authority | #48–#50 lineage or current collaboration owner |
| Durability/recovery evidence proves snapshot-first persistence insufficient | WAL/recovery principles | A real crash-recovery or durability guarantee cannot be met by current snapshot/checkpoint mechanisms | current storage/history owner; study WAL as recovery prior art, not semantic history |
| Large-data storage needs multiple interchangeable physical implementations | Access methods / operator classes | At least two real persistence/index/access strategies require one conformance seam | current storage/performance owner; do not pre-stabilize a public ABI |

The issue numbers above are routing hints from the #220 synthesis, not permanent authority. If ownership has moved, follow live repository authority.

## Explicit non-triggers

Do **not** reopen PostgreSQL research merely because:

- PostgreSQL supports a feature Tachiko does not;
- a generic registry would reduce a few `match` statements;
- a new scalar type is useful;
- one execution path is slow before alternatives have been demonstrated;
- a cache exists;
- history or audit sounds similar to WAL;
- collaboration involves more than one user;
- SQL, MVCC, OIDs, operator classes, or extension APIs are technically interesting.

A mechanism must answer a demonstrated Tachiko problem and be smaller than the complexity it introduces.

## Deliberate non-transfers

The completed #220 study found no present justification for:

- a universal `SemanticCatalog`;
- PostgreSQL-style open runtime `CREATE TYPE` semantics;
- SQL or relational storage as Tachiko semantic authority;
- a general cost-based optimizer/planner;
- database MVCC as the semantic collaboration model;
- WAL or event sourcing as Tachiko semantic history;
- a public storage/access-method ABI;
- PostgreSQL OIDs or native extension ABI;
- forcing presentation, validation, storage, execution, and extension metadata into one type descriptor.

These are not forbidden forever. They require new evidence and the normal Decision/ADR path before becoming durable Tachiko architecture.

## Working rule

When a future architecture problem appears, ask:

```text
1. What concrete Tachiko pressure exists?
2. Which current owner already governs its semantic contract?
3. Has PostgreSQL solved the same problem family at a comparable boundary?
4. What is the underlying principle, separate from database-specific machinery?
5. What is the smallest Tachiko-native adaptation?
6. What evidence would falsify the adaptation?
```

The default answer after #220 is therefore:

> **Do not continue broad PostgreSQL research. Keep it as a prior-art toolbox and reopen one mechanism only when Tachiko earns the question.**

## Related

- [PostgreSQL-like Engine, Spreadsheet-first Human Interface](postgresql-like-engine-spreadsheet-interface.md)
- [Design Principles](../vision/design-principles.md), especially 9, 10, and 12
- [Semantic Core Rationale](semantic-core-rationale.md)
- Issue [#220](https://github.com/nurockplayer/tachiko-work/issues/220) — completed PostgreSQL architecture research
- Issue [#234](https://github.com/nurockplayer/tachiko-work/issues/234) — PostgreSQL-like engine / spreadsheet-first product framing
- Issues #265–#267 — concrete semantic-type pressure test and canonical `Date` implementation
