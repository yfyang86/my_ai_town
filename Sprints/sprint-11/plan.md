# Sprint 11 — Narrator and adventure drafting

## Goal

A story with rules can be drafted from an outline, and what characters attempt off-script is ruled on by a model narrator that can only choose from a closed menu.

## Scope

1. **Adventure drafting** — the story editor's draft, when 「第五版规则」 is chosen, also produces:
   - a character sheet per role (class, abilities by standard array, skills, gear, prepared spells);
   - scripted checks on beats (DC, ways, who may try, contests, passive checks);
   - encounters on beats (zones, creatures from the shipped list, who is drawn in);
   - check and encounter milestones.

   The draft goes through the same tolerant `tidy` and the kernel's own validation; whatever does not validate is dropped with a note rather than failing the whole draft.
2. **Editing in the page** — the review screen shows sheets, checks and encounters and lets them be edited or removed before play.
3. **Narrator** — a character may `attempt` something the script has no check for. The host asks the model for a ruling restricted to a closed menu:
   - which ability and skill (from the eighteen), or "no roll needed", or "not possible";
   - a DC from the fixed ladder (5, 10, 15, 20, 25);
   - advantage, disadvantage or neither.

   The kernel validates the ruling, rolls, and records it like a scripted check. A ruling outside the menu is discarded and the attempt refused with a reason. The narrator never rolls and never decides an outcome.
4. **Limits** — attempts per character per game hour, so a model cannot spam rulings; narrator calls counted in the cost report.

## Carried over from sprint 10

- Short rest and hit dice.
- Loot and a healing potion.
- Spells that help a scripted check.
- Action surge, channel divinity.
- Conditions: frightened, restrained, blinded, poisoned.
- Spell and creature numbers were entered from memory of the SRD and need checking against the text.

These are candidates for sprint 12 unless drafting needs one of them.

## Acceptance

- Agent tests: a drafted adventure with sheets, checks and encounters survives `tidy` and validates; a draft with an unknown creature, class or spell loses only that part.
- Kernel tests: a legal ruling resolves like a scripted check; an illegal one is refused without a roll; the attempt limit holds; with no ruleset `attempt` does not exist.
- Both managers serve the extended draft.
- Real-model run: draft an adventure from a three-line outline with rules on, play it, and see at least one narrator ruling.
- Full local gate; PR into `dev`.
