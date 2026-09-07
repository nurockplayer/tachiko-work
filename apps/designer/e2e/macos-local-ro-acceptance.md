# Issue #345 — macOS Finder local `.ro` acceptance

This is a Steward-owned executable acceptance map. Live Accepted repository authority and Issue #345 outrank it.

## Evidence split

The semantic/storage path is not the missing feature. `runtime/tests/local_ro_launch_preflight.rs` must remain green for direct current `.ro` bytes and portable-package/v1 bytes. The merged #344 PWA acceptance remains evidence that the shared app-private local-document ingress preserves dirty-consent, invalid-input preservation, busy rejection, and Rust admission.

The missing production seam is the macOS desktop host:

1. a maintained Tauri 2 shell serves the existing `apps/designer/dist` rather than introducing another frontend;
2. `bundle.fileAssociations` owns exactly the custom `.ro` type, with an exported Apple type conforming to `public.data`;
3. macOS cold and warm file-open events deliver the exact selected document to the already-landed shared ingress;
4. a cold-start OS event may use one bounded startup latch until the Designer can consume it, but runtime busy opens are not queued and retain #344 visible-rejection semantics;
5. the shell owns no `.ro`/`.roproj` parser, formula engine, validation policy, semantic state, or second persistence model.

For this bounded Tauri 2 host, the native macOS file-open source is the application `RunEvent::Opened { urls }` path. A cold `Opened` event may be retained only as the one-shot startup latch needed to cross the frontend-ready boundary; after handoff it is consumed and cleared. Warm `Opened` events are delivered directly and do not become a retry queue. This host lifecycle is transport glue only and must not become another document/session authority.

`tests/macos-desktop-shell-acceptance.test.ts` is the Linux-safe production-seam oracle. It is not OS association evidence.

## Stage-0 commands

From repository root:

```sh
cargo test --manifest-path apps/designer/runtime/Cargo.toml --test local_ro_launch_preflight --locked
cd apps/designer
pnpm install --frozen-lockfile
pnpm run typecheck
pnpm exec vitest run tests/macos-desktop-shell-acceptance.test.ts
pnpm exec playwright test e2e/pwa-local-ro-launch.spec.ts
```

## Qualified pre-implementation result

- Rust local `.ro` preflight: GREEN.
- Designer typecheck and merged #344 PWA acceptance: GREEN.
- `macos-desktop-shell-acceptance.test.ts`: RED only because the production `src-tauri` host/configuration seam does not exist yet.

A missing toolchain, failed dependency install, absent browser build, or attempted Tauri build on a non-macOS runner is setup failure and does not qualify as behavioral RED.

## Final macOS evidence

Merge eligibility requires evidence from an actual locally built `.app`; browser-only or synthetic launchQueue evidence cannot substitute for it.

At the final exact head, record all of the following:

1. build the real macOS application bundle from the checked-in shell and identify its exact path;
2. inspect the built `Contents/Info.plist` and prove the emitted document/exported-type metadata owns `.ro` and conforms to `public.data`;
3. register/use the bundle through LaunchServices and prove a cold file-open launches the app with one valid current `.ro`;
4. with that app already running, open a second valid `.ro` and prove exactly one warm delivery;
5. show a semantic canary from the opened document through the existing Designer/Rust path (for the checked-in game-balance fixture: source title/current revision and calculated Iron Sword DPS `40` are acceptable canaries);
6. demonstrate dirty replacement follows the existing explicit decision, and corrupt/unsupported input preserves the valid current occurrence with a visible rejection;
7. rerun the Web/PWA Designer gates and required repository exact-head gates.

`open`, `open -b`, or similar LaunchServices commands are useful executable OS-boundary evidence, but they are not to be described as a literal Finder mouse double-click. If final proof uses such commands, report them truthfully and separately record the strongest actual Finder observation available.

Signing, notarization, DMG/App Store distribution, auto-update, package-v2, and Windows/Linux parity are outside this acceptance unless live higher authority changes the scope.
