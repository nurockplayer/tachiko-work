# Agent Instructions

This file applies to the entire repository. Read [`CONTRIBUTING.md`](CONTRIBUTING.md) before changing code, tests, release behavior, or documentation. Existing accepted ADRs, canonical specifications, and explicit issue scope take precedence over generic preferences in this file.

## Working principles

Use a DX-driven, tooling-minimalist, automation-first approach:

- prefer fast tools with strong defaults;
- preserve deterministic and reproducible behavior;
- minimize configuration and overlapping tooling;
- keep changes focused and independently reviewable;
- validate behavior automatically rather than relying on manual confidence;
- do not migrate toolchains as incidental cleanup.

Tachiko Work is currently Rust-first. Preserve the checked-in Cargo workflow, lockfile, crate boundaries, and validation gates documented in `CONTRIBUTING.md`.

For architecture placement and code-quality review, read the
[`Clean Architecture and Clean Code playbook`](docs/engineering/clean-architecture-and-code.md).
Respect its decision state and the adoption record in Issue #402. Its illustrative
module layouts and future guard ideas do not authorize refactors, new contracts,
or changes to the existing SCD gates.

For issue-driven repository work, follow the canonical
[`Repository delivery workflow`](docs/governance/project-governance.md#repository-delivery-workflow).
It defines the Ready gate, provider-neutral Steward/delivery-agent split,
single-PR handoff, review discipline, and post-merge stop conditions.
Before an implementation Issue becomes Ready, apply the repository
[`PR Decomposition Policy`](docs/governance/pr-decomposition-policy.md); do not
let the one-Issue/one-PR rule turn a separable product goal into an oversized
review unit.
Apply the [`Delivery Throughput Policy`](docs/governance/delivery-throughput-policy.md)
to classify delivery risk, choose the review/validation route, coordinate safe
parallel Ready lanes, and keep GitHub checkpoints at material stage boundaries.

## Delivery continuity

For implementation, resume, or supervision of an active Ready Issue/PR under
Stewarded Continuous Delivery, use the repository-local
`$tachiko-work-scd-delivery` Skill. It owns the repeatable continuity,
async-wait, durable-handoff recovery, and canonical stop procedure.

The Skill is operational guidance only: the canonical repository delivery
workflow, accepted specs/ADRs, owning Issue/PR, and live GitHub state remain
authoritative.

## Rust ownership, async, and unsafe boundaries

Treat compiler pressure around ownership, lifetimes, async state machines, and
pinning as a design signal rather than something to silence mechanically.
Before changing code in this area, identify the ownership/lifetime boundary and
prefer restructuring ownership over adding indirection.

- Do not introduce `Arc`, `Mutex`, broad `clone()`, `Box::pin`, boxed futures,
  `async move`, `spawn_blocking`, or `unsafe` solely to make the compiler accept
  a design. Each such escape hatch must have a semantic reason independent of
  the compiler error it happens to resolve.
- When `.await` is involved, identify which values and borrows cross each
  suspension point. Explain any `Send` or `'static` requirement instead of
  assuming task spawning requires ownership inflation everywhere.
- Do not hold a lock guard, mutable borrow, or other exclusive resource across
  `.await` unless the behavior is intentional, bounded, and explicitly
  justified.
- Changes involving `Pin`, manual `Future::poll`, self-referential structures,
  unsafe blocks/functions/impls, or comparable soundness-sensitive mechanics
  require the repository's **Guarded** delivery route and fresh independent
  deep review.
- The producer-local `packages/browser-client/runtime` crate permits Rust `unsafe_code` only
  because its browser ABI uses Rust 2024 `#[unsafe(no_mangle)]` export
  attributes. That exception is not permission to add unsafe blocks, unsafe
  functions, unsafe impls, or unrelated unsafe attributes. Expanding the unsafe
  surface requires explicit Issue scope and Guarded review.

When these mechanics are touched, record the ownership/async boundary and any
escape-hatch justification in the owning Issue or PR evidence. If they are not
touched, mark the PR boundary check as not applicable rather than inventing a
justification.

## JavaScript and TypeScript

Node.js is used by parts of the release and WASM validation workflow. If package-managed JavaScript or TypeScript tooling is introduced or modified:

- prefer TypeScript for new application or library code;
- use `pnpm` exclusively as the package manager;
- do not use `npm` or `yarn` for installs, dependency management, script execution, or lockfile generation;
- do not create or commit `package-lock.json` or `yarn.lock`;
- preserve an explicit `packageManager` declaration for `pnpm` when a `package.json` is present;
- Bun is evaluation-only. Do not migrate runtime or package-management workflows to Bun, create `bun.lock`, or introduce Bun-specific assumptions unless an explicit project decision authorizes it.

## Python

If Python tooling is introduced or modified:

- use `uv` for Python versions, environments, dependency management, locking, and command execution where applicable;
- use `Ruff` for linting and formatting;
- avoid direct `pip` workflows when `uv` can perform the same repository task reproducibly;
- do not introduce Poetry, Pipenv, Black, Flake8, isort, or equivalent overlapping tooling unless a documented project requirement makes it necessary.

## Tooling changes

Do not add a second tool when the current toolchain already covers the responsibility well. Treat changes to runtimes, package managers, formatters, linters, build systems, serialization tools, or release tooling as explicit engineering decisions when they affect repository-wide workflows or long-term maintenance.

Repository-specific correctness and compatibility requirements always outrank personal tooling preference. If a required upstream ecosystem command cannot be replaced safely, document the exception rather than forcing a migration.
