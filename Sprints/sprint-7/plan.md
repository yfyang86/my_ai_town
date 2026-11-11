# Sprint 7 — Ruleset framework and scripted checks

## Goal

A storyline can switch on the fifth-edition ruleset and resolve scripted ability checks with real dice. Everything added is inert when no ruleset is chosen.

## Scope

**Framework**

| Piece | Where | Default |
| --- | --- | --- |
| `Rules::judge` — a mode may refuse a core action with a reason | kernel `game_mode`, called when an agent queues actions | allow |
| `GameModeAction.with` — named string parameters | protocol; the tool schema gains `with` only when the open mode asks for it | absent |
| Dice | kernel `game_mode/dice.rs`: seeded, saved with the game, every roll recorded | — |
| Ruleset slot | `GameModeChoice::Storyline.ruleset`; kernel `Srd5` state inside the storyline | none |

**First mechanic: ability checks**

- Sheet per seat: class label, level 1–3, six abilities, skill proficiencies.
- A beat may carry checks: title, DC, who may try, and one or more ways to try it (an ability, optionally a skill) — *DC 15, Strength (Athletics) or Dexterity (Sleight of Hand)*.
- `mode_action check`, `target` = check id, `with.skill` or `with.ability` picks the way. The kernel rolls d20 + ability modifier + proficiency if proficient, compares to the DC, announces formula and result to everyone, and records the roll.
- A milestone can be "someone passed this check".
- One attempt per seat per check.
- With the ruleset on, shoving, robbing and stealing are refused through `judge` — harming someone will go through combat (sprint 9).
- A script with checks but no ruleset still plays: the checks simply do not appear.

**Frontend**

- `protocol.ts`; storyline layer shows each seat's sheet line and a roll feed.
- 灯塔之约 gains an optional 「第五版规则」 switch in its setup with sheets and three checks.

**Licence**

- SRD 5.1 attribution in `NOTICE`, licence text under `licenses/`.

## Acceptance

- All existing tests pass unchanged. With no ruleset: storyline saves, views and prompts are byte-identical to before (asserted in the kernel test).
- Kernel test: sheets validated (bad ability score, unknown skill, wrong sheet count refused); a check passed and one failed under a fixed seed with the expected formula and totals; second attempt refused; a way not offered refused; check milestone reached; rolls identical after save/restore and replay; force refused with the ruleset on and allowed without.
- Dice test: uniform within tolerance over many rolls; same seed, same sequence.
- Agent test: the `mode_action` schema has `with` only when the mode asks.
- Wire samples compile against `protocol.ts`.
- Browser, mock model: 灯塔之约 with the switch on starts and shows sheets.
- Full local gate; PR into `dev`.
