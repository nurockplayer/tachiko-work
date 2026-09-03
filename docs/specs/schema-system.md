# Schema System Specification

Decision state: Mixed. Stable identity and reference-address statements are
Accepted under [ADR-0015](../decisions/ADR-0015-stable-semantic-identity.md).
The durable-schema-versus-runtime-policy boundary is Accepted under
[ADR-0019](../decisions/ADR-0019-staged-semantic-validation-and-diagnostics.md).
[ADR-0021](../decisions/ADR-0021-progressive-semantic-strengthening.md) accepts
progressive semantic strengthening and mixed-strength content without weakening
the current strongly typed Entity contract. Richer future schema vocabulary
remains Provisional or Deferred as marked.

See the [validation specification](validation-engine.md) and the
[canonical reconciliation register](../governance/canonical-reconciliation-register.md).

## Principle

Tachiko Work treats structured data as typed semantic objects rather than
unvalidated cells.

Semantic-first does not require every semantic kind to declare a domain schema
at creation. A schema is required when a semantic kind claims schema-instance
semantics. Once such a declaration exists, the durable schema rules in this
specification apply unchanged.

A schema declaration defines durable semantic meaning only where conforming
clients must interpret the same semantic snapshot consistently. Validation
execution, scheduling, presentation, and workflow policy do not become schema
meaning merely because they inspect that declaration.

## Current Milestone 02 declaration surface

The implemented semantic schema model currently includes:

- opaque stable `SchemaId` identity;
- mutable human-facing `SchemaKey`;
- stable `FieldId` identity;
- mutable human-facing `FieldKey`;
- `FieldType` with Number, Text, Boolean, Date, and stable-schema Reference variants;
- field `required` declaration; and
- stable target `SchemaId` for reference field types.

Entity membership in a schema, typed field values, stable entity references,
and ADR-0018 bound formulas consume those declarations as semantic facts.

`Date` is a date-only civil Gregorian semantic value with no time or timezone;
its canonical textual form is `YYYY-MM-DD`. This declaration does not add
DateTime, Duration, date arithmetic, or generalized temporal semantics.

The current `Entity` model remains schema-required. ADR-0021 does not change it
to `Option<SchemaId>`, a dynamic property bag, or a universal weak mode. Future
freeform/simple-table semantic kinds may be additive and may have weaker
declared meaning until explicitly strengthened.

These current declarations are the concrete basis for schema/type/reference
validation. Stable identity and relationship semantics remain governed by
ADR-0015; formula meaning remains governed by ADR-0018.

## Progressive semantic strengthening

ADR-0021 accepts explicit strengthening from weaker semantic content toward
stronger schema/type/reference/computation contracts.

Inference may propose candidate schemas, field types, relationships, mappings,
or normalization, but inferred structure is advisory evidence rather than
durable schema meaning. A new schema/type declaration becomes authoritative
only through an explicitly accepted semantic transition under ADR-0020.

A strengthening transition must not silently coerce source content into a
stronger claim. Conversion must distinguish conceptually between:

- exact/lossless mappings;
- explicitly accepted lossy/coercive mappings; and
- unresolved/ambiguous source content.

The exact promotion command catalogue, mapping DTO, source-selector mechanics,
and schema-migration protocol remain Deferred.

If a semantic object already has stable identity, adding stronger schema meaning
must preserve that identity. ADR-0021 does not require universal identity for
future rows/cells/blocks that have not become first-class independently
addressable semantic objects.

## Durable schema semantics versus validation policy

A candidate declaration belongs in durable semantic schema meaning when:

1. different conforming clients/runtimes must agree that it changes the meaning
   or validity of the same semantic snapshot;
2. persistence, migration, or semantic consumers need the declaration to
   understand the data correctly; and
3. it can be evaluated from deterministic semantic inputs rather than implicit
   host state such as clock, filesystem, network, locale, environment, or user
   presentation preference.

Runtime validation policy instead includes concerns such as:

- when validation runs;
- editor debounce or scheduling;
- full versus incremental execution strategy;
- cache/index implementation;
- UI grouping/highlighting;
- human message localization;
- whether a CI profile treats advisory findings as blocking; and
- LSP/SARIF/transport rendering.

Those concerns must not be serialized as schema semantics by accident.

## References

References are semantic relationships, not untyped strings.

The Milestone 02 model separates opaque stable `SchemaId` and `FieldId`
identities from mutable `SchemaKey` and `FieldKey` authoring addresses.
Reference field types store the target stable `SchemaId`; human keys are
resolved through deterministic derived indexes and may be renamed without
retargeting existing relationships.

A missing referenced object is diagnosable through stable semantic identity.
A new object that happens to reuse an old human key does not become the old
relationship target.

Under ADR-0021, weak/freeform content may coexist with typed collections, but a
durable typed reference cannot target a weak fragment through label, coordinate,
display order, storage path, or guessed identity. The target must first satisfy
the stable identity and declared semantic contract required by the relationship.

## Schema evolution

Changing established schema meaning is an explicit semantic migration rather
than a presentation toggle.

When an evolution affects existing semantic claims, it must preserve established
identity and make ambiguity, loss/coercion, relationship effects, and affected
computation reviewable before semantic publication. It must not silently
reinterpret persisted values at read time.

Storage-format migration remains a separate ADR-0017 concern. This
specification does not define a general schema-migration engine.

## Future schema vocabulary

Earlier exploratory documentation used broad terms such as `constraints`,
`allowed values`, `computed fields`, and `validation rules`. Those terms do not
constitute an Accepted generic rule language.

The following remain future schema work until concrete product/domain evidence
justifies them:

- enum/allowed-value semantics;
- numeric ranges;
- string patterns;
- defaults;
- nominal type hierarchies;
- arbitrary declarative validation rules;
- a generic `ValidationRule` DSL;
- concrete freeform/simple-table semantic kinds;
- promotion/mapping command vocabulary; and
- detailed mixed-content identity/persistence mechanics.

Future additions must state whether they are durable schema meaning, advisory
lint/inference, domain validation, or host/external checks rather than collapsing
those categories together.

## Benefits

Typed schema declarations enable:

- deterministic validation;
- autocomplete and editor guidance;
- AI understanding over semantic facts;
- safer refactoring;
- dependency and impact analysis; and
- runtime/export integration.

ADR-0021 allows users to defer these stronger guarantees until they provide
practical value without weakening the guarantees once declared.

These benefits do not require freezing a universal constraint language, dynamic
`AnyValue` substrate, or schema-inference engine before real use cases exist.

## Related

- ADR-0015
- ADR-0018
- ADR-0019
- ADR-0020
- ADR-0021
- `validation-engine.md`
- `diagnostics-contract.md`
- Issue #13
