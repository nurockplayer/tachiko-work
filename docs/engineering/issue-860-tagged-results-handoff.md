# Issue #860: Date outcome decision handoff

Status: blocked on a public projection decision; no implementation is included.
Checked: 2026-11-21 19:37 JST.
Base: `f5d3b2b59d695d95244b21647cc53bf4b302de9c`.

## Required decision

What does the legacy `CalculationProjection::Value.value: f64` contain when
`result` is a tagged Date?

- Retain the required numeric member as a documented `0` placeholder, with
  `result` authoritative and Number-only readers rejecting tagged Dates
  (recommendation, pending approval).
- Omit the numeric member for Dates. This changes the current required wire
  member and needs an explicit amendment to the brief's no-wire-change boundary.

An Excel serial is not recommended: ADR-0047 §5 reserves serial conversion for
the XLSX boundary. No epoch, serial convention or placeholder is currently
specified for this public projection.

Authority:

- [#860](https://github.com/nurockplayer/tachiko-work/issues/860) explicitly
  leaves the legacy numeric value open ("serial or omitted").
- [#842's initial shape decision](https://github.com/nurockplayer/tachiko-work/issues/842#issuecomment-6951957787)
  retains `value: f64`.
- [#842's amendment](https://github.com/nurockplayer/tachiko-work/issues/842#issuecomment-6952817831)
  omits `result` for Numbers and includes it for other outcomes, but does not
  define the legacy numeric member for those outcomes.
- [ADR-0047 §3–5](../decisions/ADR-0047-auto-columns-tagged-results-and-cell-failures.md)
  defines tagged outcomes and Date arithmetic without defining that wire member.

## Implementation re-entry

The current `CalculationProjection` serializer and deserializer require
`value: f64`; `CalculationProjection::number()` returns it for every successful
outcome. A choice here affects existing numeric consumers as well as encoding.
Resolve the decision on #860 before implementing the Date publication path.

The engine path is `Calculation.values`, retained calculation state,
`evaluation::Evaluation::number`, and `FormulaCalculationOutcome::Value`.
The partitioned map is already generic; its lookup integration and direct
numeric consumers also need updating. The current type surface contains only
Number/Condition expressions and no Auto field type. Keep declared-column
admission atomic under ADR-0047 §4; introducing Auto is outside this brief.

Date arithmetic follows ADR-0047 §5: Date ± whole Number and Number + Date
produce Dates, Date − Date produces Number, and fractional offsets and other
Date arithmetic fail. Plain Date references must retain their tag. Text and
Boolean results remain follow-ups.

Delivery risk: Guarded (formula semantics and public projection consumers).
Ownership/async boundary: not applicable to this documentation-only checkpoint;
no ownership, async, pinning or unsafe code changes.
Tests: not run; no executable code changed. The brief's engine/runtime command
remains required after implementation. Independent review belongs to the lead
and PR review gate.
