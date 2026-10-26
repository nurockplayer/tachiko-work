# Changelog

All notable changes to Tachiko Work are documented in this file.

## Unreleased

### Added

- Number columns can store a display format with 0–10 decimal places, optional
  grouping, plain/currency/percent presentation, and revision-pinned set/clear
  with Undo/Redo. Table projections, collection copies, explicit v3 saves and
  direct `.ro/v2` preserve it; format-free document bytes stay unchanged.
  Frozen project exports refuse formats with a typed error. XLSX writes an
  equivalent column format and imports exact uniform formats; CSV retains raw
  values and reports omitted formatting. Existing merge contracts explicitly
  refuse formatted documents rather than dropping display metadata.

### Fixed

- Capacity metadata acceptance now states the imported export name policy:
  CSV round-trips quoted Unicode/XML-sensitive headers despite an un-emitted
  worksheet name containing controls; XLSX refuses that name with a located
  finding and unchanged state. XLSX positive round trips use an Excel-valid
  Unicode/XML-sensitive name. Promoted Worker probes and independent XML
  expectations agree; runtime behavior and other capacity assertions are unchanged.

- Imported-metadata export now selects its output scope before reconstruction:
  CSV checks and emits only the selected sheet's scalar values and headers;
  unrelated content and omitted XLSX presentation cannot block it. XLSX uses the
  shared native compatibility inventory for every emitted element, including
  formula numeric literals independently of cached results, reference/worksheet
  names, number-format conditions, styles and widths. Previously unfaithful
  worksheet names, subnormal literals/conditions, oversized format codes, excess
  style resources and widths Excel would quantize now refuse the whole file with
  located findings. Precision disclosures share one bounded location budget.
  Number-format attribute whitespace is encoded faithfully; supported untouched
  cells/formulas/styles retain their semantics and presentation. The imported
  part of #547 now covers every emitted text surface; general writer escaping,
  Tracker/Budget and reader work remain open.

- Imported-metadata XLSX exports check every emitted cell against the shared
  Excel compatibility policy, including untouched Text: refuse literal `_xHHHH_`,
  XML-illegal characters, Text over 32,767 UTF-16 units or 253 line feeds,
  nonzero subnormal Numbers and out-of-range Dates; disclose precision above
  15 digits. Supported cells, formulas and styles stay unchanged, and CSV retains
  its existing lexical behavior. These checks close previously unfaithful or
  undisclosed imported-cell output; shared writer/reader and Tracker/Budget work
  remains under #547. Foreign current field/entity mappings now refuse rather
  than becoming tombstones, and header mappings disclose requested/output
  differences even when collision allocation returns to the historical header.

- Imported CSV/XLSX tables that qualify structurally for native authoring now
  export current renamed headers, appended rows/columns and removed structure,
  including blanks, Undo and v3/package reopen with retained import metadata.
  Existing output limits refuse the whole file with located typed findings;
  surviving cell styles and stable formula references are retained.

### Added

- The Rust runtime, WASM bridge and public TypeScript client now author v3 field
  constraints: closed Text literal lists (also filling `dropdown_options`) and
  inclusive Number ranges. `setColumnConstraint` sets or clears one declaration;
  constrained edits publish nothing and reject with a typed
  `constraint_failure` carrying the allowed shape and offending row count, and
  declaring a constraint that existing values violate is refused atomically.
  Constrained projects save as `.roproj/v3`, reopen with constraints and values
  intact, and legacy v1/v2 `exportProject` refuses them with
  `legacy_export_unsupported` instead of dropping the declaration. The kit
  advertises `setColumnConstraint` and the `columnConstraintProjection` marker.

- Keyed grouped-sum projections now carry the overall total alongside the
  category groups. `total` is derived from the same evaluation at the same
  revision and is `null` whenever the result is unavailable, never zero or a
  stale value; the public TypeScript client exposes `number | null`.

- Strengthening an optional native Text/Number column now reports
  `required_values_missing` with the blank row count and column identity. The
  rejection preserves the current revision and undo history; host and WASM
  expose the same failure projection.

- Table columns expose optional `display_name_explicit: true` through the Rust,
  WASM and public TypeScript client projections, sharing the imported-export
  presence rule. Clients can retain metadata headers unless a stored display
  name exists, even for a rename equal to the generated key. The kit advertises
  `columnDisplayNameExplicit`; Undo and v3/package reopen preserve name presence
  without changing storage formats or rename semantics.

- The experimental TypeScript client and exported Worker kit expose optional
  Text/Number columns, sparse row initialization, explicit `clearField`, mixed
  typed/raw pastes, and `setColumnRequiredness`. Projections expose requiredness
  and sparse blank metadata with validated omission defaults; clear stays distinct
  from present empty Text and Undo/Redo preserves presence exactly.

- Rust semantic commands and the Designer runtime admit optional native Text/Number
  columns, sparse row initialization, explicit atomic set/clear edits and pastes,
  and revision-pinned requiredness changes with exact presence-preserving Undo/Redo.
  Table projections expose requiredness and blank cells. Blank remains distinct
  from empty Text; storage formats and capacity limits are unchanged.

- Native-table CSV exports add a conditional `lossy_on_export` finding,
  `native_tables_csv_blank_empty_text`, located at the selected collection, when
  that collection declares an optional Text field and any exported row has a
  blank or empty Text value in it, because CSV cannot distinguish the two. XLSX
  keeps blank cells distinct and does not emit it. The generic
  `native_tables_csv_values_only` finding now covers only typing and formatting
  loss. Canonical `.roproj/v3` and portable package round trips and both export
  formats are qualified to preserve blank optional values exactly.

### Fixed

- The browser kit artifact manifest's exposed client-method inventory now
  lists every public client method and entry function, adding the previously
  omitted `exportNativeTablesSpreadsheet`, `openImportedProject`, `newTable`,
  `duplicateCollection`, `explainField`, `createKeyedGroupedSum` and
  `queryKeyedGroupedSum`. A browser-client test now keeps the inventory in
  parity with the actual public surface.

- Native-table worksheet/header names use a bounded fixed-point caseless key
  (up to four uppercase-then-lowercase iterations), so `ß`, `ẞ`, `SS` and `ss`
  consistently collide. Allocation and final writer validation share the key;
  deterministic suffixes and source/output ledger mappings preserve identity.

- Native-table exports refuse fields with semantic constraints instead of
  dropping them: XLSX refuses the whole document, CSV the selected collection,
  with a finding naming the collection and field. Native CSV also refuses
  XML-illegal Text and headers with located findings. Worksheet allocation
  repeatedly removes boundary whitespace/apostrophes before disambiguation,
  with regression coverage for quoted `Foo` names and Greek caseless names.
  The interop profile now states the 31 UTF-16-code-unit worksheet-name limit.

- Native-tables XLSX uses one compatibility policy for name allocation and
  final validation. It renames reserved `History` worksheet names with a
  deterministic ledger mapping, measures the 31-unit name limit in UTF-16,
  refuses XML-illegal characters, more than 253 line feeds per cell, and Dates
  outside Excel's 1900-01-01 through 9999-12-31 range with located findings, and preserves legal Text/header
  whitespace. A sourced constraint inventory, table-driven boundary tests and
  independent LibreOffice readback cover the scalar Excel surface. Other
  profiles and CSV retain their existing behavior.

- Native-tables exports disambiguate worksheet names and headers with an
  uppercase-then-lowercase Unicode comparison key. Native XLSX refuses
  subnormal Numbers that Excel would turn into zero, normalizes signed zero,
  and discloses possible Excel rounding above 15 significant decimal digits
  using at most 20 located findings plus a total count. CSV keeps full Number
  precision and explicitly records that fact in its ledger.

- Native-tables XLSX export refuses literal SpreadsheetML `_xHHHH_` escape
  sequences in Text, headers and worksheet names, and Text cells or headers
  above Excel's 32,767 UTF-16-code-unit limit. Blocking findings identify the
  location so output cannot silently change or truncate the source Text.
  CSV and other XLSX profiles retain their existing behavior.

- Browser kit folder selection accepts `.roproj` root folder names containing
  backslashes while still refusing backslashes in serialized relative paths.

- Browser kit folder selection packages complete `.roproj` directories in
  storage-owned canonical order regardless of browser enumeration order.
  Unknown, duplicate, missing and unsafe paths are refused before packaging;
  Rust admission retains strict canonical-byte and v3 transfer-order checks.

### Added

- Revision-pinned native scalar tables export to typed XLSX or selected-table
  CSV through `exportNativeTablesSpreadsheet(expectedRevision, collection,
  format)`. Rust derives canonical order and values; deterministic name/header
  mappings and excluded summaries, charts, history and presentation appear in
  the fidelity ledger. A dedicated outbound profile covers native admission
  geometry; mixed XLSX documents and over-limit output are refused.

- The public browser client's `TrackerCommand` exposes atomic `edit_cells`
  batches for native and imported tables through the existing worker path,
  with one publication and one Undo/Redo unit per accepted batch.

- Accepted ADR-0044 defines complete saved grouped-sum definition facts in
  semantic delta v2. The in-process engine now emits stable-ID create, change
  and delete facts with complete role bindings, restoring definition commands
  on named and already admitted constrained occurrences with full publication
  evidence. Frozen v1 and definition-changing conflict v2 remain fail-closed;
  browser constraint admission under #452 is unchanged.
  Browser summary creation and rebinding now record one Undo/Redo action,
  restore complete saved bindings and clear redo only after publication.

- Native tables and columns retain case and Unicode display names separately
  from stable IDs and keys. Column rename changes the label with undo/redo;
  projections expose the effective name and invalidate table/column labels even
  for empty tables. Every supplied command name passes spelling admission before
  candidate construction, including names overwritten later in a batch.
  Named projects use canonical v3 and
  portable-package/v1; older writers refuse names and older binaries cannot
  reopen named documents. Existing nameless v3/package goldens stay unchanged.

- First-public portable package/v1 now carries exactly canonical `.roproj/v3`,
  preserving Date, saved grouped-sum definitions, constraints and empty shards.
  CLI pack/unpack/compare and browser portable export/open/verify share this
  profile. Older source trees require explicit `migrate_roproj_to_v3`; developmental
  packages are refused. Browser verification accepts constrained packages, while
  open retains the existing unconstrained-field capability limit.

- The experimental browser client can explain a field at an exact resident
  revision, returning its authoritative value, formula source, direct inputs
  with current values, direct dependents and diagnostics. Native and Worker
  reads preserve history and occurrence state and use the existing query budget.

- The experimental browser producer admits one bounded table with 1–16 Text
  or finite Number fields, at most 8,406 rows and 25,218 logical cells through
  import, scalar edits/history, project save/reopen and both CSV/XLSX exports.
  Ordinary admission stays first; legacy three-Text eligibility is preserved,
  and new shapes need more than 64 rows for enlarged retry. Complete projections
  and metadata use private finite
  budgets; generic tables, formulas and hostile-file defenses retain their
  existing limits. See the producer's capacity contract for the exact profile.

- Saved spreadsheet carriers have metadata-aware native
  `open_imported_project` and optional Worker `openImportedProject` routes.
  Actual open independently revalidates mappings, XLSX/CSV closure and the
  complete reply before replacement; inspection does not authorize open and
  there is no ordinary-open fallback. Eligible non-v3 Text/Number candidates may use
  the capacity profile, while v3 retains strict ordinary admission and origin
  behavior. Enlarged Text-capacity Deduplicate removals refuse before
  publication. Exact exported-kit producer qualification and Sheet consumer
  pin/save/restart qualification remain separate gates.

- The experimental browser client can explicitly export opaque `.roproj/v3`
  projects with Date and saved grouped-sum definitions together, then reopen
  them through existing project operations. V3-opened occurrences require the
  explicit export method for saves; rollback must retain a v3-capable reader.

- Rust workspace callers can explicitly select the in-process
  `tachiko.semantic-conflict/v2` merge preview. It adds atomic field-constraint
  conflict facts and validates unchanged keyed grouped-sum bindings for each
  input role; the existing v1 merge stays frozen. Application opt-in and public
  transport remain separate follow-up work.
- Rust callers can explicitly select the in-process `tachiko.semantic-delta/v2`
  comparison for admitted documents. It emits deterministic, complete direct
  facts for field constraints and the inherited semantic changes; the existing
  Provisional diff remains fail-closed for constrained meaning. Application
  opt-in and public transport are separate follow-up work.
- Rust storage callers can now encode and strictly read canonical `.roproj/v3`
  trees with bounded field constraints and Date values, and explicitly migrate
  admitted v1/v2 directories to a distinct no-clobber v3 destination. Ordinary
  Designer save remains version-bound; explicit v3 save/open and portable
  packages now use the existing v3 codec.
- The semantic core now recognizes the bounded durable Text literal-set and
  inclusive Number-range constraints. Existing core and resident validation
  reject invalid declarations, direct values, and successful formula results;
  frozen storage, diff, merge, and legacy inspection boundaries refuse this
  newer meaning rather than omitting it.
- Native tables can remove selected rows while preserving stable row identities and failed-operation selections.
- Native Designer tables can add a required Text, Number, Boolean, or Date
  column with one explicit typed initializer for every current row, rename a
  scalar column without changing its stable identity, and remove an unused
  scalar column through the existing revision-pinned semantic lifecycle and
  Undo/Redo history.
- The browser Designer can duplicate an ordinary data collection into a new
  independently addressable collection with fresh semantic identities,
  remapped internal formulas, preserved external references, and one bounded
  reversible session action. Invalid, stale, or over-capacity copies are
  rejected atomically.
- The macOS Designer desktop host can open a supported local `.ro` selected
  through Finder/LaunchServices on cold start or while already running, routing
  the complete OS-selected document set through the existing safe local-document
  ingress without granting ambient filesystem access.
- The browser Designer now offers an ordinary active-table Find / Replace
  workflow for selected Text fields. It uses case-sensitive literal matching,
  stable semantic targets, revision-pinned atomic previews, and the existing
  session Undo/Redo and save/reopen path; formulas and non-Text values remain
  outside its replacement scope.
- The Designer can create one bounded ordinary native table with user-defined
  Text, Number, Boolean, and Date columns. New rows require a complete typed
  input, invalid values are rejected without publication, and the table can be
  saved, reopened, and edited through the existing private runtime boundary.
- The experimental Designer consumer kit is built from one captured Git commit
  and includes an exact asset digest inventory, capability names, and existing
  license notices. Packaging rejects a dirty checkout and publishes only to an
  absent destination using an atomic no-replace rename. The same kit exposes
  revision-pinned canonical `.roproj/v1` files, genuine portable `.ro` codec
  export/verification/open, and the runtime's actual occurrence identity.
  Unsupported v1 data rejects without changing the active occurrence; existing
  private project export retains its Date-capable round trip.
- The Designer can now create one saved live keyed grouped summary over the
  bounded Orders/Products definition, recompute it from current source values,
  and save/reopen it through canonical `.roproj/v2`. The explicit v1-to-v2
  migration path preserves v1 reads without rewriting on open; v2 definitions
  retain stable bindings only and never persist evaluated groups, diagnostics,
  or caches. Existing delta, merge, and portable-package v1 boundaries reject
  this v2-only meaning rather than dropping it.
- The browser Designer can export the exact native Driver Tracker profile as
  values-only CSV or typed XLSX for 0–128 current rows. This outbound-only path
  preserves stable row mapping and supported cell styles without fabricating
  spreadsheet-import metadata; incoming/shared spreadsheet admission remains
  capped at 64 rows.
- The browser Designer can export the current native Budget view as values-only
  CSV or its admitted canonical source collections as typed XLSX. XLSX retains
  stable mappings, bound formulas, typed Date values, and supported cell styles;
  CSV explicitly discloses formula, additional-collection, and presentation
  loss. This is outbound-only and does not add spreadsheet reimport, a general
  formula evaluator, or Excel/Sheet-model compatibility.
- Maintainers can package an exact-commit local Designer release candidate and
  verify its static artifact with isolated browser journeys. The accompanying
  Driver acceptance matrix keeps remaining product and public-promotion gates
  explicit; this preparation does not publish or promote a release.
- The Designer creates bounded column and line reports from selected stable
  source rows and numeric fields, updates from current semantic results, and
  saves editable chart settings with browser projects. Current charts can be
  shared as static PNGs; spreadsheet exports explicitly disclose chart loss.
  Accepted chart create/edit/delete actions join the same app-private bounded
  chronological coordinator as semantic and view actions, without clearing
  Rust semantic history. Cancelled, rejected, unchanged and stale drafts do
  not alter either history direction; reopening starts a fresh history. This
  preserves their settings without adding a portable chart format or general
  spreadsheet chart compatibility.
- The browser Designer exposes bounded session Undo/Redo controls alongside
  ordinary Budget and imported tables, so accepted formula/copy and cleanup
  publications can be reversed through the existing Rust lifecycle. The
  session remains app-private and non-durable; no public or portable history
  contract is added.
- The Designer imports bounded CSV/XLSX workbooks through explicit inspection,
  typing and fidelity review; supports previewed cleanup, existing typed edits
  and numeric formulas; and downloads current CSV/XLSX output. Browser project
  saves retain source bytes, mappings, layouts and compatibility findings
  atomically. Unsupported features remain blocked or explicitly disclosed;
  this does not establish full Excel compatibility or a portable project format.

### Fixed

- CLI directory read commands and both validation entrypoints now accept exact
  canonical `.roproj/v3` trees, including portable package unpack output, through
  the existing strict storage reader. Canonical v1 reads remain supported;
  other versions and invalid trees are refused without modifying the source.

- V1 semantic patch batches that change saved grouped-summary definitions
  together with any other command now reject before publication, preventing
  omitted ordinary change evidence and skipped per-change admission.
  Definition-only batches retain their existing lifecycle.

- CSV/XLSX exports of imported work and native Budget work explicitly disclose
  omitted saved grouped-summary definitions with one aggregate fidelity warning,
  including definitions whose results are unavailable. Spreadsheet contents and
  native saved copies are unchanged; retain a Tachiko saved copy for recomputation.

- Small spreadsheet inspection keeps the ordinary reply limit unless the original
  source qualifies as safe legacy three-Text capacity. Prospective Number
  feasibility no longer grants oversized previews to small unsafe-Text sources.

- XLSX import rejects scalar type/value-carrier mismatches before empty-value
  defaults or formula-cache recovery can discard their contents. Valid inline
  strings, numeric values, legitimate numeric blanks and supported formula caches
  keep their existing behavior.

- Formula authoring in one open Designer occurrence is now a bounded reversible
  semantic action: Undo restores the exact prior Number or formula, and Redo
  re-applies the accepted formula through the normal lifecycle. The narrow
  formula-to-Number inverse remains private to this history composition; generic
  scalar editing over formula cells is still rejected.
- Net-zero AtomicBatch execution now reports typed NoChange after guarded
  authorization and exact-base checks, without installing state, advancing a
  semantic revision, emitting publication invalidation, consuming Approval, or
  recording a publication receipt. AI and Designer mappings preserve this
  nonpublication outcome.

### Changed

- [ADR-0043](docs/decisions/ADR-0043-native-display-names-separate-from-keys.md)
  accepts native table/column display names separate from IDs and keys, with
  exact Unicode comparison and label-only column rename. It supersedes the
  unreleased v3/package-v1 definition without v4 or down-conversion; older
  binaries refuse name-bearing documents. This is authority only: native implementation
  follows separately under #529, and Sheet consumer work is separate.

- ADR-0042 redefines first-public `tachiko.portable-package/v1` as a
  deterministic ZIP32 envelope over exact canonical `.roproj/v3` (Date, saved
  definitions, and constraints), superseding the unreleased developmental
  v1-over-`.roproj/v1` binding. #519 implements the CORE package codec and
  tooling; developmental packages are explicitly refused. The browser runtime
  supports Date and saved definitions within its ordinary v3 profile; constrained
  packages verify but refuse open, and capacity-profile documents exceeding that
  profile refuse portable export. Sheet attachments and consumer qualification
  remain separate; this does not complete #488.
- The legacy first-party Designer application, its Tauri/PWA hosts, and its
  product-only release route have been retired. The experimental browser-client
  producer and its standalone Rust/WASM runtime remain available for technical
  consumer qualification; this repository no longer ships a first-party UI.
- Semantic merge now admits only fully validated, completely calculated states
  of one continuing `DocumentId`; emits deterministic
  `tachiko.semantic-conflict/v1` evidence with typed stable targets, direct
  facets, explicit absence, three closed conflict kinds, parent suppression,
  and canonical semantic ordering; and keeps conflict-free candidate
  validation/calculation failures in the existing diagnostic boundary.
- Semantic document, schema, field, and entity identity is now opaque and
  stable, while mutable human keys remain the authoring address. Renames retain
  bound references/formulas and diff/merge continuity instead of rewriting or
  replacing the object.
- Valid legacy direct `.ro` v1 documents now migrate deterministically in
  memory to stable IDs. Explicit saves emit lossless `direct-ro/v2`; merely
  opening v1 never rewrites durable bytes.
- The normal direct-JSON reader now applies a shared Provisional 8 MiB Stage-0
  envelope to legacy v1, v2, and untrusted version envelopes before UTF-8/JSON
  inspection. This intentionally rejects otherwise-valid legacy v1 input over
  8 MiB; the value is a representation profile, not a semantic or product
  maximum.
- Direct `.ro` v2 adopts finite normalized binary64 semantics, ECMAScript
  shortest-roundtrip numeric spelling, stable-ID collection order, and bounded
  number/formula resources. Runtime export correspondingly advances to
  `runtime-export/v2`.
- The closed semantic scalar model now includes date-only Gregorian `Date`
  values with canonical `YYYY-MM-DD` admission, deterministic civil-date
  ordering, direct-ro/v2 persistence, Semantic API/resident/Designer
  projections, and Native/WASM parity. Frozen legacy direct-ro/v1 and
  `.roproj/v1` remain unchanged; Date-bearing `.roproj/v1` export rejects
  explicitly until a future representation version is accepted.
- Hardened legacy direct `.ro` v1 persistence with storage-owned historical
  DTOs, strict UTF-8/JSON/version handling, recursive closed-world decoding,
  and specification-ordered canonical output while preserving admitted v1
  wire meaning and canonical bytes.
- `tachiko-storage::FormatError` now adds distinct UTF-8, JSON-duplicate,
  version-envelope, and representation failures. Existing `Json`,
  `UnsupportedVersion`, and `InvalidDocument` shapes remain available, but
  downstream exhaustive matches must account for the new variants.
- First-party semantic validation now returns one deterministic
  `ValidationReport` with symbolic meaning, machine severity, stable-ID
  subjects and related facts, opaque validator provenance, and
  presentation-only paths/messages. Workspace validation is shared across
  queries, mutations, and merge finalization while authoring projection remains
  an explicit operation gate.
- Field capability discovery now requires disclosure coverage for both the
  concrete `EntityField` and corresponding schema-owned `SchemaField`, so a
  fine-grained field grant cannot disclose declared type or Reference target
  schema metadata.

### Added

- The first-party browser Designer now offers a bounded monthly Budget journey:
  named views over two collections, Gregorian Date entry, graphical reference
  selection, Rust-authoritative arithmetic authoring and atomic relative/fixed
  formula copy that bind references to stable identities,
  and Date-bearing Browser Save/close/reopen through a private `direct-ro/v2`
  host record. Currency and percentage are persisted presentation choices over
  Number, not Money or nominal Percentage semantics. This remains a bounded
  Driver slice rather than Excel formula or Sheet/Cell-model parity.
- The first-party browser Designer now offers a bounded Driver Tracker journey:
  create a fresh project, atomically paste a typed TSV range, edit cells, append
  and remove rows, and Save As / close / reopen through the existing project
  lifecycle. It reuses the Boolean dropdown, keeps formatting in a view
  sidecar, sorts view rows deterministically, and supports bounded in-session
  semantic undo/redo through the same revision-safe publication path. The
  slice does not add Excel parity or a general spreadsheet model.
- Provisional `AppendEntity` and `RemoveEntity` Semantic API command families
  now share exact-base proposals, independently scoped capabilities,
  document-wide structural disclosure, final validation, and atomic
  publication with existing field and formula commands. Dangling references
  reject removal unless the same atomic batch repairs them.
- A provisional subject-first Semantic API field capability discovery Query
  now projects the existing Number, Text, Boolean, Date, Reference, and Formula
  operation rules through workspace-engine and the AI boundary. Discovery and
  mutation share type/formula applicability authority, disclosure remains an
  independent Query capability, Reference targets are not enumerated, and no
  conversions are advertised.
- A deterministic repository command now exports a self-contained, explicitly
  unstable Designer frontend client kit containing a bounded TypeScript/ESM
  wrapper, Worker support, and the Rust-authoritative WASM runtime without npm
  publication or a stable SDK/wire promise. A separate throwaway consumer uses
  only the generated kit to open the Product Gap dogfood project, query a typed
  table, publish a revision-safe scalar edit, observe calculation/diagnostic
  projections, prove stale-edit no-drift from exact exported bytes, and reopen
  the canonical round trip.
- The first-party browser Designer now admits a second bounded ordinary domain
  without Moonfall-specific collection or control-formula requirements. A
  repository-owned canonical Product Gap project dogfoods opaque semantic IDs,
  typed Text/Number/Boolean editing, deterministic priority formulas,
  selective refresh, rejected-edit no-drift behavior, and create-only Save As
  through teardown/reopen while the existing Moonfall journeys remain intact.
  The fixture is product evidence, not a public template or project-management
  authority.
- The first-party browser Designer table now edits directly stored Text,
  Number, and Boolean values through the same Rust-authoritative,
  expected-revision publication path. Typed controls remain projection-derived,
  preserve publication-derived selective refresh and diagnostics, and the
  supported Save As / teardown / reopen journey preserves the resulting typed
  values without adding a frontend canonical model or public wire contract.
- The browser Designer can now admit a user-selected canonical `.roproj/v1`
  through the existing Rust storage/workspace authorities, edit it in a fresh
  resident occurrence, and Save As an exact revision to an absent-only
  IndexedDB destination. Browser projects reopen cleanly after occurrence
  teardown or page reload; rejected/cancelled admission and failed Save As
  preserve the current occurrence and existing durable destinations. Dirty
  occurrences require confirmation before Open or Close can discard them and
  warn before browser unload. Canonical project
  transfer is independently bounded at 64 MiB and remains a private host/WASM
  mechanism, not a public storage or wire contract.
- A first-party browser Designer vertical slice now runs the Rust-authoritative
  resident Moonfall workspace inside a Web Worker/WASM host. It renders a
  bounded typed table, publishes directly stored Number edits against the
  expected semantic revision, consumes publication-derived invalidation to
  refresh only changed and dependent formula fields, carries unrelated
  projections forward, and exposes structured stale/validation failures without
  frontend canonical drift. Its app-local DTOs and WASM ABI remain private,
  Provisional delivery mechanics rather than public SDK, storage, or Semantic
  API authority.
- The provisional resident workspace session now retains rebuildable,
  revision-scoped formula values/failures, dependency and reverse-dependency
  indexes, validation diagnostics, and the address index. Publications reuse
  Issue #94's exact projection-change classification, recompute formula dirty
  roots plus old/new reverse-dependent closures, and fall back to the full
  ADR-0018 oracle when retained impact cannot be trusted. Generated native
  multi-revision oracle tests cover mutations, failures, cycles, renames,
  deletion/restoration, and fallback recovery; the production resident
  publication corpus continues to match natively and under WASM. The retained
  state is runtime-only, is never serialized, and defines neither semantic
  identity nor a public cache protocol or performance SLA.
- The provisional resident workspace session now exposes occurrence-and-
  revision-pinned, stable-ID-selective entity and field projections without
  returning the whole semantic `Document`. Stored literals, bound formula
  definitions, calculated outcomes, stable-subject diagnostics, and mutable
  human addresses remain distinct. Each exact guarded publication derives
  occurrence/revision-paired stale entity and field projections plus
  deterministic downstream calculated projections from fresh full-oracle
  dependency facts, including renames and output-equal dependents. Internal
  consumers can identify stale revision-keyed projections without recreating
  semantic meaning; no public cache contract is introduced. Native/WASM
  conformance covers the same bounded query/mutation corpus; no retained cache,
  UI, persistence, or public wire DTO is introduced.
- A provisional `tachiko-workspace-engine` resident workspace session now owns
  one authoritative semantic `Document` occurrence and an internal monotonic
  `SemanticRevision`. Validation and calculation queries are revision-pinned
  without publication; approved `SemanticPatch` execution installs only
  through the existing guarded `SemanticPublicationAuthority`, advances once,
  and rejects stale, failed, unauthorized, or wrong-occurrence attempts without
  changing state. Full snapshots remain explicit detached exports, trusted time
  stays host-supplied, and the production session has matching native/WASM and
  `ai-api` composition evidence without defining a public wire/session DTO.
- A provisional provider-neutral bounded semantic Analysis Query now runs
  through the shared workspace authority and structured CLI. It implements
  stable-ID schema selection and optional narrowing, typed AND predicates,
  zero/one non-Formula grouping field, exact membership and Count, Number
  Min/Max, bounded per-member Number observations, same-definition paired
  exact contexts, and reproducible lineage. Formula-backed values reuse the
  ADR-0018 calculation oracle; candidate-domain and complete-result disclosure
  reuse ADR-0026 Grants and deny the whole assertion rather than aggregating a
  visible subset. Results remain ephemeral, native/WASM conformance is
  exercised, and no public wire/SDK, persistence, Sum/Mean, ranking,
  statistics, or runtime/history contract is introduced.
- A provisional provider-neutral M04 vertical slice now exposes structured
  formula-reasoning and exact-snapshot read-only Number-override scenario
  Queries through the workspace and CLI. Typed formula updates now bind their
  complete stable-reference meaning before entering the existing
  SemanticPatch Propose/Execute/Approval lifecycle. The implementation reuses
  the authoritative calculation, dependency, validation, diff, authorization,
  and publication paths; scenarios remain transient and non-publishing, and
  native/WASM conformance covers all three operations without defining a
  public wire or SDK contract.
- The Game Dev Alpha acceptance journey now starts from the durable Moonfall
  canonical `.roproj`, edits it semantically without touching internal files,
  materializes a distinct accepted tree, proves deterministic review,
  calculation, validation, and export, and rejects an invalid formula input
  locally. The same fixture now drives the optional ordinary-Git branch/CI
  proof, including localized raw diff, semantic parity, package consistency,
  and fail-closed invalid-state validation.
- A provisional provider-facing `tachiko-ai-api::security_boundary` now keeps
  system/developer/user instructions, trusted semantic metadata, and untrusted
  document/import/plugin/model content explicitly separated. Untrusted typed
  Propose/Execute requests receive effective identity and time only from a
  trusted host context, must resolve to an active Delegated lifecycle principal,
  and cannot reuse a Human session principal to avoid Approval. They delegate to
  the workspace lifecycle; raw semantic or storage mutation and persistence/
  filesystem/network/process/Git/plugin/deployment/credential effects are
  rejected with stable machine codes.
  Model explanations and validation claims remain inert evidence. Concrete
  public authentication/session/revision/transport mechanics remain deferred,
  and actual host/plugin capability mechanisms remain separately owned.
- A provisional provider-neutral SemanticPatch lifecycle in
  `tachiko-workspace-engine` now evaluates stable-ID typed field-value Commands
  and ordered AtomicBatch proposals through the shared validation, formula, and
  semantic-diff path. It derives relational scoped capabilities, gates review
  evidence with independent Query authority, binds finite one-shot Human
  Approval to the exact proposal/base/executor/policy context, publishes only
  through an opaque revision compare-and-publish seam with a fresh trusted-time
  authorization callback, consumes Approval with successful publication,
  verifies installed state, and retains disclosure-safe execution receipts.
  Issue #93 now supplies the provisional resident session/revision mechanics;
  the provider-facing hostile boundary composes this lifecycle through
  `tachiko-ai-api`, while actual external-effect capabilities remain separate
  host/plugin work.
- An optional provider-neutral Git/CI adapter now keeps canonical `.roproj/v1`
  members as LF text, accepts exact project trees in existing read-only
  semantic commands, and composes canonical/workspace validation with
  read-only generated-package consistency checks. The executable journey uses
  ordinary Git without a host API or semantic Git identity.
- Accepted a semantic authorization contract for scoped Grants, trusted
  authorization-footprint derivation, and exact finite Human Approval with
  at-most-once successful publication. Issue #29 now provides the provisional
  in-process lifecycle implementation, and #30 adds the provisional hostile-
  client adapter plus safe denial projection. Public authorization/wire DTOs
  Public resident revision/session transport mechanics remain deferred; Issue
  #93 supplies the current provisional in-process implementation.
- A provider-free, read-only Semantic Analyst slice now exposes deterministic
  document inspection, formula and dependency analysis, semantic change and
  affected-area analysis, and validation findings through shared Rust queries
  and structured CLI JSON. Results carry caller-owned source-state labels and
  document identity without introducing mutation or revision semantics.
- An Accepted `tachiko.portable-package/v1` specification and executable
  evidence fix one deterministic ZIP32 envelope over exact `.roproj/v1`
  bytes, a path-separated SHA-256 payload root, exact byte round trips, and
  tracked-source conflict behavior. Production storage now implements the
  bounded exact-byte codec, fail-closed content framing, atomic no-replace
  pack/unpack, and read-only source comparison. `tachiko roproj pack`,
  `unpack`, and `compare-package` compose those operations with workspace
  validation; fixed package evidence agrees on native and WASM.
  (Superseded by ADR-0042; see Changed.)
- Production `tachiko-storage` now encodes and decodes the canonical 18-file
  `.roproj/v1` tree. Explicit `tachiko roproj materialize`, `validate`, and
  `canonicalize` commands provide a standalone host workflow with staged,
  source-preserving, no-clobber publication, while fixed exact-tree evidence
  agrees byte-for-byte on native and `wasm32-unknown-unknown`.
- An executed production-semantic conformance corpus compares normalized
  values, complete formula failures, stable diagnostic observations,
  dependency/SCC results, and stable formula projection byte-for-byte on native
  and `wasm32-unknown-unknown` builds.

## [0.1.0] - 2026-08-20

### Added

- Typed semantic documents with schemas, entities, scalar values, references,
  validation, and canonical versioned `.ro` serialization.
- Deterministic formula evaluation with dependency tracking, cycle detection,
  semantic diffing, and derived-impact reporting.
- Typed three-way semantic merge with structured conflicts and validation of
  successful merge candidates.
- A complete game-balance CLI workflow for initializing, browsing, explaining,
  editing, validating, calculating, diffing, merging, and exporting documents.
- Validated entity duplication, relationship-safe rename, and non-cascading
  removal with actionable dependent field paths.
- Bounded formula parsing and canonical formatting plus validated formula
  authoring through the workflow and CLI.
- Evaluated runtime JSON export that preserves semantic document and entity
  identity.
- Read-only AI-oriented structure, formula, and impact APIs plus validated,
  approval-required scalar and bounded formula suggestions.
- A checked-in Moonfall game-balance example and executable first-user,
  collaboration, entity-lifecycle, and formula-authoring smoke tests.
- Reproducible checksummed native archives for four targets, including the
  project license texts and a deterministic audited notice for every locked
  all-target CLI dependency.
- A release-equivalent local gate that selects stable Rust for ordinary builds
  and independently proves compatibility with exact Rust 1.85.0.

[0.1.0]: https://github.com/nurockplayer/tachiko-work/releases/tag/v0.1.0
