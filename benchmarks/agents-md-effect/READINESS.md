# Readiness Verdict

The frozen nine-case benchmark is operationally ready for a standalone Baseline
A wave under the practical internal-experiment standard. No Baseline A or
Variant B task was executed while establishing readiness; every execution
receipt referenced here is `construction_pilot_only` and ineligible for a formal
score.

Readiness is not launch authorization. Each formal attempt must still pass the
controller's external authorization, unique-slot registration, and per-run
preflight before candidate exposure or model contact.

## Closed operational requirements

| Requirement | Operational evidence |
| --- | --- |
| Neutral runner and HOME | `preflight-run.mjs` requires an opaque `r-<128-bit-hex>` root, fresh empty HOME and CODEX_HOME, a closed environment, and recursive ancestor/tree scans for `AGENTS.md`, other agent instructions, skills, and symlink exposure. It records the exact root overlay identity and fails on semantic path labels or unexpected content. |
| Production commands for all cases | `evaluator/production-oracles.json` maps all 9 cases, 27 core commands, 58 oracle commands, 74 machine assertions, and 16 subjective groups one-to-one to the frozen locks. |
| Positive and negative oracle qualification | `evaluator/qualifications/oracles.json` records an executed all-nine construction qualification. Machine cases pass a qualified positive and discriminate a behavior-missing/base negative; TW-01, TW-02, and TW-06 use deterministic subjective packet gates. Selector-family fixtures independently qualify exact Rust-test matching, JSON pointers, and portable record sets. |
| One-shot controller and no resampling | `run-controller.mjs` atomically reserves each wave/case/phase slot, records a hash-chained attempt ledger, launches at most one agent process group, never retries candidate quality/failure/timeout, proves descendant extinction before capture, and emits one terminal disposition. A trusted adapter may resume only the same immutable captured attempt and cannot relaunch the agent. |
| Per-run preflight and recording | The controller seals its executable/control bundle and records exact environment values, agent/argument/variant/catalog identities, locally controlled Node, pnpm, Rust/Cargo/rustc/rustfmt/clippy, Git/rtk/Bash, Codex and code-mode-host binaries, WASM target artifacts, disk availability, process timing, raw JSONL/stdout/stderr, and stage input/output hashes. Formal authorization binds the effective invocation and runtime identity. |
| Trusted candidate result/diff capture | `capture-candidate.mjs` walks raw filesystem bytes and modes, includes ignored/untracked/index-hidden changes, rejects special nodes, bypasses candidate hooks/configuration/attributes/filters, writes a separate trusted object database/index, and proves patch/tree round-trip equality before validation. |
| TW-05 package-manager neutrality | `run-tw05-offline.mjs` directly executes the Rust, Node test, Worker/parity, benchmark, and portability paths under offline Cargo plus an independently probed OS network denial. Qualification fails if npm, pnpm, or yarn is invoked. |
| Deterministic blinded review | `build-review-packet.mjs` requires a hash-complete six-role input manifest, deterministic aliases/redactions, and the exact frozen R1–R4 contract. `scan-review-packet.mjs` independently verifies the public manifest and zero residual matches before a terminal release receipt. Repeat and adversarial fixtures cover near copies, paths, binary/symlink/UTF-8 failures, and tampering. |
| Same-wave base controls | Before agent exposure, the controller prepares an ancestor-only clean base and runs the exact ordered, deduplicated union of base and candidate-core commands under the same recorded validation environment. A failure terminates the slot before launch. |

The compact oracle qualification has payload SHA-256
`47cad14a2d985e2edd7dcd8dbcaf4cfa1fd4537531960bf339dd29e509869e2d`
and run-receipt SHA-256
`d8663f74cf5d4c5710889769e813d5b6fe39c52a1b8a3cd3dd4e6cdb7f8cee51`,
and evidence commitment SHA-256
`a0dc816cc3373176f2be31a2f268174669353a99488ad8a96cc9ef147659b956`.
`verify-oracle-qualification.mjs` recomputes its semantic binding; construction
smokes and the operational test suite exercise the controller without supplying
a formal authorization or frozen benchmark task to Codex.

## Per-attempt launch gates

These checks are expected to fail a specific attempt before model contact when
the local environment is unsuitable; they are implemented gates rather than
unresolved readiness defects:

- an external authorization receipt binds the phase, unique slot, IDs, frozen
  Baseline variant, Codex binary, code-mode host, materialized model catalog,
  complete local runtime identity, and exact effective arguments;
- the catalog is freshly materialized from the locked Codex binary, verified
  against its raw/canonical/model/base-instruction hashes, and staged read-only
  under the opaque run root;
- required binaries/artifacts and their hashes, minimum free space, neutral
  instruction surface, root-overlay identity, and same-wave base commands pass;
- a formal result is invalid if the process group cannot be extinguished,
  candidate capture cannot be trusted, an immutable input drifts, or the final
  blinded packet cannot obtain a zero-match scanner receipt.

No failure after agent launch permits resampling that wave/case/phase slot.

## Non-blocking limitations

- The provider exposes no immutable dated `gpt-5.6-sol` deployment fingerprint.
  The controller records the strongest available client-side model, catalog,
  base-instruction, feature, argument, and binary identities. Provider-side
  account/project/entitlement attestation remains outside repository control.
- Dedicated controller/agent/validator OS accounts and another independent
  confinement audit would provide stronger deployment isolation. This internal
  profile instead requires an exclusive host, opaque disjoint paths, closed
  environments, sealed controller artifacts, stage-separated copies, and
  pre/post content identities. The same-user runner detects and invalidates an
  `AGENTS.md` identity/content change, but cannot attest that a process did not
  mutate and restore the same bytes in place; a qualified OS deny-write boundary
  would close that residual risk.
- Multi-reviewer panels are desirable but not required for this standalone
  internal Baseline. Record the actual reviewer count and eligibility/conflict
  statement; deterministic packet building and scanning remain required.
- Additional variant-blind and A-aware neutrality audits are desirable but do
  not block Baseline A under this standard. Constructors' Baseline exposure is
  disclosed in `AUDIT.md`.
- TW-05's frozen historical target misses two stale-revision expectations. The
  frozen task and scoring contract remain unchanged: qualification uses a
  controlled real runtime as the positive, the behavior-missing base as the
  negative, and preserves the target miss as calibration evidence.

The construction-time `status`/`blocking` prose retained inside
`environment-lock.json` describes the original external-attestation ideal and
is intentionally not rewritten, because doing so would invalidate qualified
control hashes. Its pinned byte identities and execution values remain
normative; this document supplies the current practical readiness decision.
The full publication schema likewise retains the stronger provider/panel
profile. This standalone internal Baseline uses the controller's hash-bound
`awaiting_score_freeze` skeleton plus the actual eligible review sheet or sheets;
unavailable provider-signed identity and additional panels are recorded
limitations, while the frozen points, caps, thresholds, and evidence rules do
not change.

## Future controlled A/B

This verdict applies only to standalone Baseline A. Variant B must be supplied
and provenance-frozen independently. A future controlled comparison must run
fresh A and B observations in the same registered wave with identical local
controls and the frozen pair-level cancellation, blinding, and no-resampling
rules. A standalone Baseline result cannot be reused as the A side of that wave.

**READY for Baseline A**
