# Sprint 3 — report

## Shipped

**故事模式 (`storyline`) — a game mode whose script is data.**

- Protocol: `StorylineScript` (cast, beats, scenes, milestones, ending) in `GameModeChoice::Storyline`; `StorylineView` for spectators; `StorylineContext` for each resident's model.
- Kernel: `crates/kernel/src/game_mode/storyline.rs` — validates the script at open, fires beats on time, runs scenes one speaker at a time with deadlines, tracks milestones three ways (`beat`, `gather`, `declare`), closes with a summary. Registered like any other mode; nothing in the core knows about it.
- Host: seed roll and receipt.
- Frontend: on-map layer (banner, cast, timeline, milestone checklist, scene card, epilogue), music mapping, `protocol.ts` mirror.
- Demo script: **灯塔之约** — six characters, five beats, five milestones, one day. Listed under 剧本.

Agents are told what happens and what they want; they are never moved or forced. A secret milestone is visible only to whoever can declare it (and to spectators).

## Tests

| Check | Result |
| --- | --- |
| `cargo clippy --workspace --all-targets -D warnings` | clean |
| `cargo test --workspace` | pass — 140 tests (+1) |
| `pnpm build`, `typecheck`, `format:check` | pass |
| `pnpm test` | 22 pass, 1 known root-only failure |

New regression tests:

- Kernel, one full play: three invalid scripts refused (beats out of order, dangling milestone reference, more roles than residents); private goal and secret reach only their seat; beat fires and briefs only the named seats; a scene with a refused out-of-turn speaker, one timeout, one over-long line clipped, save/restore mid-scene; a secret `declare` that others never hear; a `gather` reached after two residents walk to the place; the close with one milestone missed; no trace after closing.
- Wire: storyline roster and `set_game_mode` samples compiled against `protocol.ts`.

End-to-end in a real browser with the mock model: built 灯塔之约, started it from the setup screen, ran at 60×. All five beats fired at 09:00 / 12:00 / 15:00 / 18:00 / 20:00, both scenes cycled through their speakers, the story closed at 21:00, the save shows `ended: true`. (The mock model does not speak in scenes, so turns advanced by deadline; speaking is covered by the kernel test.)

## Defects found and fixed during the sprint

1. **Closing announcement missing from my test's event list** — `World::step` returns its events; I was reading `take_events`. Test fixed; noted in the wiki.
2. **Demo world never advanced** — a fresh data root has no model, so nobody could think and the host kept the clock stopped. Not a storyline defect; the test setup now saves a provider first. Noted in the wiki because it will bite anyone testing a prefab.

## Known limits

- `gather` needs the place on the map; a script naming a place the map lacks can never reach that milestone. The editor (sprint 4) must generate the map from the script's places.
- No per-beat conditions ("only if milestone X was reached"). Branching would be the natural next step after the editor.
- The on-map layer hides the cast and board on phones; only the banner and scene card show.
