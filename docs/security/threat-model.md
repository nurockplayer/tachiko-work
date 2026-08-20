# Threat Model

## Security Philosophy

Tachiko Work handles documents, structured data, computation, plugins, and AI operations.

Security must protect:

- user data
- document integrity
- execution safety
- supply chain trust

## Untrusted Inputs

The following should be treated as untrusted:

- imported Office files
- external plugins
- AI generated operations
- network collaboration events

## Security Boundaries

External input:

```
Import / Plugin / Agent
        |
        v
Validation Boundary
        |
        v
Semantic Core
```

## Core Principles

- Never execute unvalidated transformations
- Preserve user ownership of data
- Make security behavior observable
- Prefer explicit permissions
