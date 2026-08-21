# Migration Framework Strategy

Decision state: Accepted direction; concrete mappings and adapters are Hypothesis / Open Question

Implementation state: Broad legacy migration framework not implemented in v0.1

Decision owners: #14, #18, #34

## Authority note

The project has accepted the principle that legacy compatibility belongs at explicit boundaries rather than becoming the semantic core, and that adoption should support progressive migration rather than a forced replacement event.

This document records that direction. It does not claim that XLSX, DOCX, VBA, ODF, or other source-format semantics have already been mapped or that lossless conversion is always possible.

Concrete interoperability policy, format-specific conversion semantics, fidelity reporting, and AI-assisted migration remain focused research/decision work.

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
