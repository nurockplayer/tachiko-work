# #229 operational acceptance handoff

Steward-owned expansion on existing PR #322. Baseline: `20fb5cc8d44ebb8cefc21419d2e0fea9965b0ae6`; checked main: `3887612f65cc788efa0c23417e7ed16c8289dbb0`.

This supplements, not replaces, the accepted projection tests. It does not implement a Dashboard, change `projectObservation`, weaken an existing assertion, waive runtime/UI/security acceptance, or authorize merge. Owning authority remains live #229/#274/#168 and repository delivery policy. The original 48 acceptance + 4 unit tests must remain green.

## Application seam and finite transport profile

`src/server/application.ts` declares two integration entry points. `observeGitHub(options)` observes native GitHub records; `startDashboard(options)` starts the **real** loopback HTTP application and serves its real browser UI. Both currently throw the named deliberate stub. The production CLI must call the same application path, not a fixture-specific replacement. `options.fetch` and the trust allowlist are server-only injection points. No browser route/config/query parameter may replace the source, credential, or trust configuration.

The bounded provisional v0 transport profile uses GitHub REST GET for repository/default-branch ref, open Issues, open PRs, per-Issue `dependencies/blocked_by`, PR-scoped top-level Issue comments, and a bounded updated-order closed-PR window filtered to merged records. Native PR linkage uses a GraphQL **query** of `closingIssuesReferences` with number, repository identity, and pageInfo. Its variables are `owner`, `name`, `number` (the actual PR, never a hard-coded first PR), and optional `after`; its response has the unaliased `repository.pullRequest.closingIssuesReferences` shape. HTTP POST of this read-only GraphQL query is allowed; GraphQL mutations are not.

The fixture uses real external payload field names, captures actual outgoing requests and credentials, and checks the requested linkage identity/completeness selection. It is a finite replay double, **not** a full GitHub GraphQL schema/server. A documented live read-only smoke remains required before merge; fixture success does not certify real API availability. Mechanical transport adjustments may be challenged against GitHub's documented API without relaxing observation outcomes.

Discovery must inspect pagination. It may complete the connection or stop with `partial`, but must not call a truncated retained set complete. A failed subsequent page leaves the already observed connection partial, not complete-empty. The fixture supplies an inaccessible second page. For PR-scoped positive watch evidence, retain independently observed PR-head/watch candidates under surrounding partial discovery as already required by #229. For bounded recent activity, `complete` describes the declared requested window, not all repository history.

Single-repository number-only linkage cannot represent foreign-repository targets: fail the linkage family closed as unavailable rather than binding the same number to a local Issue or pretending the known foreign link is complete-empty. Keep the PR source locator. This is a bounded adapter guard, not a multi-repository feature.

Trusted metadata comes from GitHub and a server-side producer allowlist, never comment prose. A canonical watch can have been edited in place. Parse only the strict watch header; malformed, quoted, or untrusted headers cannot grant attention. Reuse #200 semantics where applicable; do not invent a sole-Issue anchor to satisfy an older helper's API, or revive a handoff parser in current truth.

## Observable HTTP/UI contract

- `GET /` serves the real HTML/client. `GET /api/projection` serves the declared credential-free `DashboardEnvelope`, with `Cache-Control: no-store` and no permissive CORS.
- Fixed provenance fields are source URLs/availability, not a second fact registry. Current source URLs remain in the configured GitHub repository; known current main links to its exact commit. Failed source metadata must not claim success.
- Every successful API refresh observes the latest raw-source fixture; no previous-success fallback. An upstream family failure clears that family and dependent attention but leaves independent facts current. If the **browser-to-server request** fails, all affected current UI content becomes Unknown; later success restores only the latest response.
- Mutating methods on `/api/projection` return 405. Nonexistent control endpoints return 404. Host spoofing and cross-origin reads return 403; binding to loopback alone is not sufficient to establish the browser trust boundary. Secret/source/traversal paths must not be served.
- Accessible section headings: Executive strip; Delivery command center; Current work; Recent activity; Authority & attention. Refresh is an ordinary keyboard-operable button named `Refresh`.
- Stable test locators: `main-sha`, `deliveries`, `current-work`, `recent-activity`, `attention`; delivery cards show full PR HEAD/base SHA text and use `delivery-<issueNumber|none>-<prNumber|none>`. These identify rendered facts, not CSS classes or private implementation helpers.
- Unknown/partial has readable text. An empty attention list must never turn into a clean/healthy/merge-ready/no-action claim. Current-work output is the accepted graph/node projection, not a longest-path or scheduling engine.
- Source links are usable; raw titles render as text, not executable markup. Credentials may reach only the configured server-side upstream. Browser traffic, bodies, DOM, and storage must not contain the injected secret. The browser may not contact GitHub or an external CDN directly.
- At 390px, long unbroken titles/full SHAs do not overflow the document. Reduced motion suppresses running decorative animations. Refresh preserves keyboard focus. No mutation/agent controls.

## Criterion -> executable evidence

| Criterion | Existing/new evidence |
| --- | --- |
| Raw source, default branch, native linkage, REST PR filtering, credentials exercised | `runtime.test.ts`: raw observation happy path; harness self-checks |
| Nested family failure, truncated discovery, known-unlinked/partial/unavailable/foreign linkage | `runtime.test.ts`: named adapter cases; original projection matrix remains binding |
| PR-scoped trusted/edited watch, malformed/untrusted comments, multi-Issue independence | `runtime.test.ts`: structured-attention cases |
| Real HTTP, source provenance, loopback, secret/error clearing, recovery | `runtime.test.ts`: HTTP/application group |
| No mutation, cross-origin/Host protection, no secret/static traversal | `runtime.test.ts`: HTTP boundary cases |
| Actual five surfaces and source links; real refresh and focus | `e2e/operational.spec.ts`: first two browser journeys |
| Unknown/partial; browser request failure/recovery; positive-only attention | `e2e/operational.spec.ts`: middle three journeys |
| 390px/reduced motion; injected credential/XSS/traffic/storage | `e2e/operational.spec.ts`: final two journeys |

Browser happy path and upstream partial paths DO NOT mock `/api/projection` or inject a precomputed DashboardProjection. Only the explicit browser-transport-failure case aborts that request. Both browser and HTTP tests start the application on an ephemeral loopback port and close it in teardown. No real credentials or GitHub writes are needed for deterministic tests.

## Reproducible validation and promotion

Use Node >=24 and pnpm 11.25.0. Browser development dependencies use the already-selected Designer versions, but remain package-local; no workspace coupling or package-manager migration.

```sh
pnpm --dir tools/project-dashboard install --frozen-lockfile
pnpm --dir tools/project-dashboard exec playwright install --with-deps chromium
pnpm --dir tools/project-dashboard typecheck
pnpm --dir tools/project-dashboard test:acceptance
pnpm --dir tools/project-dashboard exec vitest run tests/unit tests/harness
pnpm --dir tools/project-dashboard test:operational
pnpm --dir tools/project-dashboard test:browser
pnpm --dir tools/project-dashboard test
```

Missing-seam preparation evidence is explicitly bounded: new application tests should reach `OPERATIONAL_STUB`, while typecheck, tool/browser setup, fixture self-checks and the old 52 tests pass. This is executable missing-seam RED, **not** proof of a production behavioral regression or a functioning UI. Do not skip tests, use expected-failure annotations, or turn stub failures into a green shipping gate.

The lock was generated by the supported hosted preflight (run `34067948752`, artifact `9999549923`) and imported byte-for-byte: Git blob `ec0153ae31a94258075874f1a4111f3700a17318`. The temporary preparation workflow is removed. The normal Dashboard workflow uses frozen install and executes projection acceptance, unit/fixture checks, the full Vitest suite, and browser journeys; no non-frozen substitute or expected-failure exemption remains. A browser step still executes after an earlier behavioral failure so both classes leave evidence, but any failure keeps the job RED.

Before production promotion, record the seed's exact head/main, actual supported-runtime results, an independent acceptance review and any accepted corrections in #229. Until then, the delivery agent is authorized only for explicitly named tests/harness preflight. After promotion, implementation + unit tests continue on the same PR, and final production gates must be genuinely green.

Before merge, delivery additionally supplies package build/start instructions, a documented live read-only GitHub smoke (with truthful no-credential limitations), source/license notices for any visual reuse, full required repository validation and fresh final-head independent review. No Dashboard completion claim may use Designer's browser results as a substitute.

Acceptance review corrections: distinct PRs in both discovery orders are checked independently; all four required watch header fields are tested missing/duplicated, with unknown-field and invalid-value rejections; browser failure clears current-work as well as all other surfaces; narrow-viewport cards expose complete PR HEAD/base; raw recent-activity requests must explicitly select `sort=updated&direction=desc`. These strengthen the same bounded contract.
