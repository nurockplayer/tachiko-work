# `.ro` Format Documentation Entry Point

This file is retained as a compatibility/navigation entry point for older links.

It does **not** define an independent `.ro` contract. Use the documents below according to the question you are answering.

## Current sources

- [`ro-format-and-roproj-spec.md`](ro-format-and-roproj-spec.md) — accepted representation direction under ADR-0003: `.roproj` is the target canonical editable/source materialization, `.ro` is the portable artifact, and the semantic model is authoritative over both.
- [`ro-format-v1.md`](ro-format-v1.md) — exact deterministic `.ro` JSON behavior implemented by the v0.1 Developer MVP. This is a **Provisional implemented baseline** being hardened by #21, #25, #37, #38, and #40.
- [`roproj-format.md`](roproj-format.md) and [`roproj-layout-v1.md`](roproj-layout-v1.md) — target/project representation material; `.roproj` is not yet implemented in v0.1.
- [`../decisions/ADR-0003-ro-and-roproj-representation.md`](../decisions/ADR-0003-ro-and-roproj-representation.md) — Accepted architectural authority for the long-term representation relationship.

## Current implementation state

The v0.1 CLI directly persists deterministic `.ro` files for validation, calculation, semantic diff/merge, authoring, and export.

That behavior is implemented, but implementation does not make every current identifier, encoding, ordering, or version-envelope choice a permanent ecosystem invariant. Those details remain subject to Core & Format Hardening where the repository classifies them as Provisional or Open Questions.

Do not describe the entire `.ro` v1 contract as "stable" until the relevant hardening decisions explicitly promote the durable parts of that contract.

For the project-wide authority model, see [`../governance/knowledge-authority.md`](../governance/knowledge-authority.md) and [`../governance/canonical-reconciliation-register.md`](../governance/canonical-reconciliation-register.md).
