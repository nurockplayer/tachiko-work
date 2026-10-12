# Issue #519 independent first-public package vector

Authority: ADR-0042 and `docs/specs/portable-package-v1.md` at baseline
`753bd63a20f2ccab53d00e5657a3a57a2e962912` (PR #518 head). This is Steward
acceptance evidence; it does not authorize production implementation or Ready.

## Input provenance

`input.roproj/` is the exact output of the already-merged public
`tachiko_storage::encode_roproj_v3` for the fixed document in
`acceptance/portable-package-v3/document.rs`. Its two rows contain Text, Number,
and Date, a saved keyed grouped-sum definition, TextLiteralSet and
NumberInclusiveRange constraints. All 19 paths are retained, with 14 zero-byte
shards. The document uses the existing v3 acceptance fixture pattern; its
import-style IDs are fixed test identities, not evidence of a user import.

`runtime-control.roproj/` is encoded by the same accepted encoder for the same
document with every constraint set to None. It is a supplementary control to
reach the existing Designer admission and package-export boundaries; it does
not replace the constrained acceptance requirement. Current baseline runtime
v3 admission explicitly refuses constrained fields.

The v3 encoder Git blob is `323656cd318060854e30037edfa479fd16c9cd3f`.
Live GitHub `main` at capture was `b1560ab4331847557f04dfd6833fbffcabfc1169`;
its `crates/storage/src/roproj/v3.rs` blob matches the baseline exactly. Both
capture/decode checks actually passed. No candidate package encoder was used.

To reproduce inputs in an isolated checkout of that baseline:

```sh
export CARGO_TARGET_DIR=/Users/tachikoma/Developer/tachiko-work/target
export CARGO_INCREMENTAL=0
cp docs/research/probes/issue-519-capture-v3.rs crates/storage/tests/issue_519_capture.rs
cargo test -p tachiko-storage --test issue_519_capture --locked
rm crates/storage/tests/issue_519_capture.rs
```

The capture harness is intentionally outside the automatic test discovery
surface: ordinary acceptance execution reads sealed inputs and asserts their
identity against the accepted v3 encoder instead of rewriting its oracle.

## Independent package oracle

`docs/research/probes/issue-519-portable-package-v1-v3.mjs` uses only Node
built-ins. It reads the captured input bytes, implements the spec's ZIP32 local,
central and end records directly, computes ZIP CRC-32 and the 19-leaf v3-domain
SHA-256 root, and walks the completed archive to independently read back offsets,
sizes, names, CRCs, and root. The CRC primitive checks the standard `123456789`
vector and empty input. It never imports or runs a Rust package codec.

`golden.hex`: 6,155 package bytes, 20 entries, 228-byte package manifest.

- Package SHA-256: `2e8b98f4926c931063cf2caa6ee4e73a8152cace7f9cd5ca9ccc17e212362a7c`
- Payload root: `cdf2cec90364b1562f5f8e483ace2f98f4d26a1f609818bff1194048270abf17`

`runtime-control.hex` packages the unconstrained control. `refusals.json` maps
30 independently built refusal vectors to the spec's stable symbolic classes.
Payload canonicality/semantic/mismatch cases have newly computed CRCs and roots;
CRC corruption deliberately retains stale CRCs, and integrity corruption has a
valid manifest CRC. The semantic-invalid case changes an entity's schema
reference without changing its ID-based shard or v3 DTO syntax. An additional
storage fixture check confirms the real `InvalidDocument` diagnostic.
`developmental.hex` is an unchanged copy of the historical issue-43 empty
package; the historical fixture and probe themselves remain untouched.

```sh
node docs/research/probes/issue-519-portable-package-v1-v3.mjs
```

Default mode verifies every fixture byte and refusal mapping. `--write` is an
explicit oracle-authoring mode, not an acceptance runner or implementation fix.
Inputs must remain the accepted encoder capture for the fixed document.
