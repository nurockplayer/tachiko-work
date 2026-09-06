# Game Runtime Projections and Hot Reload

Status: **Accepted Direction**

Decision issue: [#332](https://github.com/nurockplayer/tachiko-work/issues/332)

## Why this belongs in Tachiko Work

Game development is Tachiko Work's first proving ground because structured game content often lives in spreadsheets while code, review, validation, and release workflows live elsewhere.

The long-term opportunity is larger than replacing Excel as the place where designers type balance values. Tachiko's semantic model can become the authoritative source for structured game content while a lightweight runtime projection makes validated meaning directly consumable by a running game.

This extends the existing semantic-first thesis rather than creating a second game engine.

## Product direction

Tachiko Work should support publishing validated, versioned semantic snapshots that external runtimes can load through replaceable adapters.

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

### Tachiko Work

Owns semantic authoring, stable identity, typing, formulas, validation, dependency/impact analysis, versioning, review, and publication of runtime-ready content.

### Tachiko Runtime and adapters

Own lightweight loading of validated snapshots, resident typed access, revision/change observation, and integration with the consuming application's lifecycle.

The exact snapshot format, transport, SDK surface, plugin ABI, invalidation model, compatibility negotiation, and rollback protocol are intentionally not frozen by this vision document.

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
Tachiko Work
    |
    | validate / publish
    v
Versioned semantic snapshot
    |
    v
Lightweight Tachiko Runtime / adapter
    |
    v
Running game or application
```

Runtime access should be local/resident after loading. A release should be able to consume a pinned, validated snapshot/package without requiring the authoring application or a remote Tachiko service to remain available.

This preserves deterministic execution, offline development and release options, reproducibility, and clear failure boundaries.

## Architectural consequence

Game-engine integration is a first-class product direction but remains an **adapter/runtime projection**, not a reason to move rendering, physics, scene ownership, or engine-specific behavior into Tachiko's semantic kernel.

The stable core should continue to own meaning. Engine integrations should remain replaceable and should generalize only after real consumers create evidence for a shared contract.

A useful decision test is:

> If the consuming engine already knows how to execute this kind of gameplay content, can this semantic change be validated, published, and observed without recompiling the engine/application?

When the answer should reasonably be yes, Tachiko Work should make that workflow possible.
