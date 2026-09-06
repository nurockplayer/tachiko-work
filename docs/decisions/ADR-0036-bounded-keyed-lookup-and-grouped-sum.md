# ADR-0036: Bounded keyed lookup and grouped-sum definition

## Status

Accepted

Decision issue: [#320](https://github.com/nurockplayer/tachiko-work/issues/320)

Specified by: [Bounded Keyed Lookup and Grouped-Sum Definition Specification](../specs/bounded-keyed-lookup-and-grouped-sum.md)

Related authority: [ADR-0015](ADR-0015-stable-semantic-identity.md),
[ADR-0017](ADR-0017-versioned-storage-and-canonical-representation.md),
[ADR-0018](ADR-0018-bound-formulas-and-deterministic-binary64.md),
[ADR-0019](ADR-0019-staged-semantic-validation-and-diagnostics.md),
[ADR-0020](ADR-0020-first-class-headless-semantic-api.md),
[ADR-0027](ADR-0027-open-format-and-interoperability-policy.md), and
[ADR-0029](ADR-0029-current-state-authority-and-optional-history.md)

## Context

The Driver journey in [#293](https://github.com/nurockplayer/tachiko-work/issues/293)
needs one two-table computation: resolve an order's Text product key, multiply
the matched price by quantity, and sum amounts by product category. Existing
bound formulas are scalar `Number` expressions with static dependencies. The
Accepted Analysis Query is a disposable exact-snapshot read-only result and
explicitly defers joins, `Sum`, and persisted analysis. Neither may accidentally
acquire this saved live dynamic-membership meaning.

## Decision

### 1. One saved live definition family is admitted

Tachiko admits one narrowly typed semantic definition family named
`KeyedGroupedSumDefinition`. It is a saved **live definition**, not an evaluated
table, formula AST node, Analysis Query result, view setting, or spreadsheet
cache. Exact Rust, API, DTO, storage, and authoring shapes remain separately
owned implementation work.

Each independently saved definition has one opaque stable identity. Its durable
meaning binds these stable semantic identities:

- an Orders schema, its local Text lookup-key field, and its local Number
  quantity field;
- a Products schema, its remote Text key field, remote Text category field, and
  remote Number price field; and
- the fixed operation/version `lookup-exact-text -> price * quantity -> group
  by category -> SUM`.

Every current entity in its Orders schema is candidate membership. The family
has no arbitrary predicate/expression slot, second key, aggregate selection,
join type, target language, or UDF. Renames change presentation only; deleted,
missing, retyped, or invalid stable targets never retarget by a reused human
key. This is outside the scalar `Expression` language and does not amend
Analysis Query into a persisted object.

### 2. Lookup is exact, dynamic, and cardinality-safe

For each current Orders entity, the evaluator finds Products entities whose
remote Text key has exactly the same decoded Unicode scalar sequence as the
local Text key. There is no case folding, Unicode normalization, locale
collation, numeric/text coercion, wildcard, approximation, or first/last match.

Exactly one match supplies category and price. Zero matches produce
`lookup.missing_key`; two or more produce `lookup.ambiguous_key` with the full
matched stable-`EntityId` set. A local or remote missing/wrong-typed required
value is an explicit input diagnostic, not an empty string, zero, or omitted
member. Numeric operands use their authoritative effective Number; a formula-
backed operand reuses ADR-0018's calculation outcome and never a second
evaluator.

Evaluation resolves current key values on every authoritative request. Stable
IDs bind definition structure and dependency evidence, not the row that happened
to match at authoring. Orders membership plus every current Order's local key
and quantity are dependencies. Products membership and every Products key are
dependencies because a row/key addition, deletion, or edit can change
cardinality. Successful matches additionally depend on price/category,
effective-Number formula dependencies, and all bound schema/field type facts.
View sorting, row display order, storage placement, and locale are not semantic
dependencies.

### 3. Grouped SUM is finite and deterministic

A successful order contributes `price * quantity`, evaluated in that order under
ADR-0018's finite binary64 multiplication, normalization, and failure rules.
The matched product's exact Text category is its group key; equal strings
coalesce and no empty/synthetic groups exist.

Within a category, contributors sort by ascending ordinary numeric order of
their normalized finite binary64 `price * quantity` amounts, then reduce
left-to-right from semantic positive zero. Equal normalized amounts are the
same operation term, so their relative entity order cannot affect the reduction.
This `ContributionNumberOrder` is a logical order over ADR-0018 Numbers, not an
order over identities, serialized records, storage locations, or view layout.
Every addition uses one ADR-0018 binary64 operation and validates and
normalizes its intermediate result before the next term. The reduction MUST NOT
reassociate, parallel-reduce, fuse operations, or inherit view/key/category/
storage iteration order. Group presentation order is not semantic.

Any required lookup, input, amount, or reduction failure makes the **whole
definition result unavailable** and publishes no partial group values as
current. Diagnostics retain the direct row/input/lookup/numeric root plus the
definition-level failed dependency without inventing a misleading cascade per
group. This operation-wide outcome is required because an ambiguous or missing
product can make the affected category unknowable.

### 4. Semantic API and authorization remain independently checked

Evaluation is one distinct Semantic API **Query** operation family; saved
definition create/update is one distinct **Command** operation family. Their
exact identifiers and DTOs remain Provisional, but neither shares capability
meaning with Formula, Analysis Query, scalar Value, schema, or generic structure
operations. Create/update requires `Structure`; removal requires `Structure +
Destructive`. Each command continues through ADR-0020 exact-base proposal/
publication and ADR-0026 capability, Approval, and trusted-footprint laws.

ADR-0026 deliberately has no definition-specific scope atom. Therefore the
trusted command footprint uses `Document(DocumentId)` for the saved definition
as its direct target, generated/deleted identity, and owning container: create
and update each require exactly `(KeyedGroupedSumDefinitionCommand, Structure,
Document(document))`; deletion requires that `Structure` tuple and
`(KeyedGroupedSumDefinitionCommand, Destructive, Document(document))`. Binding
or rebinding a schema/field does not write that schema/field, so it adds no
write tuple. Required disclosure to inspect candidate bindings is independently
derived and authorized; a Command grant never implies it.

Before an evaluation result, diagnostic, ambiguity candidate, preview, or
dependency/currentness fact is disclosed, the trusted authority derives its
complete footprint from the exact definition and snapshot. It uses
`Document(document)` for the definition; `Schema(orders)` and
`Schema(products)` for candidate membership; the appropriate `EntityField` for
each local key/quantity, remote key/category/price, and formula dependency; and
the appropriate `SchemaField` for required type facts. It includes the entire
Products membership/key universe, not only currently matched rows. If complete
disclosure coverage for any revealed fact cannot be derived and authorized, the
Query is denied without a partial aggregate, matched identity, or diagnostic
leak. The client never supplies its own footprint.

### 5. Currentness, persistence, and interoperability stay truthful

An evaluated result is derived revision-scoped evidence: definition identity,
exact accepted input snapshot/revision, dependency outcome, and either complete
group values or unavailable diagnostics. Caches are replaceable derived state.
They become non-current after a definition/dependency change and MUST NOT be
shown, diffed, exported, or saved as current live output.

Saving/reopening preserves a definition, not evaluated group values as semantic
truth. A reopened definition evaluates the current accepted snapshot. A future
representation that persists it requires an explicit versioned DTO/migration
under ADR-0017; frozen `.roproj/v1` and other formats cannot be widened silently.
Unsupported required definition semantics fail closed.

An adapter may preserve a live definition only if it encodes the exact admitted
binding, equality, cardinality, reduction, diagnostics, and currentness. A
values-only export is permitted only as explicitly labelled evaluated/lossy
output tied to its exact source revision; it claims neither re-evaluation nor
formula preservation.

### 6. Realization remains separately Ready work

This ADR creates no formula extension, query engine, storage format, API/DTO,
frontend evaluator, export mapping, or production implementation. After this
authority lands, the Steward must re-run #293's pre-Ready decomposition and
separately assess the live definition/lookup, grouped reduction/diagnostics,
and representation migration. #293 remains not Ready until then.

## Required pressure tests

The independently specified fixture and outcomes in the linked specification
are normative. Future contracts and implementations must demonstrate them
without deriving expected values from the implementation under test.

## Rejected alternatives

- **Extend `Expression` with lookup/group operators:** rejected: the accepted
  language is scalar `Number` with static dependencies.
- **Reuse Analysis Query as saved result:** rejected: it is disposable and
  explicitly defers persisted analysis, joins, and `Sum`.
- **Permanently bind the first match, choose first/last, or coerce to zero:**
  rejected because that hides stale, ambiguous, or unavailable meaning.
- **Persist group values as definition:** rejected because derived caches are
  not current semantic state.
- **Copy spreadsheet/SQL join, aggregate, or pivot behaviour wholesale:**
  rejected because this canary does not justify a general language/catalogue.

## Related

- [#293](https://github.com/nurockplayer/tachiko-work/issues/293)
- [#320](https://github.com/nurockplayer/tachiko-work/issues/320)
