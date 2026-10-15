# ADR-0043 supplementary named vector

`request.json` describes one zero-based multi-input allocation request: `項目`,
`Item Name`, then the already-occupied ASCII-derived `column_1`. The expected
keys are `column_1`, `column_2`, `column_3`. The fixed occurrence determines IDs
through the existing native allocation seam; stable-ID order never allocates keys.

The nineteen `tree/` files were hand-authored from ADR-0043 and the v3 layout.
`named.ro` and `expected.json` were constructed by `reference.mjs`, a separate
Node standard-library SHA-256/CRC32/ZIP32 implementation over those static bytes.
No candidate Rust codec or candidate-generated JSON supplied expected bytes.
The receipt freezes archive length/hash, payload root, and all twenty CRCs.

Run `node acceptance/native-display-names/reference.mjs --check` to independently
verify the static vector. `--write` is explicit fixture maintenance, never a test
step. Existing nameless goldens remain unchanged. Storage, native runtime, CLI
fresh-process and exported-kit fresh-Worker tests consume the static vector.
This is supplementary implementation coverage; it does not replace the lead's
independent Guarded acceptance review or Sheet's consumer re-pin qualification.
