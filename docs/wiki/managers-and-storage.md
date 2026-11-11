# Managers and storage

Two implementations of one HTTP interface:

- **Node** — `scripts/town-manager.mjs` + `scripts/towns/*.mjs`. Desktop. Calls Rust through the `town-map` worker.
- **Rust** — `crates/manager`. Android, macOS and Windows shells. Calls `host` and `agent` in-process; optional session cookie.

POST bodies are JSON with header `x-town-client: 1`, at most 1 MB. Errors are `{error, code?, details?}`.

## `/api/towns`

| Method and path | Purpose | Node | Rust |
| --- | --- | --- | --- |
| GET `/health` | Liveness, source workspace | ✓ | ✓ |
| GET `/` | Town list with progress and cast | ✓ | ✓ |
| GET `/:id/map?hash=` | Map document | ✓ | ✓ |
| POST `/:id/open` · `/close` | Start / stop the host | ✓ | ✓ |
| POST `/:id/delete` | Delete (not built-ins) | ✓ | ✓ |
| POST `/:id/reset` | Wipe a `prefab-*` town's save | ✓ | ✓ |
| POST `/:id/positions` | Persist prop layout | ✓ | ✓ |
| GET `/options` | Creator options, model availability | full | `{generation: 'client', model}` |
| POST `/parse` | One sentence → build plan (model call) | via `town-map parse` | in-process |
| POST `/preview` | Run generator, return draft | ✓ | refuses (page generates) |
| POST `/drafts` | Validate a page-generated map, return draft | — | ✓ |
| POST `/save` | Draft → town | ✓ | ✓ |
| GET `/generation/:requestId` | Generation progress | ✓ | — |
| GET/POST `/models`, POST `/models/test` | Provider settings | ✓ | ✓ |
| GET/POST `/builtin-model` | Built-in model download state | ✓ | ✓ |

On desktop the generator (`scripts/mapgen`) runs in the manager. In the apps it runs in a page worker and the manager only validates the result (`/drafts`) — so the generator must not depend on Node-only APIs.

## Data root

`AI_TOWN_HOME`, else `~/.local/share/my-ai-town` (macOS: `~/Library/Application Support/MyAiTown`).

```
<root>/
  manager.lock/                 single-manager lock
  config/                       application.json, providers.json, credentials.json
  resident-library/             custom villager templates
  builtin-model/                downloaded model files
  towns/<id>/
    map.json                    sealed map; content_hash identifies the save
    receipt.json                how it was created
    creation.json               {description, worldview, residents[], prefab?}
    world.json                  the save: world + resident sessions
    prop-layout.json
    recordings/                 broadcast reels
    logs/, host.log, host-ready.json
```

Towns are created atomically (`.saving-<uuid>` then rename). Writes per town are serialized.

## Built-in towns and prefabs

`BUILTIN_TOWNS` — `default-town`, `spring-valley`, `broken-towers` — is declared in both `scripts/towns/storage.mjs` and `crates/manager/src/storage.rs`; keep them identical. Built-ins are seeded on first run and cannot be deleted. Towns with id `prefab-<id>` are created by the page from a prefab and can only be reset.

## Tests

`scripts/tests/towns.test.mjs` drives the Node manager end to end against real `host` / `town-map` binaries with the mock model.
