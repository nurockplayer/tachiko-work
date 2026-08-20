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

## Requirements

A valid `.ro` package should provide:

- deterministic serialization
- integrity verification
- version metadata
- forward compatibility
- portable assets
- schema information

## Relationship With .roproj

`.ro` and `.roproj` represent the same logical document.

`.ro` is optimized for humans moving documents.

`.roproj` is optimized for Git workflows.

Conversion must be deterministic:

```
.roproj -> .ro -> .roproj
```

must preserve semantic meaning.

## Future Considerations

Possible implementation:

- ZIP based container
- manifest
- checksums
- schema registry
- embedded assets
