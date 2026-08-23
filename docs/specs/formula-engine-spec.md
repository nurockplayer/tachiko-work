# Formula Engine Specification

Decision state: Mixed — ADR-0014 authoring syntax/limits and ADR-0018
bound-formula, dependency, and deterministic binary64 rules are Accepted;
implementation mechanisms remain Provisional where marked.

Implementation state: The engine implements bounded parsing, snapshot binding
to stable IDs, typed bound ASTs, partial round-trip-proven authoring projection,
rename preflight, finite binary64 `Number` normalization, static dependency
extraction, and the complete atomic node-keyed full-recompute oracle with SCC
and failed-dependency outcomes. Incremental recomputation remains unimplemented.

Authority: ADR-0014, ADR-0015, ADR-0016, ADR-0017, and ADR-0018. Decision
record: #24.

## Purpose and scope

The formula engine turns bounded authoring text into typed semantic computation.
It is deterministic, pure, capability-free, and portable across native and
WebAssembly targets.

This specification distinguishes three kinds of statement:

- **Accepted** rules come from existing Accepted ADRs and are current authority.
- **Provisional** mechanisms remain replaceable implementation choices behind
  the Accepted semantic contract.
- **Deferred** features are outside the Milestone 02 contract.

## Accepted authoring language

ADR-0014 owns the authoring grammar. The language contains:

- finite decimal and scientific Number literals;
- bracketed references in `[entity.field]` form;
- binary `+`, `-`, `*`, and `/` with standard precedence;
- unary `+` and `-`;
- parentheses; and
- two-argument `min(left, right)` and `max(left, right)`.

Examples:

```text
min(60, [iron_sword.damage] / [iron_sword.attack_interval] + 5)
-[enemy.defense] + 2
max(0, [attacker.power] - [target.armor])
```

The parser enforces these Accepted limits before unbounded recursive work:

| Resource | Limit |
| --- | ---: |
| Source input | 4096 bytes |
| Canonical authoring text | 4096 bytes |
| Expression nodes | 256 |
| Post-desugaring depth | 64 |

Canonical authoring text is a copyable projection used by commands such as
`tachiko explain` and `tachiko formula set --expression`. It is not persisted
numeric-token authority. The 4,096-byte limit counts UTF-8 bytes, not Unicode
scalar values or display columns.

## Formula representations and pipeline

The Accepted semantic pipeline is:

```text
source text
  -> bounded parse
  -> unbound address AST
  -> binding and type checking in one document snapshot
  -> typed bound AST
  -> static dependency extraction
  -> graph validation
  -> evaluation
```

| Representation | Contains | Durable role |
| --- | --- | --- |
| Source text | Human spelling and mutable `[entity.field]` addresses | Authoring/display projection |
| Unbound AST | Parsed structure, source locations, and human addresses | Candidate awaiting binding |
| Typed bound AST | Stable `EntityId + FieldId` references and Number-typed nodes | Semantic formula meaning |
| Runtime graph/indexes | Stable value-node edges, reverse edges, order, and caches | Rebuildable derived state |

Binding resolves each human `EntityKey + FieldKey` address to the stable
`EntityId + FieldId` identity required by ADR-0015. The complete candidate is
bound against one document snapshot; a binding or type failure produces no new
bound formula.

Renaming a human key or label preserves the same bound stable IDs and therefore
does not change formula meaning. Rendering source after rename resolves the
current human address for those IDs.

Canonical authoring projection is partial. For each bound reference, the
formatter must prove in the current document snapshot that the selected human
address resolves uniquely back to the same `EntityId + FieldId`. If any target
cannot satisfy that round-trip proof, formatter and `explain` return no source
string and instead return this formula-service outcome conceptually:

```text
CanonicalAuthoringProjectionError::UnresolvableBoundReferences {
    targets: Set<(EntityId, FieldId)>
}
```

The unordered semantic subject contains every stable-ID pair that prevents the
projection. #23 owns diagnostic envelope, ordering, wording, and non-copyable
presentation; it does not turn the error into valid source. Deletion or an
incompatible schema change is diagnosed against the stable target and MUST NOT
silently retarget a new object that reuses the old address. A reused human key,
last-known source spelling, tombstone/display token, truncation, or synthetic
address cannot satisfy projection and is not formula meaning. Replacement or
migration is explicit.

### Rename projection preflight

Any key/name rename that can change formula human projections must build the
complete candidate snapshot and preflight every affected formula through the
same canonical projector before publishing the operation. For a candidate that
is otherwise valid, every affected projection must be at most 4,096 UTF-8
bytes. The check is a shared semantic/workspace operation rule, not a
client-specific formatter choice.

- exactly 4,096 bytes is admitted;
- 4,097 bytes is rejected as a canonical-authoring resource-limit failure;
- rejection is atomic: the key/name change is not published, while all stable
  IDs and every bound AST remain unchanged; and
- no client may truncate, substitute a shorter address, or accept a candidate
  that another conforming client must reject.

If a separately permitted temporary invalid state makes an affected formula
unprojectable, preflight returns the typed stable-ID projection failure above.
#23 owns whether the surrounding editing operation may retain that pre-existing
invalid state; neither outcome authorizes tombstone or last-known spelling as
copyable source.

Required rename conformance uses the same bound AST and stable IDs for both
candidates: a rename whose affected canonical projection is exactly 4,096
UTF-8 bytes succeeds, while extending the renamed ASCII key by one byte makes
the projection 4,097 bytes and rejects the complete candidate. The rejected
snapshot must compare structurally equal to the pre-rename stable IDs and bound
ASTs and must retain the original key/name.

Directly constructed semantic expressions pass the same Accepted structural
limits as parser-produced candidates.

## Type surface and conversion

The Accepted Milestone 02 formula result type is `Number` only. Literals,
references, arithmetic, `min`, and `max` must all bind as Number.

There is no implicit conversion from text, Boolean, reference, missing,
null-like, container, or error-like values. In particular, the engine has no
spreadsheet truthiness, string-to-number parsing, blank-to-zero conversion, or
implicit scalar/container conversion. A missing dependency is an evaluation
failure, not zero or null.

## Deterministic Number contract

The following rules are Accepted under ADR-0018.

| Area | Accepted rule |
| --- | --- |
| Domain | `Number` is a finite IEEE 754 binary64 value. |
| Non-finite values | NaN and positive/negative infinity are invalid at every semantic ingress and never become values. |
| Semantic zero | IEEE `-0` and `+0` are one value, represented canonically as positive zero. |
| Literals | Convert the exact decimal value to nearest binary64 with `roundTiesToEven`; an infinity result is a literal failure. |
| Arithmetic | Each AST `+`, `-`, `*`, or `/` is one binary64 operation rounded to nearest, ties to even. |
| Evaluation order | Evaluate left child before right child and validate/normalize each node result before its parent. |
| Rewriting | Do not reassociate, fuse, contract, or substitute extended-precision operations. |
| Division | A normalized zero divisor, of either IEEE sign, is `DivisionByZero`, including `0 / 0`. |
| Overflow | A non-finite result is `NonFiniteResult`; it never flows to another node. |
| Subnormal and underflow | Preserve finite subnormals and gradual underflow; flush-to-zero is forbidden. A correctly rounded zero is valid and normalized. |
| Equality | Compare exact normalized binary64 encodings. No implicit epsilon or tolerance exists. |
| `min` / `max` | Compare normalized finite operands in numeric order and normalize the selected result; the equal-zero case always yields semantic positive zero. |
| Integer-looking input | It is still binary64. There is no separate safe-integer rejection; beyond `2^53`, adjacent mathematical integers may convert to the same Number. |
| Locale | Parsing and semantic formatting are locale-independent. |

Every semantic numeric ingress and every operation result therefore performs
the same boundary operation:

1. reject NaN or infinity; and
2. if the result compares equal to zero, replace its encoding with positive
   zero.

An integer-looking Number remains binary64 and does not promise arbitrary
integer precision. Its source decimal spelling is not preserved as numeric
meaning. Exact integer, decimal, fixed-point, and money types require explicit
future types and explicit conversions; they cannot silently change the meaning
of `Number`.

### Operation sequencing and comparison

The written bound tree fixes the arithmetic sequence. For example, `(a + b) +
c` and `a + (b + c)` remain different ASTs and may produce different finite
bits. An implementation may optimize only when the observable normalized
result and failure are identical to evaluating that tree node by node.

Number equality, calculated-impact comparison, cached-result comparison, and
numeric conformance checks use exact normalized value equality.

Formula-definition equality and no-op detection instead compare the complete
typed bound AST structurally: node kind, tree shape, stable references, and
normalized literal bits. For example, `1 + 1` and `2` are different definitions
even when they currently calculate to the same Number. A dependency-changing
formula edit is likewise reported by semantic diff even when calculated impact
is unchanged. Cache identity covers both the bound definition and dependency
outcomes.

The current language has no general comparison operators. If later language
features need approximate comparison, that must be an explicit operator or
function with its own contract.

The current `min` and `max` are binary operations, not reductions. Future
aggregation must specify stable input order and may not silently reassociate
floating-point work.

## Persisted Number spelling

For a representation version that adopts ADR-0018, the Accepted canonical JSON
token is the RFC 8785 §3.2.2.3 / ECMAScript `Number::toString` radix-10,
Note-2-enhanced shortest-roundtrip spelling.

This adopts only that numeric primitive. Tachiko retains its own whitespace,
member-order, collection-order, and Unicode profile and MUST NOT claim full RFC
8785/JCS conformance.

Before conversion, the adopting representation/version profile applies its
explicit complete-input and JSON-number-token resource limits. Exceeding either
limit is a structural representation-limit failure and the token is not passed
to Number conversion. It is not a literal, overflow, underflow,
`NonFiniteResult`, or other Number semantic failure. Exact limits belong to the
representation/profile and #74, not to this formula semantic invariant.

For every syntactically valid RFC 8259 JSON number token admitted by those
limits, the reader interprets it as an exact decimal and converts it to nearest
binary64 using `roundTiesToEven`. Conversion to infinity is rejected. A finite
subnormal or correctly rounded underflow to zero is valid, and either zero sign
normalizes to positive zero. Lexical distinctions such as `1`, `1.0`, and
`1e0` are not semantic and re-encode identically. A writer never rounds an
already-semantic Number.

The token must parse through that conversion to the same normalized semantic
value. Semantic zero always writes as `0`. Initial required vectors include:

| Binary64 bits | Canonical token |
| --- | --- |
| `0000000000000000` | `0` |
| `8000000000000000` | `0` |
| `0000000000000001` | `5e-324` |
| `8000000000000001` | `-5e-324` |
| `7fefffffffffffff` | `1.7976931348623157e+308` |
| `ffefffffffffffff` | `-1.7976931348623157e+308` |
| `444b1ae4d6e2ef50` | `1e+21` |
| `3eb0c6f7a0b5ed8d` | `0.000001` |

Ordinary serializer output is not normative. The current `serde_json` version
emits `1e-6`, whereas the adopted ECMAScript rule emits `0.000001`; storage work
must use the normative corpus rather than assume dependency conformance.

Source text remains a separate ADR-0014 projection. Source `-0` binds as
semantic positive zero and canonical authoring text emits `0` for that literal;
round-trip is judged by normalized semantic value rather than a discarded raw
zero sign.

Legacy direct-`.ro/v1` bytes remain immutable historical compatibility data.
Explicit migration to a representation adopting ADR-0018 maps either legacy
zero sign to semantic positive zero and writes the new version's token.

Opening or decoding legacy v1 alone does not apply ADR-0018 semantics. An
operation requiring the current engine first creates an explicit,
version-labelled migration candidate. That candidate may be in memory, but it
does not rewrite durable source without an explicit save. Any retained legacy
evaluator is version-scoped and does not claim ADR-0018 conformance.

## Dependency graph

A formula value node is the stable `(EntityId, FieldId)` pair of one numeric
field instance. An edge points from a formula value node to a referenced value
node:

```text
formula value node -> referenced value node
```

Static dependency extraction walks the typed bound AST without evaluating it,
deduplicates repeated references, and returns a deterministic set. The reverse
index is derived from those edges. Dependency and reverse-dependency indexes are
runtime state under ADR-0015 and must be rebuildable from canonical bound
formulas.

A self-edge or strongly connected component with more than one value node is a
cycle. Semantic cycle classification uses the induced formula-node subgraph
after excluding nodes with structural or binding/type/stale-target failure.
Graph validation identifies the complete mathematical set of cyclic SCC member
sets in that eligible subgraph. Extracted dependencies still retain every
statically available edge, including edges incident to an excluded node.

Eligible SCC membership is semantic; SCC iteration order and a particular cycle
path/witness are not. #23 owns diagnostic ordering and witness presentation, so
the formula contract neither orders opaque IDs nor freezes an SCC algorithm.

## Full and incremental recomputation

Fresh full recomputation is the Accepted correctness oracle:

1. validate each bound expression and assign node-keyed structural or
   binding/type/stale-target failures;
2. extract every statically available edge and find cyclic SCCs among formula
   nodes without an earlier failure;
3. assign each cyclic member a cycle failure;
4. process every remaining formula dependency-first: assign dependency failure
   if a directly required value failed, otherwise evaluate it; and
5. return either the complete normalized value map or a stable-value-node-keyed
   semantic failure map.

Independent acyclic components may be evaluated when another component fails,
but a failed calculation publishes no partial `CalculationState` or partial
semantic mutation. Cycle outcomes contain their SCC member set. Dependency
failures contain the set of directly required failed value nodes. Within one
expression, local evaluation and failure selection remain left-to-right. The
order among independent ready nodes is not semantic because evaluation is pure.

Each value node has at most one primary semantic failure. Phase precedence is:

```text
structural
  -> binding / type / stale target
  -> cycle
  -> failed dependency
  -> local evaluation
```

Once a node receives a failure, later phases do not overwrite or add another
primary failure. Binding/dependency phases record the set of all directly
discovered failing targets for that phase. Local expression evaluation selects
the first failure in left-to-right AST order. #23 may present additional
non-primary evidence but cannot change the primary semantic outcome.

The current Provisional Rust API exposes this authority as
`calculate_complete(&Document) -> CalculationOutcome`:

- `CalculationOutcome::Complete(Calculation)` contains the complete normalized
  value map and static dependency map;
- `CalculationOutcome::Failed(CalculationFailures)` contains the complete
  stable-value-node-keyed primary failure map and every statically extracted
  dependency set, but no successful value map; and
- `calculate()` remains a compatibility convenience that projects the first
  stable-node-keyed primary failure into the historical `CalculationError`
  family. Its derived cycle witness is presentation evidence, not SCC authority.

Incremental recomputation is allowed only when observationally equivalent to
that oracle. A dirty root is every value node whose definition, existence,
declared type, bound-expression validity, normalized value, or local failure
outcome may have changed. This includes target deletion/restoration,
missing-to-present transitions, schema numeric-type changes, and added, removed,
rebound, or replaced formulas.

Edge updates consider both the old and new graph. The dirty set is those roots
plus the union of their old-graph and new-graph reverse transitive dependent
closures. Dirty formulas recompute dependency-first. An operation whose impact
cannot be classified safely discards affected derived state and falls back to
the full oracle.

Equivalence compares normalized result bits, complete bound-AST definition
equality, failure class and stable subject-ID sets, dependency sets, and cyclic
SCC membership in the eligible induced subgraph after every operation. Map/set
iteration order, a selected cycle witness, message wording, severity, UI
location, source spans, and temporary invalid editing behavior remain
non-semantic #23 concerns.

Calculated values and calculation caches are derived runtime state. A cache key
must cover the formula semantic-contract version, bound expression, and
normalized dependency outcomes. Exact hash, collection, residency, and eviction
mechanisms are Provisional. Parallel evaluation is Deferred; if added later it
must be observationally equivalent to the serial oracle.

## Function and capability boundary

Accepted Milestone 02 functions are only the pure, total-on-valid-input
two-argument `min` and `max`, subject to the failure and numeric rules above.

Parsing, binding, dependency extraction, graph validation, and evaluation are
pure and capability-free under ADR-0016. They do not read files, clocks, random
sources, networks, processes, plugins, or ambient locale. Extension functions,
impure/external computation, cross-document references, and plugin calls are
Deferred.

## Layered failures

The formula engine distinguishes these semantic layers without taking ownership
of #23's diagnostic envelope:

| Layer | Examples |
| --- | --- |
| Parse/structure | Invalid token, source/node/depth limit, non-finite literal |
| Binding/type | Missing address, ambiguous address, non-numeric target, stale stable target |
| Graph | Self-cycle or multi-node cycle |
| Evaluation | Missing dependency outcome, `DivisionByZero`, `NonFiniteResult`, dependency failure |

Canonical-authoring projection failure is a separate formula-service result,
not an evaluation failure or formula value. It carries the unresolved stable
`EntityId + FieldId` subject set and emits no source text. A rename projection
that exceeds 4,096 UTF-8 bytes is an operation/resource-limit rejection, not a
mutation of the formula definition.

Failures are not Number values and do not participate in arithmetic. A
dependency failure identifies the set of directly required failed value nodes;
formula-local arithmetic evaluates left-to-right and returns its first local
failure. #23 owns diagnostic ordering/presentation, durable codes, envelope
fields, severity, source locations, broader aggregation, and editing policy.

## Conformance and downstream ownership

The checked-in research probe executes the same 16 edge cases natively and as
`wasm32-unknown-unknown`; the typed result records and normalized bits are
byte-identical. The release gate also compiles the production semantic/formula
crates for both targets and executes one shared production-API corpus covering
normalized values, failures, dependencies/cycles, operation order, stable
binding, rename projection, and no-silent-retarget behavior. This is bounded
positive evidence, not a proof for every compiler or runtime.

Implementation and remaining ownership under Accepted ADR-0018:

- #70 implements the version-specific v2 semantic/DTO conversion, canonical
  Number writer, stable-ID binding/projection, and implementation-critical
  numeric/resource vectors without rewriting legacy direct-`.ro/v1` bytes.
- #40 owns final broad storage golden/negative conformance closure and
  independent corpus expansion.
- Formula-engine owns the implemented complete failure oracle; later
  formula-engine work owns incremental recomputation and mutation-sequence
  equivalence tests against that oracle.
- Runtime-export JSON has an independent version contract. Existing
  `runtime-export-v1` bytes/meaning remain frozen; the stable-identity transition
  deliberately bumps current output to runtime-export/v2 for normalized Number
  and opaque document identity.

Required numeric coverage includes threshold neighbors around `1e-6` and
`1e21`, smallest/largest subnormals, minimum normal, maximum finite,
round-trip-sensitive mantissas, ties-to-even, overflow, underflow, division by
both zero signs, normalized `min`/`max`, structural formula-definition diff, and
separate exact normalized calculated-impact comparison. Representation tests
must also prove that a token exactly at each declared token/input boundary
reaches syntax/semantic conversion, while a one-byte-over candidate fails as a
structural representation limit before exact-decimal conversion regardless of
the Number it would otherwise produce.

Required binding/projection coverage includes a deleted target, a missing
current address, an ambiguous/reused human address that no longer resolves to
the same stable IDs, and a valid renamed address. Every failure carries stable
IDs and emits no copyable formula. Rename coverage includes the exact 4,096-byte
and 4,097-byte cases above and proves atomic preservation of the old human key,
stable IDs, and bound AST on rejection.

## Current implementation gaps

The implementation remains evidence, not authority, where it does not yet
cover the complete Accepted recomputation contract:

- calculation always traverses the full document; `affected_by` reports a
  closure but is not an incremental evaluator;
- cross-target execution has compile/smoke evidence but not a complete
  independent language implementation of every conformance vector.

## Deferred language features

- aggregate clauses and general reductions;
- conditional expressions and comparisons;
- lookups and schema-level computed defaults;
- user-defined or extension functions;
- simulation and optimization planners;
- cross-document references;
- persisted dependency indexes or calculated-value caches;
- exact integer, decimal, fixed-point, and money types; and
- parallel evaluation.

## Related

- [ADR-0014](../decisions/ADR-0014-computational-formula-authoring.md)
- [ADR-0015](../decisions/ADR-0015-stable-semantic-identity.md)
- [ADR-0016](../decisions/ADR-0016-milestone-02-rust-crate-layering.md)
- [ADR-0017](../decisions/ADR-0017-versioned-storage-and-canonical-representation.md)
- [ADR-0018](../decisions/ADR-0018-bound-formulas-and-deterministic-binary64.md)
- [Formula numeric semantics evidence](../research/2026-08-22-formula-numeric-semantics.md)
- [Canonical JSON profile](canonical-json-profile.md)
- [#23](https://github.com/nurockplayer/tachiko-work/issues/23)
- [#24](https://github.com/nurockplayer/tachiko-work/issues/24)
- [#40](https://github.com/nurockplayer/tachiko-work/issues/40)
- [#74](https://github.com/nurockplayer/tachiko-work/issues/74)
