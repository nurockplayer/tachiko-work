# Issue #342 Cloudflare acceptance

Baseline: `main@5061216f9d73757b545e1e044ea59876f663ab1c` after #229 merged.

This is Steward-owned acceptance preparation only. It does not authorize production Cloudflare code or a live deployment.

## Local/preview acceptance

The eventual Worker delivery must prove, against deterministic fixtures:

- the Worker returns the same #229 projection/currentness behavior rather than a second Dashboard model;
- GitHub credentials remain server-side and never appear in browser assets, HTML, API responses, diagnostics, or source maps;
- browser traffic never calls GitHub directly;
- upstream GitHub traffic remains REST GET plus the already-bounded GraphQL query profile, with no mutation/control operation;
- one Worker deployment unit serves the Dashboard application and static assets;
- mutation/control routes remain unavailable and cross-origin API reads fail closed;
- failed refresh clears affected current facts rather than retaining stale values;
- the deployment configuration introduces no secret-bearing durable cache/store.

The prepared seed is qualified only when current Dashboard typecheck/acceptance/unit/browser gates remain green and the new Cloudflare acceptance fails solely because the Worker/Wrangler adapter is absent.

## External activation evidence

These checks cannot be claimed from repository-local fixtures. Before Issue #342 closes, record actual Cloudflare evidence for:

- Cloudflare Access blocks or redirects an unauthenticated request before the Dashboard is usable;
- an owner-approved authenticated identity can load both UI and API;
- Access covers the production hostname and every enabled preview or `workers.dev` route used for acceptance;
- the GitHub credential is configured as a Worker Secret/server-side binding, not committed configuration;
- a live authenticated smoke records deployed source revision, Worker identity/hostname, successful read-only GitHub observation, and no browser-visible credential;
- rollback/removal steps are recorded without exposing secrets.

Actual Cloudflare account, Access-policy, secret, hostname, or deployment mutation remains Stage B and requires explicit maintainer authorization.