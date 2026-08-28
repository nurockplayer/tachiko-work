# Moonfall game balance

This is a small, Git-friendly balance slice for a fictional action game. It
models the relationships a designer actually edits: Alric equips a typed weapon
reference, a shop sells the sword and its upgrade, and formulas make the useful
numbers explicit.

## Model

The document contains four schemas and four entities:

- `characters.alric` has a typed `weapon` reference to `weapons.iron_sword`.
- `weapons.iron_sword` stores `damage`, `attack_interval`, and `price`. Its
  derived `dps` is `damage / attack_interval` (40 DPS in the base document).
- `items.tempered_blade` has a typed `grants_weapon` reference and costs 200
  gold.
- `economy.shop` stores the 50-gold match reward. `matches_for_sword` derives
  the 120-gold sword price / reward, and `upgrade_cost` references the item
  price.

`buffed-sword.ro` changes only `weapons.iron_sword.damage` from 36 to 45. A
semantic diff therefore shows both the direct input change and the affected
formula result (`dps`: 40 to 50); prices and the economy formulas remain
unchanged.

## Game Dev Alpha 60-second proof

Build once, then run the standalone product path from the durable Moonfall
fixture. No Git repository, host account, CSV step, internal `.roproj` edit, or
hosted LLM is involved:

```sh
cargo build -p tachiko-cli
moonfall_alpha=$(mktemp -d "${TMPDIR:-/tmp}/tachiko-alpha.XXXXXX")

./target/debug/tachiko roproj materialize \
  examples/game-balance/game-balance.ro \
  "$moonfall_alpha/base.roproj"
./target/debug/tachiko set \
  "$moonfall_alpha/base.roproj" iron_sword.damage 45 \
  --output "$moonfall_alpha/buffed-direct.ro"
./target/debug/tachiko roproj materialize \
  "$moonfall_alpha/buffed-direct.ro" \
  "$moonfall_alpha/buffed.roproj"

./target/debug/tachiko diff \
  "$moonfall_alpha/base.roproj" "$moonfall_alpha/buffed.roproj"
./target/debug/tachiko analyze changes \
  "$moonfall_alpha/base.roproj" "$moonfall_alpha/buffed.roproj" \
  --before-state base --after-state buffed
./target/debug/tachiko calculate "$moonfall_alpha/buffed.roproj"
./target/debug/tachiko roproj validate "$moonfall_alpha/buffed.roproj"
./target/debug/tachiko export \
  "$moonfall_alpha/buffed.roproj" "$moonfall_alpha/runtime.json"
```

The meaningful result is `damage: 36 -> 45`, with deterministic derived impact
`dps: 40 -> 50`. The source project remains unchanged. The accepted result is
a fresh canonical tree, while the direct candidate remains an explicit step
rather than a silently synchronized second authority.

The same deterministic engine rejects an invalid balance change before it can
create an output:

```sh
if ./target/debug/tachiko set \
  "$moonfall_alpha/base.roproj" iron_sword.attack_interval 0 \
  --output "$moonfall_alpha/invalid.ro"; then
  echo "unexpected success" >&2
  exit 1
fi
test ! -e "$moonfall_alpha/invalid.ro"
```

The error identifies division by zero in `iron_sword.dps`. Executable release
evidence reruns review, calculation, and export for byte-identical outcomes,
then exercises the same fixture in an ordinary temporary Git branch:

```sh
TACHIKO_BIN="$PWD/target/debug/tachiko" bash scripts/first-user-smoke.sh
TACHIKO_BIN="$PWD/target/debug/tachiko" bash scripts/git-ci-smoke.sh
```

The optional Git lane proves a one-record change in one canonical entity shard,
semantic review parity inside and outside Git, canonical/package consistency,
a normal branch commit, and CI rejection of the same
`iron_sword.attack_interval = 0` fault. Its deliberate invalid-tree injection
is test-only evidence that CI fails closed; it is not an authoring procedure.

## CLI workflow

Build the CLI from the repository root, then run this complete journey:

```sh
# Create the canonical starter without hand-authoring JSON.
moonfall_demo=$(mktemp -d "${TMPDIR:-/tmp}/tachiko-moonfall.XXXXXX")
./target/debug/tachiko init "$moonfall_demo/moonfall.ro" \
  --id game-balance --title "Moonfall: starter balance"

# Browse entities, current field addresses, opaque IDs, references, and calculated values.
./target/debug/tachiko show "$moonfall_demo/moonfall.ro"

# Understand where DPS comes from and which inputs it uses.
./target/debug/tachiko explain "$moonfall_demo/moonfall.ro" iron_sword.dps

# Understand what changing damage would affect.
./target/debug/tachiko explain "$moonfall_demo/moonfall.ro" iron_sword.damage

# Get the same formula, upstream, and downstream facts as structured JSON.
./target/debug/tachiko analyze field \
  "$moonfall_demo/moonfall.ro" iron_sword.dps --source-state starter

# Create a reviewed variant; the source and existing files are never overwritten.
./target/debug/tachiko set "$moonfall_demo/moonfall.ro" iron_sword.damage 45 \
  --output "$moonfall_demo/moonfall-buffed.ro"

# Check schema, required fields, references, and formulas.
./target/debug/tachiko validate "$moonfall_demo/moonfall-buffed.ro"

# Print deterministic calculated values, including iron_sword.dps and
# shop.matches_for_sword.
./target/debug/tachiko calculate "$moonfall_demo/moonfall-buffed.ro"

# Review the input change and its derived formula impact.
./target/debug/tachiko diff \
  "$moonfall_demo/moonfall.ro" "$moonfall_demo/moonfall-buffed.ro"

# Get structured semantic changes and affected stable-ID areas.
./target/debug/tachiko analyze changes \
  "$moonfall_demo/moonfall.ro" "$moonfall_demo/moonfall-buffed.ro" \
  --before-state starter --after-state buffed

# Produce evaluated entity data for downstream tooling.
./target/debug/tachiko export \
  "$moonfall_demo/moonfall-buffed.ro" "$moonfall_demo/moonfall-export.json"
```

The expected semantic highlights are:

```text
Weapons Iron Sword
damage: 36 -> 45
affected dps: 40 -> 50
```

## Collaborate on independent balance changes

Create each branch from the same base and keep all branch and merge outputs
distinct. This combines a damage change with an attack-interval change through
the semantic model, then verifies the merged result:

```sh
./target/debug/tachiko set "$moonfall_demo/moonfall.ro" iron_sword.damage 45 \
  --output "$moonfall_demo/moonfall-ours.ro"
./target/debug/tachiko set "$moonfall_demo/moonfall.ro" iron_sword.attack_interval 0.8 \
  --output "$moonfall_demo/moonfall-theirs.ro"
./target/debug/tachiko merge \
  "$moonfall_demo/moonfall.ro" \
  "$moonfall_demo/moonfall-ours.ro" \
  "$moonfall_demo/moonfall-theirs.ro" \
  --output "$moonfall_demo/moonfall-merged.ro"
./target/debug/tachiko validate "$moonfall_demo/moonfall-merged.ro"
./target/debug/tachiko calculate "$moonfall_demo/moonfall-merged.ro"
./target/debug/tachiko diff \
  "$moonfall_demo/moonfall.ro" "$moonfall_demo/moonfall-merged.ro"
```

The merged semantic values are `damage: 45`, `attack_interval: 0.8`, and
`dps: 56.25`. The merge command is an exclusive create: it writes neither a
conflicted nor an invalid candidate, and it never overwrites an existing file.
It operates on Tachiko's typed model rather than raw JSON text and does not
install a Git merge driver or provide interactive resolution.

## Grow and reorganize the roster

Duplicate an existing entity when a new roster member should start from proven
balance data. Formula references from the copy to its own fields are rebased;
relationships to other entities retain their original meaning:

```sh
./target/debug/tachiko entity duplicate \
  "$moonfall_demo/moonfall.ro" iron_sword steel_sword \
  --output "$moonfall_demo/moonfall-with-steel.ro"
./target/debug/tachiko set \
  "$moonfall_demo/moonfall-with-steel.ro" steel_sword.name "Steel Sword" \
  --output "$moonfall_demo/moonfall-steel-named.ro"
./target/debug/tachiko set \
  "$moonfall_demo/moonfall-steel-named.ro" steel_sword.damage 45 \
  --output "$moonfall_demo/moonfall-steel-tuned.ro"
./target/debug/tachiko entity rename \
  "$moonfall_demo/moonfall-steel-tuned.ro" steel_sword moonblade \
  --output "$moonfall_demo/moonfall-with-moonblade.ro"
./target/debug/tachiko explain \
  "$moonfall_demo/moonfall-with-moonblade.ro" moonblade.dps
```

The final formula projects as
`([moonblade.damage] / [moonblade.attack_interval])` and calculates to 50 DPS.
Rename changes only the human key: the entity's stable ID, stored typed
relationships, and formulas owned by other entities retain their bound IDs and
ASTs unchanged.

Removal never cascades. Trying to remove `iron_sword` reports
`alric.weapon`, `shop.matches_for_sword`, and
`tempered_blade.grants_weapon`, then writes nothing. An unreferenced copy can be
removed explicitly:

```sh
./target/debug/tachiko entity remove \
  "$moonfall_demo/moonfall-with-moonblade.ro" moonblade \
  --output "$moonfall_demo/moonfall-pruned.ro"
```

## Author a formula

Formula authoring projects directly onto the typed expression model. This
variant adds a flat 5-DPS bonus and a 60-DPS cap without editing JSON:

```sh
./target/debug/tachiko formula set \
  "$moonfall_demo/moonfall.ro" iron_sword.dps \
  --expression 'min(60, [iron_sword.damage] / [iron_sword.attack_interval] + 5)' \
  --output "$moonfall_demo/moonfall-capped-dps.ro"
./target/debug/tachiko explain \
  "$moonfall_demo/moonfall-capped-dps.ro" iron_sword.dps
./target/debug/tachiko diff \
  "$moonfall_demo/moonfall.ro" "$moonfall_demo/moonfall-capped-dps.ro"
```

The new result is 45. References use `[entity.field]`; supported operations are
finite numbers, `+`, `-`, `*`, `/`, parentheses, unary signs, `min`, and `max`.
Quote `--expression` values in the shell. Tachiko refuses invalid syntax,
references, cycles, calculation failures, unchanged formulas, and bounded
complexity violations before creating an output.

## Why `.roproj` in Git

The two checked-in `.ro` files remain direct-JSON compatibility examples for
the CLI journey above. A Git working project should materialize the same
semantic document as canonical `.roproj/v1` without hand-editing its internal
files:

```sh
./target/debug/tachiko roproj materialize \
  examples/game-balance/game-balance.ro \
  "$moonfall_demo/game-balance.roproj"
./target/debug/tachiko validate "$moonfall_demo/game-balance.roproj"
./target/debug/tachiko show "$moonfall_demo/game-balance.roproj"
```

The fixed JSONL shards make a scalar entity edit one removed and one added
record in one stable file. Ordinary Git shows that physical change, while
`tachiko diff` and `tachiko analyze changes` reconstruct both trees and report
the authoritative field/formula meaning. CI can run the same canonical and
workspace validation outside Git, then optionally use `roproj compare-package`
when a repository deliberately tracks a generated portable `.ro` artifact.
See the root README's Git/CI section for the exact attributes and commands.
