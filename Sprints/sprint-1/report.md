# Sprint 1 — report

## Shipped

- `docs/wiki/` — index, developer notes, architecture, kernel, game modes, agent runtime and memory, protocol and host, managers and storage, frontend.
- `AGENTS.md` — `docs/wiki/` and `Sprints/` are now allowed doc locations.
- `Sprints/README.md` — process, six-sprint roadmap, decisions taken with the product owner.
- `dev` branch created from `main`.
- Sprint 2 planned (`Sprints/sprint-2/plan.md`).

No code or behaviour changed.

## Baseline (before any feature work)

| Check | Result |
| --- | --- |
| `cargo test --locked --workspace` | pass — 138 tests |
| `pnpm build`, `pnpm typecheck` | pass |
| `pnpm format:check` | pass |
| `pnpm test` | 22 pass, 1 fail (see below) |

**Known local failure, not a defect:** `scripts/tests/readonly.test.mjs` asserts that writing into a `chmod 555` tree is refused. The development container runs as root, which ignores file modes, so the assertion cannot hold here. It is expected to pass in CI. Every later sprint reports this test separately.

## Findings that shape later sprints

1. **`protect-crates` check.** The repo fails any PR touching `crates/`. Decision: leave the workflow alone, merge into `dev` on green `test` + `format-check`, and call out the expected red check in each PR.
2. **Agent memory has no real database.** The SQLite in `crates/agent` is an in-memory index rebuilt from the save; search is substring matching and nothing ranks by relevance or age. Sprint 5 adds a real word index rather than replacing a store.
3. **FTS5 is available.** The bundled SQLite (3.46, `libsqlite3-sys` 0.30.1) is compiled with `SQLITE_ENABLE_FTS5`, including the `trigram` tokenizer that works for Chinese — no new dependency needed.
4. **Sleep is observable but unused.** `World::awake(id)` already tells when a resident sleeps; no model work happens then. That is the hook for dreaming.
5. **Prefab options are functions**, so a prefab cannot be saved as data. Sprint 2 makes them declarative; sprint 4's editor depends on it.
6. **The Node manager cannot call a model itself** — it goes through the `town-map` worker. The storyline endpoint therefore needs a new `town-map` mode plus the in-process path in the Rust manager.
7. **The built-in model is trained on exact prompt wording.** Scenario configuration must substitute values, not rewrite announcements.

## Next

Sprint 2 — configurable scenarios, as planned.
