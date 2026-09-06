# Game Runtime Projections and Hot Reload

Status: **Accepted Direction**

Decision issue: [#332](https://github.com/nurockplayer/tachiko-work/issues/332)

Existing boundary: [ADR-0028 — Game Engine Host Extension Boundary](../decisions/ADR-0028-game-engine-host-extension-boundary.md)

## Why this belongs in Tachiko Work

Game development is Tachiko Work's first proving ground because structured game content often lives in spreadsheets while code, review, validation, and release workflows live elsewhere.

The long-term opportunity is larger than replacing Excel as the place where designers type balance values. Tachiko's semantic model can remain the authoritative source for structured game content while lightweight runtime projections make validated meaning directly consumable by running games.

This extends the existing semantic-first thesis rather than creating a second game engine or a second semantic authority.

## Product direction

Tachiko Work should support publishing validated, versioned semantic state through the platform-owned Semantic API so external runtimes can consume runtime projections through replaceable adapters.

A snapshot, package, cache, or other runtime materialization is downstream of that Semantic API authority. It may make runtime access local and efficient, but it must not become a competing semantic contract or bypass the platform-owned API boundary established by ADR-0028 and its Semantic API authority.

For game development, compatible content changes should be able to **Hot Reload during development without recompiling the game executable** when the consuming runtime already implements the required gameplay capability.

Typical candidates include:

- balance and tuning values;
- units, characters, items, resources, and equipment;
- economy and pricing parameters;
- progression and technology data;
- board, world, spawn, and encounter metadata;
- quests, events, and other data-driven definitions;
- localization and presentation references;
- new instances of semantic types whose runtime behavior already exists.

The same direction can later support production content publication where appropriate, but development Hot Reload is the first architectural pressure to satisfy. Production live-service delivery, remote configuration, and rollback policy require their own decisions and evidence.

## Responsibility boundary

The intended split is:

### Tachiko Work and Semantic API

The semantic core owns meaning: stable identity, typing, formulas, validation, dependency/impact analysis, versioning, and authoritative semantic state.

The platform-owned Semantic API remains the governed boundary through which external runtimes consume or mutate semantic functionality. Runtime publication must preserve that authority rather than introduce an engine-specific or snapshot-specific source of truth.

### Tachiko Runtime and adapters

Runtime and engine adapters consume and re-expose versioned semantic state through the Semantic API boundary defined by ADR-0028. They may materialize validated state into local/resident projections, provide typed host-language access, observe revision/change notifications delivered through the governed boundary, and integrate updates with the consuming application's lifecycle.

They must not bypass the Semantic API, mutate authoritative semantic state directly, or make an engine-native representation, local snapshot, or transport payload canonical.

The exact snapshot/materialization format, transport, SDK surface, plugin ABI, invalidation model, compatibility negotiation, change-notification mechanism, and rollback protocol are intentionally not frozen by this vision document.

### Game engine or application runtime

Owns execution concerns such as rendering, physics, audio, input, animation, platform integration, networking implementation, and gameplay capabilities that are not already represented by supported data-driven primitives.

Godot, Unity, Web runtimes, and future clients are consumers of Tachiko semantics. None of them defines the semantic model.

## What Hot Reload means

Hot Reload applies when a semantic change is compatible with capabilities the running application already understands.

Examples:

- `sword.damage = 42 -> 45` should not require recompilation;
- changing an economy multiplier or progression threshold should not require recompilation;
- adding another weapon or encounter that uses already-supported primitives may not require recompilation;
- changing localized text or other validated content should not require recompilation.

Hot Reload does **not** mean arbitrary runtime code generation.

If a game does not implement swimming, fluid physics, a new renderer feature, or a new gameplay primitive, changing Tachiko data cannot manufacture that capability. The application or engine must implement the capability first; Tachiko can then configure and compose it within the supported semantic contract.

## Runtime independence and safety

A running game must not depend on the Tachiko Work authoring UI or a cloud service in its frame loop.

The target architecture is conceptually:

```text
Tachiko semantic core / Work
    |
    | validate / publish semantics
    v
Platform-owned Semantic API
    |
    | derive / materialize runtime state
    v
Versioned runtime projection / snapshot
    |
    v
Lightweight Tachiko Runtime / adapter
    |
    v
Running game or application
```

Runtime access should be local/resident after loading. A release should be able to consume a pinned, validated projection/package without requiring the authoring application or a remote Tachiko service to remain available.

Local runtime materialization is an execution optimization and release boundary, not a second semantic authority. This preserves deterministic execution, offline development and release options, reproducibility, and clear failure boundaries while keeping semantic ownership governed by the existing API contract.

## Architectural consequence

Game-engine integration is a first-class product direction but remains an **adapter/runtime projection**, not a reason to move rendering, physics, scene ownership, or engine-specific behavior into Tachiko's semantic kernel.

This vision is subordinate to ADR-0028's Accepted host-extension boundary: engine adapters remain replaceable client/plugin-boundary consumers of the platform-owned Semantic API. Hot Reload should evolve as a consumer of that boundary, not as a parallel semantic surface.

The stable core should continue to own meaning. Engine integrations should generalize only after real consumers create evidence for a shared contract.

A useful decision test is:

> If the consuming engine already knows how to execute this kind of gameplay content, can this semantic change be validated, published through the governed semantic boundary, and observed without recompiling the engine/application?

When the answer should reasonably be yes, Tachiko Work should make that workflow possible.
