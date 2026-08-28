# Plugin System Direction

Decision state: Mixed. The Game Studio Beta game-engine host extension
boundary is Accepted under
[ADR-0028](../decisions/ADR-0028-game-engine-host-extension-boundary.md).
A general plugin platform remains Deferred.

Implementation state: No plugin or game-engine integration is implemented by
this decision.

## Accepted Game Studio Beta boundary

Unity, Unreal Engine, and Godot integrations are host extensions/adapters, not
semantic authorities. Their semantic reads and mutation requests use the
existing Semantic API/runtime/authorization authority. Engine and deployment
effects remain separately authorized host effects.

ADR-0028 adds no new Semantic API, runtime, authorization, or approval law; it
applies ADR-0020, ADR-0022, and ADR-0026 to the game-engine boundary.

## Deferred platform contracts

Game Studio Beta does not stabilize a public plugin ABI, manifest/package
contract, scripting runtime, WASM sandbox, native loading model, marketplace,
registry, signing scheme, distribution mechanism, or compatibility/support
promise.

Later policy decisions are split by roadmap need:

- [#134](https://github.com/nurockplayer/tachiko-work/issues/134) owns legacy
  Office/VBA migration and private enterprise extension policy in M07.
- [#135](https://github.com/nurockplayer/tachiko-work/issues/135) owns public
  plugin distribution/marketplace, signing, compatibility, lifecycle, and
  support policy in M08.

Potential extension categories beyond that narrow boundary remain planning
hypotheses, not a public manifest or compatibility contract.

General plugin ABI, runtime, loading, and sandbox mechanics remain Deferred
without a decision owner assigned by ADR-0028, #134, or #135.
