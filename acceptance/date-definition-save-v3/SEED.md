# Work #374 explicit-v3 acceptance seed

Status: local acceptance-only preparation, unqualified and not Ready. The exact base is `34de067d5e4ff3813f27281e7140e14e8ffff8db` (tree `1f80379bbdf43385dc31c63eab3b51c605fdd13c`). The worktree was checked before editing: the seed files were local additions and no production source was modified. A bounded fixture/API `cargo check` passed after disabling the inherited `sccache` wrapper; no behavior tests, browser tests, heavy Cargo suites, GitHub writes, commit, push, PR, or merge were performed.

The controlling contract is Work #374 amended ruling/comment `5920381363`. Keep the v1 18-file bridge unchanged. The explicit v3 selected export uses TWDPROJ1 framing and the exact 19-file `.roproj/v3` path order. The v3-origin marker applies to the complete catalogue and actual `OpenedProjection`, blocks ordinary-export downgrade, survives Bootstrap/Edit/Undo/Redo and failed replacement, and clears on successful New, Import, legacy replacement, or Close. This seed does not pick any additional durable formats or API/error contracts.

The author-provided prior seed diff digest was `8fd9654e63643c6b5489a5e316aa07b52ed3d8fa1b621028363e09558f7e1eb5`. The initially available `git diff --binary` digest in this checkout was `ed6952fcd8b1d0a2bbfc9e3d483d4464ceb4f4bf80016d5105d370b103c0e605`; it did not include the untracked capture runner. The prior digest is retained as provenance but was not treated as proof that earlier repairs happened. All on-disk files were inspected directly. Preserve this seed's original base and additions; do not rebase it blindly. The user reports main later advanced to `33b26e2a8d34593030118c53dbe79b6f8fdc6e0e`, with only four Work #491 storage-test files changed from this base and no runtime/producer behavior change. Those storage files are outside this seed's allowlist. Recheck applicability at seed freeze; do not change the baseline or touch those files here.

## Frozen fixture and independent oracle

`fixtures/mixed.csv` is the existing 100-byte source. The frozen expected importer identity is Document `import_00000000-0000-4000-8000-000000000001_0001`; Schema `_0002`; fields `_0003` through `_0007`; entities `_0008` and `_0009`. The complete `import_..._` prefix is part of every identity. The expected title is `Imported workbook`; collection/row keys are `sheet_1` and `sheet_1_row_1/2`; all fields are optional and unconstrained. Types are Text, Text, Number, Number, then Date or Text by profile. Literal cell values are PEN/Stationery/4/200/2026-09-28 and NOTE/Paper/5/200/2026-09-27. The full keyed-sum identity and Orders/Products schema and field bindings are literal. Summary results are Paper=1000 and Stationery=800 at quantity 4, 1200 at quantity 6, and 1400 at quantity 7. Three separately hashed immutable legacy `.twd` controls remain under `fixtures/legacy/`.

## Criterion-to-case inventory

| Criterion | Executable case(s) and frozen proof |
| --- | --- |
| Fresh admission boundary (blocker 1) | `exact_full_catalogue_opened_projection_boundary_and_plus_one` independently sizes complete bootstrap/catalogue/table projections at fresh `resident/0` to 65,536 and 65,537 bytes. It separately assembles bounded Bootstrap and QueryTable responses at live `resident/19`; each is below the cap, the wrapper adds 23 bytes, and the combined projection is exactly two bytes larger. The positive path closes and destroys the original runtime before the fresh `resident/0` open. The one-over path expects the typed `ProjectionTooLarge { actual: 65537, maximum: 65536 }` at selected export and checks Inspect/Open candidate refusal while proving the resident occurrence, projections, origin marker, original Undo entries, and original Redo entry still work. Worker independently constructs the same 16-definition resident through real publication calls, measures Rust-shaped JSON with `.0` numeric tokens and the 23-byte wrapper, selects at resident/19, closes, and freshly reopens the saved at-limit bytes. |
| Honest qualification and capture (blocker 2) | `run-native-capture.sh` returns `NOTRUN_MISSING_METHOD` before Cargo if the Rust explicit-target seam is absent; otherwise it requires the exact 16 native test names and writes `native-result.json` only after every case succeeds and every fresh output and receipt exists. It requires a clean committed candidate tree and committed acceptance source files. Missing capture inputs, compile failure, missing case, or partial data are NOTRUN/incomplete, never behavior RED. The Worker tests contain no `test.skip`; missing API, capture, stale receipt, kit, or required case fails explicitly. `run-worker-acceptance.sh` requires two Worker cases, zero skips, and exact capture/kit identities before writing the completion manifest. |
| Actual Worker journey (blocker 3) | `actual Worker imports, explicitly exports and reopens every v3 profile` runs a real module Worker/WASM client for Date+definition, Date-only, definition-only, and neither profiles. It pins host-generated import identity, imports the immutable CSV, makes the explicit September 26→28 Date/Text edit, publishes the definition where applicable, exports the initial quantity-4 project, closes and freshly reopens it, checks the full literal projection and encoded schema/type/kind/keys/required/constraint/value/binding record, edits Date/Text and quantity to 6, explicitly re-exports and freshly reopens, checks again, edits/Undo/Redoes, proves ordinary-export refusal after Bootstrap/Edit/Undo/Redo, re-exports and performs a second fresh reopen. The failure/reset case checks typed failures for version 99, malformed framing, wrong/duplicate/missing/extra/unsafe paths, malformed representations and definitions, invalid catalogue references/bindings, valid constrained inputs, canonical 4,096/4,097-byte title controls, exact and over-transfer limits, malformed legacy replacement, and fresh 65,536/65,537-byte catalogue inputs. Its isolated candidate matrix runs every malformed/unsupported/capability candidate—including both transfer limits, malformed legacy bytes and the 65,537 probe—through Inspect and failed Open against both a marked mixed Date+definition resident and an ordinary definition-only legacy resident. Each cell begins with Date/Text A, quantity B, and Undo B; it checks full occurrence/Bootstrap/Table/catalogue query state immediately after the operation, then replays both history stacks before starting the next cell. The positive 4,096 title Inspect and exact 65,536 Inspect also run against both origins and replay H before any replacement or Close. Worker T3 builds the Date-only source through actual import/edit/export/reopen and publishes all sixteen definitions through the real client before measuring and selecting the live 65,536 boundary. It also constructs a live 65,537-byte catalogue at resident/19 and requires the selected export to fail with `query_too_large`, followed by immediate complete-state/origin checks and Undo/Redo replay. Successful v3, legacy, New and Import replacement paths now start from marked residents with both history stacks populated; New/Import pre-guards use the actual resident/3 revision. Each expected failure must be a `DesignerRuntimeError` with exact code and applicable current revision; broad catches do not count. |
| Opaque native negatives and history (blocker 4) | `version_gate_and_opaque_transfer_malformed_vectors_are_distinct_and_atomic` sends unknown-version, truncated header/payload, trailing-byte, wrong-order, duplicate/missing/extra/unsafe/absolute path, malformed representation, invalid IDs/schema/field/bindings/catalogue shapes, and malformed legacy bytes through opaque `inspect_project` and replacement `open_project`. It expects `unsupported_project` only for the complete well-formed version-99 manifest and `invalid_project` for malformed forms; failed replacements preserve the live v3 occurrence/projection/origin. `candidate_inspect_and_open_refuse_valid_v3_text_and_number_constraints` supplies valid storage-v3 Text and Number constraints and requires typed refusal. `transfer_limit_and_oversized_inspect_preserve_the_current_v3_occurrence` treats the exact 64 MiB zero-filled vector as transport-only, expects malformed-format refusal, and expects `ProjectTransferTooLarge` at 64 MiB+1. The catalogue case measures fresh `resident/0` and live `resident/19` separately, observes marker after failed selection, and proves pre-existing quantity Redo plus Date Undo/Redo entries remain usable; every isolated Inspect/Open compares full resident state and then replays both stacks. Native New uses the resident `NewTable` request. Successful native legacy replacement now checks typed empty Undo and Redo immediately against the unchanged literal replacement projection and v2 origin. The runtime has no resident-replacing native import adapter on this base: native import proves the literal candidate and records host slot installation as NOTRUN; actual Worker `importSpreadsheet` covers the real replacement transition. |
| Fresh historical reader (blocker 5) | `historical_reader_v3.rs` rechecks the three frozen legacy inputs and fresh ordinary candidate exports: Date-only v2, definition-only v2, neither v1. Separately, it requires the final native+Worker completion manifest and reads fresh selected-v3 captures with matching producer head/run receipts, v3 framing/version, and all 19 ordered paths. The old v2 reader must return the exact `FormatError::UnsupportedRoProjectVersion { found: 3, supported: 2 }` for each well-formed v3 capture; `is_err()` and v1 file-count rejection do not count. |
| Candidate/kit-bound capture (blocker 6) | Native capture uses a newly created exclusive directory, exact candidate commit, fresh run UUID, file receipts and SHA-256 inventory. The native result manifest is created only after all required native cases pass. The Worker runner checks the exported kit manifest's exact `sourceCommit`, full file inventory and file hashes against the candidate and records its manifest digest. It runs a strict standalone TypeScript check before Playwright, reports missing methods or type/build seams as `NOTRUN`, rejects stale or partial files, wrong candidate/run, receipt/hash mismatch, missing kit, and skipped/incomplete cases, and writes `worker-completion.json` only after all required Worker cases pass. |
| Reproducible commands (blocker 7) | Native runner invokes standalone `cargo test --manifest-path packages/browser-client/runtime/Cargo.toml --locked ...`. Browser runner invokes the strict `pnpm --dir packages/browser-client exec tsc ...` and Playwright commands in the producer package context. Historical invocation is a standalone manifest-path command in the pinned historical checkout with `--locked`. |

Native required test names (16):

1. `date_and_saved_definition_lossless_reopen`
2. `date_only_lossless_reopen`
3. `definition_only_lossless_reopen`
4. `neither_lossless_reopen`
5. `ordinary_export_keeps_the_mixed_legacy_refusal_control`
6. `frozen_codec_and_bridge_profile_controls`
7. `legacy_date_only_ingress_edit_reexport`
8. `legacy_definition_only_ingress_edit_reexport`
9. `legacy_neither_ingress_edit_reexport`
10. `fresh_candidate_ordinary_legacy_exports_remain_v1_v2_compatible`
11. `candidate_inspect_and_open_refuse_valid_v3_text_and_number_constraints`
12. `exact_full_catalogue_opened_projection_boundary_and_plus_one`
13. `version_gate_and_opaque_transfer_malformed_vectors_are_distinct_and_atomic`
14. `successful_legacy_replacement_clears_v3_origin_marker`
15. `successful_v3_replacement_preserves_origin_marker`
16. `transfer_limit_and_oversized_inspect_preserve_the_current_v3_occurrence`

Worker required cases (2): the actual four-profile import/export/fresh-reopen journey and the typed failure/marker-reset journey named above. The `historical_reader_v3.rs` target is separate because it executes in the pinned historical storage checkout, and is not counted as a candidate runtime case.

## Serial qualification (not run)

Run the native producer only after an owning candidate has added the authorized selected-v3 capability and adapted the clearly marked test-only adapter seam `runtime.export_project_v3(expected_revision)`. Until then the exact Rust producer seam is unavailable and the result remains NOTRUN. Do not reinterpret missing method, compilation, or environment failure as behavior RED.

```sh
bash acceptance/date-definition-save-v3/run-native-capture.sh
```

That runner records its fresh capture path in output. After native completion, export the kit from that same exact candidate commit into the seed checkout, using a clean candidate source checkout as required by the exporter:

```sh
bash scripts/export-experimental-designer-client.sh /tmp/tachiko-work-v3-acceptance-prep-20261001/examples/experimental-designer-client/vendor/tachiko
TACHIKO_V3_ACCEPTANCE_CAPTURE_DIR=<native-capture-path> bash acceptance/date-definition-save-v3/run-worker-acceptance.sh
```

Only after those fresh native and Worker results, copy this reader source into the pinned historical reader checkout and run its exact standalone manifest:

```sh
TACHIKO_V3_LEGACY_INPUT_DIR=<seed-path>/fixtures \
TACHIKO_V3_ACCEPTANCE_CAPTURE_DIR=<native-capture-path> \
TACHIKO_V3_ACCEPTANCE_CANDIDATE_HEAD=<exact-40-hex-candidate-head> \
TACHIKO_V3_ACCEPTANCE_RUN_ID=<native-capture-run-id> \
CARGO_BUILD_JOBS=1 cargo test --manifest-path <historical-checkout>/crates/storage/Cargo.toml --locked \
  --test v3_candidate_unsupported -- --test-threads=1
```

All candidate outputs and Worker artifacts are serial. Do not reuse an existing capture or Cargo target directory. The historical checkout path and the eventual candidate kit must be supplied by the later implementation lane; this seed does not fabricate either identity.

## Remaining proof and scope

No candidate producer method or Worker method exists on this base. Consequently no native candidate output, kit export, browser/WASM journey, fresh historical-reader result, adequacy verdict, or implementation behavior has been proven. The next independent Sol review must inspect the exact final acceptance bytes, current authority and exact selected methods, verify all case mapping and typed outcomes, and call each required proof NOTRUN until the corresponding candidate exists and serial capture succeeds. This preparation does not mark production Ready, lift STOP/HOLD, grant Guard clearance, or earn final Oracle credit.

Fresh review repair status: the complete two-origin × candidate-operation preservation matrix and the other cited source-level blockers have now been addressed in the local acceptance additions. This has not received a fresh independent adequacy review. The Worker browser and native capture lanes have not run. Lightweight formatting/type/shell checks are preparation checks only and confer no behavioral result.

Changed scope is acceptance-only: `SEED.md`, `SEED.SHA256SUMS`, the native and browser acceptance tests/config, the fixture API check and helper, historical reader, and two acceptance runner scripts. The immutable CSV and legacy fixtures are retained byte-for-byte. No product/runtime source, production contract, Work #491 storage test, browser-suite execution, heavy Cargo suite, or candidate behavior evidence was changed/run here.
