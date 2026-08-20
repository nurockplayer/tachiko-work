# Security Model

## Principles

Tachiko Work handles user documents, structured data, and AI operations.

Security must be designed into the platform.

## Areas

- document permissions
- plugin isolation
- AI action approval
- migration sandboxing
- untrusted file handling
- cryptographic integrity

## Legacy Import

External formats should be treated as untrusted input.

Importers must validate data before entering the semantic core.
