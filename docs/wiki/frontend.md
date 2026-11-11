# Frontend

Vanilla TypeScript, Vite, three.js. No UI framework, no state library, no i18n — strings are Chinese in the code. pnpm workspace with two packages:

- **`town-shared`** — no build step; imported as source (`town-shared/prefabs/index`). Protocol mirror, connection, shell and panels, residents UI, memory and diary pages, prefabs, audio.
- **`town-2d`** — the Vite app. `index.html` → `src/launcher/page.ts` (town selection and creator). `game.html` → `src/gameEntry.ts` → `startGameShell()`; the HD-2D renderer is under `src/map/hd2d/`.

DOM is built with helpers in `town-shared/src/dom.ts` (`el`, `button`, `$`). Panels implement `ShellPanel` (`handleMsg`, `roster`, `connected`) and are registered in order. CSS sits beside its feature; tokens in `design/tokens.css`.

## Talking to the world

`connection.ts` `createConnection()` gives `send(command: ClientMsg)`; replies arrive through each panel's `handleMsg`. `worldSettings.ts` wraps the setting commands (`saveWorldview`, `savePhysiology`, `saveClimate`, `saveGameMode`). The manager is reached with `api()` in `town-2d/src/launcher/api.ts`.

`protocol.ts` mirrors `crates/protocol` by hand and is checked against Rust samples.

## Prefabs (`town-shared/src/prefabs/`)

A `Prefab` is a TS object: `worldview`, map `spec`, `gameMode`, `roster` (template ids), optional `cast` (personas + looks), `climate`, `limits`, `furnish` (props to remove and place), player range, needs switches, `options` and `rules`. Registered in `PREFABS`.

Options and rules are plain data, so a prefab can be produced by a tool rather than written by hand:

- `PrefabOption` — a segmented row; `field` is a dotted path into the `GameModeChoice` (`day_hours`, `seed`), `number` parses the value, and `values` maps each choice to an arbitrary JSON value (灯塔之约 uses it to attach a whole `ruleset`).
- `PrefabRule` — a row inside the collapsed 「规则」 section: `kind` is `time`, `minutes` or `switch`, `field` is a path such as `rules.bell`, `initial` must equal the kernel default. Only edited rules are sent; an untouched section sends nothing.
- `withField` / `applyOption` fold the picked values into the choice.

The setup screen also has a collapsed 「人设」 section: each seat's name, intro, personality, situation and drives can be edited for this game only (never written back to the villager library). A seat locks once its resident has moved in.

The selection page can hide scripted towns (「显示哪些」 beside the 剧本 group); the list is stored by the manager (`/api/towns/prefabs`), and hiding never touches a game already played.

From card to running game:

1. **Selection page** — `generatePrefab()`: `POST /preview {spec}` → `POST /save {town_id: 'prefab-<id>', creation: {…, prefab}}`.
2. **Game page, first run** — `createPrefabSetup` shows the lineup, per-seat models, one row per option, the 吃喝睡 switch.
3. **「开始这一局」** (`setup.ts`), each step resumable: worldview → physiology → climate → furnish → agent limits → for each seat `preview_resident_join` + `join_resident` (with the cast persona) → looks → fold options into the choice → `set_game_mode`.

The world stays paused until the player presses play.

## Story editor

`town-2d/src/launcher/storyEditor.ts` — 「写一个故事」 on the selection page.

1. **Outline** — a story or outline (≤ 6000 chars), number of characters, number of days. `POST /api/towns/story` asks the connected model for a **story draft**.
2. **Review** — every field of the draft is editable; beats, places and milestones can be added and removed.
3. **Build** — the draft goes back through `/story` (tidied and checked with the same rules the kernel applies at open), the map is generated from the draft's places (`specOf`), and the town is saved with `creation.scenario = draft`.

`town-shared/src/prefabs/scenario.ts` holds the `StoryDraft` type and turns a draft into a generator spec (`specOf`), a storyline script (`scriptOf`) and a `Prefab` (`scenarioPrefab`). The game shell uses `prefabOf(creation.prefab) ?? scenarioPrefab(creation.scenario)`, so a story town opens the same setup screen as a hand-written scripted town.

## Game-mode looks and music

`GameModeLook` (`town-2d/src/map/hd2d/gameModes/index.ts`): `apply(view, metre)` plus optional `mount(layer)` for DOM overlays, `addToScene`, `step`, `badges`, and ground/standing shaders. All looks are constructed for every scene and react to the view's `kind`.

`GameModeMusic` (`town-shared/src/audio/gameModes/index.ts`): `cell(view, people)` picks the music cell, `sting(view, before)` a one-shot. `GAME_MODE_MUSIC` is keyed by mode id, so a missing entry is a type error.

## Model-written content today

- Town creator: one sentence → `/parse` → plan (HTTP, manager).
- Story editor: outline → `/story` → story draft (HTTP, manager).
- Villager editor and the onboarding guide: a brief → `generate_resident_template` → persona fields (WebSocket, host).

## Memory UI

- **Memory page** (`memory.ts`, `memory/`): notebook categories, search, entry detail with sources and history (`get_notebook`, `get_notebook_detail`).
- **Diary** (`diary.ts`): per day, world log and the resident's thoughts.
- **Info panel** (`info/agent.ts`): goals, last results, interactions, remembered things (`ResidentAgentView.memories`).

## Platform branches

`platform.ts` sets on `<html>`: `data-layout` (`compact` ≤ 719 px, else `wide`), `data-input` (`touch` / `mouse`), `data-shell` (`android` / `mac` / `windows`, only inside app shells). Branch on these and nothing else.

## Tests

`node --test` over `scripts/tests/*.test.mjs` and `frontend/town-shared/tests/*.test.mjs`; `tsc --noEmit` per package; prettier over `frontend` and `scripts`.
