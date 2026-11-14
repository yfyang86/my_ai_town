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

1. **Outline** — a story or outline (≤ 6000 chars), number of characters, number of days, and an optional 「第五版规则」 switch (sends `rules: "srd5"`). `POST /api/towns/story` asks the connected model for a **story draft**.
2. **Review** — every field of the draft is editable; beats, places and milestones can be added and removed. With rules on: each character card shows the sheet (class, skills, weapon, armour, spells — ids, normalised by the manager on build), each beat lists its checks and encounter with a delete button, the manager's `notes` are shown, and 「不用规则」 strips all of it. While editing, sheets ride on the characters (`StoryCharacter.sheet`) so deleting a character cannot shift them.
3. **Build** — the draft goes back through `/story` (tidied and checked with the same rules the kernel applies at open), the map is generated from the draft's places (`specOf`), and the town is saved with `creation.scenario = draft`.

`town-shared/src/prefabs/scenario.ts` holds the `StoryDraft` type and turns a draft into a generator spec (`specOf`), a storyline script (`scriptOf`) and a `Prefab` (`scenarioPrefab`). The game shell uses `prefabOf(creation.prefab) ?? scenarioPrefab(creation.scenario)`, so a story town opens the same setup screen as a hand-written scripted town. A draft with `rules` gets a 「规则」 option there (default 第五版规则, or 不掷骰).

**Resuming a start.** The setup screen (`prefabs/setup.ts`) runs its steps one by one. It keeps the chosen settings and finished steps in `localStorage` (`town.prefabSetup`, one entry, keyed by world id) and takes who has already moved in from the town's population, not from its own replies. If the page is reloaded mid-way — worldview saved, mode not yet open — `gameShell.ts` reopens the screen (`prefabSetup.resume`) and one click continues from the step that stopped. The entry is cleared on finish and on 「跳过」.

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
- **Info panel** (`info/agent.ts`): goals, last results, interactions, remembered things and dreams (`ResidentDetailView`, requested with `watch_resident` while the card is open; the roster itself only carries the small `ResidentAgentView`).

## Platform branches

`platform.ts` sets on `<html>`: `data-layout` (`compact` ≤ 719 px, else `wide`), `data-input` (`touch` / `mouse`), `data-shell` (`android` / `mac` / `windows`, only inside app shells). Branch on these and nothing else.

## Tests

`node --test` over `scripts/tests/*.test.mjs` and `frontend/town-shared/tests/*.test.mjs`; `tsc --noEmit` per package; prettier over `frontend` and `scripts`.

## Records download

Menu → 「记录」 (`town-shared/src/shell/records.ts`). The dialog asks which kinds (对话, 想法, 世界日志, 模型轨迹 — the last is off by default because it is large) and which format, fetches `GET /api/towns/<id>/transcript`, sorts the lines by world then by sim minutes, and saves one file through a Blob link:

| Format | Content |
| --- | --- |
| JSON | `{town, exported_at, counts, items: [...]}` |
| JSONL | one record per line, as stored |
| 文字 (`.md`) | one readable line per record; trace records are abbreviated |

The page does not keep its own copy: the world process writes the log (see protocol-and-host → `transcript.rs`), so a download also covers what happened while no page was open.

## Prompt studio

`town-shared/src/prompts/studio.ts` — 「提示词」, opened from the launcher's 设定 and from the in-game settings (`settings/prompts.ts`). One dialog for both generation prompts (写故事 / 生成人物), built from the shared tokens and controls (`segmented`, `ui-chip`, `.set-row` at the entries).

| Part | What it does |
| --- | --- |
| 默认 | Shows the shipped prompt (the story default already carries the 讲究一点 blocks; a blank 风格模式 is the bare prompt without them) |
| 风格模式 | Pick a preset (`prompts/presets.ts`: six for stories, five for characters) or start blank, then edit six modules: 身份, 背景, 文风, 例子, 不要, 自查. 「看拼好的整份提示词」 asks the manager for the composed template |
| 专业模式 | The whole template in one editor, with the placeholders listed; can be reset to the default or seeded from the composed style template |
| 试一试 · 比一比 | One input (a story or outline with cast/days/rules, or a one-line character brief), two prompts chosen from 默认 / 正在改的 / 已保存的 / any preset. Both run at once; results are rendered side by side with time and model, and 「就用这一份」 adopts a side as the current tuning |

Settings are saved explicitly from the dialog's own footer (`POST /api/towns/prompts`); they are application settings, not part of a town. The story trial needs the generator's place kinds and style presets, which `town-2d/src/promptOptions.ts` registers lazily for both pages.
