# Founding Open Questions

Status: Living research map

State legend: Open / Research queued / Researched / Provisional / Accepted / Deferred

This document captures questions that remain after consolidating Tachiko Work's founding motivation, mission, and design principles.

The purpose is not to force immediate answers. It is to distinguish questions that require founder judgment from questions that can be delegated to research and implementation work.

## State ledger (founding execution map)

- `#21 Semantic identity` — **Researched**. UUIDv7 provisional recommendation is in place; it is awaiting ADR review and should not be re-researched.
- `#24 + #38 Formula architecture` — **Research queued** (formula/numeric semantics + canonical encoding).
- `#25 + #37 Storage format evolution` — **Research queued** (storage DTO + format/version evolution).
- `.ro` / `.roproj` / ODF interoperability direction — **Accepted** under ADR-0003 for the source/artifact and interoperability-boundary direction; standards/primitives reuse remains implementation research.
- Japan enterprise, migration, plugins, CRDT/conformance — **Deferred** future research (not Milestone 02 blockers).

## Questions that require founder judgment

These are product-identity questions. AI can research and frame tradeoffs, but should not silently decide them from implementation convenience.

### 1. How public should the anti-lock-in origin story be?

State: Open

The private founding motivation is intentionally direct: reduce dependence on Microsoft Office and OOXML.

Open question:

Should public-facing project messaging explicitly frame Microsoft Office / OOXML as the system Tachiko Work wants to liberate users from, or should that language remain mainly in project history while the public mission emphasizes user ownership, openness, migration, and interoperability?

This is a messaging decision, not an architecture blocker.

### 2. What does "user ownership" outrank when tradeoffs become real?

State: Open

The constitution treats user ownership as foundational.

Future conflicts may include:

- perfect round-trip compatibility versus clean modern semantics
- cloud convenience versus local/offline control
- opaque optimization versus inspectability
- proprietary integration value versus open portability

Founder judgment is needed on whether any of these can legitimately override the ownership principle and under what conditions.

### 3. What is the long-term product boundary?

State: Open

Current strategy starts with game-development structured data and can later expand toward broader productivity workflows.

Open question:

Is the long-term ambition truly a general foundation for digital work, including document/spreadsheet-class workflows, or should Tachiko Work intentionally remain strongest around technical, structured, computational, and versioned work even if that means never replacing many traditional Office use cases?

This does not block the current milestone, but it will eventually affect UI, ecosystem, and market strategy.

### 4. What must always remain open?

State: Open

The project strongly values open formats and an open ecosystem, while licensing and commercial sustainability are still being worked out.

Founder-level clarification will eventually be needed for the minimum permanent openness guarantee, for example:

- format/specification availability
- semantic-core availability
- reference implementation availability
- extension protocol availability
- migration/export guarantees

This should be resolved through governance and licensing decisions rather than assumed by implementation.

### 5. What would count as mission success?

State: Open

Possible interpretations include:

- a game studio can stop treating Excel as the source of truth for balance data
- an organization can migrate a fragile spreadsheet workflow into an understandable, versioned system
- a user can preserve and manipulate important work without the original application
- an ecosystem can build independent tools around Tachiko Work formats and semantics

A small set of concrete mission-level success tests would help future product decisions stay grounded.

## Questions that should be researched rather than answered from instinct

These should normally be delegated to focused ChatGPT / Deep Research sessions and converted into ADRs, specs, or implementation-ready issues when needed.

### A. `.ro` / `.roproj` / ODF / standards research

Decision state: Accepted

Research state: Deferred

ADR-0003 already sets the accepted direction:

- `project.roproj/` as canonical editable/source representation
- `project.ro` as portable artifact
- ODF and other existing formats as interoperability boundaries where useful
- Git as storage infrastructure, not UI

Research focus should remain on reuse and maturity of standards/primitives, not on re-deciding the accepted source/artifact relationship.

### B. Historical and standards research

State: Open

- Recover and accurately document the relevant COSCUP 2017 Italo Vignoli talk, including the arguments that materially influenced the project.
- Map OOXML, ODF, CSV, JSON, Markdown, Arrow/Parquet, SQLite, and other reusable standards against Tachiko Work's actual semantic requirements.
- Identify where existing standards are sufficient and where custom Tachiko semantics are justified.

### C. Migration research

State: Deferred

- What real Excel/Office dependencies make progressive migration difficult?
- Which dependencies can be statically detected and explained?
- How should semantic conversion reports represent unsupported or behavior-changing constructs?
- What level of round-trip fidelity is valuable before it begins to contaminate the core?

### D. Japanese enterprise workflow research

State: Deferred

- Which spreadsheet-driven workflows are most affected by person-dependent operations, macros, undocumented procedures, and weak change history?
- What migration wedge creates value without requiring an Office replacement project?
- Which audit, explainability, approval, and interoperability requirements are specific to Japanese organizations?

### E. Game-development workflow research

State: Open

- Which balance/configuration workflows currently justify moving the source of truth out of Excel or Google Sheets?
- What must integrate with Unity, Unreal, Godot, build pipelines, localization, and live-ops systems?
- Which team sizes and roles feel the pain strongly enough to adopt an early tool?

### F. Core architecture research

State: Open

Milestone 02 should focus research effort on the small set of expensive-to-reverse invariants:

- semantic identity
- canonical serialization
- schema evolution and compatibility semantics
- formula semantics and dependency identity
- storage DTO versus semantic-core boundaries
- core versus extension authority
- native versus WASM runtime boundaries

The goal is not to maximize architecture. The goal is to freeze only what becomes expensive to migrate once documents, Git history, plugins, and external tools depend on it.

### G. Extension architecture research

State: Deferred

Investigate mature small-core ecosystems such as Flask and other extensible platforms to determine:

- what belongs in the stable kernel
- what extension contracts must exist early
- how extensions declare capabilities and permissions
- how schema/format evolution interacts with third-party extensions
- how to avoid plugin APIs accidentally freezing internal implementation details

### H. Format and naming research

State: Open

- Keep `.ro` naming provisional until release identity is intentionally frozen.
- Evaluate the still-open physical layout, packaging, sharding, and canonicalization details within ADR-0003's accepted `.roproj` source / `.ro` portable-artifact relationship.
- Determine which packaging and canonicalization details are true format invariants versus replaceable implementation decisions.

## Working rule

State: Accepted

When a new issue appears, first classify it:

1. Founder / product identity judgment
2. Research question
3. ADR-level architectural decision
4. Specification problem
5. Implementation task
6. Validation / evidence task

Do not ask the founder to become an expert in categories 2 through 6 merely because an issue exists.

The preferred flow is:

Canonical context
→ focused research
→ recommendation
→ ADR/spec when needed
→ implementation-ready issue
→ Codex implementation
→ tests/review
→ revision from evidence

Only return to founder judgment when the answer changes product identity, foundational principles, irreversible ecosystem commitments, or material business/governance posture.
