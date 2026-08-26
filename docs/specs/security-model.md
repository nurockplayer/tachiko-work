# Security Model

Decision state: Accepted security direction under ADR-0007 and ADR-0026.
Concrete instruction/data enforcement, authorization state, lifecycle,
diagnostic codes, and external-effect enforcement remain #29/#30/#93 work.

## Principles

Tachiko Work handles user documents, structured data, plugins, host effects,
and AI operations.

Security is enforced at trusted platform boundaries. Semantic validity,
authorization, exact Human Approval, and external-effect permission are
independent prerequisites; none substitutes for another.

## Areas

- default-deny semantic Grants over stable-ID document-local scope
- trusted derivation of disclosure/write authorization footprints
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
- independent Query, Propose, Execute, and Approve capabilities;
- Value, Formula, Structure, Schema, and Destructive mutation classes;
- stable-ID Document, Schema, SchemaField, Entity, and EntityField scopes;
- trusted `AuthorizationFootprint` derivation;
- explicit Grants with live request/execute rechecks;
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
