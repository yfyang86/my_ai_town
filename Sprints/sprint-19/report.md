# Sprint 19 — report

Requested by the owner after sprint 18: do all three follow-ups, then run the Gu Long story again.

## Changes

1. **Drafted encounters are sized to their party** (`agent/src/story/rules.rs`, `fit`). Each creature has a weight (the experience it is worth); an encounter's total may be at most 25 per character level drawn into it. Over that, the heaviest foes are removed until it fits; a lone foe that is still too heavy is swapped for the heaviest creature that fits. Every cut leaves a line in `draft.notes`. The drafting prompt now lists the weights and the limit.
2. **The default story prompt asks for mixed milestone kinds** instead of two mutually exclusive self-declared ones: some by time, some by gathering, some declared; declared ones at most half and not in conflict. Same text in `agent::tuning::default_modules` and the page's 讲究一点 preset.
3. **Grace at the end** (`kernel/src/game_mode/storyline.rs`). When the end time arrives and someone present still holds an undeclared milestone, the story waits up to 20 game minutes at normal tempo, tells those characters it is the last chance, and ends as soon as nothing is left to declare.

## Checked

Gate: clippy clean; `cargo test --workspace` 168 pass (on the first full run one upstream host test, `residents_without_a_model_wait_until_one_is_chosen`, failed; it passed three times alone and on a second full run — a timing-dependent test, not touched here); `pnpm build`, `typecheck`, `format:check` pass; `pnpm test` 24 pass, 1 known root-only failure.

- Drafting test: an encounter of a giant rat and two thugs against two first-level characters loses both thugs, with the note.
- Story test: at the end time the character holding an undeclared milestone is told it is the last chance and the story is still open at normal tempo; it ends after the wait with that milestone missed.
- The reworded rubric has no automated check beyond the existing default-template test.

## Real-model run (DeepSeek `deepseek-flash`, 12 characters, fifth-edition rules, ×8) — 「丁香镇一日」

Run with the cap at 40 per level (see below).

| | Sprint 18, final run | This run |
| --- | --- | --- |
| Milestones | 1 of 6 | **4 of 5** |
| Kinds drafted | 3 declared (two mutually exclusive), 2 checks, 1 encounter | 1 check, 1 gathering, 1 beat, 2 declared (compatible) |
| Declarations | 2, one refused for arriving after the end | 1 — made **19 minutes after the end time, inside the grace period**, and counted |
| Fight | two thugs and a bandit against 2 characters: lost | two bandits and a thug against 4 characters: lost |
| Calls / tokens | 440 / 5.9 M | 466 / 6.9 M |

- Reached: the check, the gathering, the beat, and 银雕's declaration. Missed: 「名册交刑部」 (declared kind; nobody declared it).
- **The fight was within the cap as first set (150 against a budget of 160) and was still lost**, all four down, nobody dead. The drafted party was an unarmed rogue, a rogue with a short sword, a fighter with a dagger and an unarmoured cleric; five turns timed out at ×8; the thug attacks twice for 32 hit points. The cap's first value came from a simulation of a well-armed party that never misses a turn.
- **After the run the cap was lowered from 40 to 25 per level.** With that, this encounter would have been cut to the two bandits and a note. **The lower value has not been run with a real model.**

One run; the story differs from sprint 18's, so the comparison shows direction, not size.

## Open

- The encounter cap is a single number tuned on two real fights. Drafted gear is not considered: a party of unarmed characters is weighed like a party of fighters.
- Fights at ×8 with twelve residents still lose turns to the 15-minute turn limit.
- A declared milestone still needs its owner to act; one was missed in this run.
