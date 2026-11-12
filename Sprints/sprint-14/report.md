# Sprint 14 — report

Requested by the owner: fix the half-finished start so it can resume after a reload (found in sprint 13).

## What was wrong

The setup screen of a scripted or drafted town opened only while the world had no worldview. Saving the worldview is its first step, so after a reload mid-start the screen never came back: the town was left with some residents and no game mode, and nothing in the page could finish it.

A second fault sat next to it: if a join had succeeded in the world but its reply was missed (the 15-second timeout of sprint 13), clicking 「开始这一局」 again asked the same resident to join a second time.

## Fix

- **Progress is remembered.** The chosen settings (players, models, rules, edited personas, options) and the finished steps are kept in the browser (`localStorage`, one entry keyed by world id), written when the start begins and after each step.
- **Who has joined comes from the town**, not from the page's memory of replies: every start first reads the population and matches residents to roster slots by template.
- **The screen reopens.** When a town with a setup has a worldview but no open mode, and the remembered progress belongs to this world, the setup screen opens again with 「上次开局做到一半…」; one click continues from the step that stopped.
- The entry is cleared when the start finishes or is skipped. A town whose mode was closed on purpose later does not get the screen back.

## Checked

Browser, mock model, 灯塔之约 with 「第五版规则」 chosen:

| Step | Result |
| --- | --- |
| Start, reload while resident 3 is moving in | setup screen reopens with the resume note |
| Chosen rule after reload | still 第五版规则 |
| Click start again | finishes; six residents, none twice; the story opens with character sheets |
| Reload after finishing | setup screen stays closed; remembered progress is gone |

Gate: clippy clean; `cargo test --workspace` 159 pass; `pnpm build`, `typecheck`, `format:check` pass; `pnpm test` 22 pass, 1 known root-only failure.

No automated test was added: the logic lives in the page's setup flow, which has no test harness; the check above was run by script in a real browser.

## Limits

- The progress is per browser. Reopening the town in a different browser or after clearing site data does not resume (the town is then in the same state as before this fix).
- A step that finished in the world while the page died before noting it is repeated on resume. Joining is safe (see above); saving the worldview, needs, climate and limits again is harmless; placing a scripted town's opening props twice is possible in that narrow window.
- Not re-run with twelve characters and a real model.
