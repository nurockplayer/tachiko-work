# ADR-0046: Pre-1.0 revision window for `.roproj/v3`

## Status

Accepted

Decision issue: [#668](https://github.com/nurockplayer/tachiko-work/issues/668)

Authority: [founder pre-release decision](https://github.com/nurockplayer/tachiko-work/issues/488#issuecomment-6126009165),
2026-10-12; [Work lead's design note](https://github.com/nurockplayer/tachiko-work/issues/668#issuecomment-6690046800)
and [sheet-lead's decisions on it](https://github.com/nurockplayer/tachiko-work/issues/668#issuecomment-6697638099),
2026-11-07 JST.

Amends: [ADR-0017](ADR-0017-versioned-storage-and-canonical-representation.md)
and [ADR-0041](ADR-0041-bounded-durable-field-constraints.md) only for the
bounded pre-release exception below, extending the one
[ADR-0043](ADR-0043-native-display-names-separate-from-keys.md) made for display
names; [ADR-0042](ADR-0042-first-public-portable-package-over-roproj-v3.md) only
for the payload it carries.

Specified by: [v3 DTOs](../specs/roproj-format-v3.md),
[v3 layout](../specs/roproj-layout-v3.md) and
[portable package v1](../specs/portable-package-v1.md), each amended only when a
construct's own ADR lands.

## Context

`.roproj/v3` is a closed DTO over an exact nineteen-file tree. Its spec says an
incompatible change needs a new representation version and Accepted authority,
with ADR-0043 as the one named exception. Issue #579 describes a wider practice,
extending v3 in place until a freeze, but that text was never written into a
spec or an ADR. Number formats (R3) and field order (R4a) landed under it.

The Excel-feel work in #668 adds more stored constructs: a flexible column kind,
tagged formula results, a positional range, a row order and a presentation
record. v3 has no released readers. Its only consumer is Sheet, which pins an
exact Work commit. The founder's decision rules out spending work on compatibility
with unreleased versions, so a v4 now would buy a migration edge nobody needs.

The risk runs the other way. Freezing a row or formula representation before it
has run at size would commit a public format to an unproven shape, so the end of
the window needs evidence as well as a date.

## Decision

### 1. One window, ended by the freeze

`tachiko.roproj/v3` and `tachiko.portable-package/v1` stay the single current
writer and strict reader profiles until #579 freezes them. No v4, package/v2,
dual current profile, down-conversion or migration edge is selected. This does
not permit revising a released contract: at the freeze the exception ends, and
any later stored construct needs a new representation version and migration
under ADR-0017.

### 2. A closed list

Only these constructs may enter v3 during the window.

| Construct | Item in #668 | Its ADR decides |
|---|---|---|
| `FieldType` tag `auto` and its admission rule | 1, 2 | Value model and failure rule |
| Expression: Text, Boolean and Date literals; new functions | 2 | Value model and failure rule |
| Expression: positional range | 3 | Row order and ranges |
| Expression: deleted-reference marker, added by ADR-0048 | 3 | Row order and ranges |
| Entity: optional row rank | 3 | Row order and ranges |
| Presentation member: cell styles, per-cell display, footer, title | 5, 6 | Presentation |
| Column Date format | 5 | Presentation |
| A stored construct for conditional formatting, if rules need one | 5 | Presentation |

Anything else needs an amendment to this ADR. The numeric and conditional
functions already in the Expression vocabulary stay as they are, and further
functions over Number results stay inside the second row.

### 3. This ADR grants no shape

A construct lands only after all of these exist: its own Accepted ADR, the
spec text, canonical-byte fixtures, and the strict-reader rule. Until then the
writer can't emit it and a reader refuses it. The ADR for each construct also
states its semantic delta and merge meaning, or states that it has none and why.

### 4. Rules every construct follows

- **Closed world.** Unknown members and tags still fail closed with a typed
  `invalid_project`. There is no extension bag, sidecar or alias, and a Serde
  default in a new reader doesn't change this.
- **Absence stays absent.** A document using none of these constructs keeps its
  exact canonical tree and package hashes. Absence is canonical omission, never
  a materialized default or JSON `null`. A new tree member is present only when
  it has content, so the tree stays nineteen files and the package twenty entries
  for such a document, and the checksum-pinned acceptance seeds don't change.
- **One-way.** A newer reader accepts every valid older v3 document. An older
  binary refuses a document that uses a newer construct, and that's accepted.
  Sheet pins an exact Work commit; a rollback past a re-pin can't reopen such a
  document and must be disclosed.
- **No silent loss.** Writers of older representations, including direct-ro/v2
  and roproj/v1 and v2, refuse state they can't represent instead of dropping it.
  Those formats and semantic delta and conflict v1 don't change.

### 5. What ends the window

#579 can freeze when all of these hold:

1. Each listed construct has landed or has been explicitly moved past the freeze.
   One that moves past later costs a new version and a migration edge. That's a
   real cost, but it doesn't justify freezing an unproven shape.
2. The formula-admitting windowed profile (#678) has run at its target with
   these representations, and its qualification evidence is linked on #579.
3. The spec records the frozen vocabulary, and a fixture corpus exists that a
   frozen reader must accept.

If the profile isn't proven, #579 and the v1.0 tag wait. If v1.0 runs short, Sheet
UI is cut first and the stored vocabulary stays, so a later release can add the
UI without a format change.

## Consequences

- A construct's PR series can start as soon as its own ADR is Accepted, without a
  format decision of its own.
- Older documents and fixtures stay valid, and their hashes stay exact.
- Work and Sheet must re-pin together around a construct that documents can
  start to use. A reader older than the writer refuses instead of misreading.
- #579's stated practice now has authority, and its scope is narrower: it lists
  what may enter and what must be true at the freeze.

## Alternatives rejected

- **v4 now.** It adds a migration edge and a second profile for a version nobody
  has read. The founder's decision already rejects that cost.
- **A separate ADR amending v3 per construct with no window.** Each would have to
  repeat these rules, and nothing would state the closed list or the freeze
  conditions.
- **Freeze first, then version.** Row and formula representations would be fixed
  before they've run at 10,000 rows.
