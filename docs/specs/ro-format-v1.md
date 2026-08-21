# Tachiko Work .ro Format v1

Decision state: Provisional

Implementation state: Implemented in the v0.1 Developer MVP

## Authority note

This document describes the current deterministic `.ro` JSON contract used by the v0.1 CLI. It is executable implementation evidence and the baseline being hardened, but its current identifier, encoding, and ordering choices must not be assumed to be permanent ecosystem invariants merely because they are already implemented.

Core & Format Hardening issues #21, #25, #37, #38, and #40 own the work of deciding which parts of this baseline should become durable public contracts before broader adoption.

ADR-0003 remains the accepted representation direction: `.roproj` is the target canonical editable/source materialization and `.ro` is the portable artifact. The current direct `.ro` persistence path is an implementation stage, not a superseding architecture decision.

## Purpose

The `.ro` format is the current single-file serialized representation of a Tachiko Work semantic document and the intended portable artifact in the longer-term ADR-0003 architecture.

It is not the primary semantic model. It is a serialized view of the model.

## Goals

- Portable sharing
- Deterministic serialization
- Long-term archival direction
- Integrity-verification direction
- Cross-platform compatibility

## Principles

The format stores semantic meaning rather than historical application behavior.

Legacy formats such as DOCX and XLSX are imported through adapters and never become the internal truth.

## Accepted relationship with .roproj

`.ro` and `.roproj` represent the same logical semantic work under ADR-0003.

- `.ro` is optimized for portable file handling.
- `.roproj` is optimized for canonical editable/Git-native workflows.

ADR-0003 defines this relationship. `.roproj` is not part of the implemented version-1 storage contract yet, so v0.1 operates directly on `.ro` files.

## MVP Encoding

Version 1 is a canonical UTF-8 JSON document. JSON is deliberately used for the
developer MVP so the semantic contract can be tested before the longer-term
portable/project representation work is complete. The current file envelope
contains, in this order:

```text
format_version
id
title
schemas
entities
```

Schema, entity, and field maps are currently ordered lexicographically by their
serialized identifiers. Writers use two-space indentation and exactly one
trailing newline. Equivalent supported v0.1 semantic documents therefore
produce identical bytes regardless of construction order.

Current v0.1 document, schema, entity, and field identifiers use the grammar
`[a-z0-9][a-z0-9_-]*`. The CLI uses `entity.field` as a stable path within this
implemented workflow.

These facts describe v0.1 behavior. #21 must determine durable semantic identity
and naming rules before current human-readable identifiers or field paths are
promoted into expensive-to-reverse public identity contracts.

Readers reject unknown document fields, malformed JSON, invalid semantic
content, and unsupported format versions. Version `1` readers do not guess how
to interpret later versions.

## Compatibility Rule

`format_version` is currently a required positive integer. The durable version
envelope and unsupported-version policy are being hardened in #37.

A future incompatible format requires an explicit version/migration contract;
compatibility behavior belongs in the storage/migration layer and must not leak
historical representation rules into the semantic core.

## Hardening areas

- semantic identity and naming: #21
- runtime model versus storage DTOs and migration: #25
- version envelope: #37
- canonical value encoding and ordering: #38
- deterministic golden/negative fixtures: #40
- `.roproj` physical layout: #41
- future `.ro` package profile: #43
