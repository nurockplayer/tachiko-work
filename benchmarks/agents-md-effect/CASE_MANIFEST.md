# Final Case Manifest

The nine cases below are frozen for protocol `tachiko-agents-effect-v1`. Full
expected/forbidden scope, machine contract groups, historical evidence
visibility, blinded-review criteria, and hidden failure modes are in
[`evaluator/cases.json`](evaluator/cases.json). That file is evaluator-only.

| ID | Capability focus | Historical source | Fixed base → ground truth | Primary validation | Difficulty / time limit |
| --- | --- | --- | --- | --- | --- |
| `TW-01` | ADR and repository authority | Issue [#21](https://github.com/nurockplayer/tachiko-work/issues/21), PR [#69](https://github.com/nurockplayer/tachiko-work/pull/69) | `47ce31aeaeb63a9bb21282691b918e8751cb5a21` → `b2d9c7fb8c11236a0564faa37eca1cf32b76faf8` | docs consistency + semantic authority checks + blinded review | medium / 90 min |
| `TW-02` | Review remediation and scope control | PRs [#73](https://github.com/nurockplayer/tachiko-work/pull/73), [#75](https://github.com/nurockplayer/tachiko-work/pull/75) | `b6178e48a5e9f1078a9150038fb19352d0d55325` → `1e422f1c3d152a342abee57aaad8b44aa04e11cf` | docs consistency + schema/migration completeness + blinded review | medium / 90 min |
| `TW-03` | Numeric regression and conformance | Issue [#40](https://github.com/nurockplayer/tachiko-work/issues/40), PR [#83](https://github.com/nurockplayer/tachiko-work/pull/83) | `0d7baaa069ec1c673037278cc76c956eed2347e2` → `e953877f2dfd05ae5cebc5262656c2d877c2ed9c` | isolated numeric assertions + records 10–20 + release gate | hard / 180 min |
| `TW-04` | Architecture-sensitive refactor | Issue [#72](https://github.com/nurockplayer/tachiko-work/issues/72), PR [#85](https://github.com/nurockplayer/tachiko-work/pull/85) | `e953877f2dfd05ae5cebc5262656c2d877c2ed9c` → `515b81b955a3c21ab306f24e940ea90d46efdace` | independent Cargo-DAG facts + adapted behavior/portable assertions | hard / 240 min |
| `TW-05` | Research spike and workflow discipline | Issue [#26](https://github.com/nurockplayer/tachiko-work/issues/26), PR [#91](https://github.com/nurockplayer/tachiko-work/pull/91) | `2ecb7d06115af6eb5f392504dce7610af765518a` → `16289f8a5acd48ca7fa36b265b7fdfe7df0e4d12` | normalized revision/parity contract + generic repository gates + blinded methodology review | hard / 300 min |
| `TW-06` | Governance/authority reconciliation | Issue [#15](https://github.com/nurockplayer/tachiko-work/issues/15), PR [#22](https://github.com/nurockplayer/tachiko-work/pull/22) | `caf81e116c8f48c265fec40d7d12bd23a1fa4be0` → `64410d6198296a4359053c5d2bb0912401b08056` | cross-document governance consistency + scope gates + blinded semantic review | medium / 90 min |
| `TW-07` | Formula feature/correctness oracle | Issue [#90](https://github.com/nurockplayer/tachiko-work/issues/90), PR [#97](https://github.com/nurockplayer/tachiko-work/pull/97) | `16289f8a5acd48ca7fa36b265b7fdfe7df0e4d12` → `6ad364755566bc604e69800c8656868dab60a365` | isolated complete-outcome/graph assertions + record 26 | very hard / 300 min |
| `TW-08` | Legacy persistence and strict format dispatch | Issue [#74](https://github.com/nurockplayer/tachiko-work/issues/74), PR [#80](https://github.com/nurockplayer/tachiko-work/pull/80) | `c8528409dd327a9854ac030247ecbd8fcf765db7` → `1929dd758ed580f0ccd2bc70be11560f3e88b0da` | strict decoding + closed-world conversion + canonical compatibility + blinded migration-boundary review | very hard / 300 min |
| `TW-09` | Validation/diagnostics integration | Issues [#89](https://github.com/nurockplayer/tachiko-work/issues/89), [#90](https://github.com/nurockplayer/tachiko-work/issues/90), PRs [#97](https://github.com/nurockplayer/tachiko-work/pull/97), [#99](https://github.com/nurockplayer/tachiko-work/pull/99) | `77821143e9847f62e129e553522556743c5032c1` → `156565a3d2dc7664088a24b7f6e38d02ad4e04fe` | normalized facts + isolated suppression/agreement + case-local records 27–30 | very hard / 360 min |

## Capability balance

- bugfix/regression: `TW-03`;
- feature implementation: `TW-07`, `TW-08`, `TW-09`;
- architecture-sensitive refactor: `TW-04`;
- ADR/repository-authority-sensitive work: `TW-01`, `TW-02`, `TW-06`, `TW-09`;
- diagnosis/research: `TW-05`;
- scope control: all cases, especially `TW-02`, `TW-05`, `TW-06`, `TW-08`;
- tooling/workflow compliance: all cases through repository gates, with one
  explicit locked Node workflow and native/WASM evidence in `TW-05`.

Storage and formula recur because they dominate the repository's independently
merged, strongly tested history, but the selected cases exercise different
failure classes: decimal conversion, legacy format dispatch, graph correctness,
cross-layer validation, architecture ownership, governance reconciliation, and
research methodology. No case was added solely to fill a category.

Eight replays use the target's direct parent. `TW-09` is the sole declared
exception: its base is an earlier ancestor that already contains the required
formula prerequisite but predates the repository's first root `AGENTS.md`.
Three exact first-parent commits between that base and the implementation parent
are frozen as outcome-only and are not exposed to the agent. The case-local
portable contract appends records 27–30 to that base; it does not import the
independent storage-envelope work from the intervening history. TW-09's
`historical_patch` statistics describe implementation-parent → target; the
separate replay-base → target diff is recorded only for workload disclosure and
is never used for similarity scoring.

The exact task hashes/byte counts, expected and forbidden scopes, functional
groups, candidate commands, hidden failure modes, authority cutoffs, and
evidence-visibility boundaries are machine-locked in
[`evaluator/cases.json`](evaluator/cases.json). Shortening a commit hash is never
permitted in a controller input.
