# ADR-0028: Game-engine host extension boundary

## Status

Accepted

Decision issue: [#17](https://github.com/nurockplayer/tachiko-work/issues/17)

Related authority: ADR-0020, ADR-0022, ADR-0026, and ADR-0027

## Context

Game Studio Beta needs a durable classification for future Unity, Unreal
Engine, and Godot integrations. It does not yet have evidence for a general
plugin platform or a public ecosystem compatibility promise.

ADR-0020 already owns Semantic API behavior and its no-bypass rule. ADR-0022
already owns authoritative interactive runtime state and the host-capability
boundary. ADR-0026 already owns semantic authorization and its separation from
external effects. This decision only applies those existing boundaries to
game-engine integrations and fixes what Game Studio Beta does not stabilize.

## Decision

### 1. Game-engine integrations are host extensions/adapters

Unity, Unreal Engine, and Godot integrations are host extensions/adapters at a
composition boundary. They are not semantic authorities.

Engine project data, editor state, generated assets, and engine-specific
representations MUST NOT become an independently authoritative Tachiko
semantic model. An adapter may translate between Tachiko results and an engine
boundary, but it may not redefine canonical semantic meaning.

### 2. Semantic behavior uses the existing authority

An engine adapter that reads or requests mutation of Tachiko semantic state
MUST use the existing ADR-0020 Semantic API contract hosted under ADR-0022.
It MUST NOT use engine files, storage materializations, a runtime bridge, or
lower internal crates as an alternate semantic read or mutation authority.

Canonical mutation remains subject to the existing command meaning,
validation/calculation, operation gates, authorization, and approval laws
owned by ADR-0020 and ADR-0026. This ADR adds no command, operation,
capability, scope, approval, runtime, session, or transport contract.

### 3. Engine effects remain host effects

Writing an engine project, generating or replacing engine assets, controlling
an editor, invoking a build, deploying content, or using filesystem, process,
network, or engine APIs is a host effect outside semantic authorization.

Semantic Query, Propose, Approve, or Execute authority does not grant an engine
host effect. Host-effect authority does not grant semantic authority. A future
integration must enforce both domains where one workflow crosses both.

### 4. Game Studio Beta stabilizes no general plugin platform

This decision does not stabilize a public plugin ABI, plugin manifest/package
contract, scripting runtime, WASM plugin sandbox, native plugin loading model,
marketplace or registry, signing scheme, distribution system, or public
compatibility/support promise.

Game Studio Beta may validate separately scoped engine-integration approaches,
but this ADR selects no Unity, Unreal Engine, or Godot implementation design
and authorizes no implementation.

## Deferred decisions

- [Issue #134](https://github.com/nurockplayer/tachiko-work/issues/134), in
  `07 · Migration & Enterprise Beta`, owns legacy Office/VBA migration and
  private enterprise extension policy.
- [Issue #135](https://github.com/nurockplayer/tachiko-work/issues/135), in
  `08 · Tachiko Work 1.0`, owns public plugin distribution/marketplace,
  signing, compatibility, lifecycle, and support policy.

ADR-0027 remains the general interoperability authority and resolves Issue
#14. This decision does not reopen or supersede either.

## Consequences

- Game-engine integrations can evolve outside the semantic core without
  becoming a second source of semantic truth.
- Future integration work must compose semantic and host authority explicitly.
- Game Studio Beta avoids creating a premature public plugin-platform promise.
- Private enterprise migration and the public plugin ecosystem can be decided
  at the roadmap stages that require those commitments.

## Related

- [Issue #17](https://github.com/nurockplayer/tachiko-work/issues/17)
- [Issue #14](https://github.com/nurockplayer/tachiko-work/issues/14)
- [Issue #134](https://github.com/nurockplayer/tachiko-work/issues/134)
- [Issue #135](https://github.com/nurockplayer/tachiko-work/issues/135)
- [Engine integration strategy](../product/engine-integration-strategy.md)
- [Plugin system direction](../specs/plugin-system.md)
- [Product roadmap](../product/product-roadmap.md)
