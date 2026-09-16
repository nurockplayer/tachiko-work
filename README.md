# Tachiko Work

<p align="center">
  <img src="docs/assets/brand/tachiko-work-hero.webp" alt="Tachiko Work concept: typed data, validated changes, and human approval" width="100%">
</p>

**Make structured data understandable to people, software, and AI — then check every change before it becomes real.**

Tachiko Work is for the moment when a spreadsheet stops being “just a table.” A
number might mean weapon damage, another cell might calculate DPS, and changing
one value might affect ten other things. Today, that meaning is often scattered
across column names, formulas, scripts, documents, and people’s memory.

Tachiko makes that meaning explicit: types, relationships, formulas, rules,
dependencies, and changes all live in one deterministic model.

The first proving ground is **game-balance data**. Change a weapon value, see
what depends on it, validate the result, and review the change before it becomes
the next accepted state.

> **Public pre-alpha.** There is no tagged binary release yet. The complete proof
> is still CLI-first, with an early browser Designer slice. The visuals in this
> README are approved product-direction artwork, not screenshots or evidence of
> a fully shipped Astra/spreadsheet experience.

## The problem in one example

Imagine changing an Iron Sword’s damage from `40` to `46`.

A person — or an AI — should not have to guess:

- Is `46` a valid value for this field?
- Which formulas depend on it?
- Does the change break a rule?
- What else changes because of it?
- What exactly should a reviewer approve?

Tachiko keeps enough structure to answer those questions directly instead of
reconstructing meaning from raw cells or file layout every time.

<p align="center">
  <img src="docs/assets/brand/tachiko-work-verified-change.webp" alt="Concept flow from a request to a typed, validated and approved change" width="100%">
</p>

## What Tachiko does

- **Typed data** — values have real types and identities, not just coordinates.
- **Relationships** — references stay meaningful when data moves or changes.
- **Formulas and dependencies** — Tachiko knows what is calculated from what.
- **Validation** — deterministic rules check whether a candidate state is valid.
- **Semantic diff and impact** — review what changed and what it affects.
- **Approval boundaries** — AI can propose changes without silently making them
  authoritative.

## How a safe change works

<p align="center">
  <img src="docs/assets/brand/tachiko-work-how-it-works.webp" alt="Conceptual Tachiko workflow from human intent through typed proposal, deterministic validation, impact analysis and approval" width="100%">
</p>

1. **Start with an intent.** A human, UI, CLI, or AI client asks for a change.
2. **Turn it into a typed operation.** The change targets semantic data, not a
   guessed screen position.
3. **Check it deterministically.** Tachiko evaluates types, formulas,
   dependencies, validation rules, and affected results.
4. **Review before applying.** Where approval is required, the proposal remains
   inert until it is accepted.

The important split is simple: **AI can reason about what you want; Tachiko
checks what the change actually means.**

## What works today

On current `main`, Tachiko already has an end-to-end game-balance proof with:

- typed schemas, entities, stable IDs, references, formulas, and validation;
- dependency tracking plus semantic diff and impact analysis;
- deterministic typed edits, three-way merge, and JSON export;
- canonical editable `.roproj/v1` projects and supported `.ro` forms;
- local workflows that do not require Git, GitHub, or an AI provider;
- provider-free semantic inspection and bounded analysis queries;
- approval-gated AI-facing proposals instead of raw AI file mutation;
- an early browser Designer slice running over the same Rust-authoritative
  runtime as the CLI.

## What is not shipped yet

Tachiko does **not** currently claim a completed general spreadsheet UI, full
Office compatibility, realtime collaboration, cloud SaaS, production game-engine
plugins, or the complete conversational Astra experience shown in the artwork.

Those are product directions, not current executable evidence.

## AI and Git are optional

Tachiko does not need AI to understand its own data, and it does not need Git to
work locally.

- **AI** is a semantic client: it can inspect, explain, and propose typed changes.
  It is not a second source of truth.
- **Git** is an optional storage/review protocol. It is useful for history and
  review, but it is not the semantic model or the user interface.

The deterministic engine remains authoritative in both cases.

## The names

- **Tachiko** — the umbrella brand.
- **Tachiko Work** — the platform/product vision and this repository.
- **Tachikore** — the composed Rust semantic/application engine and resident
  runtime beneath first-party clients.
- **Tachiko Sheet** — the spreadsheet client/product.

Tachikore is a name for the existing engine composition, not a new ninth crate
or a second semantic authority. See
[ADR-0038](docs/decisions/ADR-0038-tachikore-semantic-application-engine-name.md).

## Try it in five minutes

There is no published binary yet, so install the CLI from a repository checkout.
Rust 1.85 or newer is required:

```sh
cargo install --path crates/cli --locked
```

Create a tiny balance project, inspect a formula, make a change, and validate it:

```sh
tachiko_demo=$(mktemp -d "${TMPDIR:-/tmp}/tachiko-demo.XXXXXX")

tachiko init "$tachiko_demo/balance.ro" --title "My Game Balance"
tachiko show "$tachiko_demo/balance.ro"
tachiko explain "$tachiko_demo/balance.ro" iron_sword.dps

tachiko set "$tachiko_demo/balance.ro" iron_sword.damage 45 \
  --output "$tachiko_demo/buffed.ro"
tachiko diff "$tachiko_demo/balance.ro" "$tachiko_demo/buffed.ro"
tachiko validate "$tachiko_demo/buffed.ro"
```

For the durable checked-in example and expected results, start with
[`examples/game-balance/`](examples/game-balance/README.md).

## Where to go next

- **See the real example:** [`examples/game-balance/`](examples/game-balance/README.md)
- **Run the browser Designer slice:** [`apps/designer/`](apps/designer/README.md)
- **Understand the architecture:** [`ARCHITECTURE.md`](ARCHITECTURE.md)
- **See the roadmap:** [`docs/product/product-roadmap.md`](docs/product/product-roadmap.md)
- **Read specifications and decisions:** [`docs/README.md`](docs/README.md)
- **Contribute:** [`CONTRIBUTING.md`](CONTRIBUTING.md)
- **Report security issues:** [`SECURITY.md`](SECURITY.md)

## Principles

- **Meaning comes before presentation.** Views and file formats are projections
  or adapters, not competing sources of truth.
- **Correctness is deterministic.** Equivalent inputs should produce stable
  calculation, validation, diff, merge, and export results.
- **AI proposes; it does not become truth.** AI-facing changes cross the same
  deterministic and approval boundaries as other clients.
- **Work should stay portable.** The long-term direction favors open semantic
  and interoperability surfaces over vendor lock-in.

## Contributing and licensing

Tachiko Work is intentionally public while still pre-alpha. APIs, formats, and
contribution policy are evolving.

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the current contribution boundary and
[`docs/governance/licensing-posture.md`](docs/governance/licensing-posture.md) for
the current licensing posture.
