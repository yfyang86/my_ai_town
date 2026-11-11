# Sprint 6 — report

## Shipped

- **Dreams are visible.** `ResidentAgentView.dreams` carries the five most recent dreams; the resident info panel shows a 「梦」 section (when woken, how many things were understood and let go, the dream itself). Insights appear under 「记着的事」 labelled 梦里想明白的.
- **Dream material prefers substance.** Mode instructions ("your turn… use `mode_action`") now score below routine sightings, so they no longer crowd out what people said.
- **Stories are less chatty.** At most half the beats keep a scene (the ones with the most speakers); the prompt says so too.
- **Dream insights describe feelings, not meters.** The dream prompt now forbids copying need values and stock counts, as the reflection prompt already did.
- **Player-facing notes** in `README.md`; wiki brought up to date.

## Tests

| Check | Result |
| --- | --- |
| `cargo clippy --workspace --all-targets -D warnings` | clean |
| `cargo test --workspace` | pass — 143 tests |
| `pnpm build`, `typecheck`, `format:check` | pass |
| `pnpm test` | 22 pass, 1 known root-only failure |

Extended regression tests: the dream view after a dream; procedural notices rank below routine memories; surplus scenes are dropped keeping the largest; the wire sample with dreams compiles against `protocol.ts`.

## Live overnight run (DeepSeek `deepseek-flash`)

One resident in the default town, no game mode, needs on, nobody steering:

- Lived four game days in about nine minutes of wall time.
- **Slept three nights and dreamt three times**, each from 16 memories.
- **12 insights**, all tied to real events — e.g. "the canteen has no food after hours; twice I came back empty-handed, so go at mealtimes and fall back on dry rations", "baking bread is two steps, dough then oven", "not meeting anyone does not mean nobody is in charge — I still want to know who runs the canteen".
- 5 memories let go by the model; 59 of 219 records superseded in total.
- Each dream recombined the days' places and worries (the empty canteen, the notice board, the unanswered question).

This closes the last "not verified" item: the live path from sleep to dream works with a real model.

## Found in the live run

1. **Insights quoted meters** ("water back by thirty-five points"). Fixed in the prompt this sprint; not re-run afterwards.
2. **Ordinary reflection was cut off five times** with this model: the `reflect` arguments ended mid-JSON and the batch was deferred. The default output limit is 2048 tokens and a reasoning model spends part of it thinking. This predates these sprints and is a setting (agent limits), so it was not changed; it is recorded in the wiki. Dreams were not affected — their output is short.

## State of the three features

| Feature | Done | Limits |
| --- | --- | --- |
| 1. Optional, configurable scripted events | 执剑人之夜: times, deadlines, five switchable phases, editable cast; scripted towns can be hidden | Rules for 狼人杀 and 大逃杀 are not exposed (mechanism is generic) |
| 2. Storyline editor | Outline → model → reviewable draft → town; `storyline` mode with beats, scenes, three kinds of milestone; verified with a real model end to end | No conditional beats; looks are borrowed, not designed; review screen is forms, not a timeline |
| 3. Agent memory | SQLite FTS5 word index with Chinese tokenisation; relevance + recency + importance; dreaming with insights, letting go, and a kept dream; verified live with a real model | Database is in-memory and rebuilt on start by design; no embeddings |

## What I would do next

1. Raise or adapt `max_output_tokens` for reasoning models — it is costing reflections today.
2. Conditional beats ("only if milestone X was reached") so a story can branch.
3. Expose rules for 狼人杀 and 大逃杀 through the same `PrefabRule` rows.
4. Show the dream in the diary at the start of its day.
5. Enable GitHub Actions on the fork, or decide what to do about `protect-crates`, before `dev` goes to `main`.

## Not done from the plan

- **Diary entry for the dream.** The dream is in the info panel only.
- **Browser check of the 「梦」 section.** The view was verified through the protocol (the live run printed `dreams` from the roster) and the type check, not by looking at the rendered panel.
