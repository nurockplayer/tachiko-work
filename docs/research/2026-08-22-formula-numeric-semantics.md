# Formula numeric semantics evidence record

- Date: 2026-08-22
- Decision issue: [#24 — Formula AST, binding, dependency graph, and deterministic numeric semantics](https://github.com/nurockplayer/tachiko-work/issues/24)
- Decision state: **Accepted through ADR-0018**
- Milestone scope: M02 semantic `Number`, its current operators, exact comparison/diff, and persisted JSON primitive spelling

## Recommendation

ADR-0018 accepts the following contract. This record remains supporting
decision evidence rather than independent authority.

| Area | Accepted M02 rule |
| --- | --- |
| Domain | Semantic `Number` is a finite IEEE 754 binary64 value. NaN and positive/negative Infinity are invalid inputs and cannot be semantic values. |
| Zero | Normalize every numeric input and every operation result equal to either zero sign to positive zero before it enters semantic state, comparison, dependency caches, diff, or persistence. |
| Arithmetic | Decimal-literal conversion and each AST `+`, `-`, `*`, and `/` operation are correctly rounded using `roundTiesToEven`. Evaluate the written AST one operation at a time: no fusion, reassociation, contraction, extended-precision substitution, or flush-to-zero. Subnormals are supported. |
| Failure | An overflow or other non-finite result is a typed evaluation failure. Division by positive or negative zero is `DivisionByZero`, including `0 / 0`; it is diagnosed before host division. Underflow may produce a subnormal or a rounded zero, which is then normalized to positive zero. |
| `min` / `max` | Accept only normalized finite operands, compare their numeric values, and normalize the selected output. The two zero signs are therefore one semantic value. |
| Equality and diff | Compare Number outcomes by normalized finite binary64 encoding (`to_bits` after zero normalization). Compare formula definitions structurally over the complete typed bound AST; an AST edit is not a no-op merely because its current result is unchanged. |
| Persistence | For a representation that adopts this M02 contract, apply its explicit input/token resource limits first; over-limit input is structural representation failure, not Number failure. For every admitted valid RFC 8259 number, decode the exact decimal to nearest binary64 with ties-to-even, then serialize the normalized finite primitive using [RFC 8785 §3.2.2.3](https://www.rfc-editor.org/rfc/rfc8785.html#section-3.2.2.3) and radix-10 [ECMAScript `Number::toString`](https://tc39.es/ecma262/multipage/ecmascript-data-types-and-values.html#sec-number.tostring), including Note 2. Tachiko selectively adopts this number rule, **not full JCS**. Concrete limits and golden bytes remain representation/profile and #74 authority. |
| Integer-looking input | It remains binary64, with no separate safe-integer rejection. Beyond `2^53`, adjacent mathematical integers may map to the same Number; exact counts/identifiers require a future explicit type. |
| Future numeric kinds | Exact integer, decimal, fixed-point, and money semantics must be explicit new types with explicit conversions. They are Deferred; `Number` must not silently change meaning or acquire magnitude-dependent integer behavior. |

The operational reading is simple: normalize at every semantic ingress and result boundary; reject non-finite values; execute the bound tree without algebraic rewriting; compare Number outcomes by normalized bits and formula definitions by bound structure; and give storage one deterministic spelling for those bits.

## Why this contract

[IEEE 754-2019](https://standards.ieee.org/ieee/754/6210/) is the governing format/arithmetic model. Rust's accepted [RFC 3514](https://rust-lang.github.io/rfcs/3514-float-semantics.html) specifies primitive float operations as IEEE operations with `roundTiesToEven`, default non-trapping handling, and no abrupt underflow/flush-to-zero, except for NaN-specific latitude. Excluding all non-finite semantic values removes that material NaN nondeterminism. The stable Rust [`f64` documentation](https://doc.rust-lang.org/stable/std/primitive.f64.html) identifies the host type and documents the relevant correctly rounded operations.

The [WebAssembly numeric semantics](https://webassembly.github.io/spec/core/exec/numerics.html) also use nearest-ties-to-even and represent normal values, subnormals, signed zeros, infinities, and NaNs. Rejecting non-finite values, normalizing zeros, preserving subnormals, and forbidding contraction/reassociation narrows both native Rust and Wasm to the same observable semantic subset.

RFC 8785 requires ECMAScript binary64 number serialization, including Note 2, and rejects NaN/Infinity. It supplies mature spelling rules and published vectors without requiring Tachiko to adopt JCS whitespace or property sorting. [`ryu-js`](https://github.com/boa-dev/ryu-js) is a credible implementation candidate because it is explicitly adjusted to ECMAScript number-to-string behavior; it is not normative and must pass Tachiko's golden corpus before adoption.

## Current repository evidence

| Evidence | Finding | Consequence |
| --- | --- | --- |
| [`Value`, `Expression`, and `FieldRef`](../../crates/semantic-core/src/model.rs#L109-L137) | The MVP directly embeds `f64`; formula references currently contain string-backed entity/field IDs. | Binary64 is already the implementation substrate, but zero normalization and the hardened stable-ID binding boundary are absent. |
| [Semantic validation](../../crates/semantic-core/src/validation.rs#L198-L265), [`validate_finite`](../../crates/semantic-core/src/validation.rs#L308-L316) | Stored numbers and numeric AST literals are required to be finite; references are checked against the document. | The Accepted non-finite boundary strengthens an existing rule rather than introducing a new value family. |
| [Literal parser](../../crates/formula-engine/src/parser.rs#L315-L355) and [number formatter](../../crates/formula-engine/src/parser.rs#L135-L147) | Decimal text is parsed through Rust `parse::<f64>()` and non-finite results are rejected. Authoring output uses a project-local `to_string`/scientific-length choice. | Literal conversion needs conformance vectors. Authoring formatting is not evidence that persisted JSON already follows ECMAScript spelling. |
| [Calculation state and affected traversal](../../crates/formula-engine/src/lib.rs#L15-L64), [whole-document entry point](../../crates/formula-engine/src/lib.rs#L81-L105) | A calculation owns per-run value/dependency `BTreeMap`s. `calculate` visits numeric/formula fields but aborts on the first DFS error; `affected_by` reports transitive dependents without incrementally recomputing them. | Fresh whole-document traversal is the implementation baseline, not yet the Accepted complete SCC/node-failure oracle. Dependency and reverse-dependency indexes remain derived runtime state. |
| [Evaluator](../../crates/formula-engine/src/lib.rs#L132-L244) | DFS memoizes values, detects cycles, evaluates explicit AST operations, rejects both zero divisors via `right == 0.0`, and rejects non-finite results. It currently returns finite `-0` unchanged and delegates `min`/`max` zero behavior to host methods. | Division behavior and non-finite failure already align substantially. A single normalization helper must become mandatory at input/result boundaries and in `min`/`max`. |
| [Formula authoring application path](../../crates/workspace-engine/src/lib.rs) | Source is parsed into a bounded unbound expression, bound once to stable IDs, then projected, validated, fully calculated, and diffed before a preview is returned. | ADR-0015/ADR-0018 binding is implemented; document-local snapshot operations remain the application baseline. |
| [Stable-key rename and duplicate rebasing](../../crates/workspace-engine/src/lib.rs) | Renaming changes only the mutable human key; bound references retain stable targets. Duplicating an entity rebases only copied formula self-references to the duplicate's new stable ID. | Human-key changes do not rewrite formula meaning, while duplication preserves the copied entity's intended self-reference semantics. |
| [Formula impact comparison](../../crates/diff-engine/src/lib.rs#L240-L270) | Calculated results currently use `f64::total_cmp`; this distinguishes the two zero signs. Derived `Value` equality elsewhere uses ordinary `f64` equality, which does not. | Normalized-bit equality gives one rule for Number outcomes. Formula-definition diff remains structural so an AST/dependency change is visible even when its calculated impact is equal. |
| [Storage writer](../../crates/storage/src/lib.rs#L88-L100) and [canonical JSON profile](../specs/canonical-json-profile.md#L142-L188) | v0.1 delegates number emission to `serde_json::to_string_pretty`; the spec already says serializer-library output is not normative and now receives Accepted numeric meaning from ADR-0018. | #74 needs an explicit number writer/conformance boundary. Legacy direct-`.ro/v1` bytes remain historical compatibility behavior and are not retroactively rewritten. |

### Current serializer contrast

The checked dependency is `serde_json` 1.0.151. Focused output observation against the three boundary values was:

| Binary64 value | Current `serde_json` output | RFC 8785 / ECMAScript output | Result |
| --- | --- | --- | --- |
| `1e21` | `1e+21` | `1e+21` | Matches |
| smallest positive subnormal (`5e-324`) | `5e-324` | `5e-324` | Matches |
| `1e-6` | `1e-6` | `0.000001` | Does not match |

Therefore current serializer output is implementation evidence, not the normative persisted-number algorithm. Matching two samples does not make it conformant.

## Source, binding, and recomputation context

These classifications prevent this numeric decision from silently reopening adjacent architecture:

| Classification | Context for #24 |
| --- | --- |
| **Accepted** | ADR-0014 owns the bounded authoring language and its one-to-one projection to a semantic expression tree. [ADR-0015](../decisions/ADR-0015-stable-semantic-identity.md#L88-L105) owns stable IDs versus mutable human addresses, and [requires dependency/reverse-dependency indexes and calculation caches to be derived](../decisions/ADR-0015-stable-semantic-identity.md#L165-L182). ADR-0017 owns versioned DTOs and forbids storage from inventing numeric meaning. |
| **Provisional** | The durable compilation pipeline should be `source text -> parsed/unbound AST with locations -> name binding and type checking -> typed bound AST using stable IDs -> dependency extraction -> evaluation`. Source text is an authoring/display artifact, not an alternative source of formula meaning. Direct edges come from bound AST; reverse indexes, SCC/topological algorithms, dirty sets, and memoized results are rebuildable. A node's primary failure follows structural → binding/type/stale → cycle → dependency → local-evaluation precedence. Semantic cycle observation is SCC membership in the induced subgraph excluding earlier-failed nodes, while the extracted dependency set retains all statically available edges; opaque-ID iteration order and a chosen witness are non-semantic. Incremental evaluation uses old/new reverse closures and falls back to full recomputation whenever an operation's impact cannot be classified safely. |
| **Deferred** | Persisted dependency indexes/results, parallel evaluation, aggregate-order contracts beyond current binary `min`/`max`, extension functions, external/impure computation, cross-document references, and exact integer/decimal/fixed-point/money types. None is needed to accept the M02 `Number` contract. |

Promotion review added two binding/projection consequences without changing
stable-ID formula meaning. Canonical authoring projection is partial: the
current human address must resolve uniquely back to the same stable
`EntityId + FieldId`, or formatter/`explain` returns a typed no-source failure
carrying the stable-ID subject set. Reused keys, last-known spelling, and
tombstones cannot become valid source. A rename that can change projections
must preflight every affected formula in the candidate snapshot; exactly 4,096
UTF-8 bytes is valid, 4,097 is rejected atomically, and stable IDs plus bound
ASTs never change.

## Native/Wasm probe

The checked-in [Rust probe](probes/issue-24-number-parity.rs) models the Accepted finite boundary, zero normalization, `min`/`max`, current arithmetic operators, subnormal preservation/underflow, ties-to-even examples, overflow failure, and division by both zero signs. The [Node harness](probes/issue-24-number-parity.mjs) executes the same exported cases from the Wasm build.

Environment:

- `rustc 1.97.1 (8bab26f4f 2026-07-14)`
- LLVM 22.1.6
- host `aarch64-apple-darwin` on macOS
- installed target `wasm32-unknown-unknown`
- Node v24.15.0

Exact build/run commands:

```sh
probe_dir=$(mktemp -d /tmp/issue24-probe.XXXXXX)
rustc --edition=2024 -C opt-level=2 docs/research/probes/issue-24-number-parity.rs -o "$probe_dir/issue-24-number-parity-native"
"$probe_dir/issue-24-number-parity-native" > "$probe_dir/native.out"
rustc --edition=2024 -C opt-level=2 --target wasm32-unknown-unknown --crate-type cdylib docs/research/probes/issue-24-number-parity.rs -o "$probe_dir/issue-24-number-parity.wasm"
node docs/research/probes/issue-24-number-parity.mjs "$probe_dir/issue-24-number-parity.wasm" > "$probe_dir/wasm.out"
cmp "$probe_dir/native.out" "$probe_dir/wasm.out"
```

Result: all 16 output records were byte-identical. Both files had SHA-256 `de0aa6c3beaf68f92ba5330c456e6bd02c3d0068c7becd4a5d207b49f3e505dd`. This is positive evidence for the narrowed execution contract, not a proof for every compiler, target, parser input, or serializer.

## Rejected alternatives

| Alternative | Reason rejected |
| --- | --- |
| Leave arithmetic and spelling as unspecified host/library behavior | Compiler flags, optimizer transformations, runtime edge behavior, and dependency updates would become accidental semantic changes. |
| Preserve signed zero as semantic meaning | It creates observable but unhelpful divergence across equality, diff, `min`/`max`, division diagnostics, caches, and JSON, while the current product has no signed-zero domain use case. |
| Permit NaN or Infinity | NaN payload/sign behavior is not a portable deterministic semantic value, and JSON/JCS cannot encode NaN or Infinity as numbers. |
| Flush subnormals to zero | It changes valid binary64 results by target/flag and contradicts the Rust/Wasm default semantics used as the parity basis. |
| Permit fused or reassociated evaluation | It can change the final bit pattern despite algebraic equivalence, invalidating AST meaning and native/Wasm reproducibility. |
| Adopt decimal, fixed-point, or money now | These need distinct scale, precision, rounding, conversion, overflow, and schema rules. Adding them implicitly would weaken rather than complete this contract. |
| Adopt all of RFC 8785 JCS | Tachiko's versioned, Git-readable profile has different whitespace/member-order requirements. Only the compatible primitive number rule is selected. |
| Make `serde_json`, Rust display, or `ryu-js` output normative | Implementations are replaceable. The accepted spec plus golden vectors must remain the authority. |

## Limits and gaps

- The parity probe covers one Rust/LLVM/Node/macOS toolchain and one Wasm target. It does not cover other architectures, optimizer versions, or engines.
- The probe uses constructed binary64 values and arithmetic; it does not test formula decimal parsing or JSON serialization.
- The probe does not test representation input/token resource admission; #74
  must declare those concrete limits and #40 must test exact and one-byte-over
  boundaries before exact-decimal conversion.
- The probe is deliberately limited to the current binary operators and two-argument `min`/`max`; it does not establish aggregation order.
- The hardened stable-ID bound AST and incremental evaluator are not implemented. Current full calculation and dependency reporting are the reference behavior.
- `ryu-js` has not been selected as a dependency. Its output must be verified against the normative corpus, including thresholds and every adopted RFC vector.
- Legacy direct-`.ro/v1` numeric behavior remains version-scoped. Current semantics require an explicit migration candidate; ordinary open/read must not silently normalize or re-emit legacy bytes.

## Downstream test plan for #74 and #40

With ADR-0018 Accepted, #74 owns implementation of the number boundary in the version-specific semantic/DTO conversion and canonical writer without changing legacy direct-`.ro/v1` bytes. #40 owns executable storage conformance through:

1. RFC 8785 Appendix B finite binary64 vectors, plus explicit threshold neighbors around `1e-6` and `1e21`, smallest/largest subnormals, minimum normal, maximum finite, and round-trip-sensitive mantissas.
2. Reader vectors for each declared complete-input/token exact limit and
   one-byte-over failure, followed for admitted tokens by `1`/`1.0`/`1e0`, the
   `9007199254740993` ties-to-even case, finite subnormals, underflow to
   normalized zero, and rejection when conversion would produce infinity.
3. Ingress vectors proving both zero signs canonicalize to semantic positive zero and persist as `0`; NaN and positive/negative Infinity fail with stable typed diagnostics.
4. Canonical encode-decode-encode byte stability and insertion-order equivalence using the numeric edge corpus, compared by exact UTF-8 bytes on every supported target.
5. Explicit legacy-v1 migration vectors proving ordinary open does not apply the new contract, migration maps negative zero, and prior durable bytes remain untouched until explicit save.
6. A dependency-update guard: any serializer/formatter change that alters golden bytes fails until the change is identified as a bug fix preserving the contract or an explicit new representation version.

Formula-engine conformance separately owns ties-to-even arithmetic, subnormal preservation/underflow, overflow/non-finite failure, division by both zero signs, normalized `min`/`max`, structural formula-definition equality, exact calculated-outcome equality, primary-failure precedence, and native/Wasm result parity. Incremental tests run mutation sequences through both dirty-closure recomputation and fresh full recomputation, comparing failure maps, normalized bits, bound definitions, dependency sets, and cyclic SCC membership after every operation. Runtime export v1 remains independent and needs preservation evidence or a version bump before adopting the new Number contract. These obligations must not be smuggled into #74's storage responsibility.

Formula/workspace conformance also owns projection failures for missing,
ambiguous, and reused human addresses and the atomic rename cases at exactly
4,096 and 4,097 UTF-8 bytes. Those tests must prove that rejected renames retain
the old human key, stable IDs, and structurally identical bound ASTs. #23 may
present the typed failure but does not define a substitute authoring token.
