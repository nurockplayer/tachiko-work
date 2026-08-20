# Frontend and Backend Boundary

## Principle

The UI is a projection layer, not the owner of document meaning.

## Architecture

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

## Rust Core Responsibilities

- document state
- calculations
- validation
- persistence
- transformations
- AI operations

## Why

A single semantic core allows:

- web application
- desktop application
- mobile clients
- AI agents
- CLI tools

to share the same behavior.
