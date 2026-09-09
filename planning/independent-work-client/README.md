# Tachiko Work — independent client seed

Proposed repository: `nurockplayer/tachiko-work-client`.
This is a transferable planning/acceptance seed, **not a working application**.

- [Product, UX, architecture and delivery specification](docs/SPEC.md)
- [Executable acceptance, wiring contract and evidence limits](docs/ACCEPTANCE.md)
- [Observed upstream authority and sources](docs/SOURCES.md)

Run the dependency-free harness checks with `node --test tests/harness.test.mjs`.
Materialize the test-only canonical project with `node scripts/materialize-fixture.mjs`.
Real browser/runtime/agent acceptance has explicit prerequisites; missing them is
`BLOCKED` (exit 78), never a passing or qualified behavioral RED result.

No existing frontend source, runtime binary, fonts, secrets, or user documents are
included. Fixture identities/shape derive from upstream dogfood at the pinned
commit; the new release-planning content and notes field are deliberate changes.
The fixture generator is a test utility, not a production `.roproj` codec.
The source project retains its own licensing and contribution authority; this
seed does not activate new contribution terms or promise a stable SDK.

Live Epic: https://github.com/nurockplayer/tachiko-work/issues/357

Start with C0 #358; see [implementation queue](docs/QUEUE.md). The new remote has
not been created by this seed. The upstream planning branch is a staging location,
not the production frontend repository.
