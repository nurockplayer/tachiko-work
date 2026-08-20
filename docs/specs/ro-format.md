# .ro Format Specification

## Purpose

`.ro` is the portable Tachiko Work document package format.

It is not the canonical semantic model itself. It is a serialization of the semantic model designed for:

- sharing
- backup
- transport
- archival
- application interoperability

## Design Principle

The semantic model is the source of truth.

`.ro` is a representation.

For v0.1, `.ro` is the shipped storage representation used by CLI validation,
calculation, diff, merge, and export flows.

## Requirements

A valid `.ro` package should provide:

- deterministic serialization
- integrity verification
- version metadata
- forward compatibility
- portable assets
- schema information

## Relationship With .roproj

ADR-0003 defines `.roproj` as a future dual representation.

v0.1 status:

- `.ro` is implemented and stable.
- `.roproj` is not implemented in this phase.
- Deterministic `.ro` ↔ `.roproj` round-trips are not yet available.

## Future Considerations

Possible implementation:

- ZIP based container
- manifest
- checksums
- schema registry
- embedded assets
