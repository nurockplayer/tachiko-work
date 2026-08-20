# Tachiko Work

A Rust-native, Git-native, and AI-native workspace for semantic structured data
and computation.

> **Public pre-alpha:** this repository is intentionally public so users,
> reviewers, AI tools, and future contributors can inspect the implementation,
> architecture, and decision history. Public source does not mean the product,
> APIs, formats, or licensing policy are stable or formally released.

## Vision

Tachiko Work is not an Office clone.

It is a semantic document platform where structured data, formulas, Git review,
and AI operations share one typed model. Traditional document and spreadsheet
views are future projections of that model rather than separate sources of
truth.

## First usable product

The current product provides a complete, safe CLI-first game-balance workflow:

- typed schemas, entities, fields, and references;
- canonical, versioned `.ro` serialization;
- deterministic formula calculation and dependency tracking;
- semantic diff with derived formula impact;
- guided starter creation, browsing, explanation, and typed edits;
- validated entity duplication, relationship-safe rename, and protected removal;
- bounded formula creation and revision with canonical expression syntax:
  `finite literal`, `+ - * /`, unary signs, parentheses,
  `[entity.field]` references, and `min`/`max`;
- CLI validation and evaluated runtime JSON export;
- read-only AI structure/formula/impact queries and approval-required suggestions.

It deliberately does not include a spreadsheet UI, Office compatibility,
realtime collaboration, cloud infrastructure, or game-engine plugins.

## Installation status

Tachiko Work does not yet have a tagged or published binary release. Until the
first release is published, install the CLI from a repository checkout. This
requires Rust 1.85 or newer:

```sh
cargo install --path crates/cli --locked
tachiko --version
# tachiko 0.1.0
```

The installed binary is named `tachiko`. Contributors can instead use
`cargo run -p tachiko-cli -- <command>` without installing it.

### Binary archives after the first release

The release workflow is ready to produce one native archive for each supported
target, but these files will not exist until an exact version tag has passed the
release workflow and its draft has been reviewed and published:

```text
tachiko-0.1.0-x86_64-unknown-linux-gnu.tar.gz
tachiko-0.1.0-aarch64-apple-darwin.tar.gz
tachiko-0.1.0-x86_64-apple-darwin.tar.gz
tachiko-0.1.0-x86_64-pc-windows-msvc.tar.gz
```

The GitHub repository is intentionally public during pre-alpha development.
Publishing a GitHub release remains a separate release-owner decision; no tag or
binary release exists merely because the source repository is public.

Download the matching archive and its adjacent `.sha256` file from the same
release. On Linux or macOS, run the checksum command for your platform and then
extract the archive:

```sh
archive=tachiko-0.1.0-aarch64-apple-darwin.tar.gz

sha256sum --check "$archive.sha256"       # Linux
shasum -a 256 --check "$archive.sha256"  # macOS

tar -xzf "$archive"
"./${archive%.tar.gz}/tachiko" --version
# tachiko 0.1.0
```

On Windows x86-64, PowerShell can verify and extract the release without extra
tools:

```powershell
$Archive = "tachiko-0.1.0-x86_64-pc-windows-msvc.tar.gz"
$Expected = ((Get-Content "${Archive}.sha256" -Raw).Trim() -split '\s+')[0].ToLowerInvariant()
$Actual = (Get-FileHash -Algorithm SHA256 $Archive).Hash.ToLowerInvariant()
if ($Actual -ne $Expected) { throw "SHA-256 checksum mismatch" }

tar.exe -xzf $Archive
& ".\tachiko-0.1.0-x86_64-pc-windows-msvc\tachiko.exe" --version
# tachiko 0.1.0
```

The initial archives are checksummed but are not signed, and the macOS binaries
are not notarized. Operating systems may warn or block execution. If your
environment requires signed software, build from reviewed source instead of
bypassing its security policy.

Every archive also includes `THIRD_PARTY_LICENSES.md`, the generated inventory
and exact license/notice texts for the locked dependencies used by the CLI.
Revisions already published under `Apache-2.0 OR MIT` retain those historical
license grants. The long-term license-by-component policy is still being decided
in issue #15; see [`docs/governance/licensing-posture.md`](docs/governance/licensing-posture.md).

## Try it in five minutes

Create a project you can immediately understand:

```sh
tachiko_demo=$(mktemp -d "${TMPDIR:-/tmp}/tachiko-demo.XXXXXX")
tachiko init "$tachiko_demo/balance.ro" --title "My Game Balance"
tachiko show "$tachiko_demo/balance.ro"
tachiko explain "$tachiko_demo/balance.ro" iron_sword.dps
```

Make a safe balance change. Tachiko creates a new document, checks it, and
shows both the direct edit and its derived impact:

```sh
tachiko set "$tachiko_demo/balance.ro" iron_sword.damage 45 \
  --output "$tachiko_demo/buffed.ro"
tachiko diff "$tachiko_demo/balance.ro" "$tachiko_demo/buffed.ro"
tachiko validate "$tachiko_demo/buffed.ro"
tachiko export "$tachiko_demo/buffed.ro" "$tachiko_demo/buffed.json"
```

The checked-in Moonfall example and expected output are documented in
[`examples/game-balance/README.md`](examples/game-balance/README.md).

Use `tachiko init scratch.ro --template empty` only when you intentionally want
to author schemas and entities directly in canonical `.ro` JSON.

## Grow the balance roster

Entity lifecycle commands create new reviewed documents just like `set`. Copy a
working weapon, tune the copy, and give its stable identifier a final name
without breaking self-referential formulas:

```sh
tachiko entity duplicate balance.ro iron_sword steel_sword \
  --output with-steel-sword.ro
tachiko set with-steel-sword.ro steel_sword.name "Steel Sword" \
  --output named-steel-sword.ro
tachiko set named-steel-sword.ro steel_sword.damage 45 \
  --output tuned-steel-sword.ro
tachiko entity rename tuned-steel-sword.ro steel_sword moonblade \
  --output with-moonblade.ro
tachiko explain with-moonblade.ro moonblade.dps
```

`rename` rewrites typed entity relationships and formula references throughout
the document. `remove` is deliberately non-cascading: it reports every
dependent field and creates no output while another entity still references the
target. Removing an unreferenced entity is explicit and safe:

```sh
tachiko entity remove with-moonblade.ro moonblade --output without-moonblade.ro
```

## Author computations

Create or revise a formula on any schema-numeric field.

Implemented v0.1 formula grammar (copy-paste ready):

- `[entity.field]` references
- `+`, `-`, `*`, `/` operators with normal precedence
- unary `+` and unary `-`
- parentheses
- `min(lhs, rhs)` and `max(lhs, rhs)`
- finite decimal and scientific literals only

```sh
tachiko formula set balance.ro iron_sword.dps \
  --expression 'min(60, [iron_sword.damage] / [iron_sword.attack_interval] + 5)' \
  --output capped-dps.ro
tachiko explain capped-dps.ro iron_sword.dps
tachiko diff balance.ro capped-dps.ro
```

The result is 45 DPS. `explain` prints canonical syntax that can be pasted back
into `--expression`.

Valid forms are intentionally limited to this grammar; there is no conditional,
comparison, lookup, or function table extension in v0.1.

Always quote formulas in a shell so brackets, parentheses, spaces, and `*` are
passed literally. Invalid syntax, missing or non-numeric references, cycles,
division by zero, non-finite results, and expressions over the bounded
size/depth limits create no output.

Typed AI formula proposals use the same complexity and semantic gates, remain
inert, and require an approved write path.

## Combine independent balance branches

Start each branch from the same canonical document and create a distinct output
for every edit. Tachiko reconciles typed semantic changes; it does not perform
raw-text conflict resolution or modify Git configuration.

```sh
tachiko merge base.ro ours.ro theirs.ro --output merged.ro
tachiko validate merged.ro
tachiko calculate merged.ro
tachiko diff base.ro merged.ro
```

For example, if one branch changes `iron_sword.damage` to `45` and another
changes `iron_sword.attack_interval` to `0.8`, the merged document has DPS
`56.25`. A conflict or invalid candidate creates no output; resolve it by
producing new semantic inputs and rerunning the command.

## Workspace

- `semantic-core`: document, schema, typed values/references, validation
- `storage`: canonical `.ro` parsing, versioning, and serialization
- `formula-engine`: bounded expression parsing, canonical formatting, evaluation, and dependencies
- `diff-engine`: entity/field changes and calculated impact
- `merge-engine`: deterministic typed three-way semantic reconciliation
- `ai-api`: read/explain/suggest-only semantic operations
- `workflow`: reusable starter, explanation, scalar/formula editing, and entity lifecycle operations
- `cli`: creation, exploration, computational authoring, lifecycle, review, merge, validation, and export

For a fast contributor check after an edit, run:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
```

Before requesting review, install stable Rust with its formatting and linting
components plus the exact minimum toolchain. Then run the complete
release-equivalent gate from a clean local commit. The gate selects stable for
all normal and nested commands even if the caller has another rustup override;
it separately checks Rust 1.85 compatibility, warning-free documentation,
deterministic dependency notices, Cargo packages, all four real user journeys, and
the native release archive:

```sh
rustup toolchain install stable --profile minimal
rustup component add --toolchain stable rustfmt clippy
rustup toolchain install 1.85.0 --profile minimal
bash scripts/release-check.sh
```

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the current contribution boundary,
[`SECURITY.md`](SECURITY.md) for responsible reporting, the
[`licensing posture`](docs/governance/licensing-posture.md) for provisional
governance, and the
[`release-owner runbook`](docs/governance/release-process.md) for the tag and
publication boundary.

## Principles

- Semantic correctness over historical accidents
- Legacy compatibility belongs at system boundaries
- Open formats over vendor lock-in
- Git is a storage protocol, not a user interface
- AI should manipulate semantic models, not imitate users

Architecture records and specifications live under [`docs/`](docs/). Accepted
ADRs take precedence over older exploratory roadmaps.
