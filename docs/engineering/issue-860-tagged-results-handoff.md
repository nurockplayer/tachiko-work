# Issue #860: Date outcome decision handoff

Status: Date projection support added; engine production of Date outcomes is pending.
Checked: 2026-11-21 19:37 JST.
Base: `f5d3b2b59d695d95244b21647cc53bf4b302de9c`.

## Projection decision

For a Date outcome, legacy `CalculationProjection::Value.value: f64` carries
the engine's Date day number, days since 1970-01-01. The tagged Date in `result`
is authoritative. The existing `days_from_civil` calculation uses that epoch.
The engine lead recorded this decision on
[#860](https://github.com/nurockplayer/tachiko-work/issues/860#issuecomment-7061375538).

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
outcome. Numeric consumers must account for Date-tagged results.

The remaining engine path is `Calculation.values`, retained calculation state,
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
Ownership/async boundary: no ownership, async, pinning or unsafe code changes.
The brief's engine/runtime command remains required after implementation.
Independent review belongs to the lead and PR review gate.
