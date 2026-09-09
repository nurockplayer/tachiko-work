# Source and authority register

Observed on 2026-09-10 JST. Planning baseline:
`nurockplayer/tachiko-work@8bba9b09cea3c011df383216ba3846ccd003dece`.
Its tree is `7836bfa871cb13971a4f1e93f6204e074d814cb2`.
Re-read live authority at implementation checkpoints; these observations are not
permission to overwrite another active lane.

## Upstream authority actually consulted

Prefix below: https://github.com/nurockplayer/tachiko-work/blob/main/

- `AGENTS.md`, `CONTRIBUTING.md` — repository work and contribution scope.
- `docs/vision/product-constitution.md` — ownership, semantic meaning, practical
  spreadsheet interoperability and no forced single document metaphor.
- `docs/vision/design-principles.md` — shared meaning, useful views and human flow.
- `docs/governance/knowledge-authority.md` — authority and decision-state hierarchy.
- `docs/governance/project-governance.md` — acceptance-first preparation, preflight,
  readiness, canonical handoff, evidence and challenge/review process.
- `docs/governance/delivery-throughput-policy.md` — Guarded review and real parallelism.
- `docs/decisions/ADR-0007-ai-semantic-interaction-model.md` — AI authority/effects.
- `docs/decisions/ADR-0020-first-class-headless-semantic-api.md` — mandatory semantic
  boundary; typed identity; Query/Propose/Execute and atomicity.
- `docs/decisions/ADR-0022-resident-semantic-runtime-and-host-boundary.md` — resident
  shared Rust owner, disposable frontend projections and separate host effects.
- `docs/specs/semantic-authorization.md` — Accepted ADR-0026 scope/approval contract,
  ADR-0024 exact-change binding and currently implemented versus deferred mechanics.
- `docs/architecture/frontend-backend-boundary.md` — present runtime/composition map.

## Implementation evidence, not UI design authority

At the pinned baseline:

- `apps/designer/src/experimental-client.ts` exports the experimental client and
  directory-to-transfer helper, not a general stable SDK.
- `apps/designer/src/runtime/client.ts` exposes query/edit/export scalar operations;
  it does not expose a complete delegated proposal/approval client.
- `apps/designer/src/runtime/protocol.ts` supplies bounded target/projection facts.
- `apps/designer/src/host/project-transfer.ts` is an app-private 18-file v1 transfer
  helper, not a public canonical file format or permission to decode private ABI.
- `scripts/export-experimental-designer-client.sh` currently builds from the
  app-local runtime and emits the intact JS/types/Worker/WASM kit.
- `apps/designer/experimental-client-kit/README.md` explicitly disclaims public SDK
  stability and directs consumers through its single exported entry.
- `examples/experimental-designer-client/src/main.ts` and
  `apps/designer/e2e/experimental-client.spec.ts` supply the independent 3 -> 8
  canary, stale rejection and runtime round-trip expectation. That example is
  automated smoke, not a human UI or browser-save durability demonstration.
- `dogfood/product-gaps.roproj/{manifest.json,schemas.json,entities/*.jsonl}` provides
  the seed's shape and stable fixture identities. The release-plan fixture changes
  the document identity/content and adds a Text notes field; it is not byte-identical.

## Live backlog/PR reconciliation

#354 initially recommended deferring a separate repo. The present explicit founder
instruction changes that client-planning direction, not higher semantic authority.
#231 remains a third-party experiment; #313 does not freeze transport/SDK shape.
#304 supports shared human/agent work, not an autonomous-agent-platform pivot.
#315 and the spreadsheet launch family #256/#261 retain their work/release scope.
#15/#202 retain licensing and external contribution gates. #348 and #350/#353 own
their existing-client fault/maintenance work. #330/#331 and #345/#351 keep ownership.

Latest direct PR observations during this preparation:

- #331: open Draft, HEAD `ded9636f3de989d979adbf290e242b16da1dd3c0`;
  acceptance remains pre-production pending the owning Ready decision.
- #351: open Draft, HEAD `53f2946dc76dd2d73203d36fe1897d44fd461ccb`;
  existing macOS implementation lane. Do not start a competing old-app PR.

## Primary technology references checked

- https://react.dev/learn/build-a-react-app-from-scratch — Vite is a supported
  from-scratch option; routing/loading/framework integration remain our tradeoff.
- https://v2.tauri.app/security/capabilities/ — narrow capabilities/scopes; these do
  not replace correct checks inside commands and trusted code.
- https://www.w3.org/WAI/ARIA/apg/patterns/grid/ — keyboard/accessibility behavior.
- https://playwright.dev/docs/test-assertions — browser-observation testing practices.

React/Vite/Tauri are provisional choices here, not a claim that their latest
versions were installed, benchmarked or validated. Exact versions must be pinned
with real bootstrap evidence. No model benchmark or unpublished model mapping is
asserted by the Astra/Terra/Luna allocation.
