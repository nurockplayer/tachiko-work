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

## CLI workflow

Build the CLI from the repository root, then run this complete journey:

```sh
# Create the canonical starter without hand-authoring JSON.
moonfall_demo=$(mktemp -d "${TMPDIR:-/tmp}/tachiko-moonfall.XXXXXX")
./target/debug/tachiko init "$moonfall_demo/moonfall.ro" \
  --id game-balance --title "Moonfall: starter balance"

# Browse entities, stable field paths, references, and calculated values.
./target/debug/tachiko show "$moonfall_demo/moonfall.ro"

# Understand where DPS comes from and which inputs it uses.
./target/debug/tachiko explain "$moonfall_demo/moonfall.ro" iron_sword.dps

# Understand what changing damage would affect.
./target/debug/tachiko explain "$moonfall_demo/moonfall.ro" iron_sword.damage

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

## Why `.ro` in Git

`.ro` is canonical UTF-8 JSON: stable map ordering and formatting make
equivalent documents byte-identical, while each change remains reviewable as a
field-level Git diff. Typed references and formulas preserve intent that a CSV
or spreadsheet export would lose. The CLI can validate the document in CI,
calculate derived values deterministically, and render a semantic diff that
calls out downstream impact. Export is available when an engine or other tool
needs plain evaluated data, without making that export the source of truth.
