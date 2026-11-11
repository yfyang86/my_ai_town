# Architecture

## Processes

```
browser ──HTTP /api/towns──▶ manager ──spawn──▶ host (one per running town)
   │                           │                  ├─ kernel::World
   └──────── WebSocket ────────┼─────────────────▶├─ agent::ResidentRuntime
                               │                  ├─ model HTTP calls (thread pool)
                               └─ town-map (one-shot worker: validate, parse, models)
```

| Process | Binary | Port | Job |
| --- | --- | --- | --- |
| Manager (desktop) | `node scripts/start-client.mjs` → `scripts/town-manager.mjs` | 8788 (dev) / 5174 (preview, also serves `dist`) | Lists, creates, opens, closes towns; owns the data root |
| Manager (apps) | `town-manager` from `crates/manager` | chosen by the shell | Same interface, in Rust, with a session cookie |
| Host | `host` from `crates/host` | random, reported in `host-ready.json` | Runs one town |
| Worker | `town-map` from `crates/host/src/bin/town-map.rs` | — | JSON in on stdin, JSON out: map validation, description parsing (model call), model settings |

The managers bind to loopback only. The Node manager shells out to `town-map` for anything that needs Rust logic or a model call; the Rust manager links `host` and `agent` and calls the same functions in-process.

## Opening a town

1. Page calls `POST /api/towns/<id>/open`.
2. Manager spawns `host --map-document towns/<id>/map.json --save-file towns/<id>/world.json --port 0 --connection-token <uuid> …` and waits for `host-ready.json`.
3. Manager answers with `endpoint: ws://127.0.0.1:<port>/?session=<token>`.
4. Page opens the WebSocket; the first message is `world_identity`, then rosters and events stream in.
5. Closing sends SIGTERM (stdin close on Windows); the host saves and exits.

## One tick (host, every 100 ms)

1. Drain client commands (`runtime.network_command`) and avatar input.
2. `world.step(0.1 × pace)` — only while speed > 0, settings are complete and someone can think. Pace is the chosen speed × the open mode's `tempo`.
3. `driver.dispatch` hands new world events to the agent runtime; `driver.receive` collects finished model replies.
4. `driver.runtime.poll` — each resident session ingests memories, resolves pending tools and, if it wants a turn and is awake, plans one: **chat**, else **reflection**, else **life**.
5. Checkpoint the save; `driver.send` posts the new requests to the model on a thread pool.
6. Broadcast roster diffs and events; the recorder writes the same broadcasts to `towns/<id>/recordings/`.

The kernel advances in exact slices: `World::step` cuts time at the nearest boundary (an activity ending, a weather change, a game-mode deadline…) so rules always settle at the moment they are due, regardless of tick size.

## A resident's turn

```
kernel World ──observe──▶ observation JSON ─┐
memory index ──recall──▶ recent_memories ───┼─▶ messages + tools ──▶ model
notebook ──related notes──▶ memory_files ───┘                          │
                                                                       ▼
kernel World ◀── checked + executed ◀── tool calls (move, use, say, mode_action, recall …)
```

The agent crate builds requests and applies replies; it never opens a socket. The host owns transport (`crates/host/src/llm.rs`, `agent_worker.rs`), provider configuration and budgets.

## Where state lives

| State | Where | Notes |
| --- | --- | --- |
| World + resident sessions | `towns/<id>/world.json` | One save; kernel snapshot `SCHEMA_VERSION`, agent `schema_version` |
| Map | `towns/<id>/map.json` | Sealed; `content_hash` is the save's identity |
| How the town was made | `towns/<id>/creation.json` | Description, worldview, suggested residents, prefab id |
| Recordings | `towns/<id>/recordings/` | Broadcast log for replays |
| Providers and keys | `<root>/config/providers.json`, `credentials.json` | Keys never enter the repo or the wire |
| Villager library | `<root>/resident-library/` + `config/resident-library/` | Custom + built-in templates |
