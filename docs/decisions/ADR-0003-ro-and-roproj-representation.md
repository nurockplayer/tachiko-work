# ADR-0003: Source project and portable artifact representations

Status: Accepted

## Context

Tachiko Work needs two different operational properties:

- an editable, inspectable representation that works naturally with Git, branches, diffs, merge, CI, CLI tools, and AI agents;
- a portable single-file artifact that ordinary users can share, download, archive, and open without understanding repository layout.

Trying to make one physical representation optimize for both concerns would either weaken Git-native workflows or make ordinary file handling unnecessarily developer-oriented.

Tachiko Work also aims to minimize invention. Existing standards and mature primitives should be reused wherever they fit without distorting the semantic model.

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
- semantic-core independence from physical storage layout.

## Consequences

- Git normally tracks `.roproj`, not generated `.ro` artifacts.
- `.ro` may be produced by local tooling, CI, releases, or consumer save workflows.
- A low-cost `.ro` implementation may be scheduled early even though `.ro` is not an architectural MVP dependency.
- The exact `.ro` container, compression, manifest details, directory splitting rules, and other physical layout details remain evolvable implementation/specification decisions.
- ODF and other existing standards remain valuable interoperability formats rather than becoming the internal semantic source of truth.
- The project should prefer composition of established standards over format invention whenever possible.

## Principle

Git is storage infrastructure, not the user interface.

The semantic model is the truth, `.roproj` is its canonical editable materialization, and `.ro` is a portable artifact.
