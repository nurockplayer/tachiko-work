# Tachiko Work .ro Format v1

## Purpose

The `.ro` format is the portable package representation of a Tachiko Work semantic document.

It is not the primary semantic model. It is a serialized view of the model.

## Goals

- Portable sharing
- Deterministic serialization
- Long-term archival
- Integrity verification
- Cross-platform compatibility

## Principles

The format stores semantic meaning rather than historical application behavior.

Legacy formats such as DOCX and XLSX are imported through adapters and never become the internal truth.

## Planned relationship with .roproj

`.ro` and `.roproj` represent the same logical document.

- `.ro` is optimized for users.
- `.roproj` is optimized for Git workflows.

ADR-0003 proposes that both round-trip deterministically. `.roproj` is not part
of the implemented version-1 storage contract.

## MVP Encoding

Version 1 is a canonical UTF-8 JSON document. JSON is deliberately used for the
developer MVP so the semantic contract can be tested before choosing a future
portable container. The file envelope contains, in this order:

```text
format_version
id
title
schemas
entities
```

Schema, entity, and field maps are ordered lexicographically by stable semantic
identifier. Writers use two-space indentation and exactly one trailing newline.
Equivalent semantic documents therefore produce identical bytes regardless of
their construction order.

Document, schema, entity, and field identifiers use the grammar
`[a-z0-9][a-z0-9_-]*`. In particular, `.` is reserved as the separator in the
stable `entity.field` path shared by CLI, Git review, and AI operations.

Readers reject unknown document fields, malformed JSON, invalid semantic
content, and unsupported format versions. Version `1` readers do not guess how
to interpret later versions.

## Compatibility Rule

`format_version` is a required positive integer. A future incompatible format
introduces a new version plus an explicit migration into the then-current
semantic model. Compatibility behavior belongs in the storage/migration layer;
it must not leak historical representation rules into the semantic core.

## Future Areas

- manifest schema
- versioning
- migrations
- signatures
- embedded assets
