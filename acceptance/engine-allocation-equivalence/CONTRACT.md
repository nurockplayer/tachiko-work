# Engine allocation equivalence acceptance — Work #497

Parent outcome: #489 / #495 / Sheet #150. Owning authority:
https://github.com/nurockplayer/tachiko-work/issues/497.
Base: `33b26e2a8d34593030118c53dbe79b6f8fdc6e0e`.

This seed is authored before engine production. It promises no new operation,
type, constraint, formula capability, authority or resource budget. The outcome
is reduced transient allocation while every existing semantic outcome remains
equal. Mechanism selection waits for the fixed baseline/equal-entity/lazy-
diagnostics/both comparison and independent acceptance adequacy.

## Case map and independent expectations

Expected complete diagnostics and semantic changes are handwritten literals.
Tests do not obtain expected values from the candidate validator, diff,
calculation or an implementation copy. Small literal formula inputs establish
the expected 4/6 and transitive 5/7 values and exact changed-field causes.

| Criterion | New executable source |
| --- | --- |
| All scalar kinds, optional absence, Unicode/empty Text, direct constraint endpoints | `scalar_validation_equivalence::mixed_valid_scalars_optional_absence_and_exact_constraints` |
| Full diagnostic fields and order; missing schema cascade suppression; immutable source | `mixed_failures_preserve_complete_literal_ordered_diagnostics` |
| Direct constraint and declaration errors | `direct_constraint_and_declaration_failures_keep_exact_fields` |
| Typed Reference failures and formula stored in Text | `reference_errors_keep_targets_facts_and_type_mismatch` |
| Legacy versus core formula-reference authority | `legacy_formula_reference_checks_remain_distinct_from_core_validation` |
| Research cancellation completion and immediate refusal | feature-gated `cancellation_keeps_exact_completed_results_and_can_stop` |
| Cross-entity and transitive effects on unchanged stored entities, descriptors, detached owned result | `unchanged_entities_keep_transitive_impacts_descriptors_and_owned_results` |
| Equal calculated output retains a definition change | `equal_calculated_output_does_not_hide_a_formula_definition_change` |
| Schema/field keys and descriptor/presentation fidelity | `schema_key_and_field_key_changes_preserve_unchanged_entity_presentation` |
| Ordered schema/entity/field add/remove, key/Text/Reference changes | `additions_removals_keys_text_and_references_keep_exact_order` |
| Equal inputs still validate; constraints → definitions → before → after refusal priority | `equality_never_bypasses_refusal_or_before_after_error_priority` |

The two new paths are `crates/semantic-core/tests/scalar_validation_equivalence.rs`
and `crates/diff-engine/tests/unchanged_entity_equivalence.rs`. All full vectors,
typed errors and rendering assertions are mandatory; matching a code count or
accepting only a prefix does not qualify equivalence.

## Parent execution and admission

Before producer assignment, run on the unchanged parent with frozen tests:

```sh
cargo test -p tachiko-semantic-core --locked --test scalar_validation_equivalence
cargo test -p tachiko-semantic-core --locked --features issue-175-research --test scalar_validation_equivalence
cargo test -p tachiko-diff-engine --locked --test unchanged_entity_equivalence
```

All three commands actually passed on the unchanged base above. Receipts are
retained at `/workspace/scratch/work497-seed-parent-baseline`; `parent.txt`
records the base, `results.tsv` records terminal exit codes, and each command has
its complete log and resource guard receipt. `input-integrity.log` confirms all
three authoring inputs matched `inputs.sha256` through parent execution. Only
this contract's execution record changes afterward; both test files stay exact.

| Command / case map | Actual result | Guard command elapsed |
| --- | --- | --- |
| Semantic ordinary: first five semantic cases above | 5 passed; exit 0 | 8.359 s |
| Semantic research: same five plus cancellation | 6 passed; exit 0 | 2.538 s |
| Diff: all five diff cases above | 5 passed; exit 0 | 8.586 s |

Every run has zero failed, ignored, measured or filtered tests and no guard
termination. Elapsed times include command/build work and are not edit latency.
The frozen seed SHA/tree and toolchain record accompany the issue handoff;
fresh independent source ADEQUATE remains required before producer assignment.

For the selected repair, retain existing semantic-core model/diagnostic/Date/
constraint/reference tests, diff semantic/stable-identity/canonical-v2/refusal
tests, and workspace validation-report, retained-state, resident, patch-lifecycle,
Date and formula suites unchanged. Those existing suites preserve authoritative
error roles, lifecycle/currentness, publication and downstream snapshots. No
workspace-engine producer or candidate-ownership change is in this child.

The fixed four-variant scratch comparison retains every indexed sample and exact
result check; no favorable rerun selection. Its diagnostic nested timings are
not full edit latency. Final integrated #495 native/actual Worker/consumer
performance, heartbeat, memory and lifecycle gates remain unchanged and must
run with immutable exact kit identities. Engine test PASS supplies no capacity
performance or consumer credit.

## Mutation and ownership bounds

Exactly five complete paths, at most 1,000 nonblank added/deleted lines, including
these tests/contract and eventual production in only `crates/semantic-core/src/
validation.rs` and `crates/diff-engine/src/lib.rs`. Preserve predicates,
traversals, ordering, calculations, formula effects, descriptor generation and
owned before/after snapshots. No bypass, cache, public type/API or dependency
change. The mechanisms remain separately inspectable and reversible.

Preserve #493/#494's 87-site compatibility batch byte-for-byte. Actual Draft
#494 head `589fe7095d098d5e20da8ebbdfa1b6ad93b9fb34` owns the old cfg(test)
`validation.rs:1431` assertion; that hunk is excluded from this child's edits.
New dedicated tests avoid its existing test-file hunks. Sheet pin/manifest,
Date+saved-definition guard and all other HOLDs remain. Fresh independent final
deep review and the parent's stronger #374 Oracle route remain required.
