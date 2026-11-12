# Sprint 11 — report

## Shipped

**A story with rules can be drafted from an outline, and off-script attempts are ruled on by a model narrator restricted to a closed menu.**

### Narrator

- `mode_action attempt` (rules on, not during a fight or a scene): a character says what they try and how.
- The kernel stores the attempt; the host asks the asking resident's own model once; the answer comes back through the kernel.
- **Closed menu:** `roll` — one of six abilities, optionally one of eighteen skills, DC 5 / 10 / 15 / 20 / 25, advantage / disadvantage / neither — or `auto`, or `impossible`; plus a one-line note.
- **The kernel validates and rolls.** Anything off the menu is discarded without a roll. The narrator never rolls and never decides success.
- Limits: one pending attempt per character, three per game hour; an unanswered attempt lapses after 30 game minutes. Pending attempts survive a save.
- New mode hooks `rulings` / `rule`; `World::game_mode_rulings` / `game_mode_rule`; `host::narrator` (one worker thread); `agent::narrator` (prompt and parser).

### Adventure drafting

- The story editor has a 「第五版规则」 switch. With it, the model also writes a **character sheet per role**, **checks** and **encounters** on beats, and **check / encounter milestones**.
- The prompt lists every class, skill, weapon, armour, spell and creature the kernel knows, taken from the kernel itself.
- **Tolerant in, strict out.** Sheets are normalised (unknown class → fighter; abilities re-dealt as the standard array in the model's order; skills, gear and spells filtered to what the class may have; defaults when missing). Each check and encounter is probed against the kernel on its own; what fails is dropped, with a line in `draft.notes`, instead of failing the draft.
- **Review screen:** sheet fields on each character, checks and encounter on each beat with delete, the notes, and 「不用规则」.
- A drafted town's setup screen gets a 「规则」 option (default on, can be switched to 不掷骰).
- Both managers serve it: the request field `rules: "srd5"` passes through unchanged.

### Carried from sprint 10

- The fight turn brief now says how to step and act in one action, and that `dash` uses the whole turn.

## Tests

| Check | Result |
| --- | --- |
| `cargo clippy --workspace --all-targets -D warnings` | clean |
| `cargo test --workspace` | pass — 158 tests (+3) |
| `pnpm build`, `typecheck`, `format:check` | pass |
| `pnpm test` | 22 pass, 1 known root-only failure |

New regression tests:

- **Narrator in the kernel.** No ruleset → no `attempt`. An attempt needs text; a second one while the first is pending is refused; a DC off the ladder or an unknown skill is discarded without a roll; a legal ruling rolls with the character's own bonus (two dice with advantage) and cannot be applied twice; `auto` and `impossible` roll nothing; the fourth attempt in an hour is refused; pending attempts survive a reload and lapse after 30 minutes.
- **Narrator prompt and parser.** The prompt lists the menu; a reply wrapped in prose and code fences, with capitals and a string DC, is read; an unknown kind is refused; a non-roll ruling carries no ability or DC.
- **Drafting.** A draft with a wizard holding a cleric spell, an unknown class, a missing sheet, a vague check, an unknown creature and dangling milestones keeps everything sound and drops only the rest, each with a note; re-tidying the result changes nothing; without the switch none of it is kept; the prompt lists the kernel's vocabulary only when the switch is on.

## Real-model run (DeepSeek `deepseek-flash`)

**Drafting.** A three-sentence outline (a stolen medicine chest at a mountain post) with four characters and rules on produced, in one call:

- four valid sheets — fighter, wizard, rogue, cleric — with sensible skills, gear and spells;
- four checks on four beats, each with two or three ways;
- one encounter (a wolf against two of the party);
- five milestones: two by check, one by encounter, two by declaration.

Nothing needed correcting (`notes` empty).

**Play.** The town was built and played to the end in one sitting:

- **Nine narrator rulings**, all on the menu and all plausible (Survival to read tracks, Investigation to compare marks, Intimidation to silence a crowd, Persuasion with advantage when a character used a detail only an insider would know). Three succeeded, six failed.
- **Six scripted check attempts**, three passed.
- **The fight:** the rogue shot the wolf, the wolf missed, the wizard finished it with magic missile. 25 experience each.
- **Four of five milestones** reached.

Observed and adjusted afterwards:

- Every ruling used DC 15. The prompt now tells the narrator to vary the DC with the difficulty of the thing itself. Not re-run.
- An attempt's full text (up to 200 characters) became the roll's title; it is now cut to 24 characters in the roll log.

## Defects found and fixed

1. **Drafting dropped every check and encounter** in the first test: the probe script still carried the milestones that pointed at items not yet attached. The probe now leaves those milestones out.
2. Test-run only: a quoting error in the browser script.

## Not done from the plan

| Planned | State |
| --- | --- |
| Narrator calls in the cost report | Not counted separately; they appear as model events in the log. Sprint 12 |
| Editing a check's ways, or an encounter's zones and creatures, in the page | Title and DC only; the rest can be deleted, not edited |
| Short rest, loot, further conditions, action surge, channel divinity | Still open; sprint 12 decides which |
| Checking rule numbers against the SRD text | Still open |

## Known limits

- The narrator is the asking resident's own model, so a model can in principle favour itself. The menu, the kernel's validation and the hourly limit bound the damage; a separate narrator model is a setting for later.
- In mock-model mode residents never call `attempt`, so the host path of the narrator is covered by the real-model run only.
- Drafted sheets always use the standard array; scores cannot be edited by hand.

## Next

Sprint 12 — playtest, balance, cost, release: `Sprints/sprint-12/plan.md`.
