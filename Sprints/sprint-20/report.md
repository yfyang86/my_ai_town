# Sprint 20 — Second upstream cherry-pick

## What was picked

Everything on `mewamew/my_ai_town:main` after `b6e513d` (where sprint 17 stopped), five commits, each with `-x`:

| Upstream | What it is |
| --- | --- |
| `d4e99fe` | Frostlight (霜灯镇 · 风暴前夜): a ring-town survival mode with its own prefab, looks and music; music controls in the menu |
| `7e7513a`, `78da48a` | Frostlight player guide and saving notes in the README |
| `83ec498` | Ask the browser for the high-performance GPU |
| `5ddc1a3` | Save protection (shutdown path, failed commits roll back and retry), interrupted game setup recovered by the world, Android signing script |

The merge commit `203afb9` only joins these and was not picked.

## How the two sides were joined

- **Mode framework.** `Rules` now carries both sets of hooks: ours (`judge`, `takes_params`, `rulings`, `rule`) and upstream's (`control`, `pending_group`). `GameMode` lists Frostlight next to Storyline; protocol enums, wire samples, looks, music and the prefab list register both.
- **`GameModeAction.with`.** Upstream's new literals got our `with` field (empty).
- **Prefab options.** Upstream's Frostlight option used an `apply` function; ours are data (`field`, `number`), so it was rewritten in that form.
- **Game setup.** Sprint 14 kept a start's progress in `localStorage`. Upstream solved the same problem properly: the plan and its cursor live in the world and are committed with each step. Ours was removed and upstream's taken. What we had on top was folded into the frozen plan: rule edits and the data-driven options go into the mode choice, persona edits into the join steps. `PrefabSetupSelection` got two optional lists, `rules` and `personas`, so a restored screen shows the edits again; they are absent when nothing was edited, so upstream's saves and samples are byte-identical.
- **`randomized_game_mode`** (host) learned the Swordholder and Storyline choices, which upstream does not have.
- **Onboarding guide.** Our model-name list used `new Option`, which upstream's new page test (a small fake DOM) does not have; it now builds the element with `el('option')`.

## Verification

- `cargo fmt`, `cargo clippy --workspace --all-targets -D warnings`: clean.
- `cargo test --workspace`: all pass (host 39, kernel 79, agent 34 + 8, manager 3, protocol 4 …).
- `pnpm build`, `typecheck`, `format:check`: clean. `pnpm test`: 45 of 46; the one failure is `scripts/tests/readonly.test.mjs`, which cannot pass as root (unchanged from before).
- Browser, mock model: 灯塔之约 with 「第五版规则」 chosen on the setup screen — all 10 plan steps committed, six residents moved in, the saved mode carries the SRD sheets, and a reload afterwards does not show the setup screen again. A reload in the middle of a start was not caught by hand (the mock finishes too fast); that path is covered by upstream's `prefab-setup.test.mjs` and the host's `world_settings` tests.

## Not done

- No real-model run of Frostlight.
