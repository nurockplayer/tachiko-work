# Plugin System Direction

Decision state: Accepted extensibility direction; concrete runtime is Open Question

Implementation state: No public plugin runtime in v0.1

Decision owner: #17

## Authority note

Tachiko Work has accepted the product/architecture principle that the ecosystem should be able to extend the platform without requiring every domain capability to enter the core.

This does not mean that any specific plugin language, runtime, sandbox, package format, registry, API surface, or compatibility promise is already Accepted.

#17 owns those concrete decisions. Current core/API work should preserve plausible extension seams without prematurely freezing a public plugin ABI.

## Goal

Tachiko Work should become an extensible platform, not a closed monolithic application.

## Candidate plugin categories

Potential extensions include:

- import/export adapters
- renderers/views
- formula or rule extensions
- AI integrations
- validators
- system/data integrations
- game-engine connectors

These categories are planning hypotheses, not a v0.1 plugin manifest.

## Examples

Game development may eventually include:

- Unity integration
- Unreal integration
- Godot integration

Knowledge and business work may eventually include:

- GitHub integrations
- documentation systems
- data connectors
- private company workflow adapters

## Accepted design principles

The semantic core should remain small and stable.

Extensions should operate through explicit semantic/capability boundaries rather than redefine canonical state or bypass validation.

Domain-specific behavior should be able to evolve outside the core where practical.

## Open questions

#17 must determine, using security and ecosystem evidence:

- public semantic API stability required for plugins;
- scripting/WASM/native extension tiers, if any;
- capability/sandbox model;
- private versus public distribution;
- version/compatibility guarantees;
- whether validators/formula extensions must be deterministic or pure;
- supply-chain, signing, update, disable, and audit behavior;
- how legacy Office/VBA workflows migrate into semantic rules/extensions without recreating VBA as the foundational abstraction.

## Long-term vision

A healthy ecosystem around Tachiko Work is more valuable than a large monolithic application, but the project should freeze extension contracts only after the semantic foundation is mature enough to support them responsibly.
