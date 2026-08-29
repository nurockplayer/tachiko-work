# Performance Model

## Principle

Performance must be designed around semantic operations, not UI tricks.

## Priorities

1. Incremental computation
2. Lazy loading
3. Parallel evaluation
4. Deterministic caching
5. Efficient serialization

## Examples

Spreadsheet recalculation should update affected dependency nodes instead of recomputing the entire workbook.

Document rendering should update affected regions instead of rebuilding everything.

Issue #95 provides the current resident-runtime evidence: formula dirty roots
plus old/new reverse-dependent closures reuse unaffected calculation nodes,
while validation reports and other state are rebuilt where no safe incremental
classification exists. The reproducible 10/100/1000-entity harness reports
both deterministic work counters and observed timings; neither is a product
SLA or a new semantic/cache contract.

## Rust Advantages

Rust provides memory safety, predictable performance, and strong concurrency primitives for large structured documents.
