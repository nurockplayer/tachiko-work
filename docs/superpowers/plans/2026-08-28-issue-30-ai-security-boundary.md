# Issue 30 AI Security Boundary Implementation Plan

**Goal:** Make the existing AI threat-model boundary executable: untrusted content remains data, AI semantic mutation crosses the #29 trusted lifecycle, and raw mutation or host effects cannot masquerade as semantic operations.

**Architecture:** `tachiko-ai-api` will expose a provisional provider-facing admission seam over `workspace-engine::patch_lifecycle`. Untrusted requests contain typed proposal/execution intent and non-authoritative evidence only; a trusted host context supplies the effective principal and trusted time. The adapter delegates all semantic proposal and publication decisions to the existing lifecycle and returns disclosure-safe machine codes. Its closed operation classifier rejects raw semantic/storage mutation and filesystem/network/process/Git/plugin/deployment/credential effects. The existing ADR-0016 dependency gate keeps storage and host crates out of `ai-api`.

**Tech stack:** Rust 2024 (MSRV 1.85.0), existing `tachiko-ai-api`, `tachiko-workspace-engine`, `thiserror`, Cargo, and the repository release gate. No new dependency, transport, provider, wire format, runtime/session, filesystem, network, process, or plugin implementation.

**Scope:** GitHub Issue #30 only.

**Authority:** ADR-0007, ADR-0019, ADR-0020, ADR-0022, ADR-0024, ADR-0026, [`semantic-api.md`](../../specs/semantic-api.md), [`semantic-authorization.md`](../../specs/semantic-authorization.md), [`ai-agent-api.md`](../../specs/ai-agent-api.md), and [`threat-model.md`](../../security/threat-model.md).

## Constraints

- Treat system/developer/user instructions, trusted semantic metadata, and untrusted document/import/plugin/model content as distinct context classes; none grants semantic capability by itself.
- Do not accept effective principal, principal kind, Grant, Approval, trusted time, validation truth, authorization footprint, or trust upgrades from an untrusted AI request.
- Reuse the immutable `SemanticPatch` and #29 Propose/Execute lifecycle; do not introduce an AI-only patch vocabulary or gate.
- Keep lifecycle enforcement in `workspace-engine`; the AI seam is hostile-client admission and safe projection, not the sole authority.
- Preserve disclosure-safe denials and separate authorization, Approval, stale, semantic-gate, publication-conflict, verification, raw-bypass, and host-effect outcomes with stable symbolic codes.
- Keep raw semantic state, `.roproj`/storage, filesystem, network, process, Git, plugin, deployment, durable persistence, and credential effects outside the semantic adapter.
- Leave concrete authentication/session/revision/clock/transport mechanics to #93 and future host work; leave actual host/plugin capability mechanisms to #17 and their owning decisions.

## Tasks

### 1. Failing security acceptance tests

- [x] Add a prompt-injection-like document-content fixture that remains explicitly untrusted data and cannot request a host effect.
- [x] Add raw semantic/storage mutation denial fixtures.
- [x] Add a typed proposal denied without Propose capability.
- [x] Add an invalid typed proposal whose natural-language evidence claims safety, proving deterministic validation still wins.
- [x] Add an external-effect request denied independently of semantic capability.
- [x] Add one successful approved Delegated execution proving the admitted path delegates to the trusted lifecycle.
- [x] Add Human-context proposal/execution regressions proving an AI request cannot inherit the lifecycle's direct-Human Approval exemption.
- [x] Run the focused test and capture the initial missing-boundary compilation failure.

### 2. Provider-neutral boundary implementation

- [x] Add explicit context-source/treatment and untrusted-evidence value types.
- [x] Add trusted-host context lookup that is absent from untrusted request DTOs and fails closed when identity/time is unavailable.
- [x] Require the trusted lifecycle registry to prove an active Delegated occurrence before either AI-facing operation.
- [x] Add a closed operation boundary for typed semantic proposal/execution versus raw mutation and host effects.
- [x] Delegate proposal/execution to `PatchLifecycle` and project lifecycle failures into stable disclosure-safe security/result families.
- [x] Keep validation reports only on authorized semantic-gate rejection; never preserve client validation/approval claims as authority.

### 3. Documentation and delivery

- [x] Reconcile AI API, threat/security model, architecture, changelog, and canonical register implementation-state prose.
- [x] Run focused tests, all-target workspace tests, formatting, strict Clippy/rustdoc/MSRV/native-WASM/package/smoke release gates.
- [x] Review the complete diff against #30 and Accepted authority, fix every actionable finding, and validate the exact head.
- [ ] Open one #30 PR, maintain one canonical `agent-handoff:v1` comment, merge with head protection after all gates, and recalibrate live Roadmap.
