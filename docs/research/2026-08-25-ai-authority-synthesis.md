# Issue #9 AI authority synthesis

Decision state: Research / decision evidence. This record supports the ADR-0007 amendment; the ADR remains architecture authority.

## Outcome

Issue #9 retains ADR-0007's core direction and sharpens its authority model:

- AI is a semantic client that may act only through delegated authority;
- provider/model identity is provenance, not privilege;
- semantic validity and operation gates do not grant authorization;
- authorization cannot override semantic failure;
- MVP AI-originated canonical mutation remains explicitly approval-gated;
- approval must not silently carry over to a materially changed transition or materially changed relevant authorization context;
- first-party AI mutation must use the shared Semantic API/workspace-engine semantic path and trusted authorization/approval enforcement;
- semantic publication, durable persistence, and external publication are separate authority domains; and
- future bounded autonomy may be introduced only through explicit scoped delegation, without making AI a source of truth.

## Deliberately not frozen

This decision does not define capability identifiers, principal structs, scope grammar, approval tokens/digests, revision binding, lifetime/replay/revocation, provenance storage, runtime/session placement, external API DTOs, or host/plugin security mechanics. Those remain with #27/#28/#30/#26 as applicable.

## Narrowing from research wording

The architecture does not require exact byte/digest equality between an approved proposal and later execution as an Accepted law. It requires that approval not silently survive a materially changed semantic transition or materially changed relevant authorization context. The concrete comparison/binding mechanism remains #28.

Likewise, trusted Execute enforcement is a required logical role, not a new crate or service decision. Its implementation placement remains open.

## Relationship to ADR-0020

ADR-0020 already defines the first-class Semantic API laws, including Query/Command, Propose/Execute, authoritative gates, semantic atomicity, capability-addressability, and compatibility. Issue #9 adds authority constraints on who may exercise Execute; it does not reopen or duplicate ADR-0020.
