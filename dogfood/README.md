# Repository dogfood projects

This directory contains small canonical projects used as real Tachiko Work
product evidence. They remain ordinary semantic projects: repository and
GitHub governance continue to own delivery state, and these projects do not
define public templates or normative schema authority.

`product-gaps.roproj` is generated from the app-local deterministic constructor
in `apps/designer/runtime/fixtures/product_gaps.rs` through the existing storage
materializer:

```sh
cargo run --manifest-path apps/designer/runtime/Cargo.toml \
  --example materialize_product_gaps -- \
  dogfood/product-gaps.roproj
```

The Designer runtime fixture test pins the checked-in tree to that constructor
byte-for-byte and validates its formulas through workspace authority. Remove
the existing destination before deliberately regenerating it; materialization
is create-only and never overwrites a project.
