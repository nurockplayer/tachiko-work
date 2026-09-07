# Operational Dashboard

Repository-local, loopback-only read-only Dashboard for the configured GitHub repository.
It has no write, merge, dispatch, or agent-control routes.

Build and start it with Node 24+ and pnpm 11.25.0:

```sh
pnpm --dir tools/project-dashboard install --frozen-lockfile
DASHBOARD_REPOSITORY=nurockplayer/tachiko-work \
DASHBOARD_GITHUB_TOKEN="$(gh auth token)" \
DASHBOARD_TRUSTED_STEWARD_LOGINS=nurockplayer \
pnpm --dir tools/project-dashboard start
```

`DASHBOARD_PORT` optionally selects a loopback port (default `4311`). The token
and trust allowlist are process-only configuration; they are never accepted from
browser routes or emitted in the projection/UI.

## Private Cloudflare hosting

Repository-local Worker support and the separately authorized activation steps
are documented in [the Cloudflare runbook](CLOUDFLARE.md). Stage A does not claim
that a live private deployment is active.
