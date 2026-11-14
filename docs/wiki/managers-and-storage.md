# Managers and storage

One manager: `crates/manager` (`town-manager`), in Rust, for every platform — the desktop dev and preview commands start it too (`scripts/start-client.mjs`), and the Android, macOS and Windows shells embed it. It calls `host` and `agent` in-process and takes an optional session cookie. The Node manager that used to serve the desktop was removed upstream in 0.5.0.

POST bodies are JSON with header `x-town-client: 1`, at most 1 MB. Errors are `{error, code?, details?}`.

## `/api/towns`

| Method and path | Purpose |
| --- | --- |
| GET `/health` | Liveness, source workspace |
| GET `/` | Town list with progress and cast |
| GET `/:id/map?hash=` | Map document |
| POST `/:id/open` · `/close` | Start / stop the host |
| POST `/:id/delete` | Delete (not built-ins) |
| POST `/:id/reset` | Wipe a `prefab-*` town's save |
| POST `/:id/positions` | Persist prop layout |
| GET/POST `/prefabs` | Which scripted towns the selection page lists: `{hidden: [prefab id]}` |
| GET `/options` | Creator options, model availability — `{generation: 'client', model}` |
| POST `/parse` | One sentence → build plan (model call) |
| POST `/story` | Story editor: `{outline, cast, days, rules?}` → model writes a story draft (`rules: "srd5"` adds character sheets, checks and encounters; `prompt: <tuning>` overrides the saved prompt tuning for this call); `{draft}` → tidy and validate an edited draft (no model call) |
| GET/POST `/prompts` | Prompt tuning: read `{settings, defaults, limits}`; save `{settings}` |
| POST `/prompts/preview` | `{target, tuning}` → the composed template and warnings, nothing saved |
| POST `/prompts/character` | `{brief, tuning?}` → one generated character, nothing saved (for comparing prompts) |
| GET `/:id/transcript?kinds=chat,thought,event,trace` | The town log: `{kinds: {chat: "<jsonl>", …}}`, one raw JSONL string per requested kind (all four when `kinds` is omitted), empty when nothing was recorded. `chat`, `thought` and `event` come from `transcript/`; `trace` is read from the capped agent trace files in `logs/`, newest 64 MB at most |
| POST `/preview` | Run generator, return draft — refuses (page generates) |
| POST `/drafts` | Validate a page-generated map, return draft |
| POST `/save` | Draft → town |
| GET/POST `/models`, POST `/models/test` | Provider settings |
| GET/POST `/builtin-model` | Built-in model download state |

On desktop the generator (`scripts/mapgen`) runs in the manager. In the apps it runs in a page worker and the manager only validates the result (`/drafts`) — so the generator must not depend on Node-only APIs.

## Data root

`AI_TOWN_HOME`, else `~/.local/share/my-ai-town` (macOS: `~/Library/Application Support/MyAiTown`).

```
<root>/
  manager.lock/                 single-manager lock
  config/                       application.json, providers.json, credentials.json, launcher.json
  resident-library/             custom villager templates
  builtin-model/                downloaded model files
  towns/<id>/
    map.json                    sealed map; content_hash identifies the save
    receipt.json                how it was created
    creation.json               {description, worldview, residents[], prefab?, scenario?}
    world.json                  the save's commit point: current state only (world + resident sessions)
    prop-layout.json
    recordings/                 broadcast reels
    history/                    append-only journals: events, finished days, action groups, interactions, memory records
    transcript/                 chat.jsonl, thought.jsonl, event.jsonl — appended while the world runs
    logs/                       resident-agent-*.jsonl agent traces (32 MB per file, 256 MB per town), builtin-model.log
    host.log, host-ready.json
```

`creation.scenario` is the story draft a town was built from (≤ 64 KB, stored as-is); the game page turns it into the setup screen.

Towns are created atomically (`.saving-<uuid>` then rename). Writes per town are serialized.

## Built-in towns and prefabs

`BUILTIN_TOWNS` — `default-town`, `spring-valley`, `broken-towers` — is declared in `crates/manager/src/storage.rs`. Built-ins are seeded on first run and cannot be deleted. Towns with id `prefab-<id>` are created by the page from a prefab and can only be reset.

## Tests

`scripts/tests/towns.test.mjs` drives the manager end to end against real `host` binaries with the mock model (saving, opening, isolation, restart, deletion, one town at a time). Request validation, the story and prompt routes are covered in `crates/manager/tests/http.rs`.
