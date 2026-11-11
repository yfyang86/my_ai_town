# Sprint 9 — Combat

## Goal

A fight that follows the rules: initiative, turns, attacks and damage, death saves — between the characters and against creatures the story brings in.

## Scope

1. **Encounters in the script** — a beat may start an encounter: a place, a few named zones (e.g. 门口 / 屋里 / 楼梯), which creatures are there, who of the cast is drawn in.
2. **Creatures** — combatants in the ruleset's state, not bodies on the map. `data/rules/5e/creatures.json`: about ten from the SRD (bandit, guard, wolf, giant rat, skeleton, zombie, goblin, cultist, thug, mastiff), each citing its section. Tactics by simple priorities; no model calls.
3. **Turn machine** — initiative (Dexterity check), rounds, one combatant at a time with a deadline, like the judges in the other modes.
4. **On a turn** — move one zone; one action: attack, dash, dodge, disengage, help, hide, use an object; a short line of speech. Reactions: opportunity attack when someone leaves a zone you threaten.
5. **Attacks and damage** — attack roll vs. armour class, critical on a natural 20, damage dice, weapon list (a short one from the SRD), light/medium/heavy armour and shields giving AC.
6. **Hit points and death** — damage reduces hit points; at 0 a character is unconscious and makes death saves on their turns; three failures is death, three successes is stable. Hit points drive the kernel's health while the ruleset is on, so the rest of the world sees a hurt person as hurt.
7. **Conditions** — prone, grappled, restrained, frightened, poisoned, unconscious, invisible, blinded: the ones the creatures and actions above can cause.
8. **Core violence routed through the rules** — with the ruleset on, a shove or a blow outside an encounter starts one (the `judge` hook gains a "resolve differently" outcome).
9. **Frontend** — initiative bar, zone picture, hit points, the roll feed.

## Acceptance

- Kernel tests: seeded fights with expected outcomes; opportunity attacks; death saves to death and to stable; conditions applied and expiring; a shove outside an encounter starting one; save/restore mid-round.
- Simulation: a thousand runs of three encounter sizes land within the difficulty band the encounter budget predicts.
- Data test: every creature and weapon cites the SRD.
- Real-model run: a party of four against a small encounter, with token cost per round reported.
- Full local gate; PR into `dev`.

## Risks

- Cost and pace: combat prompts must be short. A combat turn sends a combat line (hit points, AC, zone, legal options), not the life prompt's full context.
- Two health systems: one direction only — rules write the kernel's health, never the reverse, while a ruleset is on.
