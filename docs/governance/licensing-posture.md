# Licensing Posture

Status: Founder-accepted product/governance direction; legal implementation gate pending in GitHub issue #202. Final policy/implementation remains tracked through #15 and #202.

## Public pre-alpha

Tachiko Work is intentionally developed in a public repository so users, reviewers, AI tools, and future contributors can inspect the implementation, architecture, and decision history.

Public repository access does not mean the project has reached a stable public release. APIs, formats, workflows, and licensing implementation details may still change before the first tagged release.

## Historical license grants

Repository revisions already published under `Apache-2.0 OR MIT` remain available under those terms. A later license or policy change cannot revoke rights already granted for those published revisions.

## Founder-accepted direction

The earlier provisional direction that treated `MPL-2.0` as the leading candidate for future core and CLI revisions is superseded by the founder decision recorded in Issue #15.

The accepted product/governance direction is to retain `Apache-2.0 OR MIT` for the adoption-facing semantic substrate and interoperability surfaces, including the core Rust crates, CLI, local runtime, WASM/reference implementation, SDKs/public plugin APIs, import/export and compatibility libraries, host/game-engine adapters, schemas/examples/conformance code, and baseline local client capability required to read, edit, validate, and export legitimately held Tachiko work.

Tachiko should earn primarily through separately bounded managed and enterprise value such as hosted coordination, reliability/operations, enterprise administration and compliance, hosted AI/compute, distribution/curation, migration/integration, support, certified/LTS builds, warranty/indemnity, and expertise. Subscription or service state must not become semantic authority or make legitimately held local work unreadable or unexportable.

A future separately bounded collaboration server may revisit stronger network copyleft only through a separate evidence-backed decision. No such license is selected by this policy.

## Contributor governance direction

The preferred contributor model is DCO 1.1 plus explicit inbound-equals-outbound terms rather than a blanket CLA, copyright assignment, or founder-only relicensing right.

This is a governance direction, not active contribution terms. Issue #202 must first confirm chain of title, legal sufficiency, corporate-contributor handling, and exact wording.

## Specifications, patents, trademark, and compatibility

Provisional specifications and documentation remain under the current repository terms for now; do not create immediate license sprawl.

Before a stable normative public specification or an external normative-spec contribution program, Issue #202 must advise on a legally sound copyright and royalty-free format-essential patent framework. The current working recommendation is CC BY 4.0 for normative prose, `Apache-2.0 OR MIT` for schemas/examples/reference and conformance code, plus a counsel-reviewed patent commitment.

Trademark should protect the `Tachiko Work` name and logos without becoming permission control over independent implementation. Truthful compatibility claims should remain possible under distinct branding, and any certification claim must rest on public, versioned, reproducible conformance evidence.

## Contributions while the legal gate is open

External implementation/code and normative-specification contributions remain temporarily paused until Issue #202 records the approved non-privileged legal outcome and governance approves the exact inbound terms and ownership baseline.

Issue reports, architecture/product discussion, review findings, and documentation feedback remain welcome.

Do not activate DCO, CLA, assignment, relicensing, stable-spec patent terms, or other policy-dependent contribution mechanics merely because the founder direction is accepted.

## Principles

- Keep the project genuinely open and independently implementable.
- Do not use user data, semantic meaning, or file-format lock-in as the business model.
- Treat already-granted open-source rights as permanent.
- Keep adoption and interoperability surfaces open rather than turning them into tollbooths.
- Prefer commercial value from operations, distribution, enterprise capabilities, support, and expertise.
- Keep stronger-copyleft, marketplace, OEM, and foundation choices evidence-triggered and component-specific.

This document records project governance direction, not legal advice. Issue #202 is the required legal implementation gate before policy-dependent changes.
