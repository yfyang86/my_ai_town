# Game modes

A game mode is a rule set with its own goal, timetable and ending, layered on the ordinary town. At most one is open per world. It is saved with the world as `{"<id>": {...}}`.

## The contract (`crates/kernel/src/game_mode/mod.rs`)

```rust
trait Rules {
    const ID: &'static str;
    fn boundary(&self, world: &World) -> f64;          // minutes until the mode next needs to act
    fn advance(&mut self, world: &mut World);          // settle at world.minutes()
    fn rates(&self, …) {}                              // extra bodily cost for an actor
    fn view(&self, world: &World) -> GameModeView;     // broadcast with the roster
    fn context(&self, world: &World, actor: &str) -> Option<GameModeContext>; // the model's column
    fn tempo(&self, world: &World) -> f64 { 1.0 }      // speed multiplier, clamped 0.25–8
    fn tool(&self, world: &World, actor: &str) -> Option<String> { None }     // mode_action description
    fn act(&mut self, world, actor, action: &GameModeAction) -> Result<String>;
    fn clash(&mut self, world, actor, target, text) {} // someone used force on someone
}
```

The core calls these and nothing else. The mode is taken out of the world while it runs, so it may read and write the world freely.

| Hook | Called from |
| --- | --- |
| `boundary`, `advance` | `kernel/src/simulation.rs` |
| `rates` | `kernel/src/physiology/` |
| `view` | `kernel/src/projection.rs` (roster) |
| `context` | `agent/src/observation.rs` — added under `GameModeContext::key()` |
| `tool`, `act` | `agent/src/tools.rs` — the `mode_action` tool |
| `tempo` | `host/src/runtime.rs` |
| `clash` | `kernel/src/force.rs`, prop harm in physiology |

What a mode does to the world today: write journal facts, send notices, interrupt actions, log (including speech), place and remove props, hurt or remove residents. It does not teleport anyone and has no generic way to forbid core actions.

## Wire shapes (`crates/protocol/src/game_mode/`)

| Type | Role |
| --- | --- |
| `GameModeChoice` | Client → host: which mode and its settings. Tagged by `kind`. Absent = close |
| `GameModeView` | Host → clients with every roster. Tagged by `kind` |
| `GameModeContext` | What one resident's model sees. Untagged; keyed by `key()` |
| `GameModeAction` | `{action, target?, text?, reason?}` — shared by all modes |

## The three modes

| | `tide` 大逃杀 | `werewolf` 狼人杀 | `swordholder` 执剑人之夜 |
| --- | --- | --- | --- |
| Shape | Shrinking safe circle over four days, a boat at the end | Judge state machine: day talk and vote, night role actions | Judge state machine: one election day, handover, a ten-minute decision |
| Settings at open | `seed` | `seed`, `day_hours` | `seed`, `day_hours` |
| `mode_action` | none | speak, vote, role actions | speak / pass, vote / abstain, appoint, press / hold |
| Touches bodies | wading costs energy | kills and exiles | no |
| Places | geometry of the walk map | named locales | locales found by name: 广场, 联合国/大厅, 天线 |

### 执剑人之夜 in detail

Seats follow move-in order: 1 holder, 2–3 candidates, 4 envoy, the rest voters (6–12 players).

`Waiting → Day → Gather → Speech → Vote → (Tiebreak) → Interval → Handover → Attack → Aftermath → Over`

| Constant | Value | Meaning |
| --- | --- | --- |
| `BELL` / `MEET` / `HANDOVER_AT` | 16:30 / 17:00 / 19:00 | Bell, speeches, handover |
| `SPEAK_MINUTES`, `VOTE_MINUTES` | 15, 15 | Deadline per speaker; voting window |
| `APPOINT_MINUTES`, `HANDOVER_MINUTES` | 10, 10 | Tie-break by the holder; handover remark |
| `ATTACK_DELAY`, `ATTACK_MINUTES` | 5, 10 | Droplet arrives after handover; time to decide |
| `STATEMENT_MINUTES` | 15 | Envoy's closing statement |
| `FREE_TEMPO` | 3× | Speed in phases without a deadline |
| Text limits | 100 / 200 / 400 chars | Remark / stance / address |

The judge advances on two conditions only: the person whose turn it is has acted, or the deadline passed. It wakes only the person who must act; everyone else gets the line written into their experience. Endings: `Broadcast` (pressed) or `Failed` (held or timed out). All timetable values and every announcement are constants and inline text in `swordholder.rs`; the cast lives in `frontend/town-shared/src/prefabs/swordholder.ts`.

## Adding a mode — checklist

1. `crates/kernel/src/game_mode/<mode>.rs` implementing `Rules`; add to `GameMode`, `each!`, `World::set_game_mode`.
2. `crates/protocol/src/game_mode/<mode>.rs`; add a variant to `GameModeView`, `GameModeChoice`, `GameModeContext` (+ `key()`).
3. `crates/host/src/world_commands.rs`: seed roll and the receipt text in `save_game_mode`.
4. `crates/protocol/tests/wire/samples.rs`: a roster sample and a choice sample.
5. `frontend/town-shared/src/protocol.ts`: the mirrored types.
6. `frontend/town-2d/src/map/hd2d/gameModes/<mode>.ts` implementing `GameModeLook`; add to `gameModeLooks`.
7. `frontend/town-shared/src/audio/gameModes/<mode>.ts`; add to `GAME_MODE_MUSIC`.
8. Optional: a prefab in `frontend/town-shared/src/prefabs/`.
9. A kernel test in `crates/kernel/tests/game_mode.rs` that plays a full game through contexts and actions, saves and restores mid-game, and ends with `no_trace` after closing.

The compiler and the wire test point at whatever is missing.
