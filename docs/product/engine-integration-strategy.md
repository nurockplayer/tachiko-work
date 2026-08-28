# Engine Integration Strategy

Decision state: The Game Studio Beta host-extension boundary is Accepted under
[ADR-0028](../decisions/ADR-0028-game-engine-host-extension-boundary.md).
Concrete integrations remain unimplemented and separately scoped.

## Goal

Connect Tachiko Work data with game engines without making the engine the source of truth.

Current release status: engine integrations are future host adapters, not
implemented CLI features or semantic authorities.

## Boundary

Unity, Unreal Engine, and Godot adapters consume or request changes through the
existing Semantic API and shared semantic runtime. They cannot redefine
canonical semantic state or bypass canonical validation, operation gates,
authorization, or approval.

Writing engine projects or assets, controlling an editor, building, and
deploying are separate host effects. Semantic authority does not grant those
effects, and host authority does not grant semantic authority.

## Unity

Possible integration:

- importer
- generated assets
- validation before build
- editor tooling

## Unreal Engine

Possible integration:

- generated data assets
- JSON/structured data pipeline
- build validation

## Godot

Possible integration:

- resource generation
- project data synchronization

## M04 non-goals

No public plugin ABI, scripting runtime, WASM plugin sandbox, marketplace,
distribution/signing system, or compatibility promise is stabilized for M04.
The possible integration shapes above remain product hypotheses, not selected
implementation designs.
