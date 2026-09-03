# Runtime Export JSON v2

Decision state: Provisional implemented derived-output contract

Implementation state: Current CLI runtime export

## Purpose

Runtime export materializes one validated, calculated stable-identity document
as ergonomic game/runtime JSON. It is generated output, never semantic source
truth, and its version namespace is independent from direct `.ro` and
`.roproj`.

## Envelope and projection

Version 2 emits fixed top-level order:

```text
format_version
document_id
title
entities
```

- `format_version` is `2`.
- `document_id` is the opaque semantic `DocumentId` token.
- `entities` is ordered and keyed by current human `EntityKey`.
- Each entity records its current human `SchemaKey` and an ordered field map
  keyed by current human `FieldKey`.
- Stored numeric/text/Boolean inputs become JSON scalars. Stored Date inputs
  become their canonical `YYYY-MM-DD` JSON string; the export remains a
  derived ergonomic projection and is not a lossless semantic persistence
  format.
- Formula fields become their normalized calculated Number.
- Entity references become `{"reference": "current_entity_key"}`.

The current key projection is deliberate for runtime ergonomics. The artifact
is not a lossless semantic persistence format and must not be used to reconstruct
stable entity/field identity; use `direct-ro/v2` for that purpose.

## Encoding and compatibility

Output uses UTF-8 JSON, two-space indentation, deterministic map ordering, and
one trailing LF. Existing outputs are never overwritten. Consumers must check
`format_version` and reject unknown versions.

Version 2 is required because ADR-0015 changes document identity from a
name-like address to an opaque stable ID and ADR-0018 normalizes Number meaning.
Those are observable changes that cannot be published under frozen
runtime-export/v1.
