# Frontend and Backend Boundary

## Principle

The UI is a projection layer, not the owner of document meaning.

## Architecture

In this document, `Rust Core` means the shared Rust semantic/application
runtime, not the `semantic-core` crate alone. The proposed crate ownership and
dependency direction are recorded in
[ADR-0016](../decisions/ADR-0016-milestone-02-rust-crate-layering.md); detailed
native/WASM host binding remains owned by #26.

```
React / Desktop UI
        |
Semantic API
        |
Rust Core
        |
Document Model
```

## Frontend Responsibilities

- rendering
- interaction
- visual editing
- user workflows
- accessibility

## Shared Rust Runtime Responsibilities

- document state
- calculations
- validation
- persistence transformation through an explicit storage/host boundary
- transformations
- AI operations

## Why

A single semantic authority and shared workspace engine allow:

- web application
- desktop application
- mobile clients
- AI agents
- CLI tools

to share the same behavior.
