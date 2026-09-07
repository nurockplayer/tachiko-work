# Issue #344 — installed-PWA local `.ro` launch acceptance

This file is a Steward-owned executable acceptance map. Live repository Accepted authority and Issue #344 outrank it.

## Evidence split

The production storage/Designer capability is a prerequisite, not the missing feature. `runtime/tests/local_ro_launch_preflight.rs` must stay green for both the current direct `.ro` fixture and a portable-package/v1 artifact generated from the same admitted document. That test proves current Rust storage framing plus the bounded Designer profile already accept the source.

`pwa-local-ro-launch.spec.ts` owns the browser/distribution seam:

1. shipped page links a PWA manifest with exactly one `.ro` file handler;
2. an available `launchQueue` receives one registered consumer;
3. current raw `.ro` bytes open through normal Rust admission and show the source title/current calculated DPS;
4. dirty warm-open consent is honored;
5. corrupt, unreadable, invalid and multi-file launches are visible failures that preserve current work;
6. an empty `LaunchParams.files` delivery is an ordinary no-file launch and is a silent no-op that preserves current work;
7. ordinary Web startup remains healthy without `launchQueue`.

`tests/local-document-busy-acceptance.test.ts` owns the concurrent warm-launch edge: while another Designer operation is still pending, a distinct valid local-file launch must fail visibly and non-destructively, must not call Rust local-document admission, and must leave the active operation/current occurrence intact. v0.1 does not queue the launch; a later distinct OS launch may retry after Designer becomes idle.

`tests/pwa-file-launch.test.ts` keeps the host-level no-file and duplicate-delivery rules deterministic without treating frontend code as storage authority.

The injected `launchQueue`/file handles are deterministic browser-boundary acceptance, not proof that an operating system registered the installed PWA. Final delivery must separately report the strongest real installed desktop-Chromium file-association smoke available and must not claim Safari/Firefox OS association from these tests.

## Stage-0 / affected commands

From repository root:

```sh
cargo test --manifest-path apps/designer/runtime/Cargo.toml --test local_ro_launch_preflight --locked
cd apps/designer
pnpm install --frozen-lockfile
pnpm run typecheck
pnpm test -- tests/pwa-file-launch.test.ts tests/local-document-busy-acceptance.test.ts
pnpm exec playwright test e2e/pwa-local-ro-launch.spec.ts
```

Before final delivery also run the directly affected Designer/repository gates required by live repository policy.

## Reconciled expected result at implementation checkpoint

- Rust preflight remains GREEN.
- Manifest/file handling, valid `.ro` admission, dirty consent and existing Web fallback remain GREEN.
- Empty launch is RED until production ignores it before local-document ingress.
- Busy warm-launch acceptance is RED until production surfaces a visible non-destructive rejection instead of silently returning.

A compile/install/fixture/runtime failure is not qualified behavioral RED. Do not weaken the acceptance to convert setup failure into progress.
