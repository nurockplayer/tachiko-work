# J-Space Notes

`WORKSPACE.md` is an execution ledger. It intentionally preserves phase-local assumptions, review evidence, and historical reasoning, including assumptions that were later superseded.

It is **not** the canonical architecture authority.

For current decisions, use this precedence:

1. Accepted ADRs in `docs/decisions/`
2. Current architecture/specification documents under `docs/`
3. Current implementation and verified tests
4. Historical j-space phase entries

In particular, any older `WORKSPACE.md` entry describing ADR-0003 as proposed/deferred is historical. ADR-0003 is now **Accepted**: `.roproj` is the target canonical editable/source representation and `.ro` is the portable artifact. The v0.1 CLI currently implements deterministic `.ro` persistence as a transitional product path; `.roproj` materialization and deterministic pack/unpack remain implementation work.
