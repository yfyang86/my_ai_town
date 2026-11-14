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
| `story.rs` | Story editor step called by the manager: outline → draft via the model, or tidy an edited draft. `rules: "srd5"` in the request asks for sheets, checks and encounters as well |
| `prompts.rs` | Prompt-tuning settings shared by all towns, stored as `prompts.json` beside the providers file: `read`, `write`, `preview` (the composed template for an unsaved tuning) and `try_character` (one model call, nothing saved). `story::draft` uses the saved story tuning, or `prompt` in the request for a trial; the in-game character generator reads the saved character tuning on every call |
| `transcript.rs` | The town log: every broadcast passes through `Transcript::note`, which appends world events to `transcript/event.jsonl`, speeches to `chat.jsonl` and new roster thoughts to `thought.jsonl`. Always on when the world has a town folder; lines carry `kind`, wall time `at`, `world` id and sim `minutes`. `read` returns those files plus, for `trace`, the agent trace from `agent_trace::recent` |
| `narrator.rs` | One worker thread that asks a model for narrator rulings; `ResidentDriver::narrate` feeds it from `World::game_mode_rulings` each tick and returns answers through `World::game_mode_rule` |
| `maps.rs` | Map validation |
| `bin/town-map.rs` | Validates one map document from stdin (map design scripts) |
| `agent_trace.rs` | The agent trace log in the town's `logs/`: one file per run, rotated at 32 MB, at most 256 MB per town; full prompts only when the device's 「日志」 setting asks for them. `recent` returns the newest lines for the records download |

### Changing a world setting

Commands that change the world run inside `transact`: mutate, save, and roll the world back if the save fails. The pattern for game modes (`save_game_mode`):

1. Fill in a random seed when the client sent none.
2. `transact(|rt| rt.world.set_game_mode(choice))`.
3. Broadcast `world_settings` to all; reply `WorldSettingsSaved { ok, message }` to the requester.

### Asking the model for something once

Long work runs on `self.tasks` (a small thread pool) and replies only to the requesting connection. `generate_template` is the model to copy: validate input → handle mock mode → build the prompt → `chat_completion` → parse and tidy → reply with data that fills a form and saves nothing.

### Providers

`config/providers.json` holds `{active: {provider, model, resident}, providers: [{id, name, base_url}]}`; keys live in `credentials.json` (mode 0600, ignored by git). Transport is OpenAI-compatible `POST {base_url}/chat/completions`. `--mock-llm` replaces the model with a script for tests.
