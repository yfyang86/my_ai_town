# Protocol and host

## Protocol (`crates/protocol`)

All wire types. Enums are `snake_case`, messages tagged by `"type"`.

- `ClientMsg` (`client_msg.rs`) — commands: settings (`SetWorldview`, `SetPhysiology`, `SetClimate`, `SetGameMode`, `SetSpeed` …), props, models, resident library and joining (`GenerateResidentTemplate`, `PreviewResidentJoin`, `JoinResident` …), reads (`GetDiary`, `GetNotebook`, `GetNotebookDetail`, `GetPopulation` …), recordings, avatar.
- `ServerMsg` (`server_msg.rs`) — `WorldIdentity`, `Roster`, `Event`, `WorldSettings`, `WorldSettingsSaved`, results and views for each command.

Requests carry a `request_id`; the receipt echoes it to the requester, and state changes are broadcast to everyone.

### Wire tests

`crates/protocol/tests/wire.rs` + `tests/wire/samples.rs`:

- A `variants!` list makes the build fail until a new variant is listed and sampled.
- `no_geometry_on_the_wire` rejects coordinates and pixel sizes outside an exempt list.
- `samples_for_typescript` writes all samples to the target temp dir; `frontend/town-shared/tests/protocol-wire.test.mjs` type-checks them against `protocol.ts`. Run `cargo test -p protocol --test wire` before `pnpm test`.
- `old_saved_values_still_read` guards serde aliases.

## Host (`crates/host`)

| File | Role |
| --- | --- |
| `main.rs` | Load application config and map, build the runtime, serve |
| `runtime.rs` | Command dispatch and the tick (see [Architecture](architecture.md)) |
| `world_commands.rs` | World settings: worldview, physiology, climate, game mode |
| `library_commands.rs` | Villager templates, incl. model-written personas |
| `network.rs` | WebSocket server; checks `?session=<token>` |
| `agent_worker.rs`, `llm.rs` | Model transport, provider targets, built-in model routing |
| `config.rs`, `application.rs`, `models.rs` | Providers, credentials, model settings |
| `recording.rs`, `recording_commands.rs` | Broadcast recorder |
| `story.rs` | Story editor step shared by both managers: outline → draft via the model, or tidy an edited draft |
| `maps.rs` | Map validation |
| `bin/town-map.rs` | One-shot worker used by the Node manager |

### Changing a world setting

Commands that change the world run inside `transact`: mutate, save, and roll the world back if the save fails. The pattern for game modes (`save_game_mode`):

1. Fill in a random seed when the client sent none.
2. `transact(|rt| rt.world.set_game_mode(choice))`.
3. Broadcast `world_settings` to all; reply `WorldSettingsSaved { ok, message }` to the requester.

### Asking the model for something once

Long work runs on `self.tasks` (a small thread pool) and replies only to the requesting connection. `generate_template` is the model to copy: validate input → handle mock mode → build the prompt → `chat_completion` → parse and tidy → reply with data that fills a form and saves nothing.

### Providers

`config/providers.json` holds `{active: {provider, model, resident}, providers: [{id, name, base_url}]}`; keys live in `credentials.json` (mode 0600, ignored by git). Transport is OpenAI-compatible `POST {base_url}/chat/completions`. `--mock-llm` replaces the model with a script for tests.
