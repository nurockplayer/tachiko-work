# AI-Native Architecture

Decision state: Accepted direction under
[ADR-0007](../decisions/ADR-0007-ai-semantic-interaction-model.md) and
[ADR-0020](../decisions/ADR-0020-first-class-headless-semantic-api.md).
Reviewable semantic proposals use the immutable revision-pinned SemanticPatch
contract Accepted by
[ADR-0024](../decisions/ADR-0024-revision-pinned-semantic-patch.md).
Capability, approval, provenance, digest/integrity, and execution enforcement
remain #28/#29 work.

## Principle

AI should operate on semantic objects, not simulate mouse and keyboard actions.

Traditional workflow:

User interface → file format → AI workaround

Tachiko Work workflow:

AI agent
→ typed Command or ordered AtomicBatch
→ revision-pinned immutable SemanticPatch / Propose
→ deterministic semantic review evidence
→ separately authorized Execute
→ canonical semantic state

The proposal envelope reuses the Semantic API operation vocabulary. It is not a
second AI-only mutation API, does not itself become a Command, and does not
write `.roproj` or grant approval.

## Examples

An AI agent can:

- update structured data
- explain formula dependencies
- generate documentation
- detect inconsistencies
- propose migrations
- review changes

The document model itself becomes an API for intelligent operations.

AI provider/model metadata, prompts, confidence, intent prose, and rendered
diff summaries may help review, but they are not semantic authority or the
exact change. Exact review binds the Semantic API compatibility contract,
semantic base, typed command semantics, and AtomicBatch order under ADR-0024.
