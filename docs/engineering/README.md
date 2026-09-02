# Engineering guides

This directory contains implementation guidance and executable-workflow notes.
These documents explain how to work with current repository behavior. They do
not independently establish product, architecture, format, API, or compatibility
authority.

When an engineering guide conflicts with the Product Constitution, an Accepted
ADR or policy, or an applicable normative specification, the higher-authority
source wins. See [`docs/README.md`](../README.md) for the repository knowledge
hierarchy.

## Frontend integration

Start here when building a Tachiko UI outside the main repository:

1. **[Build an experimental Tachiko frontend](frontend-integration-guide.md)**
   - human-first onboarding for the Issue #231 external-client pilot;
   - explains the job, authority boundary, first table, edit flow, current limits,
     and feedback format;
   - does not require reading Rust internals or ADRs before starting.
2. **[Experimental Designer client kit: first contact](experimental-designer-client-kit.md)**
   - exact export command, generated kit shape, Product Gap walkthrough, and
     executable smoke evidence;
   - use this while wiring the current browser Worker/WASM kit.
3. **[`examples/experimental-designer-client/`](../../examples/experimental-designer-client/)**
   - smallest external-style consumer that imports only the generated kit.

The current client kit is deliberately experimental. It is not an npm package,
stable SDK, stable wire protocol, or compatibility promise.

## Historical implementation planning

- [MVP implementation plan](mvp-implementation-plan.md)
- [Prototype roadmap](prototype-roadmap.md)

These older planning notes preserve the early implementation shape. Use the
current [Product Roadmap](../product/product-roadmap.md), live GitHub Issues, and
Accepted authority for present sequencing and contracts.
