# Issue 123 `.roproj/v1` Production Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the Accepted `.roproj/v1` codec, deterministic canonical tree, bounded canonicalizer, atomic no-clobber host publication, and minimal first-party CLI composition required by GitHub Issue #123.

**Architecture:** `tachiko-storage` owns an independent `.roproj/v1` DTO and a pure exact-path/exact-byte tree value. Native filesystem code admits either the exact canonical tree or ADR-0023's bounded non-canonical family, while the CLI composes storage with `workspace-engine` validation before publication. Direct `.ro` readers/writers remain unchanged and no filesystem or Git dependency enters the semantic/application engines.

**Tech Stack:** Rust 2024 (MSRV 1.85.0), `serde`/`serde_json`, `ryu-js`, RustCrypto `sha2`, Cargo, existing native/WASM conformance harness.

**Spec:** [`docs/specs/roproj-format.md`](../../specs/roproj-format.md), together with [`docs/specs/roproj-layout-v1.md`](../../specs/roproj-layout-v1.md) and [ADR-0023](../../decisions/ADR-0023-roproj-v1-canonical-tree-and-sharding.md).

## Global Constraints

- Preserve the exact eighteen-file tree: `manifest.json`, `schemas.json`, and `entities/0.jsonl` through `entities/f.jsonl`.
- Select `.roproj/v1` from the manifest before decoding schema or entity bodies.
- Keep `.roproj/v1` DTOs storage-owned and independent from semantic-core Serde layouts and direct `.ro` DTOs.
- Use the first SHA-256 nibble of exact decoded `EntityId` UTF-8 for placement and unsigned UTF-8 byte order for all unordered ID collections.
- Preserve Unicode scalar sequences, canonical finite-binary64 spelling, exact LF/final-newline behavior, recursive closed-world decoding, and duplicate-member rejection.
- Keep `workspace-engine` filesystem-free; the CLI is the first-party composition root for full validation plus publication.
- Never overwrite a destination or expose a partially written canonical tree.
- Do not implement portable ZIP packaging, Git integration, semantic delta/merge formats, or legacy direct `.ro` conversion policy.
- Preserve the checked-in Cargo workflow and validate stable plus exact Rust 1.85.0 compatibility.

---

### Task 1: Pure canonical tree writer

**Files:**
- Create: `crates/storage/src/roproj/mod.rs`
- Create: `crates/storage/src/roproj/v1.rs`
- Create: `crates/storage/tests/roproj_v1.rs`
- Modify: `Cargo.toml`
- Modify: `crates/storage/Cargo.toml`
- Modify: `crates/storage/src/lib.rs`

**Interfaces:**
- Consumes: `tachiko_semantic_core::Document`, `Number`, and the existing `FormatError` boundary.
- Produces: `ROPROJ_V1_FORMAT_VERSION`, `ROPROJ_V1_PATHS`, `CanonicalRoProjectFile`, `CanonicalRoProjectV1`, and `encode_roproj_v1(&Document) -> Result<CanonicalRoProjectV1, FormatError>`.

- [ ] **Step 1: Write the failing empty-tree golden test**

```rust
#[test]
fn empty_document_emits_the_normative_eighteen_file_tree() {
    let tree = encode_roproj_v1(&Document::empty("doc-empty", "Empty")).unwrap();
    assert_eq!(tree.files().len(), 18);
    assert_eq!(tree.file("manifest.json").unwrap(), EXPECTED_EMPTY_MANIFEST);
    assert_eq!(tree.file("schemas.json").unwrap(), b"[]\n");
    for path in ROPROJ_V1_PATHS.iter().skip(2) {
        assert_eq!(tree.file(path).unwrap(), b"");
    }
}
```

- [ ] **Step 2: Run the test and prove red**

Run: `cargo test -p tachiko-storage --test roproj_v1 empty_document_emits_the_normative_eighteen_file_tree --locked`

Expected: compilation fails because the `.roproj/v1` public API does not exist.

- [ ] **Step 3: Implement the minimal independent v1 DTO and canonical renderers**

```rust
pub struct CanonicalRoProjectFile {
    path: String,
    bytes: Vec<u8>,
}

pub struct CanonicalRoProjectV1 {
    files: Vec<CanonicalRoProjectFile>,
}

pub fn encode_roproj_v1(document: &Document) -> Result<CanonicalRoProjectV1, FormatError>;
```

Define manifest/schema/entity/value/expression DTOs inside `roproj::v1`, convert explicitly from semantic types, render pretty JSON for the first two files and compact JSONL for entities, and add `sha2 = { workspace = true }` for the normative placement function.

- [ ] **Step 4: Run the focused storage test**

Run: `cargo test -p tachiko-storage --test roproj_v1 --locked`

Expected: the empty-tree golden test passes.

### Task 2: Strict decode, semantic conversion, and canonical laws

**Files:**
- Modify: `crates/storage/src/roproj/v1.rs`
- Modify: `crates/storage/src/roproj/mod.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/storage/tests/roproj_v1.rs`

**Interfaces:**
- Consumes: Task 1's exact tree value and the crate's duplicate-aware JSON frontend.
- Produces: `CanonicalRoProjectV1::try_from_files(Vec<(String, Vec<u8>)>)`, `decode_roproj_v1(&CanonicalRoProjectV1) -> Result<Document, FormatError>`, and explicit `.roproj` format/version/representation errors.

- [ ] **Step 1: Add one failing full-shape round-trip test**

Construct a semantic document containing all four field types, all five value kinds, all eight expression operators, stable references, composed/decomposed Unicode, and entities in multiple shards. Assert `decode_roproj_v1(encode_roproj_v1(document)) == document` and exact re-encoding stability.

- [ ] **Step 2: Run the round-trip test and prove red**

Run: `cargo test -p tachiko-storage --test roproj_v1 full_shape_round_trip_is_exact --locked`

Expected: compilation fails because decoding is not implemented.

- [ ] **Step 3: Implement strict manifest-first decode and DTO-to-semantic conversion**

```rust
pub fn decode_roproj_v1(tree: &CanonicalRoProjectV1) -> Result<Document, FormatError> {
    v1::decode(tree).map_err(map_roproj_v1_error)
}
```

Validate syntax and recursive duplicate members, inspect `format` plus lexical `format_version` before any body decode, reject unknown members/tags, prove scoped ID uniqueness, enforce field/value/reference/formula meaning, convert to one `Document`, and apply semantic-core validation.

- [ ] **Step 4: Add and run one vertical negative test at a time**

Cover missing/extra/duplicate paths, missing/malformed/unsupported manifest versions, unknown recursive members/tags, malformed JSON/JSONL, missing/duplicate/out-of-order IDs, wrong shard placement/order, empty IDs, wrong value types, bad references, formula structural limits, and invalid semantic documents.

Run after each slice: `cargo test -p tachiko-storage --test roproj_v1 <test_name> --locked`

- [ ] **Step 5: Add deterministic ordering, rename, and Unicode law tests**

Assert equivalent construction order gives identical path/byte vectors; a mutable key rename changes only the containing record and never its shard; composed/decomposed strings remain distinct and round-trip unchanged; and published shard vectors match SHA-256.

- [ ] **Step 6: Run the storage crate gate**

Run: `cargo test -p tachiko-storage --locked`

Expected: all existing direct `.ro` tests and new `.roproj` tests pass.

### Task 3: Native admission, canonicalization, and atomic publication

**Files:**
- Create: `crates/storage/src/roproj/host.rs`
- Create: `crates/storage/tests/roproj_host.rs`
- Modify: `crates/storage/src/roproj/mod.rs`
- Modify: `crates/storage/src/lib.rs`

**Interfaces:**
- Consumes: Task 2's canonical tree constructor/decoder.
- Produces: `read_canonical_roproj`, `canonicalize_roproj`, `load_roproj`, `publish_roproj`, and `materialize_roproj`.

- [ ] **Step 1: Add a failing no-clobber publication test**

```rust
#[test]
fn publication_never_overwrites_or_leaves_partial_destination() {
    let tree = encode_roproj_v1(&Document::empty("doc-empty", "Empty")).unwrap();
    publish_roproj(&destination, &tree).unwrap();
    let before = read_tree_bytes(&destination);
    assert!(matches!(publish_roproj(&destination, &tree), Err(FormatError::AlreadyExists { .. })));
    assert_eq!(read_tree_bytes(&destination), before);
}
```

- [ ] **Step 2: Run the host test and prove red**

Run: `cargo test -p tachiko-storage --test roproj_host publication_never_overwrites_or_leaves_partial_destination --locked`

Expected: compilation fails because host publication is not implemented.

- [ ] **Step 3: Implement staged sibling-directory publication**

Encode and validate before touching the destination, create a unique sibling staging directory with `create_dir`, write all eighteen files there, and rename only after every write succeeds. Remove staging on every failure; classify pre-existing destinations as `AlreadyExists`.

- [ ] **Step 4: Add the exact canonical-tree reader**

Reject root/entity symlinks, non-regular entries, missing/extra paths, nested directories, and byte-level noncanonical forms. Dispatch the manifest before reading schema/entity bodies, then construct the canonical tree and decode it.

- [ ] **Step 5: Add the bounded canonicalizer**

Admit only exact top-level names plus ordinary nested `*.jsonl` entity files; require LF-delimited one-object records; allow noncanonical JSON/member/ID order, shard names/placement, missing empty shards, and extra empty JSONL files; reject blank records, symlinks, non-JSONL files, dead directories, duplicates, unknown data, and invalid semantics; return a fresh canonical tree without mutating the source.

- [ ] **Step 6: Run focused host tests**

Run: `cargo test -p tachiko-storage --test roproj_host --locked`

Expected: exact read, bounded canonicalization, outside-Git operation, no overwrite, and no partial publication tests pass.

### Task 4: First-party CLI composition

**Files:**
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/cli/src/commands.rs`
- Modify: `crates/cli/tests/cli.rs`

**Interfaces:**
- Consumes: storage `load_roproj`/`canonicalize_roproj`/`publish_roproj` and workspace `validate`.
- Produces: explicit `tachiko roproj materialize`, `tachiko roproj validate`, and `tachiko roproj canonicalize` commands.

- [ ] **Step 1: Add a failing outside-Git CLI journey**

Create a direct `.ro` fixture in a temporary non-repository directory, materialize it to an absent `.roproj`, validate the result, reject a second materialization to the same destination, and canonicalize an admitted noncanonical copy to a distinct destination.

- [ ] **Step 2: Run the journey and prove red**

Run: `cargo test -p tachiko-cli --test cli roproj_workflow_operates_outside_git --locked`

Expected: command parsing fails because `roproj` is not defined.

- [ ] **Step 3: Add the explicit CLI subcommands**

```rust
enum RoprojectCommands {
    Materialize { input: PathBuf, output: PathBuf },
    Validate { path: PathBuf },
    Canonicalize { input: PathBuf, output: PathBuf },
}
```

For materialize/canonicalize, run the shared workspace validation report before publication. Never infer or convert a direct `.ro` representation during ordinary read; only the explicit materialize command composes the two formats.

- [ ] **Step 4: Run CLI and storage gates**

Run: `cargo test -p tachiko-cli --test cli --locked`

Run: `cargo test -p tachiko-storage --locked`

Expected: all tests pass with existing direct `.ro` behavior unchanged.

### Task 5: Native/WASM evidence and user-facing documentation

**Files:**
- Modify: `scripts/portable-conformance-check.rs`
- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Modify: `docs/specs/roproj-format.md`
- Modify: `docs/specs/roproj-layout-v1.md`
- Modify: `docs/specs/canonical-json-profile.md`
- Modify: `docs/specs/storage-versioning-and-migration.md`
- Modify: `THIRD_PARTY_LICENSES.md`

**Interfaces:**
- Consumes: the pure Task 2 codec and Task 4 CLI contract.
- Produces: executed native/WASM exact-byte parity plus accurate implementation-status and usage documentation.

- [ ] **Step 1: Add a failing portable conformance record**

Hash the ordered canonical `.roproj/v1` paths and bytes for the existing portable semantic document, decode the tree, re-encode it, and expose a fixed record through the existing native/WASM harness. Increment `CASE_COUNT` and assert the record is identical on both targets.

- [ ] **Step 2: Run the conformance harness**

Run: `bash scripts/portable-conformance-check.sh`

Expected before implementation wiring: failure from the new missing/mismatched record; expected after wiring: native and WASM outputs are identical and contain no unexpected class.

- [ ] **Step 3: Update implementation-status and CLI documentation**

Document the three explicit commands, no-clobber behavior, standalone operation, and the pure exact-tree consumer seam. Change only implementation-status prose in Accepted specs; do not alter their normative contract.

- [ ] **Step 4: Regenerate dependency notices**

Run: `bash scripts/generate-third-party-licenses.sh > /tmp/tachiko-issue-123-third-party.md`

Apply the generated diff to `THIRD_PARTY_LICENSES.md` and verify it matches exactly.

- [ ] **Step 5: Run focused formatting, lint, and test gates**

Run: `cargo fmt --all -- --check`

Run: `cargo clippy -p tachiko-storage -p tachiko-cli --all-targets --locked -- -D warnings`

Run: `cargo test -p tachiko-storage -p tachiko-cli --all-targets --locked`

Expected: all commands pass.

### Task 6: Repository verification, review, and delivery

**Files:**
- Inspect: every changed file since `origin/main`

**Interfaces:**
- Consumes: completed tasks 1 through 5.
- Produces: one focused reviewed commit series and one Issue #123 pull request.

- [ ] **Step 1: Check diff hygiene and run the fast workspace gate**

Run: `git diff --check`

Run: `cargo fmt --all -- --check`

Run: `cargo clippy --workspace --all-targets --locked -- -D warnings`

Run: `cargo test --workspace --all-targets --locked`

- [ ] **Step 2: Commit the complete focused change**

Commit with an Issue-linked message such as `feat: implement canonical roproj v1 codec (#123)` and verify the worktree is clean.

- [ ] **Step 3: Run the release-equivalent gate from the clean commit**

Run: `bash scripts/release-check.sh`

Expected: formatting, dependency graph, Clippy, tests, native/WASM conformance, docs, MSRV, notices, packages, smoke journeys, and release archive checks all pass.

- [ ] **Step 4: Run two-axis code review against `origin/main`**

Use the repository `code-review` skill with Issue #123 as the spec source and `AGENTS.md` plus `CONTRIBUTING.md` as standards. Resolve every actionable Standards and Spec finding, rerun affected tests, and repeat review if material code changes.

- [ ] **Step 5: Push, open, and merge the focused PR**

Push without force, open a PR that closes #123 and records exact verification, monitor required checks/review, address every actionable finding in new commits, and merge only when repository requirements are satisfied.

- [ ] **Step 6: Recalibrate after merge**

Fetch live `main`, inspect the canonical roadmap plus GitHub Project, confirm dependencies/open PRs, select the next genuinely Ready critical-path issue, and begin a fresh issue branch and PR without bundling work.
