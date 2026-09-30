# Tachiko Work

**An open engine for data, formulas, and rules—not another spreadsheet UI.**

When important work is scattered across spreadsheets and scripts, it is hard to
know what a change will break. Tachiko Work makes those relationships explicit:
change a value, inspect its effects, and validate the result. Different tools can
use the same rules instead of rebuilding them.

The goal is work that people, software, and AI can understand, review, and carry
between tools.

**Today: developer-facing pre-alpha.** This repository provides the Rust engine,
CLI, and experimental browser-client kit.
[Tachiko Sheet](https://github.com/nurockplayer/tachiko-sheet) owns the spreadsheet
interface. Work is not a finished Excel replacement.

## See the value

In the included game-balance example, increasing a sword's damage from **36 to
45** changes its calculated DPS from **40 to 50**. Work explains the dependency,
shows both changes in a semantic diff, and rejects an attack interval of zero
rather than saving a broken result.

That is the foundation: typed data and references, deterministic formulas,
validation, and meaningful diff/merge. Game balance is the first proving ground,
not the limit of the [product vision](docs/vision/product-constitution.md).
Neither a hosted AI service nor a Git workflow is required to use the engine.

## Try the CLI

From a macOS/Linux shell with Rust 1.85 or newer; building also downloads the
locked dependencies. This creates a separate output and leaves the fixture alone.

```sh
git clone https://github.com/nurockplayer/tachiko-work.git
cd tachiko-work
cargo build --locked -p tachiko-cli

demo=$(mktemp -d "${TMPDIR:-/tmp}/tachiko-demo.XXXXXX")
./target/debug/tachiko set examples/game-balance/game-balance.ro \
  iron_sword.damage 45 --output "$demo/buffed.ro"
./target/debug/tachiko diff examples/game-balance/game-balance.ro "$demo/buffed.ro"
./target/debug/tachiko validate "$demo/buffed.ro"
```

Expected highlights: `damage: 36 -> 45` and `affected dps: 40 -> 50`.
[Continue the walkthrough](examples/game-balance/README.md) for explanation,
rejection, export, and optional Git/CI review.

## Find your next step

| You want to… | Start here |
| --- | --- |
| Understand the whole project or work as an AI agent | [Three-minute engineering map](ARCHITECTURE.md) |
| Find the authoritative answer to a specific question | [Documentation by task](docs/README.md) |
| Build, test, or contribute | [CONTRIBUTING](CONTRIBUTING.md); agents also read [AGENTS](AGENTS.md) |
| Integrate a browser client | [Experimental producer boundary](packages/browser-client/README.md) |
| See direction and active work | [Roadmap](ROADMAP.md) · [Live campaign handoff](https://github.com/nurockplayer/tachiko-work/issues/374) |

Full Office compatibility, hosted realtime collaboration, and a stable public
SDK are not current product promises. Source availability is not a claim that a
particular client or release supports every engine capability.

Code is dual-licensed under [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT).
See [SECURITY](SECURITY.md) for reporting vulnerabilities and
[CONTRIBUTING](CONTRIBUTING.md) for the current contribution boundary.
