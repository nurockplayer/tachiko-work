# #515 field explanation acceptance seed

Steward-owned tests only, **unverified until implemented**. This seed does not
grant production Ready, merge or release authority. No production implementation
or generated kit changes are included.

- Baseline: `b1560ab4331847557f04dfd6833fbffcabfc1169`.
- Branch: `codex/issue-515-field-explanation`.
- Native: `packages/browser-client/runtime/tests/field_explanation_acceptance.rs`.
- Actual-kit/module-Worker: `packages/browser-client/e2e/experimental-client.spec.ts`
  (the existing `playwright.config.ts` harness).
- Seed identity: the commit containing this file, with subject
  `test: seed #515 field explanation acceptance (unverified until implemented)`.

## Pinned API and independent oracle

The implementer must provide:

```rust
DesignerRequest::ExplainField { expected_revision: String, target: FieldTarget }
DesignerResponse::FieldExplanation(FieldExplanationProjection)

FieldExplanationProjection {
    revision: String,
    target: FieldTarget,
    value: FieldProjection,
    formula_source: Option<String>,
    direct_dependencies: Vec<FieldDependencyProjection>,
    downstream_impacts: Vec<FieldTarget>,
    diagnostics: Vec<DiagnosticProjection>,
}
FieldDependencyProjection { target: FieldTarget, value: FieldProjection }
```

`FieldProjection` is the existing complete field DTO returned by `QueryFields`;
it retains stored scalars, calculated values/failures, formula and diagnostics.
`DiagnosticProjection` is also reused. The Worker case exercises these requests
through the actual exported kit's `client.explainField(revision, target)` method
and compares complete serialized projections with native results.

The synthetic numeric fixture defines damage = 120, attack_interval = 2,
dps = damage / attack_interval, and burst = dps, alpha = dps. Thus dps, burst and
alpha equal 60; editing damage to 180 yields 90; Undo restores 60. Formula text
is independently pinned to `([weapon.damage] / [weapon.attack_interval])` as
well as checked against `QueryFields`. Expected edges are literal fixture facts,
not output from `analyze_field` or candidate explanation logic.

Stable field IDs order interval before damage, although the expression reads
damage first; they order burst before alpha, although the human keys sort alpha
first. Assertions require exact ordered lists without sorting the reply.

Criterion 3 explicitly requires **direct-only downstream impacts**. In this
fixture damage lists only dps, excluding burst/alpha. Burst lists only dps as its
direct dependency, excluding damage/interval. The baseline core's
`analyze_field` reports transitive downstream impacts; blindly forwarding that
list will fail the issue's explicit direct-only contract. The implementer must
source direct edges from core facts without frontend semantic inference. Any
challenge to this requirement returns to the owning Steward under governance.

## Criterion to executable case map

All native names below are in `field_explanation_acceptance.rs`.

| Criterion | Case(s) |
| --- | --- |
| 1 Input | `input_has_value_no_rule_or_dependencies_and_one_direct_impact` |
| 2 Calculated | `calculated_field_has_exact_rule_current_direct_values_and_stable_id_order` |
| 3 Chain scope | `chain_excludes_transitive_impacts_and_upstream_dependencies` |
| 4 Edit / Undo | `edit_refreshes_values_and_undo_restores_the_explanation_at_its_new_revision` |
| 5 Refusals | `stale_unknown_and_foreign_targets_refuse_without_mutating_state` |
| 6 Purity | `repeated_reads_do_not_create_undo_availability_on_a_fresh_occurrence`; `repeated_reads_preserve_occurrence_export_and_both_history_directions` |
| 7 Worker parity | `worker_parity_capture_uses_the_same_native_fixture_and_requests`; `#515 actual kit module Worker matches native field explanations` in the existing Playwright harness |
| 8 Budget | `oversized_explanation_uses_the_existing_query_budget_and_refuses_atomically` |

The refusal case checks stale revision, unknown field and foreign entity, exact
occurrence/revision and native export preservation, plus usable Undo. Purity
checks both empty and populated Undo history, repeated reads, native export
bytes, and retained Redo after Undo. Undo restores explanation content at its
new revision; it does not rewind the resident revision.

The budget fixture is a synthetic one-row Tracker-shaped document with a
12,000-byte Text task consisting of U+0001, a Number estimate and a Boolean
done. JSON escapes the text to 72,000 bytes before DTO overhead. The existing
Tracker table allowance admits this document; the ordinary field-query budget
still refuses it. The test obtains `maximum` from the existing `QueryFields`
`ProjectionTooLarge` error and requires ExplainField to refuse with that same
limit and the existing `query_too_large` failure code. No new budget, capacity
increase, truncated oracle, ignored test or applicability exception is used.

## Commands

From the repository root:

```sh
cargo fmt --all
cargo fmt --manifest-path packages/browser-client/runtime/Cargo.toml --all
bash scripts/codex-cargo.sh test \
  --manifest-path packages/browser-client/runtime/Cargo.toml \
  --test field_explanation_acceptance --locked
```

After implementation, on a clean committed candidate, produce a fresh native
capture and export that candidate's actual kit. The export destination must not
already exist (the existing exporter fails closed). Use the same capture for
the native and Worker invocations:

```sh
seed_515_capture_dir="$(mktemp -d /tmp/tachiko-seed-515.XXXXXX)"
export TACHIKO_FIELD_EXPLANATION_CAPTURE="$seed_515_capture_dir/native.json"
bash scripts/codex-cargo.sh test \
  --manifest-path packages/browser-client/runtime/Cargo.toml \
  --test field_explanation_acceptance --locked
bash scripts/export-experimental-designer-client.sh \
  examples/experimental-designer-client/vendor/tachiko
pnpm --dir packages/browser-client install --frozen-lockfile
pnpm --dir packages/browser-client exec playwright install chromium
pnpm --dir packages/browser-client exec playwright test \
  --config playwright.config.ts --grep '#515 actual kit module Worker'
```

The native capture includes the same fixture's native project bytes, two exact
requests (input damage and calculated dps), and complete native responses.
The Worker opens those bytes and runs those requests. Missing capture is an
explicit failure, not a skip. The native case still performs semantic assertions
when capture is not requested. Preserve the fresh capture and exported kit's
`artifact-manifest.json` with the candidate SHA and Playwright results as parity
evidence. Do not reuse an older capture/kit to qualify a later candidate.

## Observed baseline and harness preflight

Baseline command:

```sh
bash scripts/codex-cargo.sh test \
  --manifest-path packages/browser-client/runtime/Cargo.toml \
  --test field_explanation_acceptance --locked --no-run
```

Observed exit 101: five compiler errors, exclusively the missing new API:
one E0432 for the two absent projection imports, one E0599 for `ExplainField`,
and three E0599 uses of the absent `FieldExplanation` response variant.
This is an expected compilation failure, **not behavioral RED evidence or PASS**.

Temporary local DTO/enum stubs with the exact pinned types and an
`unimplemented!` dispatch arm allowed the complete final native seed to compile
with `--no-run` (exit 0, no further errors). No explanation behavior was
implemented or executed. All stubs were removed and the production file restored
byte-for-byte before the baseline was rerun and the seed committed.

A temporary tests-only preflight exercised the existing native boundaries:
numeric fixture admission, literal formula/value, edit/Undo, typed unknown and
foreign target errors, plus Tracker budget-fixture admission, complete table
read, unchanged export/occurrence and existing QueryFields refusal. Both cases
passed (2 passed, 0 failed); the observed query maximum was 65,536 bytes. An
initial three-Text attempt hit that capacity profile's 4,096-byte cell limit and
was replaced by the successfully preflighted Tracker fixture. Temporary
preflight files were removed. This verifies fixture reachability only.

`cargo fmt --all`, the standalone runtime format/check, `git diff --check`, and
`node --check packages/browser-client/e2e/experimental-client.spec.ts` passed.
The Worker journey has not run: the required runtime/Worker/TS API is absent,
and this seed worktree has no installed Playwright dependencies or exported kit.

## Remaining external evidence

The implementation delivery owner must run all native cases and the actual-kit
module-Worker case on the same candidate, retain native capture and kit identity,
and complete the applicable browser-client and repository validation gates.
Fresh independent review and required CI remain separate delivery gates.
No manual product check is substituted for executable criteria 1–8. Sheet
presentation/qualification belongs to Sheet #134 and is outside this seed.
