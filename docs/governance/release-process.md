# Release Process

## Goals

Releases should prioritize:

- stability
- reproducibility
- compatibility
- trust

## Release Layers

### Core

Semantic model, storage, computation engines.

### Applications

Desktop, web, and collaboration products.

### Integrations

Unity, Unreal, GitHub, and ecosystem plugins.

## Requirements

A release should include:

- changelog
- migration notes
- validation results
- security review when required

## Compatibility

Legacy format support should be handled through adapters and migration tooling rather than changing the semantic core.
