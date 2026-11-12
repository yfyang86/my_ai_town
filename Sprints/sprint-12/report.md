# Sprint 12 — report

## Shipped

### Numbers checked against the SRD

Every entry in `data/rules/5e/` was compared with SRD 5.1 data (open5e API, document `wotc-srd`).

| Data | Result |
| --- | --- |
| Ten creatures: AC, hit points, Dexterity, Constitution, Wisdom, attack bonus and damage, type, challenge → experience | all match |
| Weapons: damage dice, finesse, ranged | all match |
| Armour: base AC, Dexterity cap | all match |
| Four classes: hit die, saving throws, skill list and count | all match |
| Fourteen spells: level, class list, dice, save, concentration, higher-slot scaling | all match |
| Slots at levels 1–3, experience for levels 2 and 3 | match |

No number had to change. Three **rules** differed and were corrected:

1. **Thug** has Multiattack: two mace attacks a turn (was one).
2. **Hold person:** the held creature loses its turn and repeats the save at the *end* of it (was: saved first and could act at once).
3. **Sneak attack** needs a finesse or ranged weapon (was: any weapon).

### Short rest

`mode_action rest`, outside a fight or scene: spend one hit die, regain the roll plus the Constitution modifier. As many dice as the level, once per game hour; a long rest returns half. Shown on the sheet (「生命骰剩 n/m」).

### Spectator view

- Narrator rulings are labelled 「临场：…」 in the roll feed and cut to a readable length.
- A character who is gone is marked on their sheet line.
- The full sheet line appears on hover over a character in the cast list.

### Docs

- `README.md`: a player-facing entry for 「第五版规则」 — what it does, how to turn it on, that it needs a connected model.
- Wiki, `licenses/SRD-5.1.txt`, the sprint index and the ruleset plan brought up to date.

## Tests

| Check | Result |
| --- | --- |
| `cargo clippy --workspace --all-targets -D warnings` | clean |
| `cargo test --workspace` | pass — 158 tests |
| `pnpm build`, `typecheck`, `format:check` | pass |
| `pnpm test` | 22 pass, 1 known root-only failure |

No new test functions; existing ones gained assertions: the thug attacks twice; an unarmed rogue has no sneak attack; short rest heals by the die plus Constitution, is refused when unhurt, within the hour and with no dice left, and a long rest returns a die; in a story, the loser of a duel can rest once and not twice. The thousand-fight simulation still lands in its bands with the thug's second attack.

## Cost (DeepSeek `deepseek-flash`, 灯塔之约, six characters, one game day, speed ×8)

| Session | Model calls | Prompt tokens | Completion tokens | Total |
| --- | --- | --- | --- | --- |
| No rules | 249 | 2,750,538 | 129,525 | 2,880,063 |
| Fifth-edition rules | 261 (9 narrator) | 3,103,117 | 143,618 | 3,246,735 |

Rules cost about **5 % more calls and 13 % more tokens** on the same story: the sheet and open checks in every prompt, turn-by-turn fight calls, and nine narrator calls. Creature turns cost nothing. Drafting an adventure is one call (about a minute). These are single runs; a different day will differ by more than the gap between them.

## Real-model observations

- With rules: 21 rolls logged, the fight won in four rounds, six of eight milestones.
- **Narrator:** nine calls; seven rulings were seen announced in the watched log, five of them with a roll. DCs now vary (10 and 15 seen; sprint 11 was 15 every time). What became of the other two calls was not traced.
- **First real use of healing outside a fight:** after the fight the cleric cast cure wounds on the keeper.
- No fight stalled and no off-menu ruling was applied.
- Not seen with a real model: `duel`, `rest`, a long rest (the story is one day).

## Not done from the plan

| Planned | State |
| --- | --- |
| A sheet panel per character | Hover text only |
| Simulation at level 3 | Not done; the simulation still uses first-level parties |
| A second full session on a drafted story this sprint | Done in sprint 11, not repeated |
| Healing potion / loot | Not done (short rest was chosen) |
| PR `dev` → `main` | **Not opened.** `dev` is ready; opening it is the owner's call |

## Known limits of the ruleset as shipped

- Levels 1–3, four classes, fourteen spells, ten creatures; zones in a line instead of a grid.
- Creature traits beyond the attack are not modelled (undead fortitude, pack tactics, nimble escape, vulnerabilities, secondary attacks).
- Bless picks the first three allies; healing word takes an action; ray of frost does not slow.
- Conditions: prone, dodging, unconscious, held.
- Hit points live in the ruleset; the kernel's health meter is touched only by death.
- A short rest costs no world time, only its hourly limit.
- The narrator is the asking resident's own model, bounded by the menu, kernel validation and an hourly limit.
- The built-in small model cannot follow these action formats; a connected model service is required.
