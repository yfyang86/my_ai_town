# Plan — an optional fifth-edition rule suite

Status: approved; sprints 7–10 are built and merged into `dev` (see each `sprint-N/report.md`), 11 and 12 remain. Six sprints (7–12), same loop as before.

## What "optional rule suite" means here

A **ruleset** is a layer of mechanics — character sheets, dice, checks, combat, magic, rest, progression — that a story can switch on. Without it, a storyline plays exactly as it does today. With it, the same beats and milestones run, but what characters attempt is resolved by rules and dice instead of by declaration.

It is not a new world. The town, the residents, their memory and their free will stay; the ruleset decides *whether things work*.

## Why this is harder than the modes so far

Every mode to date is a **judge**: a state machine that announces, waits for one person's choice, and moves on. Fifth edition needs things the framework does not have:

| Needed | Today |
| --- | --- |
| Per-character numeric state (abilities, HP, AC, slots, conditions) | Modes keep seats; nothing per-actor beyond role |
| Chance, visible and auditable | One seeded generator per mode, used for ordering only |
| Rules that decide the outcome of *ordinary* actions (hit someone, pick a lock, sneak) | A mode cannot gate or alter core actions; it is only told afterwards (`clash`) |
| Rich choices ("cast *X* at level 2 on seat 3") | `mode_action` carries `action`, `target`, `text`, `reason` |
| Opponents who are not residents | Every participant is a resident with a model |
| Someone to adjudicate the unscripted ("I try to talk the guard round") | Storyline has no adjudicator; milestones are self-declared |
| Distance and position in a fight | Residents live on a node graph of locales; only avatars have coordinates |
| Many small turns | Each turn is a model call; a six-person fight is dozens of calls |

So the first sprint is framework, not content.

## Design

### 1. Where it lives

- A **ruleset trait** inside the kernel's game-mode area, separate from `Rules`: `Ruleset` owns sheets, dice, resolution. The storyline mode holds an optional ruleset; other modes may adopt one later.
- `GameModeChoice::Storyline { script, ruleset: Option<RulesetChoice> }`. Absent → today's behaviour, byte-identical saves and prompts (same guarantee and same tests as before).
- Rules **content** is data, not code: `data/rules/5e/` — classes, spells, creatures, equipment, conditions. A second suite later is a second folder plus a `Ruleset` implementation.

### 2. Framework extensions (all default to "no effect")

| Extension | Purpose |
| --- | --- |
| `Rules::judge(world, actor, intent) -> Verdict` — allow / refuse with reason / resolve differently | Lets a ruleset turn a shove or a weapon blow into an attack roll, and refuse actions that the turn order forbids |
| `GameModeAction.with: Option<Map>` | Structured parameters (spell, slot, item, stance). Omitted when empty, so existing wire and prompts do not change |
| Mode-owned actors | Creatures spawned and removed by the mode, driven by a small tactics routine, never by a model |
| Dice service | Seeded, replayable; every roll is recorded with its formula, modifiers, advantage, and result, and published in the view |
| Sheet in context and view | The model sees its own sheet compactly; spectators see everyone's |
| Zones | A fight happens in one locale divided into a handful of named zones (near / far / cover…); range and movement are zone steps, not coordinates |

### 3. Rules scope (first release)

| Area | In | Out (later) |
| --- | --- | --- |
| Characters | Six abilities, proficiency, skills, saves; four classes — fighter, rogue, cleric, wizard; levels 1–3; a few backgrounds | Other classes, subclasses beyond one each, feats, multiclassing |
| Checks | Ability and skill checks, saves, advantage / disadvantage, contests, passive scores, DC ladder | — |
| Combat | Initiative, rounds, action / bonus / reaction / move, attack and damage, criticals, cover, opportunity attacks, death saves, about ten conditions | Grappling detail, mounted, underwater |
| Magic | Slots, cantrips, concentration, ~25 spells across the two casters | Rituals, material components, the long tail of spells |
| Rest and growth | Short and long rest (long rest = a real night's sleep in the world), hit dice, XP and level-up | Downtime |
| Gear | Weapons, armour, AC, a short equipment list, loot as real props in the inventory | Encumbrance, crafting |
| Creatures | ~20 from the open rules, tactics by simple priorities | Legendary and lair actions |

### 4. Who adjudicates

Two layers, built in this order:

1. **Scripted** — the adventure script names its checks: *this door, DC 15 Strength or Dexterity (thieves' tools)*; *this guard, DC 13 Persuasion*. Encounters list creatures and zones. No model needed; fully testable.
2. **Narrator** — an optional model-driven game master for what the script did not foresee. It never rolls and never invents numbers: given what a character attempts, it picks from a closed menu — *which ability or skill, which DC rung (5/10/15/20/25), advantage or not, or "no roll needed"* — and afterwards narrates the result the kernel computed. Its output is validated like every other model output here.

The kernel always rolls. A model can never claim a result.

### 5. Keeping it affordable and watchable

- Combat turns use a short prompt (sheet, zone picture, legal options) rather than the full life prompt.
- Creatures cost no model calls.
- Tempo: fast between scenes, real-time in a fight — as the judge modes already do.
- Legal options are listed for the model each turn, so a wrong choice is rare and a refusal explains itself.

### 6. Authoring

The story editor gains an "adventure" switch: the model also drafts sheets that fit each persona, the checks attached to beats, and encounters. The same *tidy-then-validate* path as today clamps everything to legal values (ability scores from the standard array, spells the class can actually prepare, encounters within a difficulty budget).

### 7. Licensing and naming

- Build only from the **System Reference Document** released under CC BY 4.0. Content outside it — certain iconic creatures, named settings, named characters — must not appear. Each data file records its source section.
- Add the required attribution to `NOTICE` and a licence file under `licenses/`.
- In the product, call it **「第五版规则」/ "5e rules"**; do not use the trademarked name or logo as the feature's name.

## Sprints

| # | Theme | Outcome | Proof |
| --- | --- | --- | --- |
| 7 | Framework | `judge` hook, `with` parameters, mode-owned actors, dice service and roll log, zones, ruleset slot on storyline — all inert by default | Existing 143 tests unchanged; new tests that each extension is a no-op when unused; dice distribution and replay tests |
| 8 | Sheets and checks | Data schema and loader; four classes at levels 1–3; checks, saves, contests; scripted checks on beats; sheet panel; setup screen assigns sheets | A scripted story where milestones are reached by passing checks, deterministic under a seed; save/restore mid-check |
| 9 | Combat | Initiative and turn machine; actions; zones; creatures with tactics; damage mapped onto the kernel's health and death; conditions; core force routed through `judge` | Seeded fights with expected outcomes; a thousand simulated encounters land in the difficulty band the budget predicts |
| 10 | Magic, rest, growth, gear | Slots, spells, concentration; rests tied to real sleep; XP and levels; equipment and loot as props | Caster-vs-martial simulations; a two-day adventure with a long rest in the middle |
| 11 | Narrator and authoring | Model game master with closed-menu rulings; adventure drafting in the editor; tidy + validate | Sloppy-reply tests for rulings and drafts; a full adventure with a real model |
| 12 | Playtest and release | Spectator view (initiative bar, roll feed, sheets), balance pass, cost report, docs | Real-model playthroughs at two party sizes; token cost per fight; wiki and player notes |

## Risks

| Risk | Mitigation |
| --- | --- |
| The built-in fine-tuned model cannot follow structured combat choices | Ruleset stories require a connected model; say so in the setup screen. Do not change the built-in prompt format |
| Cost: a fight is many calls | Short combat prompt, creature turns free, optional "resolve the rest quickly" for one-sided fights |
| Models ignore the rules | Legal options listed each turn; `judge` refuses with a reason; kernel is the only source of dice |
| Rules sprawl | The "In" column above is the contract; anything else needs its own decision |
| Two death systems (kernel health vs. hit points) | One source of truth: hit points drive the kernel's health while a ruleset is on; dropping to 0 is "incapacitated", death saves decide the rest |
| Balance | Simulation tests each sprint, before any real-model run |
| Licence drift | A test that every data entry names an SRD section; review list of excluded names |

## Decisions (product owner, 2026-11-11)

All as recommended:

1. **Rules source** — Dungeons & Dragons fifth edition, SRD 5.1 (CC BY 4.0).
2. **Adjudication** — scripted checks first; the model narrator arrives in sprint 11.
3. **Positioning** — zones, not a grid.
4. **First release** — fighter, rogue, cleric, wizard; levels 1–3.
5. **Relation to storyline** — a layer on the storyline mode.

## Changes to this plan since it was written

- **Creatures are combatants in the ruleset's own state, not bodies on the map** (decided at the start of sprint 7). Every actor in the kernel is a full resident or an avatar — population, roster, rendering and the host all assume it. Giving a goblin a body means touching all of those. In the first release creatures appear in the fight panel (initiative order, zone, hit points) and in what characters are told; a walking sprite is a later item.
- **Sprint 7 also delivers scripted ability checks** with a minimal sheet (abilities, proficiency, skills). The framework extensions need a real mechanic to be tested through the full path, and checks are the smallest one. Sprint 8 keeps classes, saves, contests, advantage, the data files and the sheet UI.
