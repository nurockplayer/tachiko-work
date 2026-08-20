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

## Relationship with .roproj

`.ro` and `.roproj` represent the same logical document.

- `.ro` is optimized for users.
- `.roproj` is optimized for Git workflows.

Both must round-trip deterministically.

## Future Areas

- manifest schema
- versioning
- migrations
- signatures
- embedded assets
