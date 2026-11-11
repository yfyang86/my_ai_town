# Sprint 10 — report

## Shipped

**Magic, rest, growth and duels, on top of the sprint 9 combat.**

- **Spells.** Fourteen cleric and wizard spells in `data/rules/5e/spells.json` (cantrips, 1st and 2nd level), each citing its SRD section. Six kinds: spell attack, saving throw (single target or a whole zone), automatic hit, healing, bless, hold.
- **Prepared spells on the sheet**, validated at open: own class, a level the character has slots for, at most eight.
- **Slots.** Cantrips are free; other spells spend a slot of their level or higher, and some grow with a higher slot. An illegal cast is refused before anything is rolled or spent.
- **Derived numbers.** Spell attack bonus and save DC come from proficiency and the casting ability.
- **Concentration.** One spell at a time; damage forces a Constitution save; losing it ends what the spell granted.
- **Casting in a fight** (`cast` as a turn action) **and outside one** (healing someone in the same place).
- **Class features.** Fighter's second wind; rogue's sneak attack.
- **Long rest.** A real sleep of six hours or more in the world restores hit points, slots and second wind.
- **Experience and levels.** Winning an encounter and reaching a milestone in your own name give experience; levels 2 and 3 raise hit points and slots.
- **Duels.** Two characters in the same place can fight by the turn rules until one drops. Nobody dies. While a ruleset is on this replaces shoving, robbing and stealing, which stay refused.
- 灯塔之约: the cleric and the wizard now have prepared spells.

## Tests

| Check | Result |
| --- | --- |
| `cargo clippy --workspace --all-targets -D warnings` | clean |
| `cargo test --workspace` | pass — 155 tests (+3) |
| `pnpm build`, `typecheck`, `format:check` | pass |
| `pnpm test` | 22 pass, 1 known root-only failure |

New regression tests:

- **Spell data and levels** (`srd5.rs`). Every spell cites the SRD. A fighter with spells, a wizard with a cleric spell, an unknown spell and a 2nd-level spell at level 1 are all refused. Healing outside a fight spends a slot and is refused with none left. 300 and 900 experience give levels 2 and 3 with the right hit points and slots; spent slots do not come back on levelling. Under six hours of sleep is not a long rest; six hours is.
- **Casting in a fight** (`combat.rs`). Magic missile needs no attack roll and spends a slot; a cantrip spends none; an unprepared spell or an empty slot changes nothing; a higher slot adds a missile. Touch healing fails across zones and on an enemy without spending; an unconscious ally wakes. Bless ends when the caster fails the concentration save. Hold is refused on a wolf, stops a thug's turn, and ends with the caster's concentration. Sneak attack applies once a turn. Second wind works once.
- **A thousand fights with casters.** The same hard encounter (four bandits and a thug): 66 % wins with four weapon users, 87 % when the back two are a cleric and a wizard with two slots each.
- **In a story** (`tests/game_mode.rs`). A duel is refused against someone elsewhere, against oneself and while another fight runs; it ends with both alive, the loser at 1 hit point, no experience. The cleric cannot heal someone elsewhere or "heal" with bless; a real heal spends a slot. Eight hours in a bed restores the slot. A declared milestone gives 50 experience. Sheets survive a save and reload.

## Real-model runs (DeepSeek `deepseek-flash`)

**Run 1 — the shipped story.** Played to the end. Fighter and rogue beat the two dogs in three rounds, each got 25 experience; the mayor got 50 for a milestone. The casters are not in that fight, so no spell was cast.

**Run 2 — casters drawn in** (a temporary edit of the encounter: wizard and cleric added, a wolf and a thug added; reverted afterwards).

- The wizard opened with **magic missile** (11 damage, one dog down), then **fire bolt** three times: a critical for 13 that dropped the wolf, then 8 and 9 on the thug. All well-formed, slot use correct.
- The cleric finished the thug with **sacred flame** (the thug failed its Dexterity save).
- The apprentice went down and **died on death saves** before anyone reached him.
- The party won; 66 experience each.

What the model got wrong:

- The cleric tried `cast cure_wounds` on the wizard **before his turn** and was refused ("not your turn yet"), then missed his own turn's deadline.
- To reach the fallen apprentice he used `dash` — which takes the whole turn — instead of stepping and casting in one action (`with.move`). The apprentice died the following round.

Neither is a rules defect. Both are prompt problems: the turn brief should say "to heal someone in another zone, `cast` with `with.move`". Carried to sprint 11.

## Defects found and fixed

1. A stale test expectation: the sheet line now shows experience.
2. Two lint failures (functions with eight arguments) — arguments grouped.
3. During testing only: the relay crashed because its log folder had been deleted before start; the first run was discarded and repeated.

## Not done from the plan

| Planned | State |
| --- | --- |
| About twenty spells | Fourteen. No utility spells that help a scripted check |
| Short rest and hit dice | Not done; only the long rest |
| Action surge, channel divinity | Not done; second wind and sneak attack only |
| Loot, healing potion | Not done |
| Conditions frightened, restrained, blinded, poisoned | Not done; `held` (hold person) is the one condition added |
| Real-model use of `duel`, of healing outside a fight, and of a long rest | Covered by kernel tests only; no model chose them in the two runs |

## Known limits

- **Numbers need checking against the SRD text.** Spell dice, slot tables, creature statistics and experience values were entered from memory. Each entry names its SRD section so it can be checked; this has not been done.
- Sneak attack does not require a finesse or ranged weapon, and bless picks the first three allies rather than letting the caster choose.
- Ranges are reduced to "touch / at range / a zone".
- A dead character's sheet still reads 0 hit points rather than saying so.

## Next

Sprint 11 — model narrator and adventure drafting: `Sprints/sprint-11/plan.md`.
