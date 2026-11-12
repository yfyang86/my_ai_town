# Sprint 7 — report

## Shipped

**Framework, inert unless a mode uses it**

- `Rules::judge` — a mode may refuse a core action before it is queued, with a reason.
- `GameModeAction.with` — named string parameters; the `mode_action` schema gains `with` only when the open mode asks (`Rules::takes_params`).
- Dice — seeded, saved with the game, replayable; every roll recorded.
- Ruleset slot — `GameModeChoice::Storyline.ruleset`.

**Fifth-edition rules, first mechanic: ability checks**

- Sheets: class, level 1–3, six abilities, skill proficiencies; validated at open.
- Checks on beats: DC, who may try, one or more ways (ability, optional skill).
- `mode_action check` → the kernel rolls d20 + modifier (+ proficiency) against the DC, announces formula and result to everyone, logs the roll, and reaches any milestone tied to that check.
- Each character sees their own bonus for every way, so they can pick their best approach.
- One attempt per seat; a passed check closes for everyone.
- With rules on, shoving / robbing / stealing are refused through `judge`.
- A script with checks plays without a ruleset too: checks and check-milestones simply do not appear.

**Frontend** — sheets beside each role, a roll feed in the story panel, and a 「规则」 switch on 灯塔之约 (three checks, six sheets).

**Licence** — SRD 5.1 attribution in `NOTICE` and `licenses/SRD-5.1.txt`, with the list of what is taken and where.

## Tests

| Check | Result |
| --- | --- |
| `cargo clippy --workspace --all-targets -D warnings` | clean |
| `cargo test --workspace` | pass — 146 tests (+3) |
| `pnpm build`, `typecheck`, `format:check` | pass |
| `pnpm test` | 22 pass, 1 known root-only failure |

New regression tests:

- **Kernel, rules on and off in one test.** Off: no sheet, no checks, no `check` action, no `rules` in the save, check-milestones absent, shoving allowed. On: bad sheets refused (score out of range, unknown skill, wrong count); checks appear only after their beat; each seat sees its own bonus; missing or unoffered way and ineligible seat refused without spending the attempt; a sure pass and a sure fail with the expected formula and totals; second attempt refused; milestone reached and credited; identical rolls from two copies of the same save; shoving refused with the reason, walking still allowed; no trace after closing.
- **Dice.** Same seed, same sequence; continues identically after a save round-trip; 200 000 d20 rolls within 5 % of uniform on every face.
- **Agent.** `mode_action`'s schema has `with` only when the mode asks, and differs in nothing else.
- All previously existing tests pass unchanged.

## Real-model run (DeepSeek `deepseek-flash`)

灯塔之约 with the rules switch on, six characters, about five minutes of wall time:

| Roll | Result |
| --- | --- |
| 白露 · 说动围观的人 · 魅力·游说 · d20+4 → 10 = 14 vs DC 13 | 成功 |
| 阿渡 · 看懂镇上的账 · 感知·洞悉 · d20+1 → 11 = 12 vs DC 13 | 失败 |
| 小满 · 看懂镇上的账 · 智力·调查 · d20+4 → 11 = 15 vs DC 13 | 成功 |
| 沈望 · 修好灯芯 · 智力 · d20+0 → 4 = 4 vs DC 12 | 失败 |
| 小满 · 修好灯芯 · 敏捷·巧手 · d20+4 → 19 = 23 vs DC 12 | 成功 |

- Every `check` call was well-formed, including `with`.
- Characters with a proficient option chose it.
- Failure changed the story: the keeper failed to fix the wick, his apprentice stepped in and succeeded, and described it that way ("师父的手抖了").
- All six milestones were reached; the story closed.

## Change of plan during the sprint

**Creatures will be combatants in the ruleset's state, not bodies on the map.** Every kernel actor is a full resident or an avatar; population, roster, rendering and the host all assume that. Recorded in `Sprints/ruleset-5e/plan.md`.

## Known limits

- The story editor does not draft sheets or checks yet (sprint 11); a milestone of kind "check" is shown read-only there.
- `judge` can only allow or refuse. Resolving a core action *differently* (a shove becoming an attack roll) comes with combat.
- No advantage / disadvantage, saves, or contests yet (sprint 8).
- In the real run one character attempted a check with a way they were not proficient in although a teammate was better — legal, and arguably in character, but worth watching once stakes are higher.
