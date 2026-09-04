# ADR-0027: Open format and interoperability policy

## Status

Accepted

Decision issue: [#14](https://github.com/nurockplayer/tachiko-work/issues/14)

Amendment decision: [#275](https://github.com/nurockplayer/tachiko-work/issues/275)

Related authority: ADR-0003, ADR-0017, ADR-0023, ADR-0025, the Product Constitution, and the Design Principles

## Context

Tachiko Work exists to let users understand, preserve, version, migrate, automate, and extend their work without permanent dependence on one application vendor or one historical file format. That goal requires two things at once:

- an open, independently implementable ownership path for Tachiko-native work; and
- useful bridges to established external formats where product evidence justifies them.

Those requirements do not imply that an existing office or interchange format should define Tachiko Work's semantic core. ODF, OOXML, CSV, Markdown, notebook formats, JSON-family standards, and other ecosystems each encode useful conventions, but none is automatically an adequate ontology for typed semantic identity, references, validation, computation, revision-safe operations, and other Tachiko-specific behavior.

Since Issue #14 was opened, later Accepted decisions and production evidence have resolved the representation questions that originally motivated much of the discussion. ADR-0003 establishes `.roproj` as the canonical editable/Git-native materialization and `.ro` as a derived portable artifact. ADR-0017 establishes representation-local versioning, explicit migrations, and fail-closed decoding. ADR-0023 fixes `.roproj/v1`, and ADR-0025 fixes portable-package v1 as a deterministic package over exact canonical `.roproj/v1` bytes. Production work has implemented those boundaries and optional Git/CI composition.

The remaining need is therefore a policy for deciding when Tachiko Work should adopt, extend, wrap, or replace external standards without either reinventing mature infrastructure or allowing compatibility formats to distort the semantic model.

Issue #275 clarifies a complementary product constraint: keeping Excel and OOXML outside the semantic core does not make practical spreadsheet interoperability optional. Established Excel workflows are a first-class product interoperability target even though their historical representation and behavior remain boundary concerns.

## Decision

### 1. Reuse before invention

Tachiko Work SHOULD reuse mature standards, formats, algorithms, encodings, identifiers, container conventions, and libraries when they satisfy the required behavior without violating Accepted semantic, persistence, security, or product invariants.

A Tachiko-specific format or semantic construct is justified only when at least one concrete required capability cannot be represented or enforced adequately through an existing standard without one or more of these failures:

- changing or weakening Tachiko semantic meaning;
- making mutable presentation, paths, names, cells, or storage coordinates into semantic identity;
- preventing deterministic, versionable, or Git-reviewable behavior required by Accepted contracts;
- losing required validation, reference, computation, revision, or capability semantics;
- requiring silent data loss or misleading compatibility claims; or
- importing a legacy constraint into the stable core when it can remain an adapter concern.

Product identity, implementation convenience, aesthetic preference, or the mere absence of an exact existing product analogue is not sufficient reason to invent a new standard.

When reuse begins to require large custom extension surfaces that effectively replace the host standard's semantics, Tachiko Work SHOULD prefer an explicit Tachiko representation plus a documented adapter rather than present the result as ordinary compatibility with that standard.

### 2. Meaning outranks external representation

External formats are interoperability boundaries unless an independent Accepted decision explicitly makes one part of a Tachiko representation contract.

ODF/ODS/ODT, OOXML/XLSX/DOCX, CSV, Markdown/CommonMark, notebook formats, Quarto, JSON-LD/RO-Crate concepts, and similar ecosystems MUST NOT become semantic authority merely because Tachiko Work imports from or exports to them.

Legacy Office behavior remains a compatibility and migration concern. Importers may detect, preserve, emulate, convert, or explain legacy behavior where required, but the semantic core MUST NOT inherit historical document/cell behavior by default.

The compatibility boundary MUST NOT be interpreted as permission for weak practical interoperability. Tachiko Work MUST treat established spreadsheet workflows, especially Microsoft Excel, as a first-class product interoperability target. Import, export, migration, translation, emulation, preservation, or explanation SHOULD be provided at explicit boundaries as required by Accepted product scope and validated workflows.

Historical Tachiko Work implementation behavior is not external or architectural authority merely because it already exists. If an early Tachiko Work design materially obstructs an Accepted interoperability requirement, the project SHOULD prefer an explicit migration or supersession path over permanently institutionalizing that design, while preserving user data and durable external contracts through the normal compatibility and migration process.

### 3. ODF and common formats

ODF is an important open interoperability target, but it is not Tachiko Work's canonical storage model or semantic ontology.

ODF, Office formats, CSV, Markdown, JSON, and other common formats SHOULD be implemented as product-need-driven import, export, migration, or integration adapters. Their priority is determined by validated workflows and roadmap stage, not by a blanket requirement that every format exist in early versions.

JSON/JSONL, UTF-8, Unicode, URI/MIME conventions, ZIP, CRC-32, SHA-256, and similar mature primitives MAY also be used inside Tachiko representation profiles when an Accepted versioned specification adopts their exact role. Reusing such primitives does not make their generic data model the Tachiko semantic model.

### 4. Tachiko-native ownership path

Every supported Tachiko-native document MUST have an open ownership path that does not depend on the official Tachiko Work application or a private hosted service.

For stable public format promises, the project MUST provide enough public normative specification and conformance evidence for an independent implementation to read or otherwise preserve supported Tachiko representations according to its claimed capability class.

This requirement does not mean every Tachiko document must also be losslessly representable in ODF, OOXML, CSV, Markdown, or another second external format. Requiring universal projection into a legacy or lower-expressiveness format would allow that format to cap Tachiko semantics and would recreate the lock-in pressure this policy is meant to avoid.

External-format escape paths remain valuable and SHOULD be provided where they preserve useful user meaning and are justified by real workflows.

### 5. Fidelity must be explicit

Import, export, and migration tooling MUST NOT silently claim semantic equivalence when the target format cannot represent all relevant source meaning.

When a transformation is not exact, the user or calling tool MUST be able to distinguish loss, approximation, unsupported behavior, assumptions, or other material semantic changes from an exact transformation.

The exact fidelity ledger schema, diagnostic catalogue, per-format mapping rules, and UI are not fixed by this ADR. Those belong to the format-specific or migration work that has the necessary evidence. Later work may define categories such as exact, converted, approximate, or unsupported, but it must preserve the core rule that material loss or changed meaning is explicit and reviewable.

Round-trip fidelity is a tested capability claim, not a default promise attached to a file extension.

### 6. Versioning and migration preserve user ownership

Durable Tachiko representations MUST remain versioned under their owning representation contracts. Unsupported required semantics fail closed rather than being guessed or silently discarded.

Migration between incompatible Tachiko representation versions is explicit, testable, and does not make the latest Rust structures an implicit historical wire contract. Historical compatibility may be retained through readers and explicit migration edges as required by Accepted storage authority.

External-format adapters likewise MUST NOT treat a successful parse as proof of lossless semantic migration.

### 7. Current milestone boundary

For Game Dev Alpha, the minimum interoperability commitment is satisfied by:

- the documented and versioned canonical `.roproj/v1` ownership path;
- deterministic portable `.ro` packaging over canonical project bytes;
- ordinary filesystem/tool inspection of the canonical source representation;
- standalone deterministic validation and semantic review; and
- optional provider-neutral Git/CI interoperability that does not make Git semantic authority.

Game Dev Alpha does not require implementation of broad ODF, OOXML, CSV, Markdown, notebook, or Office/VBA adapters merely to satisfy the anti-lock-in principle.

Evidence-backed Office/ODF/CSV interoperability, gradual legacy migration, and detailed loss/fidelity tooling belong to the Migration & Enterprise Beta horizon unless an earlier concrete user workflow creates a narrower justified requirement.

Public conformance classes, broader independent implementation suites, hostile-file interoperability corpora, signatures/trust, and the final 1.0 compatibility promise remain separately owned by their existing roadmap work.

## Non-goals

This ADR does not:

- require perfect or symmetric round trips with every supported external format;
- require every Tachiko semantic construct to have a representation in ODF, OOXML, CSV, Markdown, or another legacy/interchange format;
- make ODF, Office, Git, ZIP, JSON, or another external technology semantic authority;
- define format-specific import/export mappings;
- define a universal migration or fidelity-ledger wire schema;
- authorize implementation of Office/VBA compatibility in the current milestone;
- freeze all future `.roproj`, package, adapter, or extension versions; or
- require the core project to implement every useful adapter itself.

## Consequences

Positive:

- Tachiko-native work has an anti-lock-in guarantee that does not depend on fitting into a weaker historical format.
- External standards can be adopted aggressively where they reduce cost and ecosystem friction without redefining the semantic core.
- Practical Excel/spreadsheet interoperability is protected as a product requirement without making Excel or OOXML semantic authority.
- Historical Tachiko Work implementation accidents cannot silently outrank an Accepted interoperability requirement merely because they already exist.
- New Tachiko-specific semantics require concrete architectural justification rather than product branding.
- Compatibility claims become evidence-based and explicit about loss instead of implying perfect round trips.
- Game Dev Alpha remains focused on the already-proven open source/artifact and Git interoperability path while later migration work can be driven by real user estates.

Costs:

- Some Tachiko documents may not have a lossless projection into common office/interchange formats.
- Format adapters require explicit semantic mappings and fidelity evidence instead of generic serialization.
- Independent implementation and conformance work remains a real maintenance obligation for the stable public platform promise.

## Revisit conditions

Revisit this policy if evidence shows that:

- an established external standard can represent Tachiko's required semantics sufficiently well to replace a custom representation without losing Accepted invariants;
- a major user segment requires an earlier interoperability target than the current roadmap assigns;
- independent implementation of the Tachiko-native ownership path is impractical despite public specifications and conformance work; or
- adapter maintenance becomes large enough that a different shared standard or extension strategy materially reduces ecosystem cost.

## Related

- [Issue #14](https://github.com/nurockplayer/tachiko-work/issues/14)
- [Issue #275](https://github.com/nurockplayer/tachiko-work/issues/275)
- [Issue #18](https://github.com/nurockplayer/tachiko-work/issues/18)
- [Issue #256](https://github.com/nurockplayer/tachiko-work/issues/256)
- ADR-0003
- ADR-0017
- ADR-0023
- ADR-0025
- [Product Constitution](../vision/product-constitution.md)
- [Design Principles](../vision/design-principles.md)
- [Migration framework](../specs/migration-framework.md)
- [Product roadmap](../product/product-roadmap.md)
- [Issue #34](https://github.com/nurockplayer/tachiko-work/issues/34)
- [Issue #51](https://github.com/nurockplayer/tachiko-work/issues/51)
