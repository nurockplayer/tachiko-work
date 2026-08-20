# Runtime Export JSON v1

## Purpose

Runtime export materializes a validated Tachiko Work document into plain JSON
for game code and other downstream tools. It is generated output, never the
semantic source of truth.

## Envelope

Version 1 contains these keys in canonical order:

```text
format_version
document_id
title
entities
```

Each entity contains its schema identifier and a sorted field map. Stored
numbers, text, and booleans become JSON scalars; references become
`{"reference": "entity_id"}`; formulas become their calculated numeric value.
The file uses two-space indentation and one trailing newline.

## Compatibility boundary

Runtime export versioning is independent from `.ro` storage versioning. Either
contract may evolve without forcing the other to change. Consumers must check
`format_version` and reject versions they do not support.
