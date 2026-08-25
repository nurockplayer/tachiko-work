# Result Recording Contract

One immutable JSON document conforming to
`schemas/result-record.schema.json` represents one process/case observation.
Each independent review sheet conforms to
`schemas/blinded-review-score.schema.json`. Content-addressed stdout, stderr,
final message, patch, receipts, adapters, review packets, and validation outputs
remain separate artifacts; the result stores their SHA-256 values.

## Identity and phase

- `experiment_id`, `run_id`, `wave_id`, `attempt_id`, `pair_id` where present,
  and `blinded_candidate_id` are independent lowercase 128-bit CSPRNG hex values
  with no arm/case mnemonic; every record binds their generation receipt.
  `experiment_id` equals `registration.wave_id`. The attempt registration binds
  the exact `run_id` and `blinded_candidate_id`, and the semantic validator proves
  equality across the result, attempt ledger, arm-key commitment, review packet,
  and score sheets.
- `construction_pilot` pairs only with `construction`, `baseline_a` only with A,
  `controlled_ab` with A or B, and `ultra_optional` with Ultra.
- `valid_run=false` and `outcome=invalid_run` are used for infrastructure,
  contamination, identity, or adapter-integrity failure. Such records receive no
  quality score and are excluded from paired analysis.

`registration` is always present, including for preflight invalidation. It binds
`wave-registration-v1`, immutable wave/case/variant/arm-key/order/control
commitments, neutrality audits, reviewer allocator/seed commitment, opaque IDs,
and the pre-run attempt registration. The append-only ledger later binds the
completed record hash; no result contains the terminal-entry hash, so the
linkage is one-way rather than circular. `record_validation_receipt_sha256` is
also always present and is computed from the detached semantic projection.

The top-level outcome is authoritative only after semantic validation. For a
valid observation, `run_stage=complete`, `invalid_reason` and `invalidation` are
null, the full evidence objects are present, and the top-level outcome must equal
`scoring.outcome`. For an invalid observation, the top-level outcome is
`invalid_run`, `invalidation` identifies the terminal stage and preserves only
the receipts actually available at that point, while `scoring` and
`blinded_review` are null. Unreached-stage objects remain null; producers must
not fabricate evidence merely to satisfy the record schema. The semantic
controller additionally proves that `run_stage` equals `invalidation.stage` and
that protected-content exposure, observation/pair discard, and retry policy are
consistent with the registered contamination rule. `timing` is always factual:
a termination failure records `process_group_extinct=false`, preserves any
termination receipt in `invalidation.available_receipts`, and invalidates the
observation; only a valid completed record requires it to be true.

An actual root-overlay content or path-identity change is also an invalid
observation, not a scored hard failure: preserve the last trustworthy overlay
receipt in `invalidation.available_receipts`, leave any unreconstructible
post-state object null, and record initiating-agent versus control-failure
attribution. A fully denied deliberate/repeated mutation attempt may be scored
as a hard failure only when all protected bytes and identity fields remain
unchanged.

## Evidence linkage

The record binds the exact base/task/AGENTS bytes, the absence of historical
root `AGENTS.md`, candidate tree and binary patch, model/catalog/instruction/
feature/provider attestations, environment, base health control, candidate
preparation, oracle-capsule preparation/isolation, constructed contracts, command outputs,
adapter qualification/review, both pre-run neutrality-audit receipts, and the
detached record-validation receipt. The forbidden same-workspace overlay
prototype cannot satisfy the capsule receipt. Candidate
capture additionally binds a trusted raw-tree digest, the frozen exclusion-list
hash, the no-filter capture receipt, and a round-trip digest; the candidate Git
index/configuration is not an authority source.

The environment object also binds an agent-visible identity/path leak-scan
receipt covering account, hostname, working/home/tool/cache paths, environment,
mount/process visibility, and denied-path errors. No valid record may expose a
benchmark/protocol, case, arm, variant, or evaluator construction-source label
through those surfaces. A separate home-isolation receipt proves the per-attempt
`HOME` began empty, remained supervisor-owned/read-only and path-identical, and
was not reused from another observation.

Formal validation/review evidence additionally binds one immutable controller
context containing phase, wave/run/attempt/candidate IDs, candidate-capture hash,
and external-authorization hash. Base, core, and oracle receipts bind the active
kernel network-denial probe. A formal TW-05/TW-09 adapter record binds the sealed
scaffold lock, disjoint config/probe, eligible independent integrity approval,
fresh adapter-only TMP, sandbox profile, pre/post candidate and trusted-input
identities, process-group extinction, and controller-owned post-extinction output
materialization. A packet or scanner receipt produced without formal context is
construction-only and cannot satisfy the formal evidence linkage.

For machine assertions the record also binds the qualified production oracle
command-manifest hash. The historical/construction `commands` in
`oracle-lock.json` do not satisfy this field; the production manifest must map
the same frozen semantic assertion IDs to separated artifact-build, probe-build,
and expectation-free execution receipts.

Every `core_checks` ID/points pair must equal `core-score-lock.json`. Every
functional `check_results` item must equal one `oracle-lock.json` assertion and
record selector kind, evidence hash, and exact-test match count where relevant.
Group availability and assessment must equal `cases.json`; assertion points may
be awarded once only. Dimensions, machine/review availability, total, caps, and
outcome are recomputed by the semantic result validator rather than trusted from
the producer JSON.

Controller qualification must include schema-positive fixtures for failures at
preflight, agent execution, capture, validation, and review, plus negative
fixtures that attempt to score an invalid run, omit an available receipt, forge
an unreached-stage receipt, mismatch the two outcome fields, or mismatch
`run_stage` and `invalidation.stage`. Until those fixtures and the controller
receipt exist, the schema is a fail-closed contract rather than an operational
result writer.

## Review and unblinding

Store exactly two independent primary review-sheet hashes,
case-specific reviewer-eligibility attestation hashes, the disjoint-panel
registry hash, packet-only reviewer-environment receipt hashes, redaction
receipt, randomization seed hash, and adapter-integrity review. Set
`adjudication_required` from the frozen disagreement triggers; when true,
`adjudication` must bind the allocated adjudicator, eligibility/environment,
separate score sheet, points, trigger, allocation receipt, and pre-unblind
freeze, and when false it must be null. The semantic validator must prove that
all IDs replay from the arm-key-blind allocation contract and that the two arms
of a case have no
quality reviewer/adjudicator in common and that no reviewer saw the paired
candidate before both panels froze.

An ambiguous denied-access event uses the separate arm-blind tooling
adjudicator path. The record must bind that adjudicator's case eligibility,
normalized event decision, and receipt; all four fields are null when no
tooling adjudication was required. Tooling adjudication occurs only after
non-tooling quality scores freeze and cannot alter those scores.

Before unblinding, freeze a canonical anonymous score payload containing every
score, outcome cap, evidence hash, receipt, reviewer decision, and anonymous
candidate ID, while omitting `arm`, `experiment_phase`, variant identity/hash,
and `unblinded_at`. The exact removed JSON pointers, wrapper, unknown-field
rejection, sorted-key serialization, numeric rules, UTF-8 encoding, and digest
are frozen in `pre-unblind-score-projection-v1.json`; that projection also
excludes its own future hash and the later record-validation receipt. Store the
payload hash and freeze timestamp in a separate controller receipt. Only after
every payload in the wave is immutable may the custodian join the arm key. The
final record supplies the removed fields and pre-unblind hash. Then
`semantic-result-projection-v1.json` validates the final record while excluding
only `record_validation_receipt_sha256`; the completed external receipt is
hashed and that hash is inserted. Both projections must reconstruct
byte-for-byte, without self-hashing cycles.

The schemas intentionally require a provider deployment attestation, isolated
runner identity, trusted raw capture, reviewer eligibility/pair blindness,
oracle-capsule receipt, and detached record-validation receipt. These requirements
define the stronger governed publication profile. `READINESS.md` separately
defines the practical standalone internal Baseline launch artifact and records
unavailable provider-side identity and additional review panels as limitations;
it does not change any score, cap, threshold, or evidence rule.
