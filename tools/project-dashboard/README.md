# Tachiko Work live project dashboard

This package is a read-only, repository-local operational projection for
Tachiko Work. It does not create product semantics, roadmap authority, a public
API, or a control plane. GitHub and repository authority remain canonical.

The localhost server reads `nurockplayer/tachiko-work`, normalizes exact-head
checks together with the live required-check rules, reviews, canonical
`agent-handoff:v1` state, dependencies, authority drift, and recent merges,
then exposes only the credential-free projection to the browser. Source
failures remain explicit `partial`, `unavailable`, or `unknown` states.

## Run locally

Use pnpm 11.25.0 and Node.js 24 or newer:

```sh
pnpm --dir tools/project-dashboard install --frozen-lockfile
pnpm --dir tools/project-dashboard build
pnpm --dir tools/project-dashboard start
```

Open <http://127.0.0.1:4178>. The server uses `GITHUB_TOKEN` or `GH_TOKEN` when
provided, otherwise it reads the token from the existing authenticated `gh`
CLI session. The credential stays in the server process and is never part of
the browser DTO, assets, errors, or logs. Use a minimum-scope read credential.

The server binds only to `127.0.0.1`. It has two GET-only API endpoints:
`/api/health` and `/api/projection`. There are no write, merge, Issue mutation,
scheduling, dispatch, or agent-control endpoints. Its 30-second in-memory cache
is disposable and non-authoritative.

The bundled CYBERCORE CSS visual foundation is MIT-licensed. Its required
attribution is shipped with the built dashboard at
`/THIRD_PARTY_LICENSES.txt`.

## Validate

```sh
pnpm --dir tools/project-dashboard lint
pnpm --dir tools/project-dashboard typecheck
pnpm --dir tools/project-dashboard test
pnpm --dir tools/project-dashboard build
pnpm --dir tools/project-dashboard exec playwright install chromium
pnpm --dir tools/project-dashboard test:browser
```

`DASHBOARD_FIXTURE=pressure-tests` is reserved for the automated browser
journey. It exercises separate Ready, stale-head, and substantive-review lanes
without making live GitHub state deterministic test data.
