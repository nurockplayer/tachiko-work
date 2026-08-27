# Issue 3 Portable Package v1 Production Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the Accepted portable-package/v1 contract as a thin, deterministic, lossless `.roproj/v1` container, including exact native/WASM bytes, bounded hostile-input validation, atomic no-clobber host publication, CLI workflows, and tracked-source comparison.

**Architecture:** `tachiko-storage` owns one pure store-only ZIP32 codec layered over the production `CanonicalRoProjectV1` type from Issue #123. Native host functions perform bounded reads and staged publication; the existing CLI remains the composition root for workspace validation. Direct JSON remains the current writer, while byte readers content-frame the exact ZIP local signature and never fall back after selecting package handling.

**Tech Stack:** Rust 2024 (MSRV 1.85.0), existing `serde`/`serde_json` and RustCrypto `sha2`, a pinned `renamore` wrapper for platform atomic no-replace rename, Cargo, and the existing native/WASM conformance harness.

**Authority:** [ADR-0025](../../decisions/ADR-0025-portable-package-v1.md), [`portable-package-v1.md`](../../specs/portable-package-v1.md), ADR-0023, and the production `.roproj/v1` codec merged for Issue #123.

## Global constraints

- Preserve the exact 19-entry byte profile and the checked-in 2,692-byte golden package.
- Consume the exact production `.roproj/v1` tree; never introduce another payload DTO, canonicalizer, or semantic schema.
- Dispatch a syntactically valid unsupported package version before applying v1-only manifest, entry, payload, or semantic rules.
- Validate in the Accepted order: framing/structure, manifest dispatch, canonical ZIP and entry set, CRC/sizes, SHA-256 root, inner claims, canonical payload, semantic validity, publication.
- Use checked arithmetic, an explicit finite package byte limit, fixed entry counts after v1 dispatch, and exact fixed ASCII names.
- Keep the codec pure and WASM-compatible. Keep filesystem and atomic publication mechanisms native.
- Publish only fully validated candidates to absent destinations. An actual publication race must preserve the competing destination.
- Keep package identity representation-local: the payload root is not semantic revision, freshness, trust, authorization, or Git identity.
- Do not add compression, ZIP64, signatures, synchronization, Git policy, generic receipts, last-known-good state, or package mutation.
- Never change the direct JSON writer or infer authority from `.ro` filenames.

---

### Task 1: Pure exact-byte package writer and golden vector

**Files:**
- Create: `crates/storage/src/portable_package/mod.rs`
- Create: `crates/storage/src/portable_package/v1.rs`
- Create: `crates/storage/tests/portable_package_v1.rs`
- Modify: `crates/storage/src/lib.rs`

**Interfaces:**
- `encode_portable_package_v1(&CanonicalRoProjectV1) -> Result<Vec<u8>, FormatError>`
- `portable_package_payload_root(&CanonicalRoProjectV1) -> [u8; 32]`
- `PORTABLE_PACKAGE_V1_MAX_ARCHIVE_BYTES`

- [ ] Add a failing test that encodes the normative empty `.roproj/v1` fixture and compares every byte, length, complete SHA-256, and payload root with the checked-in golden vector.
- [ ] Implement the domain-separated 18-leaf SHA-256 root over exact path/NUL/body bytes.
- [ ] Implement a manual canonical store-only ZIP32 writer with fixed headers, standard reflected CRC-32, checked ordinary-field bounds, exact names/order, no extras/comments/descriptors, and a finite archive admission limit.
- [ ] Prove repeated encoding and path/host metadata independence are byte-identical.

### Task 2: Bounded strict decoder and stable failure classes

**Files:**
- Modify: `crates/storage/src/portable_package/v1.rs`
- Modify: `crates/storage/src/portable_package/mod.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/storage/tests/portable_package_v1.rs`

**Interfaces:**
- `PortablePackageError` with the Accepted machine-distinguishable meanings.
- `VerifiedPortablePackageV1`, exposing the exact payload tree and verified root.
- `decode_portable_package_v1(&[u8]) -> Result<VerifiedPortablePackageV1, FormatError>`.

- [ ] Add failing vertical tests for a valid golden decode and exact `unpack(pack(P))` / `pack(unpack(R))` byte laws.
- [ ] Parse ordinary ZIP32 records with checked offsets and exact end-of-input accounting; reject stubs, tails, comments, split records, sentinels, overlaps, and impossible bounds.
- [ ] Locate one stored `package.json`, reject duplicate JSON members, dispatch its positive lexical version, and let unsupported versions win before all v1-owned checks.
- [ ] Enforce the closed canonical manifest, exact 19-name set/order, fixed ZIP fields, local/central agreement, CRC/sizes, and exact payload root.
- [ ] Verify inner `.roproj` claims, then construct the production `CanonicalRoProjectV1`; map representation and semantic failures separately.
- [ ] Add focused negatives for corruption, stale root, missing/duplicate/aliased/unknown entries, reordered/noncanonical records, malformed metadata, local/central disagreement, ZIP64/descriptors/comments, unsupported future manifest shape, resource/capacity bounds, and no parser fallback.

### Task 3: Content framing and semantic read coexistence

**Files:**
- Modify: `crates/storage/src/lib.rs`
- Modify: direct-storage regression tests

- [ ] Add a failing test that exact initial `50 4b 03 04` selects package decoding and that malformed/unsupported packages remain package errors.
- [ ] Dispatch package bytes from `from_bytes`/`load`, decode their production `.roproj/v1` payload into one semantic `Document`, and preserve the existing direct JSON path for all non-package input.
- [ ] Prove `save` and canonical direct JSON bytes remain unchanged.

### Task 4: Host pack, unpack, compare, and real no-replace publication

**Files:**
- Create: `crates/storage/src/portable_package/host.rs`
- Create: `crates/storage/tests/portable_package_host.rs`
- Modify: `crates/storage/src/portable_package/mod.rs`
- Modify: `crates/storage/src/roproj/host.rs`
- Modify: `crates/storage/tests/roproj_host.rs`
- Modify: `Cargo.toml`
- Modify: `crates/storage/Cargo.toml`
- Modify: `Cargo.lock`

**Interfaces:**
- `read_portable_package`, `pack_roproj`, `unpack_roproj`, and `compare_portable_package_with_roproj`.
- Pinned `renamore = 0.3.2` as the safe wrapper around Linux `renameat2(RENAME_NOREPLACE)`, Darwin `renamex_np(RENAME_EXCL)`, and Windows no-replace move behavior.

- [ ] Add failing host journeys for pack/unpack exactness, standalone operation, existing destinations, source mismatch, source noncanonicality, and cleanup on failure.
- [ ] Strengthen staged `.roproj` directory publication to use the actual atomic no-replace primitive, failing closed when unsupported.
- [ ] Add a deterministic two-publisher race test: exactly one staged directory wins, the loser reports destination existence, and the winning tree is intact.
- [ ] Implement pack as bounded exact-source admission, complete in-memory encode/verify, sibling staged-file write, and exclusive rename.
- [ ] Implement unpack as complete package verification followed by existing canonical-tree staging and exclusive directory rename.
- [ ] Implement read-only root comparison; mismatch returns `portable_package.source_mismatch` without changing either input.
- [ ] Verify native and wasm32 builds with the dependency present; host-only behavior must not enter the pure conformance path.

### Task 5: CLI composition and workspace validation

**Files:**
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/cli/src/commands.rs`
- Modify: `crates/cli/tests/cli.rs`

**Interfaces:**
- `tachiko roproj pack <INPUT.roproj> <OUTPUT.ro>`
- `tachiko roproj unpack <INPUT.ro> <OUTPUT.roproj>`
- `tachiko roproj compare-package <PACKAGE.ro> <TRACKED.roproj>`

- [ ] Add a failing outside-Git CLI journey that materializes, packs twice to distinct names, validates the package through ordinary read commands, unpacks byte-identically, compares as consistent, and refuses every overwrite.
- [ ] Compose storage admission with the workspace validation gate before pack/unpack publication; never publish a workspace-invalid candidate.
- [ ] Emit stable success output and preserve machine-distinguishable package errors on stderr.
- [ ] Add mismatch and corruption journeys proving both source trees and destinations remain untouched.

### Task 6: Native/WASM conformance and documentation

**Files:**
- Modify: `scripts/portable-conformance-check.rs`
- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Modify only implementation-status prose in relevant Accepted specs/research reconciliation
- Regenerate: `THIRD_PARTY_LICENSES.md`

- [ ] Add a native/WASM conformance record that encodes and decodes the normative package, asserts the root and complete package digest, and increments the exact case count.
- [ ] Document package framing, pack/unpack/compare commands, exact-source/no-clobber behavior, finite host limit, and the fact that direct JSON remains the writer.
- [ ] Update only stale implementation-status statements; do not alter Accepted architecture or normative bytes.
- [ ] Regenerate exact third-party notices after the lockfile/runtime dependency change.

### Task 7: Repository gates, review, and delivery

- [ ] Run `cargo fmt --all -- --check`, exact Rust 1.85 build/test coverage, clippy with repository flags, workspace tests, portable conformance, dependency graph validation, documentation/link checks, and `scripts/release-check.sh` from a clean commit.
- [ ] Review the full diff against Issue #3 and ADR-0025, including every conformance case and unrelated direct JSON/storage regressions.
- [ ] Request an independent code review, address every actionable finding with focused tests, and rerun affected plus full gates.
- [ ] Open one Issue #3 PR from the final reviewed head, monitor all required checks/review threads, merge with head-match protection, and verify live `main`, Issue closure, and Project status.
- [ ] Recalibrate the live repository and Product Roadmap before selecting the next genuinely Ready critical-path Issue.
