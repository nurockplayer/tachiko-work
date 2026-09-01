# Live Project Dashboard

The repository-local Dashboard is a read-only projection of live GitHub facts,
live-main identity, Product Roadmap data, and strict Steward/operational
evidence. It is internal/dogfood tooling, not a semantic object, workflow
engine, control plane, or merge authority.

It presents five surfaces: executive status, delivery Issue/PR lanes with
exact identities, current dependencies, bounded recent activity, and
authority/human attention. GitHub and Steward values are displayed directly;
the Dashboard does not interpret review prose or compute `can_merge`, a final
merge verdict, or policy semantics. Missing or incomplete evidence is shown as
partial or Unknown rather than inferred as healthy.

Run the focused validation from the repository root:

```sh
bash scripts/project-dashboard-check.sh
```

Build and serve locally on loopback:

```sh
pnpm --dir tools/operational-evidence install --frozen-lockfile
pnpm --dir tools/operational-evidence build
pnpm --dir tools/project-dashboard install --frozen-lockfile
pnpm --dir tools/project-dashboard build
pnpm --dir tools/project-dashboard serve
```

Open `http://127.0.0.1:4174`. Optional `GITHUB_TOKEN` or `GH_TOKEN` is read by
the server only; it is never sent to the browser. The server exposes only
loopback `GET`/`HEAD` routes, uses no-store responses, and has no mutation or
command endpoint. Without credentials, affected observations remain
Unavailable/Unknown. pnpm 11.25.0 and Node.js 24 are required; browser tests
use Chromium.

The Dashboard consumes the private
`@tachiko-work/operational-evidence` package and does not reimplement its
authority or exact-head rules. See [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md)
for the CSS dependency notice.
