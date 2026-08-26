# Threat Model

Decision state: Accepted security direction under ADR-0007 and ADR-0026.
Concrete instruction/data labeling, bypass enforcement, security diagnostics,
and host-effect enforcement remain implementation work under Issue #30.

## Security Philosophy

Tachiko Work handles documents, structured data, computation, plugins, and AI
operations.

Security must protect:

- user data;
- document integrity;
- execution safety;
- authorization and approval integrity; and
- supply chain trust.

## Untrusted Inputs

The following are untrusted data unless a narrower trusted boundary explicitly
proves otherwise:

- imported Office or other external files;
- document text, comments, labels, formulas, and metadata;
- external plugin and connector results;
- AI-generated operations, explanations, capability claims, approval claims,
  and validation claims;
- network collaboration events; and
- client-supplied proposal, approval, principal, grant, or provenance records.

Untrusted data may influence a typed proposal or review evidence. It does not
become platform instruction, principal identity, privilege, approval, or
semantic truth merely because a model emitted it in structured form.

## Security Boundaries

External input:

```text
Import / Plugin / Agent / Network
        |
        v
Admission + instruction/data boundary
        |
        v
Typed Semantic API request / immutable SemanticPatch
        |
        v
Deterministic validation and operation gate
        |
        v
Trusted authorization + exact Human approval boundary
        |
        v
Authorized semantic publication
```

Host effects remain beside, not inside, the semantic authorization domain:

```text
semantic publication
    != durable persistence
    != filesystem/network/process/Git/plugin/deployment authority
```

## Authorization and Approval Threats

The platform must prevent at least:

- a model, document, plugin, or request body selecting or upgrading its own
  PrincipalId;
- Query or Propose authority silently becoming Execute authority;
- a broad `editor` or `write:*` grant silently including Formula, Schema,
  Destructive, or external effects;
- validation success being treated as authorization;
- authorization or approval overriding a failed semantic gate;
- approval of rendered prose or a diff being reused for a changed proposal;
- approval surviving exact base change, grant revocation, principal disablement,
  expiry, explicit revocation, or consumption;
- replay or concurrent double use of one ApprovalId;
- provider/model identity becoming privilege;
- Machine self-approval satisfying the Human approval requirement;
- raw semantic-core, storage, `.roproj`, filesystem, or host mutation bypassing
  the authorized Execute path; and
- ordinary semantic authority granting network, process, Git, plugin,
  deployment, credential, or persistence effects.

## Required Security Laws

- AI and automation have no intrinsic authority.
- Effective principal identity comes from a trusted host/session boundary.
- Authorization is default-deny and based on explicit immutable grants.
- Query, Propose, Approve, and Execute are independent actions.
- Data, Formula, Schema, and Destructive mutation authority are independently
  grantable and additive.
- Every Machine Execute in the current MVP requires exact approval from a
  distinct Human principal.
- Approval binds one proposal occurrence, complete ADR-0024
  `ExactChangeBinding`, exact base, exact executor, document, mutation classes,
  approval profile, and the exact grants relied upon.
- Approval has finite lifetime, is revocable, and is single-use.
- Stale base fails closed before candidate construction against the changed
  base and never performs implicit rebase or replay.
- Deterministic validation and operation gates remain authoritative.
- Security-relevant denials are machine-readable and distinguish authorization,
  approval, stale, semantic-gate, representation, and host-effect failures.
- Provenance records actor, agent/provider/model evidence, grants, approval,
  gate outcome, base/result revisions, and execution result without becoming
  canonical semantic state or privilege.
- Unverifiable principal, grant, approval, time, registry, or integrity state
  fails closed.

## Effect Separation

ADR-0007, ADR-0022, and ADR-0026 keep these authority domains separate:

- semantic Query/Propose/Approve/Execute;
- durable storage/materialization;
- filesystem and browser persistence;
- network access;
- process and shell execution;
- Git operations;
- plugin/connector execution;
- deployment/publication; and
- credentials/secrets access.

An authorization in one domain does not imply another. A host may materialize
or externally publish an already-authorized semantic result only under separate
host authority.

## Core Principles

- Never execute unvalidated transformations.
- Never treat semantic validity as authorization.
- Never treat model/provider claims as privilege or approval.
- Preserve user ownership and document integrity.
- Make security behavior observable and machine-readable.
- Prefer explicit, scoped, finite, revocable permissions.
- Fail closed on stale, modified, expired, revoked, consumed, or unverifiable
  approval.
- Keep semantic and host/external effects explicitly separated.
