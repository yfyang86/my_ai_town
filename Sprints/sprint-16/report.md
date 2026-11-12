# Sprint 16 — report

Owner's decision after sprint 15: make 讲究一点 the default story prompt. ("It is like wishes to the LLM, but that is fine.")

## Change

- The **default story prompt** now carries the 讲究一点 blocks between the task description and the output contract: a role (an ensemble playwright), style (concrete habits and objects instead of adjectives; announcements that read like news in town), one example, four no-gos, five rubrics (goals collide, secrets matter to someone, every pair has a reason to talk, beats force a choice, two self-declared milestones that cannot both happen).
- The text lives in `agent::tuning::default_modules`; the 讲究一点 preset in the page is the same text.
- **Unchanged:** the output contract, the character prompt, and anyone's saved tuning. A blank 风格模式 is now the bare prompt without these blocks; 专业模式's "恢复成默认" gives the new default.
- These blocks are requests to the model, not guarantees. Nothing checks that a draft meets the rubrics.

## Checked

Gate: clippy clean; `cargo test --workspace` 160 pass; `pnpm build`, `typecheck`, `format:check` pass; `pnpm test` 22 pass, 1 known root-only failure. The unit test now also pins that the story default starts with the new role and contains the rubrics, and that the character default has nothing extra.

Real model (DeepSeek `deepseek-flash`), one run each:

| Outline | Prompt | Result |
| --- | --- | --- |
| A lighthouse to be sold, 5 characters | new default | valid; 5 beats, 4 milestones (all self-declared) |
| same | bare (the old default) | valid; 7 beats, 5 milestones |
| The owner's wuxia outline, 6 characters, fifth-edition rules | new default | valid; 6 sheets, 6 checks, 1 encounter, no notes |

Lighthouse, side by side:

| | Old default | New default |
| --- | --- | --- |
| Keeper | 「旧海塔最后一位守塔人，把灯看得比镇上任何账目都重。」 | 「总把铜油壶挂在腰上，进灯室先摸三下锁扣，灯室钥匙用旧渔线缠在手腕。」 |
| Keeper's secret | an anonymous letter about the buyer's plans | the main lens cracked in last year's typhoon and she has been hiding it; an inspection would expose it |
| First beat | 「清晨，镇公所外贴出傍晚表决公告与灯塔买卖摘要……」 | 「清早六点四十，罗敏把一把新挂锁套在老灯塔铁门的锁梁上，林砚的铜钥匙还插在旧锁里……」 |

## Limits

- Three runs on two outlines; not measured over many, and not on other models. A weaker model may follow the blocks less well or produce a longer, slower reply.
- The new default made every lighthouse milestone a self-declared one (the old one had a gather milestone). Declared milestones depend on a character choosing to declare, so fewer may be reached in play. Not tested in a full playthrough.
- The default character prompt is unchanged.
