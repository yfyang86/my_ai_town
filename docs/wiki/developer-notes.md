# Developer notes

## What this is

An LLM-driven town simulation. A deterministic Rust **kernel** owns the world (map, time, bodies, props, weather). Each resident is an **agent**: the runtime turns what the resident can perceive into a prompt, the model answers with tool calls, the kernel checks and executes them. A browser client draws the town in HD-2D (three.js) and lets the player watch, configure, or walk around as an avatar.

On top of the free-running town sit **game modes** — self-contained rule sets with their own timetable and ending: 大逃杀 (`tide`), 狼人杀 (`werewolf`), 执剑人之夜 (`swordholder`). A **prefab** is a ready-made town that opens one of them.

## Layout

| Path | What |
| --- | --- |
| `crates/protocol` | Every type that crosses a process boundary (WebSocket messages, views, game-mode shapes) |
| `crates/kernel` | The world: no I/O, no model calls, no threads |
| `crates/agent` | Resident runtime: observation → prompt → tool calls; memory and notebook. No network either |
| `crates/host` | One process per running town: wires kernel + agent, talks to the model, serves the WebSocket, saves, records |
| `crates/manager` | Rust manager used by the Android / macOS / Windows shells |
| `scripts/` | Node manager for desktop (`town-manager.mjs`, `towns/`), map generator (`mapgen/`), built-in map scripts (`maps/`), replay, packaging |
| `frontend/town-shared` | Shared client code: protocol mirror, shell, panels, prefabs, audio |
| `frontend/town-2d` | Vite app: town selection page (`index.html`) and the game (`game.html`), HD-2D renderer |
| `config/` | Application config, provider list, built-in villager library, rule tables |
| `data/` | Shipped maps, generator templates, prop definitions |
| `android/`, `macos/`, `windows/` | App shells; they start `crates/manager` and open the same pages |

## Run and check

```sh
corepack pnpm install --frozen-lockfile
cargo build --locked -p host --bins      # host + town-map; the Node manager spawns both
corepack pnpm build
corepack pnpm preview                    # http://127.0.0.1:5174
corepack pnpm dev                        # Vite on 5174, manager on 8788
```

The full gate, same order as CI (`.github/workflows/test.yml`, `format.yml`):

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked -p host --bins
corepack pnpm build
corepack pnpm typecheck
corepack pnpm test
corepack pnpm format:check
```

Order matters: `pnpm test` reads protocol samples that `cargo test` writes, spawns `target/debug/host` and `town-map`, and reads `frontend/town-2d/dist`.

Useful switches: `TOWN_MOCK_LLM=1` runs residents on a scripted fake model; `AI_TOWN_HOME` moves the user data root; `TOWN_MANAGER_PORT` moves the manager.

## House rules (from `AGENTS.md`, enforced in review)

- **Source, tests and data are the truth.** Docs are limited to `README.md`, `docs/README.md`, `AGENTS.md`, `docs/wiki/` and `Sprints/`. Implementation notes go in source comments.
- **Tests guard key behaviour and data safety through the full call path.** One rule is tested at one layer. No tests for small implementation details, UI arrangement, or parameter sweeps.
- **A game mode never leaks into the core.** It lives in four directories plus registrations (see [Game modes](game-modes.md)). With no mode open, saves, wire messages and model prompts must be byte-for-byte what they were before the mode existed.
- **Two managers, one interface.** Anything added under `/api/towns` exists in both `scripts/towns/` and `crates/manager/` with the same shape.
- **One frontend.** Phone/desktop differences branch only on `data-layout`, `data-input`, `data-shell`.
- **User data is the user's.** `towns/<id>/` is never migrated into the repo, never deleted without an explicit instruction.
- Comments, UI strings, commit messages and prompts are Chinese. There is no i18n layer.

## Traps

- **`protect-crates` CI check.** Inherited from upstream: it fails any pull request that touches `crates/`. In this fork feature work has to touch `crates/`, so that check is expected to be red on such PRs; `test` and `format-check` are the real gates.
- **The agent's SQLite is an index, not a store.** `crates/agent/src/memory/index.rs` opens an in-memory database rebuilt from the saved `MemoryStore`. Durable state is the serialized session inside the town's save.
- **`Session::Sleep` is the agent's wait timer**, not the resident sleeping. A resident is asleep when `World::awake(id)` is false.
- **`protocol.ts` is hand-written.** It mirrors `crates/protocol`; the wire test compiles Rust-generated samples against it. A new message needs: the Rust type, a sample in `crates/protocol/tests/wire/samples.rs`, the entry in the `variants!` list, and the TS type.
- **The wire bans geometry.** Samples may not contain coordinates or pixel sizes outside a short exempt list; a game-mode view carries locale ids, not positions.
- **Changing prompt or tool wording for the built-in model** may require bumping `FORMAT` in `crates/agent/src/builtin.rs`; the fine-tuned model was trained on the exact template.
- **Mode tool descriptions are identical for every seat** on purpose — it keeps the provider's prompt cache warm. Per-seat information goes in the context column.
- **GitHub Actions do not run on this fork.** PRs show no checks; the local gate above is the gate. Run it before every merge.
- **`pnpm test` as root.** `scripts/tests/readonly.test.mjs` asserts a `chmod 555` tree refuses writes; root ignores file modes, so it fails in root containers and nowhere else.
- **The launcher strip clips overlays.** The town list scrolls horizontally (`overflow-x: auto`), which also clips vertically — popovers anchored in it are cut off. Put controls inline.
- **Only one town runs at a time** (`MAX_RUNNING = 1`); opening another closes the least recently used.
