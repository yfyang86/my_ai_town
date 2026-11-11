# Kernel (`crates/kernel`)

Pure, deterministic world logic. No I/O, no clocks, no model. Depends only on `protocol`, `serde`, `sha2`. Everything else drives it through `World`.

## Time

World time is minutes as `f64`; `t = 0` is day 1, 00:00. Constants in `lib.rs`: `DAY = 1440`, `DAWN = 360`, `DUSK = 1140`, plus `time_of_day` and `clock_string`.

`World::step(dt)` (`simulation.rs`) advances in slices. Each slice ends at the nearest of: an activity finishing, an interaction deadline, a calendar mark, a weather change, a physiology threshold, a scheduled happening, a walk arriving, or **the open game mode's `boundary()`**. After moving everything forward it calls `advance_game_mode()`. `quiet_minutes` tells the host when it is safe to fast-forward.

## Module map

| Area | Files | Notes |
| --- | --- | --- |
| World | `world.rs` | `struct World`; `notify`, `spawn`, `log(EventInfo, text)` |
| Actions | `action_commands.rs`, `action_check.rs`, `action_results.rs`, `intent.rs`, `execution.rs`, `activity.rs`, `actor.rs` | `run_actions`, `interrupt_actions`; validation is separate from execution |
| Between people | `interactions/`, `force.rs` | Chat, invites, appointments, pledges; push / rob / steal / search |
| Body | `needs.rs`, `physiology/` | Hunger, thirst, energy, health; rates can be extended by a mode |
| Weather | `weather/` | Schedule, exposure, policy, rule table |
| Things | `props/`, `inventory/`, `content/`, `affordance.rs` | Catalog, placement, making, readable works |
| Place | `topology.rs`, `path.rs`, `navigation/`, `avatar/`, `exploration.rs`, `presence.rs`, `sightings.rs` | Node graph + locales for residents; 2D walk map for avatars |
| Timed events | `happenings.rs`, `calendar.rs` | Scheduled events with their own RNG streams; `depart()` |
| What others see | `projection.rs`, `journal.rs`, `discovery.rs` | Roster projection; per-actor facts and notices; `awake(id)` |
| Saves | `snapshot.rs` | `SCHEMA_VERSION`; the save is the serde shape of the runtime types; `checkpoint` / `restore_checkpoint` back host transactions |
| Game modes | `game_mode/` | See [Game modes](game-modes.md) |

## How information reaches a resident

- `actor.journal.fact(...)` — written into what the resident has experienced; they see it next time they think. Silent.
- `world.notify(id, text)` — a notice that also wakes the resident's agent.
- `world.log(EventInfo, text)` — the public world log (the feed in the UI, the recording). `EventInfo::speech` marks a line as dialogue.

A resident is never moved by fiat. Rules ask ("go to the plaza"); the agent decides; the kernel records whether they were there.

## Saves and compatibility

Fields added to saved types take `#[serde(default)]` (and `skip_serializing_if` when "absent" must stay byte-identical). Renames keep a serde alias. Bump `SCHEMA_VERSION` only for changes old saves cannot be read through.

## Tests

One binary: `crates/kernel/tests/kernel.rs` (`autotests = false`). Helpers in `tests/common/mod.rs`: `tiny_topology()`, `world_with(topo, start, &[(id, name, locale)])`, `run`, `finish`, `restore` (snapshot → JSON → new world). Run with `cargo test -p kernel [filter]`.
