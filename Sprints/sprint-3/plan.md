# Sprint 3 — Storyline mode (runtime)

## Goal

A game mode whose whole script is data: a cast with roles and private goals, beats that fire on schedule, milestones that are tracked and shown. Agents are told the story and nudged at each beat, but decide for themselves — the ending may diverge.

Sprint 4 builds the editor that writes this data from a story outline; this sprint makes the data playable.

## The script (`StorylineScript`, carried in `GameModeChoice::Storyline`)

| Part | Fields | Meaning |
| --- | --- | --- |
| Header | `title`, `premise` | Shown to every participant |
| `cast[]` | `seat`, `role`, `goal`, `secret?` | By move-in order. `goal` and `secret` are private to that seat |
| `beats[]` | `id`, `title`, `day`, `minute`, `place?`, `who[]`, `announce`, `brief?`, `scene?` | At that time: announce publicly, brief the named seats privately, optionally run a scene |
| `scene` | `speakers[]`, `minutes`, `chars` | The named seats speak in turn at `place`, one at a time, each with a deadline |
| `milestones[]` | `id`, `title`, `detail`, `when` | Tracked outcomes |
| `when` | `beat` · `gather` · `declare` | Reached when a beat has run; when enough of the named seats are at a place; or when a named seat declares it with evidence |
| Ending | `ends_day`, `ends_minute`, `epilogue?` | The story closes; milestones reached and missed are summed up |

Limits (validated at open, refused with a reason): 2–12 seats, ≤ 24 beats, ≤ 12 milestones, text lengths capped, beats in time order, ids unique, references resolve.

## Behaviour

- **Soft, not scripted.** A beat announces and briefs; nobody is moved. A scene records whether each speaker was at the place (`away`), as 执剑人之夜 does.
- **`mode_action`**: `speak` / `pass` during a scene turn; `declare` with `target` = milestone id and `text` = what happened.
- **Context for the model**: title and premise, my role, my goal and secret, what is happening now and what comes next, the milestone list with status, today's scene lines, available actions.
- **View for spectators**: timeline of beats (done / current / upcoming), milestone checklist, cast with roles, the current scene.
- **Tempo**: 3× between beats, 1× during a scene.
- **Losses**: a seat who died or left is skipped; milestones depending only on them are marked missed at the end.

## Work

1. `crates/protocol/src/game_mode/storyline.rs`; register in the three enums.
2. `crates/kernel/src/game_mode/storyline.rs` implementing `Rules`; register.
3. Host: seed roll and receipt.
4. Wire samples; `protocol.ts`.
5. `frontend/town-2d/src/map/hd2d/gameModes/storyline.ts` (+ css): banner, timeline, milestones, scene card.
6. `frontend/town-shared/src/audio/gameModes/storyline.ts`.
7. One hand-written demo prefab so the mode can be played before the editor exists.

## Acceptance

- Kernel test plays a full script through contexts and actions: beats fire on time, a scene runs with one timeout, all three milestone kinds resolve, mid-scene save/restore, ending with one milestone missed, `no_trace` after closing. Invalid scripts are refused.
- Wire sample compiles against `protocol.ts`.
- Browser run with the mock model: demo prefab starts, timeline and milestones render.
- Full local gate green; PR merged into `dev`.
