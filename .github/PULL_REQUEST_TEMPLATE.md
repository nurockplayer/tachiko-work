## Summary

<!-- What changed and why? -->

## Traceability

- Implements: <!-- issue / spec / decision, or None -->
- Related: <!-- neighboring work, or None -->
- Authority: <!-- governing ADR / policy / spec, or None -->
- Decision impact: <!-- None | Implements existing decision | Introduces or amends durable decision -->
- Authority impact: <!-- None | Discussion / Research | ADR / Policy | Specification | Architecture | Governance -->
- Supersedes: <!-- previous artifact/behavior, or None -->

## Delivery risk / execution route

<!-- Follow docs/governance/delivery-throughput-policy.md and the owning Issue. -->

- Risk class: <!-- Fast | Standard | Guarded -->
- Review route: <!-- Fast: independent non-author review; Standard: independent final-head review; Guarded: fresh independent deep review -->
- Parallel ownership: <!-- primary write surface and any neighboring active lanes -->
- Validation route: <!-- targeted iteration checks; exact-head applicable acceptance/unit/document/repository checks; broader Guarded/milestone/release gate when applicable -->

## Validation / evidence

<!-- Tests, fixtures, CI, manual evidence, benchmarks, compatibility checks. -->

## Acceptance / unit-test handoff

<!-- Follow docs/governance/project-governance.md#acceptance-first-preparation-and-handoff. When product acceptance or delivery-agent unit tests do not apply, link the explicit bounded applicability exception decided by the Steward in the owning Issue and the applicable document/refactor checks. A handoff may point to that Issue decision but cannot create the exception. Do not invent red product tests. -->

- Acceptance baseline / seed or applicability baseline / final head: <!-- full commits, criterion-to-test mapping, or linked owning-Issue applicability decision plus document/refactor baseline -->
- Acceptance changes: <!-- none, disclosed mechanical repairs, or material changes with Steward decision and revised commit -->
- Delivery-agent unit tests / applicability: <!-- cases, commands and actual results, or linked explicit Steward applicability exception in the owning Issue -->
- Final validation / independent review: <!-- exact-head evidence and risk-appropriate review; keep missing or unexecuted checks explicit -->
- Remaining challenges: <!-- none, or evidence and decision needed -->

## Documentation impact

<!-- Docs updated, intentionally unchanged, or follow-up required. -->

## Boundary check

- [ ] Important rationale is not trapped only in chat, an agent handoff, or this PR description.
- [ ] Any new expensive-to-reverse contract is backed by explicit decision work.
- [ ] Superseded or stale authority is linked rather than silently overwritten.
- [ ] Acceptance changes are accounted for; no unapproved weakened or suppressed requirement remains.
- [ ] Risk classification and review route match the owning Issue; Guarded work received fresh independent deep review.
- [ ] Applicable acceptance/unit/document/repository gates and independent review cover the exact final head; a red seed or unchecked box is not merge readiness.
