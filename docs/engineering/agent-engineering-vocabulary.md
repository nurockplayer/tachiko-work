# Agent Engineering Vocabulary

Issue: [#423](https://github.com/nurockplayer/tachiko-work/issues/423)

This document defines a compact, provider-neutral engineering vocabulary for
Stewarded Continuous Delivery (SCD). The terms are **leading words**: short names
for established engineering ideas that agents can use to select a useful
implementation or review shape without repeatedly loading long explanations.

This is engineering guidance only. The Product Constitution, Accepted ADRs and
policies, normative specifications, the active Issue scope and acceptance
criteria, executable evidence, and repository delivery gates always win. A
leading word is never evidence that an implementation is correct.

Use this vocabulary selectively. Prefer the smallest number of terms that
actually change the plan, implementation, debugging strategy, or review. Do not
inject this whole document into every runtime prompt.

## Vocabulary

| Term | Meaning | Use when | Observable behavior | Boundary / misuse |
| --- | --- | --- | --- | --- |
| **tracer bullet** | A narrow but complete end-to-end path through the real layers required for one observable behavior. | Decomposing a cross-boundary feature or choosing the first implementation slice. | The slice can be demonstrated and verified on its own and reaches the relevant layers early instead of completing whole horizontal layers first. | It does not override the [PR Decomposition Policy](../governance/pr-decomposition-policy.md), justify an oversized PR, or require every mechanical refactor to be vertically sliced. |
| **walking skeleton** | The thinnest real end-to-end system path that proves a new subsystem can compose before breadth is added. | Bootstrapping a new subsystem, client, adapter, or deployment path whose basic composition is still unproven. | Real boundaries connect and one minimal journey runs through them before feature breadth or polish is added. | It is not a fake scaffold, mock-only demo, or excuse to declare an incomplete product finished. |
| **deep module** | A module whose useful interface stays small while substantial complexity is hidden behind it. | Reviewing API shape, responsibility placement, or whether callers are being forced to understand internals. | Callers need fewer concepts and parameters while implementation complexity remains owned behind the boundary. | A small interface must not hide required authority, failure modes, or product semantics; do not create abstraction merely to reduce line count. |
| **information hiding** | Keep volatile implementation decisions behind the boundary that owns them. | An implementation detail, representation, provider choice, cache, or mechanism is leaking into callers. | Callers depend on stable meaning or capability rather than on replaceable internal choices. | Do not hide behavior that is itself a public contract, accepted invariant, required diagnostic, or review evidence. |
| **ubiquitous language** | Use the same domain terms across Issues, specs, types, APIs, UI concepts, and tests where they denote the same meaning. | Naming a domain concept or reviewing terminology drift across layers. | One concept keeps one deliberate name, and synonyms are removed or explicitly distinguished when meanings differ. | Repository wording does not create product authority by itself; do not force one word across genuinely different bounded meanings. |
| **functional core, imperative shell** | Keep deterministic meaning and calculation in a pure or tightly controlled core, with host effects at explicit outer boundaries where the existing architecture supports it. | A change mixes semantic calculation with filesystem, network, clock, UI, provider, or persistence effects. | Deterministic logic can be exercised without performing host effects; effects remain explicit and separately observable. | This does not authorize new layers or override Tachiko's accepted runtime, storage, or host ownership. See the [Clean Architecture and Clean Code playbook](clean-architecture-and-code.md). |
| **make illegal states unrepresentable** | Prefer types or state transitions that prevent invalid combinations when a real invariant can be expressed clearly. | Several booleans, nullable fields, or ad-hoc checks represent mutually exclusive or lifecycle-constrained states. | Invalid combinations cannot be constructed through the ordinary typed path, or are rejected at one explicit admission boundary. | Do not introduce elaborate type-state ceremony for weak or unsettled invariants, and do not mistake a Rust type for proof across an untrusted boundary. |
| **seam** | The smallest controllable boundary where behavior can be reproduced, observed, substituted, or tested. | Debugging, isolating a failure, or choosing where a focused test should enter the system. | A failing behavior can be reproduced with fewer unrelated dependencies while preserving the relevant real boundary. | A test-only seam must not become a second product authority or bypass the actual path whose behavior is under review. |
| **characterization test** | A test that captures current observable behavior before risky repair or refactor when that behavior is not yet adequately encoded. | Changing legacy or poorly specified code where accidental behavior loss is a material risk. | The pre-change behavior is recorded and a later change makes any deliberate behavior difference explicit. | Captured behavior is evidence, not authority; do not preserve a known bug or contradiction merely because the characterization test observed it. |
| **differential test** | Run two implementations or execution targets against the same inputs and compare them with the same oracle. | Equivalence, migration, portability, incremental-vs-full, native-vs-WASM, or old-vs-new behavior is the property under test. | Both sides are evaluated from the same cases and mismatches are reported explicitly rather than normalized away. | Equality is only meaningful for the contract actually shared by both sides; do not compare outputs that legitimately differ by authority, permissions, host effects, or representation. |
| **tight feedback loop** | Use the smallest check that can disprove the current change quickly, then broaden validation at material gates. | Implementing, debugging, or repairing a scoped change with a clear local hypothesis. | Focused checks run early and frequently; broader repository and exact-head gates still run before delivery. | Fast local checks never replace required acceptance, hosted CI, independent review, or exact-head validation. |

## Stage mapping

Use the table as a shortlist, not as a mandatory checklist.

| Stage | Terms to consider first |
| --- | --- |
| Ticket decomposition | **tracer bullet**, **walking skeleton** |
| Implementation | **seam**, **tight feedback loop**, **functional core, imperative shell**, **make illegal states unrepresentable** |
| Architecture / API review | **deep module**, **information hiding**, **ubiquitous language** |
| Debugging / refactor | **seam**, **characterization test**, **differential test**, **tight feedback loop** |
| Final review | **deep module**, **information hiding**, **ubiquitous language**, plus the repository's existing authority/risk/evidence vocabulary |

## How to use a leading word

A useful leading word must change observable engineering behavior.

- Saying **tracer bullet** is useful only if the plan reaches a narrow real
  end-to-end behavior early.
- Saying **deep module** is useful only if the caller-facing surface becomes or
  remains smaller than the complexity it hides.
- Saying **seam** is useful only if the failure or test is isolated at a boundary
  that still exercises the behavior under question.
- Saying **tight feedback loop** is useful only if quick disproving checks are
  used without skipping broader required gates.

If the term does not fit the active authority or task shape, do not force it.
If a term is repeatedly echoed without changing behavior, remove it from the
prompt or workflow rather than adding more explanation.

## Relationship to existing Tachiko guidance

The [PR Decomposition Policy](../governance/pr-decomposition-policy.md) remains
the authority for Ready-Issue decomposition. **Tracer bullet** is the preferred
leading word for its existing vertical-slice rule; it does not create a second
policy.

The [Clean Architecture and Clean Code playbook](clean-architecture-and-code.md)
remains the repository's architecture-placement and code-quality guidance. This
vocabulary points at compact engineering concepts and does not restate that
playbook's ownership map.

SCD Ready, acceptance, risk, exact-HEAD validation, independent review, handoff,
merge, and post-merge rules remain unchanged.

The runtime experiment for stage-specific leading-word steering is tracked
separately in
[tachiko-conductor#77](https://github.com/nurockplayer/tachiko-conductor/issues/77).
Its results may justify keeping, changing, or removing runtime use of these
terms; they do not change this document's authority boundary.

## Method references

These references explain the writing/decomposition technique; they are not
Tachiko product authority.

- Matt Pocock, [Writing for Agents](https://github.com/mattpocock/skills/blob/main/docs/productivity/writing-for-agents.md) — leading words, context pointers, progressive disclosure, and pruning.
- Matt Pocock, [To Tickets](https://github.com/mattpocock/skills/blob/main/docs/engineering/to-tickets.md) — tracer-bullet vertical slices and the layer-first failure mode.
