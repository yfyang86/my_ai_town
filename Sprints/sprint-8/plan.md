# Sprint 8 — Characters and the full check system

## Goal

Characters built by the rules rather than typed in by hand, and every kind of d20 test outside combat.

## Scope

1. **Rules data** — `data/rules/5e/`: the four classes (hit die, saving-throw proficiencies, skill choices, level 1–3 features that affect checks), backgrounds from the SRD, the skill list. Each entry names its SRD section; a test enforces it.
2. **Character building** — from class + background + ability assignment (standard array 15/14/13/12/10/8): derive proficiency bonus, saves, skills, hit points, passive Perception. The setup screen shows and lets the player adjust; `tidy`-style validation refuses illegal builds.
3. **Tests beyond the basic check**
   - Saving throws.
   - Advantage and disadvantage (roll two, keep one), with the reason recorded.
   - Contests (two characters roll against each other).
   - Passive checks (no roll: 10 + modifier).
   - Help: one character gives another advantage on a check.
   - Expertise for the rogue.
4. **Script support** — a check may be a save or a contest, may grant or impose advantage by condition (a milestone reached, a place), and may be repeatable.
5. **Sheet panel** — a readable character sheet for spectators.

## Acceptance

- Kernel tests: derived values for each class at each level match the rules; advantage picks the higher die and disadvantage the lower under a fixed seed; contests resolve ties the rules' way; help consumes the helper's turn in a scene and applies once; statistical test that advantage shifts the success rate as expected.
- Data test: every entry cites an SRD section; no entry outside the allowed list.
- Wire samples and `protocol.ts`.
- Real-model run of a short story using a save, a contest and help.
- Full local gate; PR into `dev`.
