# Security Model

Decision state: Mixed. The semantic authorization laws summarized from
ADR-0007 and ADR-0026 are Accepted; ADR-0019 owns diagnostic-code meaning.
Issue #29 supplies the provisional trusted in-process authorization/Approval
lifecycle seam, and #30 supplies a provisional provider-facing instruction/data,
bypass-denial, safe-code, and host-effect-denial adapter over it. Semantic
revision/session mechanics belong to Issue #93. Durable authorization/audit
state, public wire codes, authentication/transport integrity, and actual
external-effect mechanisms remain Provisional or Deferred. Plugin isolation and
migration sandbox mechanics remain Deferred; Issues #134 and #135 own only their
narrower private-enterprise and public-ecosystem policies.

## Principles

Tachiko Work handles user documents, structured data, plugins, host effects,
and AI operations.

Security is enforced at trusted platform boundaries. Semantic validity,
authorization, exact Human Approval, and external-effect permission are
independent prerequisites; none substitutes for another.

## Areas

- default-deny semantic Grants over stable-ID document-local scope
- trusted relational derivation of operation-family/disclosure/write
  authorization footprints
- plugin isolation
- exact finite Human Approval for Delegated-origin or Delegated-authority
  semantic publication
- migration sandboxing
- untrusted file handling
- replay/revocation and provenance
- separate filesystem/network/process/Git/plugin/deployment authority
- future cross-boundary cryptographic integrity profiles

## Authorization Boundary

ADR-0026 and [`semantic-authorization.md`](semantic-authorization.md) define
the provider-neutral MVP contract:

- trusted Human and Delegated Principal classes in one authorization domain;
- independent Query, Propose, Execute, and Approve actions and operation
  families;
- Value, Formula, Structure, Schema, and Destructive mutation classes;
- stable-ID Document, Schema, SchemaField, Entity, and EntityField scopes;
- trusted `AuthorizationFootprint` derivation with complete
  operation-family/mutation-class/scope coverage combined with the requested
  action;
- explicit non-reusable Grants with per-use validity checks, authorizing
  Approve-reference rechecks, and fresh Execute coverage;
- structural exact binding to ADR-0024 `SemanticPatch` and
  `ExactChangeBinding`;
- finite, revocable Approval consumed atomically with at most one successful
  semantic publication; and
- minimum proposal/execution provenance outside canonical Document state.

The MVP does not select canonical approval bytes, a digest/hash/signature/MAC,
portable bearer token, public Rust/wire DTO, enterprise IAM, or generic policy
language.

Denials remain machine-distinguishable from semantic `ValidationReport` and
must not disclose semantic subjects outside Query authority.

## Legacy Import

External formats, document instructions, model output, plugin results, and
client-supplied identity/Grant/Approval claims are untrusted input.

Importers must validate data before entering the semantic core. No untrusted
content may select its Principal, declare authoritative scope, mint a Grant or
Approval, bypass the Semantic API, or authorize an external effect.

The current `ai-api` boundary makes this executable for provider-facing calls:
untrusted request evidence cannot carry effective Principal or trusted time;
typed proposal/execution requires the workspace lifecycle to prove an active
Delegated occurrence, so a Human session principal cannot become an AI
credential; raw semantic or storage mutation and durable-persistence/filesystem/
network/process/Git/plugin/deployment/credential requests return separate stable
denial codes. Concrete transports must preserve this boundary rather than
serialize trusted host context as a client credential.
