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

For issue-driven repository work, follow the canonical
[`Repository delivery workflow`](docs/governance/project-governance.md#repository-delivery-workflow).
It defines the Ready gate, provider-neutral Steward/delivery-agent split,
single-PR handoff, review discipline, and post-merge stop conditions.

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
