# Bounded Keyed Lookup and Grouped-Sum Definition Specification

Decision state: Accepted target under
[ADR-0036](../decisions/ADR-0036-bounded-keyed-lookup-and-grouped-sum.md)

Implementation state: Not implemented. This specification grants no production
implementation authority; implementation requires a separately Ready child of
[#293](https://github.com/nurockplayer/tachiko-work/issues/293).

Decision issue: [#320](https://github.com/nurockplayer/tachiko-work/issues/320)

## Contract

`KeyedGroupedSumDefinition` is a stable-ID-bound saved live definition for only:
Orders/local Text key/local Number quantity; Products/remote Text key/remote
Text category/remote Number price; exact lookup; `price * quantity`; category
grouping; and `SUM`.

From one accepted snapshot, it evaluates every Orders entity. Text equality is
the exact decoded Unicode scalar sequence; only one Products match succeeds.
Zero/plural matches are `lookup.missing_key`/`lookup.ambiguous_key` (the latter
retains all matched stable `EntityId`s); missing or wrong-typed required values
are input diagnostics. Numeric operands use authoritative effective Numbers and
formula-backed values reuse ADR-0018 calculation outcomes. No coercion, case folding,
normalization, locale collation, wildcard, approximate match, or silent row
selection is admitted.

Matched amount uses ADR-0018 `price * quantity`. Category equality, like lookup
equality, compares the exact decoded Unicode scalar sequence with no case
folding, normalization, locale collation, or coercion. Within an exact category,
contributors sort by ascending ordinary numeric order of their normalized finite
binary64 amounts, then are left-folded from semantic positive zero,
validating/normalizing each binary64 intermediate. Equal normalized amounts are
the same operation term, so their relative entity order cannot affect the
result. `ContributionNumberOrder` is not identity, record, storage, view, or
presentation order. View, key, category, storage, and presentation order cannot
change membership or results. There are no empty/synthetic groups.

Any member lookup/input/amount/reduction failure yields one Unavailable
definition result, direct root diagnostics plus definition-level failure, and no
current group map. A Complete result exposes the definition and exact evaluated
snapshot/revision. Dependencies include Orders membership plus every local
lookup-key/quantity value, Products membership plus the full Products key
universe, successful-match price/category, schema/field type facts, and any
effective-Number formula dependencies. If a required operand is formula-backed,
dependencies also include the whole authoritative document `Calculation`
outcome and every formula root that can determine whether a complete
`CalculationState` exists. Retained output becomes non-current after a
dependency root or definition change.

Ambiguity candidate sets, diagnostics, and a Complete group map are bounded
complete Query projections. Once trusted Query coverage permits classifying the
complete outcome, an applicable trusted deterministic finite result profile may
return structured `result-too-large`; that makes the definition Unavailable and
MUST NOT truncate, sample, implicitly paginate, or expose a partial candidate/
diagnostic/group collection as complete. Its profile identity is evaluated
lineage/currentness evidence: changing it makes retained Complete or Unavailable
output non-current. Exact limits and DTO spelling remain Provisional.

Only the definition is durable semantic meaning. A future persisted shape must
be versioned/migrated under ADR-0017 and fail closed when unsupported; frozen
`.roproj/v1` cannot be widened. Evaluated caches are never saved/displayed/
exported/diffed as current output. Values-only export must be labelled
evaluated/lossy and source-revision-bound; live preservation requires encoding
the whole admitted semantic contract.

`tachiko.semantic-delta/v1` cannot represent this definition. A snapshot pair
that creates, deletes, or changes the durable meaning of any
`KeyedGroupedSumDefinition`—including an in-place update with unchanged stable
identity—is unsupported by that v1 contract: a requested v1 diff fails and
never omits the definition change or returns a partial delta. That does not
block current-snapshot publication, because delta and retained semantic
transitions are optional derived evidence rather than mutation authority. A
future versioned delta/direct-target/order/conflict contract is separately Ready
work before any diff or retained-transition support exists.

Current semantic merge/conflict v1 also cannot represent this definition. If a
three-way base/left/right comparison creates, deletes, or changes the durable
meaning of any `KeyedGroupedSumDefinition`—including an in-place update with
unchanged stable identity—the whole merge/conflict request is unsupported and
fails closed. It never omits, silently merges, or invents a partial definition
conflict. A versioned direct-target/facet/order/conflict contract is separately
Ready work before this family has merge support.

## Semantic API, capability, and disclosure boundary

Evaluation is a distinct Semantic API Query operation family. Definition
create/update is a distinct Command family requiring `Structure`; deletion
requires `Structure + Destructive`. Exact names/DTOs remain Provisional, but no
Formula, Analysis Query, scalar Value, schema, or generic Structure capability
implies either family. Commands retain ADR-0020 exact-base and ADR-0026
trusted-footprint/Approval requirements. Because ADR-0026 has no
definition-specific atom, create/update each derive exactly
`(KeyedGroupedSumDefinitionCommand, Structure, Document(document))` for the
definition direct target/generated identity/owning container; deletion derives
that tuple plus `(KeyedGroupedSumDefinitionCommand, Destructive,
Document(document))`. Binding/rebinding does not write a bound schema/field, and
Query disclosure remains independent of Command authority.

For each Query/preview/diagnostic, the trusted authority derives—not the
caller—the complete disclosure footprint: `Document(document)` for the
definition; `Schema(orders)`/`Schema(products)` for candidate membership;
`EntityField` for local operands, the full Products membership/key universe, and
matched category/price; `SchemaField` for type facts; and `EntityField` for
every effective-Number formula dependency. If a formula-backed operand requires
the whole document `Calculation` outcome, it additionally covers every formula
root whose outcome determines whether that complete `CalculationState` exists.
It denies the whole Query when that coverage cannot be authorized; it never
leaks a matched ID, ambiguity candidate, diagnostic, partial group, cache, or
currentness fact through a visible subset.

## Independently specified pressure fixture

| Stable entity ID | Table | Key / fields |
| --- | --- | --- |
| `product-A` | Products | `code = "P-100"`, `category = "hardware"`, `price = 2` |
| `product-B` | Products | `code = "P-200"`, `category = "services"`, `price = 5` |
| `order-C` | Orders | `product_code = "P-100"`, `quantity = 3` |
| `order-A` | Orders | `product_code = "P-100"`, `quantity = -1` |
| `order-B` | Orders | `product_code = "P-200"`, `quantity = 2` |

Presentation order is `order-C`, `order-A`, `order-B`; it supplies no semantic
reduction order. The base Complete result is exactly `hardware = 4` (`-2 + 6`)
and `services = 10`, with no other group.

| Case | Independent change | Required outcome |
| --- | --- | --- |
| Unique match | Base fixture | Complete: `hardware = 4`, `services = 10`. |
| Missing key | Set `order-C.product_code = "P-404"` | Unavailable; `lookup.missing_key` for `order-C`; no current group values. |
| Duplicate key | Add `product-D(code = "P-100", category = "other", price = 9)` | Unavailable; `lookup.ambiguous_key` for affected orders; no first/last selection. |
| Products membership dependency | Add unmatched `product-D(code = "P-404", ...)` | Freshly evaluated Complete base result, not a retained current cache, even though group values are unchanged. |
| Cardinality dependency edit | Add unmatched `product-D(code = "P-404", ...)`, edit only its code to `"P-100"`, then restore `"P-404"` | Base becomes Unavailable with `product-D` in every `P-100` ambiguity set, then returns as a freshly evaluated Complete base result. |
| Case-different key | Set `order-C.product_code = "p-100"` | Unavailable; `lookup.missing_key`; lowercase differs. |
| Unicode lookalike | Product uses NFC `"é"`; order uses NFD `"é"` | Unavailable; `lookup.missing_key`; no Unicode normalization/collation. |
| Key edit/restoration | Change `order-C` to `"P-404"`, then restore `"P-100"` in a later snapshot | First Unavailable; restored snapshot is freshly Complete, never revived cache. |
| Category move | Set `product-A.category = "supplies"` | Complete: `supplies = 4`, `services = 10`; `hardware` disappears. |
| Source deletion | Remove `product-B` | Unavailable; missing-key root for `order-B`; no partial `hardware = 4`. |
| Reorder | Reorder views/storage presentation only | Same base result and binary64 bits. |
| Zero and negative | Add `order-D(P-200, 0)`; retain negative `order-A` | Complete base result unchanged; both values are valid finite contributors. |
| Missing/wrong input | Make required quantity absent/Text; separately use zero quantity with missing product | Unavailable with root evidence. `0 * missing` is not zero; invalid rows are not skipped. |
| Non-finite amount | Set matched price to finite binary64 maximum and quantity to `2` | Unavailable with ADR-0018 non-finite multiplication failure. |
| Non-finite reduction | Two valid amounts in one group each equal finite binary64 maximum | Unavailable with ordered-addition non-finite failure. |
| Cancellation-sensitive reduction | In one category set `order-A = 1e16`, `order-B = -1e16`, `order-C = 1`; present `order-A`, `order-C`, `order-B` | `ContributionNumberOrder` is `-1e16`, `1`, `1e16`, so the specified left fold produces semantic positive zero (binary64 `0x0000000000000000`), independent of presentation and EntityId representation. |
| Signed zero | A category has only `price = 0`, `quantity = -1` | Complete value is semantic positive zero. |
| Formula-backed operand | Make price or quantity a calculation-failed formula | Unavailable using the ADR-0018 root failure; no second evaluator or stale effective Number. |
| Formula-backed effective Number | Replace `product-A.price` with a successful ADR-0018 formula whose effective Number is `3` | Fresh Complete: `hardware = 6`, `services = 10`; evaluation uses formula result, not a prior stored/cache price. |
| Whole Calculation dependency | With a required formula-backed price/quantity, change an otherwise unrelated formula root from valid to failed | Unavailable because the authoritative document Calculation has no partial `CalculationState`; a prior Complete result is non-current. |
| Whole Calculation disclosure | With a required formula-backed price/quantity, deny disclosure of one unrelated formula root needed to determine complete Calculation existence | Deny the whole Query without revealing Complete/Unavailable, root identity, diagnostic, cache, or currentness evidence. |
| Empty Orders | Remove every Orders entity while definitions/fields remain valid | Complete empty group map; no synthetic category/group. |
| Save/reopen live | Save definition, change `product-A.price` from `2` to `3`, reopen against that snapshot | Fresh Complete: `hardware = 6`, `services = 10`; stored `4` is not truth. |
| Stale cache | Retain base output, then change a dependency root | Retained output is non-current; require fresh Complete/Unavailable evaluation. |
| Result profile | Use a duplicate population, diagnostics, or category groups whose complete projection exceeds the applicable finite result profile | Structured `result-too-large` Unavailable outcome; no partial IDs, diagnostics, groups, truncation, sample, or implicit pagination. |
| Result-profile change | Retain a Complete or `result-too-large` outcome, then change only the trusted deterministic finite result profile | Retained output becomes non-current; re-evaluation uses the active profile and cannot bypass its bound. |
| Delta v1 boundary | Create, update, or delete the definition, then request `tachiko.semantic-delta/v1` | Unsupported; it emits neither a partial delta that omits the definition nor a fabricated v1 fact. |
| Merge/conflict v1 boundary | Base/left/right differ by definition create, delete, or in-place durable-meaning update, then request current semantic merge/conflict | Unsupported whole request; it does not omit, silently merge, or invent a partial definition conflict. |
| Export disclosure | Export base values only and to a hypothetical live-preserving target | Values are evaluated/lossy and source-revision-bound; live claim is rejected unless every admitted semantic is encoded. |

## Deferred extensions and decomposition

Non-Text/composite keys, cardinality variants, first/last/approximate/wildcard
lookup, predicates/joins, arbitrary amount expressions, more aggregates, partial
results, empty groups, persisted Analysis Query, public API/DTO/wire shape,
storage version, frontend evaluation, and external formula syntax remain Deferred.

After this authority lands, the Steward must decide whether (1) live saved
definition/exact lookup, (2) grouped reduction/diagnostics, and (3) representation
migration are independent #293 implementation children. This creates or Readies
none of them.
