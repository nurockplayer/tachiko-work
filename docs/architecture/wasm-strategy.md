# WASM Strategy

## Principle

The semantic core should be portable across environments.

## Targets

- Desktop application
- Web application
- Embedded tools
- AI execution environments

## Architecture

```
Rust Semantic Core
        |
        +-- Native binary
        +-- WASM module
        +-- Server runtime
```

## Benefits

- Shared business logic
- Consistent document behavior
- Reduced platform divergence
- Easier plugin ecosystem

WASM is an execution target, not the semantic foundation itself.
