# Sprint 9 — report

## Shipped

**Combat by the rules, inside a story.**

- A beat can start an **encounter**: zones in a line, creatures, the characters drawn in.
- **Initiative and turns.** One combatant at a time; creatures act instantly by simple tactics and cost no model calls; a character is woken on their turn with the situation and their options.
- **On a turn:** step to an adjacent zone, then attack, dodge, disengage, dash, help, flee or pass.
- **Attacks:** d20 + bonus vs. armour class, natural 1 and 20, critical hits, advantage and disadvantage from prone / dodging / help / shooting with an enemy beside you; opportunity attacks when leaving a zone.
- **Gear on the sheet:** weapon, armour, shield → attack bonus, damage and AC, all derived.
- **0 hit points:** unconscious, death saves each turn; a creature at 0 is out.
- **Consequences:** hit points persist through the story; a character who dies in a fight dies in the world.
- **Data:** ten creatures, nine weapons, four armours, each citing its SRD section.
- **Frontend:** a fight panel — zones, who is where, hit points and states, whose turn.
- 灯塔之约 gets a small fight at the lighthouse steps.

The combat engine does not touch the world; the storyline mode announces what it returns. Fights are therefore simulated by the thousand in a unit test.

## Tests

| Check | Result |
| --- | --- |
| `cargo clippy --workspace --all-targets -D warnings` | clean |
| `cargo test --workspace` | pass — 152 tests (+4) |
| `pnpm build`, `typecheck`, `format:check` | pass |
| `pnpm test` | 22 pass, 1 known root-only failure |

New regression tests:

- **Data.** Every creature, weapon and armour cites the SRD; unknown creatures, bad zones and empty encounters are refused.
- **Turn rules.** Out-of-reach attack refused without rolling or spending the turn; ranged reaches; opportunity attack on walking away, once per reaction, not after disengaging; dodge gives attackers disadvantage; help gives one attack advantage; cannot flee beside an enemy; death saves always end in stable, dead or revived; same seed → same fight; a fight saved mid-way finishes identically.
- **A thousand fights at three sizes.** Four first-level characters win more than 95 % against two bandits, between 15 % and 85 % against four bandits and a thug, and under 5 % against eight thugs. Every fight ends.
- **In a story.** No ruleset → no fight and no fight milestone. With it: the fight starts on its beat; a character outside it cannot act in it; out-of-turn and out-of-reach are refused; a missed deadline becomes a dodge; later beats wait; a mid-fight save restores to the same fight; winning reaches the milestone; the finished fight clears at the next beat. Over forty seeds of a hopeless fight, characters who fail their death saves are dead in the kernel and the others come round with 1 hit point.

## Real-model runs (DeepSeek `deepseek-flash`)

**Run 1 — exposed a defect.** The keeper acted twice, then missed four turns in a row and went down. The deadline was 5 game minutes; a model needs longer than that at this speed.

**Fix:** 15 game minutes, the same order as a scene turn.

**Run 2 — clean.** Four rounds:

- 小满 was bitten twice and fell unconscious, then rolled death saves (one success, one failure) while the fight went on.
- 沈望 missed twice, then dropped both dogs with his staff.
- Every character turn was a well-formed `attack`; no timeouts.
- 小满 came round with 1 hit point; 沈望 then lit the lamp.

## Defects found and fixed

1. **Combat turn deadline too short for a real model** (above).
2. **A revived character was still shown as prone** after the fight. Cleared on revival.
3. Two test-writing mistakes (a fight too easy to exercise anything; a patch script matching an earlier test's line) — caught before commit.

## Not done from the plan

| Planned | State |
| --- | --- |
| Violence between characters routed into combat | **Not done.** `judge` still refuses it while a ruleset is on. Character-versus-character fights need sides other than "party vs. creatures" |
| Eight conditions | Three: prone, dodging, unconscious. The creatures shipped cause no others |
| Hit points driving the kernel's health meter | **Only death.** Writing hit points into the kernel's health would make a hurt character "incapacitated" there and unable to take their turn |
| Hide, use an object | Not included |
| Token cost per round | Not measured |

## Caveats

- **Creature and weapon numbers were entered from memory of the SRD,** not copied from the document, which is not available in this environment. They should be checked against the SRD text before release.
- Characters are treated as proficient with whatever weapon and armour their sheet lists.
- A two-dog fight can drop a first-level character with 9 hit points in one round. That is the system at level 1, but the demo is deadlier than it looks.
