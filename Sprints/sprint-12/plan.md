# Sprint 12 — Playtest, balance, cost, release

## Goal

The fifth-edition ruleset is fit to hand to players: numbers checked, the spectator can follow a session, the cost is known, and `dev` is ready for `main`.

## Scope

1. **Numbers against the SRD.** Go through `data/rules/5e/*.json` entry by entry against the SRD 5.1 text (classes, weapons, armour, creatures, spells, slots, experience) and correct what differs. Sneak attack requires a finesse or ranged weapon.
2. **Spectator view.**
   - A sheet panel per character: class, level, hit points, slots, prepared spells.
   - Narrator rulings in the roll feed, distinguishable from scripted checks.
   - A dead character's sheet says so.
3. **Prompts.** Whatever the playtests show: healing a fallen ally, when to `attempt` versus use a scripted check, casters holding slots.
4. **Cost report.** Model calls and tokens for one session with rules and one without, same story: resident calls, narrator calls, drafting. Written into the sprint report.
5. **Balance.** The thousand-fight simulation at levels 1 and 3, with and without casters; the drafting prompt's advice on encounter size adjusted to match.
6. **One of the carried mechanics**, chosen by what the playtests miss most: short rest with hit dice, or a healing potion as a real item.
7. **Docs and release.** Player-facing notes in `README.md` (what 「第五版规则」 is, that it needs a connected model), wiki brought up to date, `NOTICE` and `licenses/SRD-5.1.txt` final, PR `dev` → `main`.

## Acceptance

- Data tests still pass after the corrections; a note in the report lists every number changed.
- Two full real-model sessions with rules (the shipped story and a drafted one), no stuck fight, no off-menu ruling applied.
- Cost table in the report.
- Full local gate; PR into `dev`, then `dev` → `main`.
