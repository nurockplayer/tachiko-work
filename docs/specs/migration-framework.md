# Migration Framework Strategy

Decision state: Accepted direction under ADR-0027; concrete mappings and adapters are Hypothesis / Open Question

Implementation state: Broad legacy migration framework not implemented in v0.1

Decision authority: ADR-0027; concrete mapping and migration owners: #18, #34

## Authority note

The project has accepted the principle that legacy compatibility belongs at explicit boundaries rather than becoming the semantic core, and that adoption should support progressive migration rather than a forced replacement event.

ADR-0027 now defines the repository-wide open-format/interoperability policy: reuse mature standards before invention, keep external formats at explicit adapter boundaries unless separately Accepted, require an open independently implementable Tachiko-native ownership path, and make material fidelity loss or changed meaning explicit rather than silently assuming equivalence.

This document records the migration direction beneath that policy. It does not claim that XLSX, DOCX, VBA, ODF, or other source-format semantics have already been mapped or that lossless conversion is always possible.

Format-specific conversion semantics, fidelity-ledger mechanics, and AI-assisted migration remain focused later-stage research/decision work under their existing owners.

## Principle

Legacy compatibility belongs at the boundary, not in the core.

Tachiko Work should be able to import existing formats without automatically inheriting their historical mistakes as permanent modern semantics.

## Candidate sources

Future adapters may include, where validated by product need:

- XLSX
- DOCX
- ODF/ODS/ODT
- CSV
- Markdown
- JSON
- proprietary game data formats

The existence of this list is not a promise of full round-trip fidelity.

## Directional migration flow

```text
Legacy File / Existing Workflow
    ↓
Import Adapter
    ↓
Compatibility Analysis
    ↓
Migration / Fidelity Report
    ↓
Semantic Candidate
    ↓
Deterministic Validation
    ↓
Human Review where assumptions or loss exist
    ↓
Modern Semantic Model
```

## Example research case

An Excel workbook may require detection of:

- legacy date behavior
- unsupported VBA/macros
- formulas requiring conversion
- external links or hidden dependencies
- cell-coordinate workflows whose domain meaning must be reconstructed

A future migration system should report:

- detected behavior/dependencies
- affected source objects/data
- conversion assumptions
- exact versus approximate/unsupported outcomes
- validation evidence
- recommended migration steps

The exact loss/fidelity ledger is owned by later migration work such as #34.

## Accepted migration principle

Historical compatibility is a service and an interoperability boundary.

It must not become the foundation of future data models merely because an imported file contains legacy behavior.

At the same time, progressive migration means compatibility failures and semantic changes must be explained rather than hidden or discarded.
