# Sprint 18 — report

Requested by the owner after the previous Gu Long run reached two of five milestones with no character ever using `declare`: remind characters to declare, then run the story again.

## Change (`crates/kernel/src/game_mode/storyline.rs`)

- **Last call.** Once the last beat has run and less than two game hours remain, each character who still holds a milestone only they can declare is told privately which ones, and woken. The text says that declaring *is* the act when the milestone is a public statement, and not to declare what did not happen.
- **Hint after a success.** After a passed scripted check, or a narrator ruling that succeeded, a character with undeclared milestones gets one private line: if this was one of them, declare it.
- Nothing is declared for anyone; the character still decides. The reminder state is saved (`last_call`), and stories without declare milestones are unaffected.

The first version (one reminder, 60 minutes before the end, wording "only if it is truly done") was tried first and did not help — see run 2.

## Checked

Gate: clippy clean; `cargo test --workspace` 168 pass; `pnpm build`, `typecheck`, `format:check` pass; `pnpm test` 24 pass, 1 known root-only failure. The main story test now also checks that the last call reaches the character who still holds an undeclared milestone and nobody else. The hint after a success has no automated check.

## Real-model runs (DeepSeek `deepseek-flash`, 12 characters, fifth-edition rules, ×8)

| Run | Build | Story | Milestones | `declare` actions | Calls / tokens |
| --- | --- | --- | --- | --- | --- |
| 1 | before this sprint | 清泉丁香会 | 2 of 5 (a check, a beat) | 0 | 488 / 6.2 M |
| 2 | first version of the reminder | 海崖丁香一日 | 1 of 6 (a check) | 0 | 430 / 5.9 M |
| 3 | final version, **same story as run 2** replayed on a reset world | 海崖丁香一日 | 1 of 6 (a declaration) | 2 — one accepted mid-story, one arrived just after the story ended | 440 / 5.9 M |

What the runs show:

- **Run 2:** the reminder was delivered (it is in eight model requests) and answered honestly — the things were not done, so nobody declared. One hour was too late to finish anything, and characters kept using `attempt` (50 times) without connecting a success to a declaration.
- **Run 3:** 韩浪 declared 「账册随船」 at 14:15, right after acting it out. 南宫玉 declared 「南宫封账」 with a full public statement, but the reply landed after the story had ended and was refused.
- **Declaring was not the main reason milestones were missed in run 3:**
  - two check milestones failed on the dice (DC 15, three attempts, all low rolls);
  - the encounter milestone was unwinnable as drafted — two thugs and a bandit against two first-level characters; both went down in three rounds (and came round with 1 hit point);
  - 「南宫封账」 and 「账册随船」 are mutually exclusive by the default prompt's own rubric, so at most one could be reached;
  - 「银雕离镇」: the character went to do it and did not get there.

So the reminder does what was asked — declarations now happen — but milestone completion on this outline did not improve. One run per configuration; the two comparable runs (2 and 3) share a script but not dice or model choices.

## Open

- **Drafted encounters can be far too hard.** Nothing checks an encounter's size against the party drawn into it; the drafting prompt only advises restraint.
- **At ×8 the last stretch is short in real time.** A declaration decided in the last game minutes can miss the end, as 南宫玉's did.
- **The default prompt asks for two mutually exclusive declared milestones**, which caps how many a story can reach.
- With twelve characters, `attempt` dominates play (43–50 per game); each costs a narrator call.
