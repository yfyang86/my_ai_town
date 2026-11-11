# Sprint 8 — report

## Shipped

**Characters by the rules**

- `data/rules/5e/classes.json`: fighter, rogue, cleric, wizard — hit die, saving-throw proficiencies, skill list and count, expertise. Every entry cites its SRD section.
- A sheet is validated against the class: skills are class picks plus two from background, at most two outside the class list; expertise only for the rogue and only on proficient skills.
- Hit points and every bonus are derived from the sheet, never typed in.

**Every d20 test outside combat**

| Mechanic | How it works here |
| --- | --- |
| Saving throw | A way marked `save`; proficiency comes from the class |
| Advantage / disadvantage | Conditions on a check — a milestone reached, being at a place, particular seats; two dice, keep high or low; both cancel |
| Contest | The other character responds with their best listed way, rolled by the kernel; ties fail; both rolls logged |
| Passive check | No roll; 10 + best bonus when the beat fires; noticed privately |
| Help | Give up your own attempt, be in the same place, and the other's next attempt has advantage |
| Expertise | Proficiency counted twice |
| Repeatable check | Fail and try again |

Each character's context shows, per open check: DC or opponent, whether it can be retried, any advantage or disadvantage with its reason, and their own bonus for each way.

灯塔之约 now uses a passive check, a contest with a conditional advantage, and a repeatable check with a conditional advantage.

## Tests

| Check | Result |
| --- | --- |
| `cargo clippy --workspace --all-targets -D warnings` | clean |
| `cargo test --workspace` | pass — 148 tests (+2) |
| `pnpm build`, `typecheck`, `format:check` | pass |
| `pnpm test` | 22 pass, 1 known root-only failure |

New regression tests:

- **Rules data and derivation.** Only the four planned classes; each cites the SRD; hit points for a fighter at levels 1 and 3 and a rogue at 1; bonuses for expertise, proficient and non-proficient skills and saves; four kinds of illegal sheet refused.
- **Odds.** 20 000 attempts each at +0 against DC 11: plain ≈ 50 %, advantage ≈ 75 %, disadvantage ≈ 25 %, each within two points.
- **One kernel play** covering: scripts with a dangling advantage condition or a passive contest refused; a passive check noticed by exactly the one character who qualifies, privately, with no dice; a save using class proficiency; a contest where both rolls are logged and the comparison holds; a repeatable check with advantage (two dice, higher kept) that stays open after failing; advantage and disadvantage cancelling; help refused for oneself and for someone ineligible, then applied once and consumed; identical dice after a save round-trip.
- Sprint 7's tests updated for the new fields and still passing.

## Real-model run (DeepSeek `deepseek-flash`)

灯塔之约 with rules on, about five minutes:

| Roll | Result |
| --- | --- |
| 白露 · 说动围观的人 · 游说 d20+4 → 3 = 7 vs DC 13 | 失败 |
| 老莫 · 听出话里的破绽 · passive, DC 14 | noticed, alone |
| 阿渡 · 看懂镇上的账 · 洞悉 d20+1 → 13 = 14 vs DC 13 | 成功 |
| 阿渡 · 当面逼问镇长 · 威吓 d20+2, advantage (16, 10) = 18 — 韩宁 欺瞒 d20+7 → 21 | 失败 |
| 小满 · 当面逼问镇长 · 洞悉 d20+1, advantage (12, 3) = 13 — 韩宁 欺瞒 d20+7 → 15 | 失败 |
| 沈望 · 修好灯芯 · 巧手 d20+0, advantage (14, 20) = 20 vs DC 12 | 成功 |

- All submissions were well-formed.
- The conditional advantages applied when their milestone had been reached.
- The mayor's Deception expertise (+7) held off both confrontations — the rules made her hard to corner, which is what the sheet says.
- Five of seven milestones; the lamp was fixed but nobody declared it lit, so that one was missed. That is the story diverging, not a defect.

## Not done from the plan

- **Sheet panel for spectators.** Sheets still appear only as one line beside each role.
- **Setup-screen character building** (assigning the standard array, choosing skills). Sheets come from the script. This belongs with the editor work in sprint 11.
- **Help was not used by the real model** in this run, so it is verified by test only.

## Notes for sprint 9

- `judge` still only allows or refuses; combat needs it to *resolve* a blow.
- Hit points exist on the sheet but nothing reduces them yet.
- The one-line sheet in each prompt is getting long; combat should send a compact combat line instead.
