# Tachiko Work

A Rust-native, Git-native, and AI-native workspace for semantic structured data
and computation.

## Vision

Tachiko Work is not an Office clone.

It is a semantic document platform where structured data, formulas, Git review,
and AI operations share one typed model. Traditional document and spreadsheet
views are future projections of that model rather than separate sources of
truth.

## First usable product

The current product provides a complete, safe game-balance workflow:

- typed schemas, entities, fields, and references;
- canonical, versioned `.ro` serialization;
- deterministic formula calculation and dependency tracking;
- semantic diff with derived formula impact;
- guided starter creation, browsing, explanation, and typed edits;
- CLI validation and evaluated runtime JSON export;
- read-only AI structure/formula/impact queries and approval-required suggestions.

It deliberately does not include a spreadsheet UI, Office compatibility,
realtime collaboration, cloud infrastructure, or game-engine plugins.

## Install from source

Tachiko Work requires Rust 1.85 or newer. From a repository checkout:

```sh
cargo install --path crates/cli --locked
tachiko --version
```

The installed binary is named `tachiko`. Contributors can instead use
`cargo run -p tachiko-cli -- <command>` without installing it.

## Try it in five minutes

Create a project you can immediately understand:

```sh
tachiko_demo=$(mktemp -d "${TMPDIR:-/tmp}/tachiko-demo.XXXXXX")
tachiko init "$tachiko_demo/balance.ro" --title "My Game Balance"
tachiko show "$tachiko_demo/balance.ro"
tachiko explain "$tachiko_demo/balance.ro" iron_sword.dps
```

Make a safe balance change. Tachiko creates a new document, checks it, and
shows both the direct edit and its derived impact:

```sh
tachiko set "$tachiko_demo/balance.ro" iron_sword.damage 45 \
  --output "$tachiko_demo/buffed.ro"
tachiko diff "$tachiko_demo/balance.ro" "$tachiko_demo/buffed.ro"
tachiko validate "$tachiko_demo/buffed.ro"
tachiko export "$tachiko_demo/buffed.ro" "$tachiko_demo/buffed.json"
```

The checked-in Moonfall example and expected output are documented in
[`examples/game-balance/README.md`](examples/game-balance/README.md).

Use `tachiko init scratch.ro --template empty` only when you intentionally want
to author schemas and entities directly in canonical `.ro` JSON.

## Workspace

- `semantic-core`: document, schema, typed values/references, validation
- `storage`: canonical `.ro` parsing, versioning, and serialization
- `formula-engine`: deterministic expression evaluation and dependencies
- `diff-engine`: entity/field changes and calculated impact
- `ai-api`: read/explain/suggest-only semantic operations
- `workflow`: reusable starter, overview, explanation, and safe-edit operations
- `cli`: the complete `init` → `show` → `explain` → `set` → `diff` workflow

Run all quality gates, including the real product journey, with:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
bash scripts/first-user-smoke.sh
```

## Principles

- Semantic correctness over historical accidents
- Legacy compatibility belongs at system boundaries
- Open formats over vendor lock-in
- Git is a storage protocol, not a user interface
- AI should manipulate semantic models, not imitate users

Architecture records and specifications live under [`docs/`](docs/). Accepted
ADRs take precedence over older exploratory roadmaps.
