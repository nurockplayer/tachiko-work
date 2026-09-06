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

Matched amount uses ADR-0018 `price * quantity`. Within an exact category,
contributors are opaque Orders `EntityId` sorted by unsigned UTF-8-byte lexical
order and are left-folded from semantic positive zero, validating/normalizing
each binary64 intermediate. View, key, category, storage, and presentation order
cannot change membership or results. There are no empty/synthetic groups.

Any member lookup/input/amount/reduction failure yields one Unavailable
definition result, direct root diagnostics plus definition-level failure, and no
current group map. A Complete result exposes the definition and exact evaluated
snapshot/revision. Dependencies include Orders membership, the full Products key
universe, successful-match price/category, schema/field type facts, and any
effective-Number formula dependencies. Retained output becomes non-current after
a dependency root or definition change.

Only the definition is durable semantic meaning. A future persisted shape must
be versioned/migrated under ADR-0017 and fail closed when unsupported; frozen
`.roproj/v1` cannot be widened. Evaluated caches are never saved/displayed/
exported/diffed as current output. Values-only export must be labelled
evaluated/lossy and source-revision-bound; live preservation requires encoding
the whole admitted semantic contract.

## Independently specified pressure fixture

| Stable entity ID | Table | Key / fields |
| --- | --- | --- |
| `product-A` | Products | `code = "P-100"`, `category = "hardware"`, `price = 2` |
| `product-B` | Products | `code = "P-200"`, `category = "services"`, `price = 5` |
| `order-C` | Orders | `product_code = "P-100"`, `quantity = 3` |
| `order-A` | Orders | `product_code = "P-100"`, `quantity = -1` |
| `order-B` | Orders | `product_code = "P-200"`, `quantity = 2` |

Presentation order is `order-C`, `order-A`, `order-B`; semantic order is
`order-A`, `order-B`, `order-C`. The base Complete result is exactly
`hardware = 4` (`-2 + 6`) and `services = 10`, with no other group.

| Case | Independent change | Required outcome |
| --- | --- | --- |
| Unique match | Base fixture | Complete: `hardware = 4`, `services = 10`. |
| Missing key | Set `order-C.product_code = "P-404"` | Unavailable; `lookup.missing_key` for `order-C`; no current group values. |
| Duplicate key | Add `product-D(code = "P-100", category = "other", price = 9)` | Unavailable; `lookup.ambiguous_key` for affected orders; no first/last selection. |
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
| Cancellation-sensitive reduction | One category has contributor amounts `1e16`, `1`, `-1e16` under IDs intentionally unlike view order | Complete result uses the specified EntityId left-fold result; no view/hash/parallel reassociation is allowed. |
| Signed zero | A category has only `price = 0`, `quantity = -1` | Complete value is semantic positive zero. |
| Formula-backed operand | Make price or quantity a calculation-failed formula | Unavailable using the ADR-0018 root failure; no second evaluator or stale effective Number. |
| Empty Orders | Remove every Orders entity while definitions/fields remain valid | Complete empty group map; no synthetic category/group. |
| Save/reopen live | Save definition, change `product-A.price` from `2` to `3`, reopen against that snapshot | Fresh Complete: `hardware = 6`, `services = 10`; stored `4` is not truth. |
| Stale cache | Retain base output, then change a dependency root | Retained output is non-current; require fresh Complete/Unavailable evaluation. |
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
