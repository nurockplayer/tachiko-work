# Private Dashboard Worker

Issue [#342](https://github.com/nurockplayer/tachiko-work/issues/342) separates
repository support (Stage A) from explicitly authorized activation (Stage B).
Repository tests and merge do not prove or authorize a live private deployment.

## Stage A: local validation

From `tools/project-dashboard`, using the pinned pnpm and Node version:

```sh
pnpm install --frozen-lockfile --engine-strict
pnpm typecheck
pnpm test
pnpm test:browser
pnpm test:worker
pnpm dev:worker
```

`check:worker` bundles locally with `wrangler deploy --dry-run`; it does not
upload, create a Worker, configure Access, or provision secrets. `dev:worker`
binds to loopback and uses local Workers emulation. Do not use `--remote` for
Stage A. Without a GitHub binding the API fails closed. For an explicitly
requested local live observation, supply a read-only repository credential in
an ignored `.dev.vars` file; never paste it into commands, evidence, fixtures,
client configuration, or Git. Deterministic tests use canaries, not credentials.

The Worker reuses the local Dashboard observer, projection and HTML. The asset
build copies only that shared HTML into `dist/assets`; server code and Worker
bundles stay outside that directory. Every route runs the Worker first. API
responses are observations with `no-store`; no durable storage or cache authority
is configured. The local CLI remains available through `pnpm start`.

Checked-in configuration disables `workers.dev`, preview URLs and routes. It
contains no account/domain identity, GitHub credential, or Access allow policy.
Cloudflare Access is an external prerequisite, not a conclusion drawn from a
request header. With Static Assets, Cloudflare's router does not forward
`ctx.access` to user Worker code; do not use its absence as a substitute for
verifying the actual Access application.

## Stage B: maintainer-owned activation runbook

Do not execute this section without explicit activation authorization and an
available account/Zero Trust configuration. Re-read live #342 before activation.
Record the authorized Worker identity and allowed user identity/policy first.

1. Validate the exact reviewed source revision with the Stage A commands. Record
   its full Git SHA and the resulting local bundle hash. Start with all public
   ingress disabled as in the checked-in configuration.
2. Deploy only `tachiko-work-dashboard` into the approved account while ingress
   remains disabled (`pnpm exec wrangler deploy`). Do not configure a GitHub
   secret or turn on a hostname until the Worker has an owner-approved Access
   policy for **all traffic**, not previews only. Use Worker-level Access so UI,
   API, production and preview routes share protection; do not alter unrelated
   account-wide policy or create a bypass.
3. Configure the read-only credential using the interactive
   `pnpm exec wrangler secret put GITHUB_TOKEN` prompt. Repository and trusted
   Steward logins are non-secret server bindings in `wrangler.jsonc`. Scope the
   GitHub credential to the observed repository and its required read access.
4. Only after protection is configured, explicitly enable the approved
   `workers.dev` hostname in deployment configuration, or attach an authorized
   custom domain. Keep preview URLs disabled unless separately required and
   proven protected. Persist the approved ingress configuration so a later
   deploy does not silently restore defaults or remove expected routing. Apply
   the approved configuration with `pnpm exec wrangler deploy` before step 5;
   record that deployment/version ID and confirm the intended hostname is enabled.
5. For every enabled hostname, test `/`, `/index.html` and `/api/projection`
   without a session: expect Access denial or its login redirect, never usable
   HTML/API data. Then sign in as the approved identity and prove all five
   surfaces, a successful read-only GitHub refresh, and source revision/build
   identity using the deployment record for the exact bundle. Inspect browser
   network requests, responses and loaded assets for the credential canary/value;
   browser traffic must stay on the protected origin and never call GitHub API.
6. Record Worker ID, hostname(s), deployment/version ID, source SHA/bundle hash,
   Access application and all-traffic policy state, unauthenticated/authenticated
   outcomes, and the no-secret inspection outcome in the canonical handoff.
   Do not publish cookies, Access assertions, tokens, HAR files containing them,
   or raw request logs. Only this live evidence can satisfy external acceptance.

If smoke fails, disable ingress immediately while retaining Access protection.
For rollback use `pnpm exec wrangler rollback <verified-version-id>` only after
checking that revision's security and Access requirements; recheck every route.
For removal, disable ingress, remove the bounded Worker with
`pnpm exec wrangler delete`, revoke its GitHub credential, then remove only its
owned Access application/domain configuration. Record what was removed. Never
remove Access first while an application remains reachable.

## Upstream references

- [Workers Static Assets](https://developers.cloudflare.com/workers/static-assets/)
- [Worker-level Access and Static Assets limitations](https://developers.cloudflare.com/workers/configuration/cloudflare-access/)
- [Worker Secrets](https://developers.cloudflare.com/workers/configuration/secrets/)
- [Wrangler configuration](https://developers.cloudflare.com/workers/wrangler/configuration/)
