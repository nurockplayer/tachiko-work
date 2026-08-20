# ADR-0003: Source project and portable artifact representations

Status: Accepted

## Context

Tachiko Work needs two different operational properties:

- an editable, inspectable representation that works naturally with Git, branches, diffs, merge, CI, CLI tools, and AI agents;
- a portable single-file artifact that ordinary users can share, download, archive, and open without understanding repository layout.

Trying to make one physical representation optimize for both concerns would either weaken Git-native workflows or make ordinary file handling unnecessarily developer-oriented.

Tachiko Work also aims to minimize invention. Existing standards and mature primitives should be reused wherever they fit without distorting the semantic model.

The current `.ro` and `.roproj` names originated as early working names. Their architectural roles are accepted by this ADR, but the names themselves have not been selected or validated as permanent public-standard names.

## Decision

The semantic model remains the architectural source of truth.

Tachiko Work defines a canonical editable project representation:

```
project.roproj/
```

`.roproj` is the normal working and version-control representation. It is optimized for:

- deterministic serialization;
- human-readable changes where practical;
- Git diff and merge;
- branch-based workflows;
- CI validation;
- CLI and third-party tooling;
- AI and semantic operations.

Tachiko Work may also produce a portable packaged artifact:

```
project.ro
```

`.ro` is produced from the canonical project representation and is optimized for:

- sharing;
- downloads;
- archival;
- ordinary file handling;
- consumer-facing open/save workflows.

`.ro` is not an independent source of truth that must be continuously synchronized with `.roproj`.

Conceptually:

```
Semantic Model
      |
      v
Canonical Project Representation
    .roproj/
      |
      | validate / canonicalize / pack
      v
Portable Artifact
      .ro
```

## Naming status

`.ro` and `.roproj` are provisional working extensions.

They MAY be used throughout MVP development, prototypes, internal dogfooding, and experimental releases. Their continued use during this phase is a convenience, not a compatibility promise or permanent naming decision.

The permanent public format name and extension MUST be decided before the first stable public format release and before Tachiko Work makes long-lived ecosystem commitments around the extension, including stable third-party SDKs, compatibility guarantees, MIME-type registration, or broad external integration contracts.

A future rename of `.ro` or `.roproj` SHOULD be representational only. It MUST NOT require changing the semantic model or document meaning.

Core semantic types and APIs MUST NOT depend on the provisional names. Names such as `Document`, `SemanticDocument`, `DocumentGraph`, `Node`, `PackageSerializer`, and `ProjectSerializer` are preferred for core concepts. `ro`-specific naming SHOULD be confined to format adapters, CLI/file-extension boundaries, and other representation-specific code.

The implementation SHOULD isolate extension strings and format labels so that replacing the working names does not require invasive changes.

## Packaging requirements

The portable artifact MUST preserve enough information to reconstruct an editable project with equivalent semantic meaning.

Packaging MAY perform:

- canonical ordering;
- compression;
- indexing;
- integrity generation;
- other reversible packaging optimizations.

Packaging MUST NOT change document meaning.

A deleted or corrupted `.ro` artifact can be regenerated from its source `.roproj` project.

For the initial implementation, `.ro` SHOULD remain a thin, lossless package over the canonical project representation using mature standard primitives rather than introducing custom encodings unnecessarily.

## Interoperability principle

`.ro` MUST NOT become an MVP blocker.

This does not imply that `.ro` must be deferred or treated as low priority. If a thin deterministic pack/unpack implementation is inexpensive, it MAY be implemented early. The constraint is that packaging sophistication must not delay proof of the semantic core, `.roproj`, validation, formula, diff, or CLI workflows.

Early product value should be proven through the semantic core and `.roproj` workflow. Existing open formats such as OpenDocument, together with CSV, JSON, Markdown, and other appropriate standards, should be supported as first-class interoperability boundaries where useful.

Tachiko Work should introduce custom format semantics only when existing standards cannot represent required Tachiko semantics without distorting the core model.

## Requirements

- canonical ordering;
- UTF-8 where textual representation is used;
- stable identifiers;
- deterministic canonical serialization;
- Git-friendly `.roproj` materialization;
- lossless semantic pack/unpack;
- semantic-core independence from physical storage layout;
- semantic-core independence from provisional extension names.

## Consequences

- Git normally tracks `.roproj`, not generated `.ro` artifacts.
- `.ro` may be produced by local tooling, CI, releases, or consumer save workflows.
- A low-cost `.ro` implementation may be scheduled early even though `.ro` is not an architectural MVP dependency.
- `.ro` and `.roproj` can continue to be used as working names without blocking implementation.
- The permanent format name remains intentionally open until the pre-stable naming freeze.
- Renaming the working extensions must remain cheap and must not become a semantic migration.
- The exact `.ro` container, compression, manifest details, directory splitting rules, and other physical layout details remain evolvable implementation/specification decisions.
- ODF and other existing standards remain valuable interoperability formats rather than becoming the internal semantic source of truth.
- The project should prefer composition of established standards over format invention whenever possible.

## Principle

Git is storage infrastructure, not the user interface.

The semantic model is the truth, `.roproj` is its canonical editable materialization, and `.ro` is a portable artifact. The current extension names are provisional until the first stable public format release.
