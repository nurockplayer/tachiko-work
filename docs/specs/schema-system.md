# Schema System Specification

Decision state: Mixed. Stable identity and reference-address statements are
Accepted under [ADR-0015](../decisions/ADR-0015-stable-semantic-identity.md).
The durable-schema-versus-runtime-policy boundary is Accepted under
[ADR-0019](../decisions/ADR-0019-staged-semantic-validation-and-diagnostics.md).
Richer future schema vocabulary remains Provisional or Deferred as marked.

See the [validation specification](validation-engine.md) and the
[canonical reconciliation register](../governance/canonical-reconciliation-register.md).

## Principle

Tachiko Work treats structured data as typed semantic objects rather than
unvalidated cells.

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
- `FieldType` with Number, Text, Boolean, and stable-schema Reference variants;
- field `required` declaration; and
- stable target `SchemaId` for reference field types.

Entity membership in a schema, typed field values, stable entity references,
and ADR-0018 bound formulas consume those declarations as semantic facts.

These current declarations are the concrete basis for schema/type/reference
validation. Stable identity and relationship semantics remain governed by
ADR-0015; formula meaning remains governed by ADR-0018.

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
- a generic `ValidationRule` DSL; and
- progressive/mixed typed and untyped regions (#13).

Future additions must state whether they are durable schema meaning, advisory
lint policy, domain validation, or host/external checks rather than collapsing
those categories together.

## Benefits

Typed schema declarations enable:

- deterministic validation;
- autocomplete and editor guidance;
- AI understanding over semantic facts;
- safer refactoring;
- dependency and impact analysis; and
- runtime/export integration.

These benefits do not require freezing a universal constraint language before
real use cases exist.

## Related

- ADR-0015
- ADR-0018
- ADR-0019
- `validation-engine.md`
- `diagnostics-contract.md`
- Issue #13
