# Sprint 10 — Magic, rest, growth

## Goal

Casters cast, wounds heal with rest, and characters grow.

## Scope

1. **Spells** — `data/rules/5e/spells.json`, about twenty from the SRD for cleric and wizard at levels 1–3 (cantrips, 1st and 2nd level), each citing its section. Kinds: attack-roll spells, save-or-suffer spells, healing, and a few utility spells that grant advantage or help a check.
2. **Slots and preparation** — slots by class and level; a prepared list on the sheet, validated.
3. **Casting in a fight** — `cast` as a turn action: spell, slot level, target; spell attack bonus and save DC derived; concentration (one at a time, a Constitution save when hurt).
4. **Casting outside a fight** — healing between scenes; a spell may satisfy or give advantage on a scripted check when the script says so.
5. **Rest** — short rest (spend hit dice) and long rest (hit points and slots back). A long rest is a real night's sleep in the world: the ruleset is told when a character wakes after enough sleep.
6. **Growth** — experience from encounters won and milestones reached; level-up to 2 and 3 with hit points, slots and the class features that matter here (rogue sneak attack, fighter second wind and action surge, cleric channel divinity: turn undead).
7. **Loot** — an encounter or a check may yield an item; items are real props in the kernel's inventory; a healing potion can be drunk.

## Carried over from sprint 9

- Character-versus-character fights.
- The conditions spells need: frightened, restrained, blinded, poisoned.

## Acceptance

- Data test: every spell cites the SRD.
- Kernel tests: slot use and refusal when empty; attack and save spells; healing an unconscious ally brings them back; concentration broken by damage; long rest tied to sleep restores; level-up changes derived values.
- Simulation: the same party with and without casters against the three encounter sizes.
- Real-model run with a cleric and a wizard.
- Full local gate; PR into `dev`.
