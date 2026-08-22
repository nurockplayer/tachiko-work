# ADR-0018: Bound formulas and deterministic finite binary64 semantics

## Status

Accepted

Decision issue: [#24](https://github.com/nurockplayer/tachiko-work/issues/24)

## Context

ADR-0014 accepts Tachiko Work's bounded formula authoring language. ADR-0015
requires human addresses to bind to stable semantic identities. ADR-0016 places
formula parsing, binding, dependency analysis, and evaluation in the pure,
portable formula engine over semantic-core contracts. ADR-0017 prevents storage
or a serializer from inventing numeric meaning.

The v0.1 implementation is strong evidence but not yet a durable contract:

- semantic and expression numbers are Rust `f64` values;
- formula references are structured but still use mutable name-like IDs;
- finite values and literals are required, and non-finite results fail;
- arithmetic uses Rust `f64`; division rejects both signs of zero;
- `min` and `max` call Rust `f64::min` and `f64::max`;
- dependencies are collected while evaluating into ordered runtime maps; and
- every calculation is a fresh whole-document traversal.

Several details are therefore still inherited from a host or library. Rust
documents that `f64::min` and `f64::max` may return either operand when `+0`
and `-0` compare equal. Stored-value equality and calculated-impact comparison
also treat signed zero differently today. Serde JSON currently emits some, but
not all, RFC 8785 number spellings. Those accidents cannot freeze persisted
meaning or native/WASM behavior.

The current language and game-balance workflows need approximate real-number
calculation. They do not provide evidence for exact money, arbitrary integers,
or decimal arithmetic now. The stable-core rule therefore favors a precise
finite-binary64 contract while reserving exact numeric types as later explicit
types.

Authoritative external evidence supports that boundary:

- [IEEE 754-2019](https://standards.ieee.org/ieee/754/6210/) defines binary
  floating-point formats and operations whose results are determined by the
  inputs, operation sequence, and destination format.
- [Rust RFC 3514](https://rust-lang.github.io/rfcs/3514-float-semantics.html)
  specifies primitive `f64` operations as IEEE 754 operations using
  `roundTiesToEven`, without flush-to-zero, apart from NaN payload caveats that
  disappear when non-finite values are outside the semantic domain.
- [WebAssembly numerics](https://webassembly.github.io/spec/core/exec/numerics.html)
  uses IEEE binary64, round-to-nearest ties-to-even, and gradual underflow; its
  remaining floating-point nondeterminism concerns NaN payloads.
- [RFC 8785 §3.2.2.3](https://www.rfc-editor.org/rfc/rfc8785.html#section-3.2.2.3)
  and ECMAScript `Number::toString` provide an interoperable shortest-roundtrip
  spelling and published binary64 vectors without requiring Tachiko to adopt
  the rest of JCS.

The detailed evidence and reproducible native/WASM probe are recorded in
[`../research/2026-08-22-formula-numeric-semantics.md`](../research/2026-08-22-formula-numeric-semantics.md).

## Decision

The following sections are Accepted invariants. Milestone 02 mechanisms and
deferrals are classified separately.

### 1. Formula source is an authoring projection; bound AST is meaning

Formula authoring follows this pipeline:

```text
source text
  -> bounded parse
  -> unbound address AST
  -> bind and type-check in one document snapshot
  -> typed bound AST
  -> static dependency extraction
  -> graph validation
  -> evaluation
```

The source syntax and resource limits remain exactly those accepted by
ADR-0014. An unbound reference contains a human `EntityKey + FieldKey` address.
Binding resolves that address to a typed `EntityId + FieldId` reference under
ADR-0015. The semantic formula stores the bound AST; original spelling and
mutable keys are not durable bound identity.

Renaming a key or label does not rebind or rewrite formula meaning. Rendering
formula source after rename looks up the current human address for the same
stable IDs. Canonical authoring projection is a partial operation: every
rendered address must be proven, in the current document snapshot, to resolve
uniquely back to the same `EntityId + FieldId`. If any bound target has no such
address, the formatter and `explain` return a typed projection failure whose
semantic subject is the set of unresolved stable-ID pairs and return no formula
source text.

Moving or changing physical storage does not affect binding. A deleted or
type-incompatible bound target is diagnosed by stable ID and never silently
retargeted to an object that happens to reuse the old human key. A reused key,
last-known source spelling, tombstone/display token, truncation, or another
synthetic address cannot make the canonical authoring projection succeed and
does not become valid formula meaning. #23 may define non-copyable presentation
for temporarily invalid state without weakening this rule. Replacement and
migration require explicit semantic operations.

Any key or name rename that can change a formula's human projection preflights
every affected formula against the complete candidate snapshot before the
rename is published. A successful candidate must project canonically within
ADR-0014's 4,096 UTF-8-byte canonical-authoring limit. If any affected
projection would be 4,097 bytes or more, the complete rename candidate is
rejected atomically: no key/name change is published, and stable IDs plus bound
ASTs remain unchanged. Exactly 4,096 bytes is valid. This shared semantic
preflight is not optional client behavior; clients may neither truncate the
projection nor apply different renderability rules. If an affected formula is
already temporarily unprojectable under a separately permitted #23 editing
policy, the partial projection returns its typed stable-ID failure rather than
fabricating text; that policy does not turn a tombstone into authoring source.

Parser-produced and directly supplied typed expressions pass the same accepted
ADR-0014 structural limits before recursive work. Binding failure produces no
new bound formula candidate.

### 2. Milestone 02 formulas have one result type and no coercion

The Milestone 02 formula type surface is `Number` only. Literals, arithmetic,
`min`, `max`, and referenced formula fields all produce `Number`.

Formula binding requires a schema-numeric target. Text, Boolean, reference,
missing, null-like, container, and error-like values are not coerced to Number.
There is no spreadsheet-style truthiness, string parsing, blank-to-zero, or
implicit scalar/container conversion. Missing input is an evaluation failure,
not a numeric zero or null value.

### 3. Number is finite binary64 with one semantic zero

Milestone 02 `Number` is the set of finite IEEE 754 binary64 values, quotiented
so that IEEE `-0` and `+0` are one semantic value represented canonically as
positive zero.

Every semantic ingress and every operation result:

1. rejects NaN and positive or negative infinity; then
2. normalizes any zero encoding to positive zero.

NaN and infinities are not formula values, stored Number values, or bound
literal values. A finite operation that produces a non-finite IEEE result has a
typed `NonFiniteResult` evaluation failure. Non-finite values never flow to a
later expression as numbers.

An integer-looking Number literal is still binary64. Milestone 02 does not
promise arbitrary-integer precision or impose ECMAScript's safe-integer range
as a separate validity rule. Beyond `2^53`, adjacent mathematical integers may
convert to the same Number; source decimal spelling is not preserved as numeric
meaning. Exact counts or identifiers require a future explicit type rather than
an implicit exception inside Number.

### 4. Literal conversion and arithmetic are correctly rounded binary64

A finite decimal/scientific source literal denotes the nearest binary64 value
to the exact mathematical decimal value, using `roundTiesToEven`. Conversion
that would produce an infinity is a literal failure. Conversion that produces a
subnormal or zero is valid and then follows zero normalization.

Each accepted `+`, `-`, `*`, and `/` AST node:

- evaluates its left child, then its right child;
- performs exactly the named IEEE binary64 operation at a binary64 destination;
- uses round-to-nearest, ties-to-even;
- does not reassociate expressions or contract multiply-plus-add into a fused
  operation; and
- validates and normalizes that node's result before its parent evaluates.

Subnormal inputs and finite subnormal results are valid. Gradual underflow is
required; flush-to-zero and abrupt underflow are forbidden. Underflow that
correctly rounds to either signed zero becomes semantic positive zero.

Overflow to infinity, or any other non-finite result, is `NonFiniteResult`.
The engine does not expose IEEE status flags or alternate rounding modes.

Division evaluates both operands in the normal left-to-right order and checks
the normalized divisor before division. Either IEEE zero sign produces
`DivisionByZero`, including `0 / 0`; it is not allowed to become NaN first.

### 5. Equality, ordering, `min`, and `max` use normalized finite values

Number-value equality, calculated-impact comparison, cached-result comparison,
and numeric conformance checks compare the exact normalized binary64 value.
Equivalent zero encodings are equal; all other finite values are equal exactly
when their binary64 encodings are equal after normalization. No tolerance or
epsilon is implicit.

Formula-definition equality is different: it compares the complete typed bound
AST structurally, including node kinds, tree shape, stable references, and
normalized literal bits. A definition change is never a no-op merely because
its current calculated Number is unchanged. Semantic diff reports the bound-AST
change separately from any calculated impact, and cache identity covers both
the bound definition and dependency outcomes.

The formula language still has no general comparison operators. `min` and
`max` evaluate left then right, operate on normalized finite operands using
ordinary numeric order, and normalize the result. Consequently:

```text
min(0, -0) == 0
max(0, -0) == 0
```

This rule is semantic and must not delegate the equal-zero case to Rust,
WebAssembly, JavaScript, or CPU-specific `min`/`max` behavior.

There is no aggregation operator in the accepted language. Future reductions
must specify a stable input order and may not reassociate floating-point
operations silently.

### 6. Persisted Number spelling reuses the RFC 8785 numeric primitive only

A representation version that adopts this semantic Number contract writes each
Number as the RFC 8785 §3.2.2.3 / ECMAScript `Number::toString` radix-10,
Note-2-enhanced shortest-roundtrip token.

This selects the numeric primitive only. Tachiko's canonical JSON profile keeps
its own whitespace, member-order, collection-order, and Unicode rules and MUST
NOT claim full RFC 8785/JCS conformance.

Before numeric conversion, the adopting representation/version profile applies
its explicit resource limits to the complete input and the JSON number token.
Exceeding either limit is a structural representation-limit failure, not a
Number literal, overflow, underflow, or non-finite semantic failure. The
concrete input/token limits are version/profile mechanisms owned by that
representation and #74; they are deliberately not part of this ADR's semantic
`Number` invariant.

For every syntactically valid RFC 8259 JSON number token admitted by those
limits, the reader interprets the token as an exact decimal, converts it to
nearest binary64 using `roundTiesToEven`, rejects a conversion to infinity,
accepts finite subnormals or correctly rounded underflow to zero, and
normalizes either zero sign. Alternate valid spellings such as `1`, `1.0`, and
`1e0` therefore decode to one Number and re-encode as one canonical token. A
writer never rounds an already-semantic Number.

The persisted token must round-trip through that conversion to the same
normalized semantic value. The canonical writer emits `0` for semantic zero.
RFC 8785 Appendix B supplies the initial required edge vectors, including:

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

The accepted ADR-0014 formula-source formatter remains a separate authoring
projection. Once this Number contract is adopted, source `-0` binds as semantic
positive zero and the canonical authoring projection for that literal is `0`;
shortest round-trip is judged by normalized semantic value, not a discarded raw
zero sign. Other UI, explain, diff, or source text need not reuse persisted JSON
spelling unless their own contract says so; none may redefine numeric meaning.

Legacy direct-`.ro/v1` bytes remain immutable compatibility history. Explicit
migration into a representation adopting this ADR maps legacy negative zero to
semantic positive zero and emits the new version's canonical token.

Merely decoding or opening legacy v1 does not apply this ADR. An operation that
requires the current semantic engine first creates an explicit, version-labelled
migration candidate; it may remain in memory, but it is not a silent read-time
reinterpretation and does not replace durable source without an explicit save.
A version-scoped legacy evaluator, if retained, follows frozen v1 compatibility
behavior and does not claim ADR-0018 conformance.

### 7. Dependencies are extracted statically from bound AST

The formula value node is the stable `(EntityId, FieldId)` pair of a numeric
field instance. A directed dependency edge is:

```text
formula value node -> referenced value node
```

Dependency extraction walks the bound AST without evaluating it, deduplicates
repeated references, and returns a deterministic set. The reverse-dependency
index is derived from those edges. Both indexes are runtime state under
ADR-0015 and must be rebuildable from canonical bound formulas.

A self-edge or strongly connected component with more than one node is a cycle.
Semantic cycle classification uses the induced formula-node subgraph after
excluding nodes with structural or binding/type/stale-target failure. Graph
validation identifies the complete mathematical set of cyclic SCC member sets
in that eligible subgraph. The extracted dependency set still retains every
statically available edge, including edges incident to an excluded node.

Eligible SCC membership is semantic; iteration order and a particular cycle
path/witness are not. Diagnostic ordering and witness presentation remain owned
by #23, so #24 does not invent an ordering for opaque IDs or freeze an SCC
traversal algorithm.

### 8. Full recomputation is the correctness oracle

The deterministic full-recompute oracle:

1. validates every bound expression and assigns node-keyed structural or
   binding/type/stale-target failures;
2. extracts every statically available dependency edge and finds cyclic SCCs
   among formula nodes without an earlier failure;
3. assigns a cycle failure to each cyclic member;
4. processes every remaining formula dependency-first: if a directly required
   value failed, assigns dependency failure; otherwise evaluates using this
   ADR's numeric rules; and
5. produces either a complete normalized value map or a value-node-keyed map of
   semantic failures.

Independent acyclic components may be evaluated even when another component
fails, but a failed calculation publishes no partial `CalculationState` and no
partial semantic mutation. Cycle outcomes contain the SCC member set;
dependency failures identify the directly required failed value-node set. Within
one expression, local evaluation and failure selection remain left-to-right.
Because nodes are pure, the order among independent ready nodes is not semantic.

Each value node has at most one primary semantic failure. Assignment follows
this precedence and a node assigned at an earlier phase is ineligible for later
failure assignment:

```text
structural
  -> binding / type / stale target
  -> cycle
  -> failed dependency
  -> local evaluation
```

Within the binding/dependency phases, the semantic subject is the set of all
directly discovered failing targets for that phase. Within local expression
evaluation, the first failure follows left-to-right AST order. #23 maps this
primary semantic outcome into client diagnostics and may present additional
non-primary evidence without changing formula meaning.

Incremental recomputation is permitted only when observationally equivalent to
that oracle. A dirty root is every value node whose definition, existence,
declared type, bound-expression validity, normalized value, or local failure
outcome may have changed. This includes target deletion/restoration,
missing-to-present transitions, schema numeric-type changes, and any added,
removed, rebound, or replaced formula.

Updating graph edges uses both the old and new graph; the dirty set is those
roots plus the union of their old-graph and new-graph reverse transitive
dependent closures. Dirty formulas recompute dependency-first. An operation
whose impact cannot be classified safely discards the affected derived state
and falls back to the full oracle.

The observations compared with the oracle are normalized result bits, complete
bound-AST definition equality, failure class and stable subject-ID sets,
extracted dependency sets, and cyclic SCC membership in the eligible induced
subgraph. Map/set iteration order, a selected cycle witness, message wording,
severity, UI location, and temporary invalid editing policy remain non-semantic
#23 concerns.

Derived calculation caches are not persisted meaning. A valid cache key must
cover the formula semantic-contract version, bound expression, and normalized
dependency outcomes; a cache must be invalidated by the same old/new reverse
closure. Exact hashes, index containers, residency, and eviction are
Milestone 02 Provisional mechanisms.

### 9. Deterministic functions are pure and capability-free

The current arithmetic operators and two-argument `min`/`max` are pure. Formula
evaluation has no filesystem, network, locale, wall clock, environment,
process, UI, thread-order, or implicit-randomness capability, as already
required by ADR-0016.

Future deterministic functions must define versioned typed semantics and obey
the same purity, numeric, dependency, and portability rules. External or impure
computation, if ever justified, is outside the deterministic formula core and
requires a separate future contract. This ADR does not design a plugin runtime.

### 10. Failures are layered but are not formula values

The formula boundary distinguishes these semantic phases:

- parse/structural failure: syntax or accepted resource limit;
- binding/type failure: unresolved, ambiguous, missing, or wrong-type address;
- graph failure: dependency cycle;
- evaluation failure: missing input, division by zero, non-finite result, or a
  failed dependency.

An error is not coerced into Number, null, or a spreadsheet-style error value.
A dependent formula fails deterministically when its required dependency has no
Number result; its semantic subject is the set of directly required failed value
nodes. Formula-local arithmetic still evaluates left-to-right and returns its
first local failure. The stable cross-client code envelope, ordering,
presentation, severity, source spans, broader aggregation policy, and temporary-
invalid-state behavior remain owned by #23.

### 11. Native and WASM must pass the same executed conformance corpus

Supported native targets and `wasm32-unknown-unknown` execute the same semantic
fixtures. Compile-only WASM evidence is insufficient. Conformance compares
normalized value bits, failure classes, dependency/cycle results, and canonical
numeric bytes.

Parallel evaluation may be introduced only for independent ready nodes. It may
not change expression operation order, reassociate arithmetic, select a failure
by thread completion, or publish results in completion order. Milestone 02 may
remain serial; a parallel scheduler is Deferred.

## Milestone 02 Provisional mechanisms

The invariant is the finite-binary64 behavior, not a Rust public field layout.
The following remain replaceable behind it:

- a private Rust `f64` or validating Number newtype as the in-memory carrier;
- the parser, binder, dependency-index, SCC, and topological-sort algorithms;
- ordered-map/set container choices;
- a conforming formatter implementation such as `ryu-js` or an independently
  verified equivalent;
- representation-version input and number-token resource-limit values, which
  #74 must make explicit before conversion begins;
- cache hash/fingerprint algorithms, residency, and eviction; and
- serial evaluation and the exact internal error types.

No new crate or dependency edge is required. The bound-reference minimum stays
in semantic-core and the formula engine owns parsing, binding, analysis, and
evaluation under ADR-0016.

## Deferred

- exact integer, decimal, fixed-point, rational, money, unit, date/time, and
  quantity types;
- general comparisons, conditionals, lookups, collections, aggregations,
  user-defined functions, and spreadsheet compatibility;
- configurable rounding modes or precision contexts;
- public extension ABI, sandbox, registry, or impure/external computation;
- parallel scheduling;
- the #23 cross-client diagnostic and temporary-invalid-state policy;
- #26 worker, IPC/FFI, state-residency, and host durability mechanics; and
- persisted dependency indexes or materialized formula results.

Adding an exact numeric type later requires a distinct tagged semantic type,
explicit mixed-type/conversion rules, a representation-version decision, and
native/WASM conformance. It does not silently change `Number`.

## Rejected alternatives

### Add exact integer and decimal/money types now

Rejected for Milestone 02. The current language and use cases require one
approximate numeric domain; they do not establish scale, overflow, rounding,
currency, or mixed-type requirements for exact types. Adding them now would
freeze speculative coercion and storage rules.

### Preserve IEEE signed zero as distinct semantic meaning

Rejected. Current authoring has no sign-of-zero operation, while distinct zero
leaks into `min/max`, diff, division, and bytes. Normalization preserves useful
arithmetic behavior and removes a cross-target/runtime trap.

### Permit NaN or infinities as values

Rejected. Their propagation, ordering, payload, comparison, JSON, and
cross-target behavior would expand the language without a product use case.
Typed failure is deterministic and matches current validation intent.

### Let Rust, JavaScript, Serde, or a selected serializer define behavior

Rejected. Hosts and libraries are implementations. Rust `min/max` signed-zero
ties and current Serde exponent thresholds already demonstrate observable
differences from the desired contract.

### Adopt decimal arithmetic instead of binary64

Rejected for the current type. It would change shipped numeric results, has no
native WebAssembly numeric primitive, and introduces precision/scale rules not
justified by current formulas. A future explicit Decimal type remains open.

### Adopt full RFC 8785 JCS

Rejected by ADR-0017 and unchanged here. Only the tested number-serialization
primitive is reused; Tachiko retains Git-readable structural formatting and
semantic collection ordering.

### Persist formula source or dependency indexes as semantic authority

Rejected. Source uses mutable addresses, and indexes are derived. Bound stable
IDs and the bound AST preserve meaning and can rebuild every dependency index.

### Make incremental or parallel evaluation the reference behavior

Rejected. Full recomputation is simpler to specify and test. Optimization is
allowed only behind observational equivalence.

## Consequences

Positive:

- ADR-0017, #74, and #40 can freeze numeric bytes from semantic authority rather
  than serializer behavior;
- native and WASM share a standards-backed domain whose excluded NaN behavior
  removes the principal floating-point nondeterminism;
- rename no longer rewrites or retargets formula meaning;
- signed-zero normalization makes equality, diff, zero checks, `min/max`, and
  persistence consistent;
- exact future types retain a clean, explicit migration and conversion boundary;
- static dependencies support cycle diagnosis and incremental correctness
  independently of successful evaluation.

Costs and migration work:

- current name-bound `FieldRef`, direct semantic Serde types, and rename rewrite
  behavior require the ADR-0015/#70 migration;
- every Number ingress and operation boundary must validate and normalize;
- current `f64::min/max` delegation and diff's signed-zero asymmetry must change;
- the canonical writer needs an ECMAScript-compatible number formatter rather
  than relying on ordinary Rust Display or current Serde output;
- canonical authoring projection can fail for stale/unresolvable stable
  references, and rename operations must preflight every affected projection
  against the Accepted 4,096-byte limit;
- representation readers must bound input/token resources before exact-decimal
  conversion without turning those limits into Number meaning;
- executed WASM conformance becomes a release gate for the portable formula
  contract; and
- current evaluation-coupled dependency collection must become a static bound-
  AST pass before incremental recomputation can be trusted; and
- the current first-error DFS/cycle witness must become an internal complete SCC
  and node-failure outcome before incremental/full conformance can be measured.

## Acceptance and downstream implementation ownership

Final promotion review verified:

1. the numeric table and RFC 8785 vectors in
   `docs/specs/formula-engine-spec.md` are unambiguous;
2. ADR-0018 does not amend ADR-0014's language, ADR-0015's identity,
   ADR-0016's crate DAG, or ADR-0017's storage ownership;
3. the native/WASM probe and authoritative standards evidence are reproducible;
4. #23/#26/#17 concerns remain explicitly deferred; and
5. #74/#40 can implement and test canonical numeric bytes without another
   semantic choice;
6. stale or unresolvable stable references yield a typed no-source projection
   failure, including when a human key has been reused;
7. rename conformance accepts an affected 4,096-byte canonical projection and
   atomically rejects the equivalent 4,097-byte candidate without changing
   stable IDs or bound ASTs; and
8. representation/profile resource-limit failures occur before exact-decimal
   conversion, while every admitted token retains the complete conversion
   contract in section 6.

With this ADR Accepted, downstream ownership is:

- #74 implements validating/normalizing Number conversion and the
  representation-specific ECMAScript number primitive;
- #40 owns canonical-number golden, negative, round-trip, and cross-target
  storage fixtures;
- formula-engine implementation owns native/WASM arithmetic and incremental-
  versus-full recomputation fixtures;
- #70 integrates stable-ID-bound formula migration;
- #23 maps the semantic failure classes into the cross-client diagnostic
  envelope; and
- #24's decision work is complete; the promotion PR closes it when merged.

## Related

- ADR-0014, ADR-0015, ADR-0016, ADR-0017
- Issues #17, #23, #24, #26, #40, #70, #74
- `docs/specs/formula-engine-spec.md`
- `docs/specs/canonical-json-profile.md`
- `docs/research/2026-08-22-formula-numeric-semantics.md`
