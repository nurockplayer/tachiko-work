# Licensing Posture

Status: Provisional governance direction. Final policy is tracked in GitHub issue #15.

## Public pre-alpha

Tachiko Work is intentionally developed in a public repository so users, reviewers, AI tools, and future contributors can inspect the implementation, architecture, and decision history.

Public repository access does not mean the project has reached a stable public release. APIs, formats, workflows, and licensing policy may still change before the first tagged release.

## Historical license grants

Repository revisions already published under `Apache-2.0 OR MIT` remain available under those terms. A later license change cannot revoke rights already granted for those published revisions.

## Direction for future revisions

The founder direction is to begin with a more protective open-source posture and relax selected components later when adoption and ecosystem evidence justify it.

`MPL-2.0` is currently the leading candidate for future core and CLI revisions because its file-level copyleft requires modifications to covered files to remain available while still allowing integration into larger proprietary works.

`AGPL-3.0` remains a candidate only if preventing proprietary hosted forks becomes more important than embedding, OEM, and game-studio adoption.

No final relicensing action is authorized by this document. Issue #15 must still resolve the license-by-component model, contributor copyright model, specification/document licensing, trademark/compatibility policy, and commercial/hosted boundary.

## Contributions while the decision is open

External code contributions are temporarily not accepted while the project preserves the ability to choose a coherent future licensing model. Issue reports, design discussion, documentation feedback, and review findings are welcome.

Before accepting external code contributions, Tachiko Work must decide whether contributions use a CLA, DCO, copyright assignment, or another model that preserves the intended licensing and commercial options.

## Principles

- Keep the project genuinely open and interoperable.
- Do not use user data or file-format lock-in as the business model.
- Prefer reversible policy changes where possible.
- Treat already-granted open-source rights as permanent.
- Make future license relaxation deliberate and component-specific rather than accidental.

This document is project policy, not legal advice.