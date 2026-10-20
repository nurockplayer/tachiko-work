# ADR-0045: Optional native Text and Number fields

## Status

Accepted

Decision issue: [#544](https://github.com/nurockplayer/tachiko-work/issues/544)

Authority: [founder acceptance of the #544 direction](https://github.com/nurockplayer/tachiko-work/issues/544),
2026-10-20, for optional (blank) Text/Number fields in native tables; Opus
lead draft under [#541](https://github.com/nurockplayer/tachiko-work/issues/541)
and the Work lead directive in [#374](https://github.com/nurockplayer/tachiko-work/issues/374),
with GPT-6 Astra's read-only semantics consultation recorded on #544.

Amends: [ADR-0040](ADR-0040-bounded-native-table-field-evolution.md) only to
admit the bounded optional-field and requiredness operations below; every other
ADR-0040 exclusion remains.

Relies on, without changing: [ADR-0019](ADR-0019-staged-semantic-validation-and-diagnostics.md)
(required-field validation), [ADR-0020](ADR-0020-first-class-headless-semantic-api.md)
(gating and atomic publication), [ADR-0030](ADR-0030-canonical-semantic-delta.md)
and [ADR-0031](ADR-0031-semantic-merge-conflict-protocol.md) (present/absent value
facts and requiredness facets in their current v2 successors),
[ADR-0033](ADR-0033-snapshot-first-semantic-history-and-checkpoints.md)
(forward-only undo), [ADR-0036](ADR-0036-bounded-keyed-lookup-and-grouped-sum.md)
(grouped-sum missing-input behavior), [ADR-0041](ADR-0041-bounded-durable-field-constraints.md)
(constraints apply to present values), and the
[`.roproj/v3` format](../specs/roproj-format-v3.md) (optional values are
omitted; requiredness is a separate member).

## Context

Spreadsheet users routinely enter part of a record now and the rest later. In
the Tachiko Sheet client, a native table row can only exist once every field
holds a typed value, because ADR-0040 admits only required scalar fields and
the native authoring operations demand one initializer per declared field.
Sheet's in-grid draft row and explicit column fills (Sheet #207, #212) improve
entry, but a partially filled record still cannot be saved.

The core already represents absence. An entity's `fields` map is sparse;
validation reports `MISSING_REQUIRED_FIELD` only when a field is `required`;
`.roproj/v3` omits an optional stored value rather than writing a placeholder;
semantic delta and conflict v2 already carry present/absent value facts and a
field requiredness fact/facet; and the engine already has an `UnsetField`
command. What is missing is a bounded authority for native tables to declare
optional fields, to leave or make a value absent, and to change requiredness.

## Decision

### Meaning of blank

1. **Blank** is a declared **optional** Text or Number field with **no stored
   value** for an entity. It is represented only by omission from the entity's
   `fields` map. There is no `Value::Blank`, null placeholder, or second
   absence encoding.
2. Blank is distinct from present empty Text (`Text("")`), from `0`, from
   `false`, and from any calculation error or diagnostic. None of these is ever
   substituted for blank, and blank is never substituted for them.
3. Requiredness remains a separate, explicit field property. A required field
   keeps today's meaning: every entity must hold a value.

### Native scalar table admission

Native scalar table admission (today: every field required, every row
complete) is extended: a native scalar table may declare **optional Text and
Number fields alongside required scalar fields**. An absent optional entry is
valid; every present entry keeps its direct scalar type requirement. This
decision grants **new authoring authority** for Text/Number only. Classification
is structural: an imported table qualifies for native authoring
when every field is scalar and its optional fields are Text/Number; imports with
optional Date/Boolean remain non-native. “Does not invalidate” means no stored
field or value changes, including the optional declarations import already
creates. Import origin is not an authoring exception. See [#557](https://github.com/nurockplayer/tachiko-work/issues/557)
and the [imported-workbook export contract](../specs/imported-workbook-spreadsheet-export.md)
for current-content export or located whole-file refusal after structural edits.

### Admitted native-table operations

For native scalar tables only, and only for fields of type **Text** or
**Number**:

1. **Declare an optional field** when creating a native table or when adding a
   field (ADR-0040 "add field" is extended). An optional added field needs no
   initializer; supplied initializers set only the named entities; every other
   existing entity is blank for that field. Required fields keep ADR-0040's
   complete-initialization rule.
2. **Insert a row with blanks**: optional fields may be omitted from a row's
   initializers; omitted optional fields are blank. Omitting a required field
   still rejects the whole operation.
3. **Set and clear values**: a typed set validates against the field's
   declared type even when the entity currently has no value for it, so a
   blank cell can be filled. An explicit clear removes the value of one
   optional field of one entity (the existing `UnsetField` command); clearing a
   required field rejects the whole operation. Batch edits and pastes may mix
   typed sets and explicit clears in one atomic operation. Untargeted cells
   remain unchanged, omitted optional fields of newly created rows remain
   blank, and empty Text requires an explicit typed set. Clearing a value that
   is already blank changes nothing for that cell; an operation whose every
   target is unchanged is `NoChange` and records no history. Undo restores the
   prior presence or absence exactly.
4. **Change requiredness** of one existing native Text or Number field:
   - required → optional (relaxation) changes no values and introduces no
     missing-required-field failure; ordinary admission, authorization, and
     final-validation gates still apply.
   - optional → required (strengthening) rejects unless every entity already
     holds a value for the field; it never fills, defaults, or coerces.
   Both directions exist so that Undo/Redo of either change is an ordinary
   forward command under ADR-0033.

Every operation is revision-pinned, gated, and atomic under ADR-0020: it
rejects without publishing a prefix on a stale base, an absent target, a type
outside Text/Number, a required-field omission or clear, or a failing final
validation. In particular, clearing an optional value that a surviving formula
consumes rejects the whole operation if the candidate's final calculation
fails; a blank is never published together with a broken formula. One accepted operation is one user-history action; rejected and
no-change attempts create none.

### Interfaces distinguish blank from empty Text

Public authoring inputs must express **clear/blank** explicitly and separately
from setting empty Text. A client may choose how its own UI maps an empty input
to one or the other (for example, clearing an optional cell), but the semantic
boundary never infers blank from an empty string or empty Text from an omitted
value. Projections report each field's type, requiredness, and, per entity,
whether the value is blank.

Concrete Rust names, transport DTO spellings, diagnostics codes, and
capability footprints remain Provisional, as in ADR-0040.

### Calculation, analysis, and constraints stay strict

This decision adds no coercion and no new aggregate semantics:

- A formula or scalar `min`/`max` whose input is blank reports the existing
  missing-input failure; blank is never treated as `0` or empty Text.
- For an ADR-0036 keyed grouped sum, any missing input required by its
  existing evaluation dependency rules makes the whole result unavailable; it
  never skips a row or publishes a partial total. Binding validation checks
  field type, not requiredness, so a binding survives a requiredness change.
- Existing analysis behavior for missing metrics and group keys is unchanged;
  no synthetic blank group is created.
- Typed comparisons on a blank value are false. An explicit "is blank"
  predicate, skip-blank `SUM`/`AVG`/`COUNT`, and blank-aware sorting are
  deferred to separate decisions that must define their denominators and
  all-blank outcomes.
- ADR-0041 constraints apply only to present values; requiredness alone forbids
  blank. Optionality neither removes a constraint nor expands any runtime's
  constraint admission; present values and constraint declarations remain
  validated.

### Representation, delta, merge, and interop

- **No storage-version change.** `.roproj/v3` already omits optional values and
  stores requiredness; portable package v1 carries that tree unchanged. Writers
  must preserve absence exactly; readers must not fabricate values.
- **Delta and merge** use the existing present/absent value facts and the
  existing requiredness fact/facet. A consumer that does not support an
  encountered change kind fails closed. Qualification must show that a clear
  against a concurrent change of the same value is a structural conflict, and
  that strengthening against a concurrent clear may reconcile structurally but
  fails final validation under ADR-0031.
- **Capacity and profiles.** Existing size and profile limits are unchanged:
  the 128-row native insertion gate and the separate Text/Number capacity
  profile keep their bounds. Each supported profile must reopen, edit, and
  record history for blank values with parity across host and WASM; this
  decision expands no limit.
- **Interop.** Spreadsheet import keeps mapping empty source cells to absence.
  Native-table export writes blank as an empty cell; because CSV cannot
  distinguish blank from empty Text, the export ledger discloses that loss.
- **Compatibility.** Under the pre-release policy recorded in #374, no legacy
  reader is added. A client or binary whose editing profile assumes all-required
  native fields must refuse or reject a document it cannot represent rather
  than silently fill or drop values.

### Deferred work and non-goals

Optional Date and Boolean fields, optional fields outside native tables,
defaults, type changes, skip-blank aggregates, "is blank" predicates, blank-aware
sort/filter semantics, automatic requiredness relaxation, and any storage-format
change are not admitted. Each needs its own authority.

## Delivery and qualification

This ADR adds no production behavior by itself. Delivery follows the accepted
#544 slices, each with its own acceptance, hosted CI, and independent review:

1. Engine: optional Text/Number field declaration, clear, insert-with-blanks,
   and requiredness changes, with history and validation.
2. Runtime and public client: explicit clear inputs, projections of
   requiredness and blank status, and parity across host and WASM.
3. Persistence and interop qualification: canonical and package round trips
   preserving absence; import/export fidelity, including the CSV disclosure.
4. Sheet integration: create optional columns, save partially filled rows,
   clear cells, and display blank distinctly from empty Text.

## Consequences

Users can keep partially filled records in native tables without invented
values, and every derived result stays truthful because calculation remains
strict. The semantic model gains no new value kind. The cost is that clients
must handle blank explicitly in projections and inputs, and summaries over
blank inputs report "unavailable" until a later decision defines skip-blank
aggregates.
