# Project Dashboard v0

The Tachiko Work Live Project Dashboard is a repository-local, read-only
projection over live GitHub facts, live-main repository authority, and the
strict structured evidence implemented by `@tachiko-work/operational-evidence`.
It is reversible internal/dogfood tooling, not a Tachiko semantic object, public
API, roadmap authority, workflow engine, or control plane.

## What it shows

The v0 UI intentionally contains five surfaces:

1. an executive strip for live `main`, Product Roadmap horizon, fetch health,
   active/Ready lanes, and human attention;
2. a delivery command center with independent Issue/PR lanes and exact
   head/base/merge-base/main identities;
3. a current dependency/critical-path projection;
4. bounded recent merge context; and
5. an authority/attention panel.

Every important state carries source links. Missing, truncated, conflicting,
stale, or unavailable required evidence is `Unknown` or partial, never green by
default. GitHub review prose is never classified for ownership, readiness,
severity, validation, blocking, or merge authority. Native checks, automated
browser evidence, perceptual review, and local delivery validation remain
independent claims.

## Run locally

From the repository root, validate the complete tool:

```sh
bash scripts/project-dashboard-check.sh
```

Build and serve the live dashboard on loopback only:

```sh
bash scripts/operational-evidence-check.sh
pnpm --dir tools/project-dashboard install --frozen-lockfile
pnpm --dir tools/project-dashboard build
pnpm --dir tools/project-dashboard serve

# Optional: enable authenticated live GitHub observations.
GITHUB_TOKEN="$(gh auth token)" pnpm --dir tools/project-dashboard serve
```

Open `http://127.0.0.1:4174`. `GITHUB_TOKEN` or `GH_TOKEN` is read only by the
server process and is never included in the browser projection. The adapter
uses a fixed GraphQL query and REST compare GETs for
`nurockplayer/tachiko-work`; it has no GitHub mutation path. Without a usable
credential, the UI remains available but reports the affected observation as
Unavailable/Unknown.

The server binds only to `127.0.0.1`, exposes only `GET`/`HEAD`, returns
`Cache-Control: no-store`, and has no merge, Issue mutation, agent dispatch,
scheduling, or command endpoint. Public/shared deployment and authentication
are deliberately out of scope.

## Evidence boundary

The Dashboard depends directly on the private local
`@tachiko-work/operational-evidence` package. Its strict handoff, Steward watch,
operational-evidence, native-fact precedence, and exact-head lifetime rules are
not reimplemented here. The Dashboard adds only ordinary application DTOs,
read-only source collection, finite display derivation, and presentation.

CYBERCORE CSS 0.3.0 supplies the selected visual foundation under the MIT
license. See [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
