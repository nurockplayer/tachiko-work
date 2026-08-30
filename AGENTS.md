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

## Delivery continuity

For a Ready Issue with an active agent-owned PR, continue the bounded one-Issue
delivery loop autonomously until a canonical stop or escalation condition is
actually reached.

A pending non-terminal sub-agent result, CI job, hosted review, or other
asynchronous validation is **not** a completion, handoff, or stop condition.
When the current execution environment can remain active, wait or poll for the
result, consume it when available, and continue the same review-fix /
exact-head-validation loop.

Runtime liveness is a separate concern from agent behavior. If an individual
agent run may terminate before a canonical stop condition is reached, first
make the continuation state recoverable outside ephemeral local state. Keep the
single canonical `agent-handoff:v1` exact and persist meaningful active work to
Git or another repository-approved durable artifact when necessary; do not leave
the only copy of progress as an uncommitted local diff.

If a run terminates before a canonical stop condition is reached, the scheduler
or orchestrator is responsible for automatically re-entering the same Issue/PR
from that durable handoff/checkpoint. A human copying and pasting a "continue"
prompt is not part of the intended delivery workflow.

Intermediate progress may update the single canonical `agent-handoff:v1`, but
do not present intermediate progress as task completion solely because an
asynchronous gate is still running or because one agent runtime ended. Return
control only for the stop/escalation conditions defined by the canonical
repository delivery workflow, including a genuine durable decision or authority
contradiction, an external/human-only permission requirement, or no genuinely
Ready work remaining after the required live-state recalibration.

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
