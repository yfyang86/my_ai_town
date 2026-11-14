# Sprint 17 — report

Requested by the owner: cherry-pick the updates from the original repository's `main` (`mewamew/my_ai_town`).

## What was picked

The 14 upstream commits our `main` did not have (the two before them — pixel art and the README restructure — were already merged), in order, each with `cherry-pick -x`:

| Upstream | What |
| --- | --- |
| `202b88e` | version 0.5.0 |
| `197f74f` | fix: a save made while leading someone could not be read back |
| `b17df19` | fix: a handshake 6 ms late dropped the connection |
| `f188658` | perf: the roster carries only what the picture needs; character-card details are sent to the page watching that resident |
| `3961e18` | perf: the save is no longer rewritten whole every second — history goes to append-only journals, `world.json` keeps current state, writing moved to a background thread |
| `d05612e` | fix: agent trace log capped (32 MB per file, 256 MB per town), full prompts optional, a 「日志」 setting |
| `7ecbc41` | refactor: one manager, in Rust — the Node manager is deleted |
| `d381e66` | chore: no pre-compressed `.br` / `.gz` in the frontend build |
| `4fb182c` | feat: `start-client` honours `TOWN_MANAGER_BINARY` |
| `c97f7a5` | fix: two concurrency faults in the manager (lost keys on simultaneous saves; two towns opened at once) |
| `7d87ed5` | feat: a model service remembers model names |
| `0bbb7ea` | fix: the scripts row keeps registration order; own towns sort by pinyin |
| `71649d2` | fix: a recording left by a killed world is closed on the next open |
| `b6e513d` | fix: no reflections on the first 3D frame |

## Conflicts and how they were settled

| Commit | Conflict | Resolution |
| --- | --- | --- |
| roster split | our `dreams` sat on the per-tick `ResidentAgentView`, which upstream emptied | `dreams` moved to `ResidentDetailView` with the memories; panel, wire sample and the dream test follow |
| save journals | `MemoryStore` gained our `dream` state; the tick's change tracking was removed | dream state stays in `world.json` (small, current state); the narrator call kept in the tick. Dreams retire and add memories through the same per-record revisions upstream journals by |
| trace limits | our copy of every trace record into `transcript/trace.jsonl` | **dropped our copy.** The records download now reads upstream's capped trace files (`agent_trace::recent`, newest 64 MB) |
| Rust-only manager | our story, records and prompt routes existed in the deleted Node manager; `town-map` lost its modes; the Node test was rewritten | deletions accepted — every route already existed in `crates/manager`. Our Node-side checks moved into `crates/manager/tests/http.rs`; the records check stays in the rewritten `towns.test.mjs` |
| scripts row order | our "hide scripts" list drawing | upstream's ordering applied inside it; hidden scripts still not listed |

## After the picks

- `scripts/towns/prompts.mjs` deleted (it belonged to the Node manager).
- **Records, trace kind:** lines come straight from the trace log (`{trace, model_snapshot}`); the page tags them. Whether full prompts are in there follows the device's 「日志」 setting — off by default in the packaged apps.
- Wiki rewritten where it described two managers, the old save layout and the old agent view.

## Defects found and fixed while integrating

1. The trimmed trace download could cut in the middle of a multi-byte character and panic. Found by the new cap assertion; trimming now works on bytes and starts at a line.
2. The records check in `towns.test.mjs` called the manager instance that the test had already restarted.

## Checked

| Check | Result |
| --- | --- |
| `cargo clippy --workspace --all-targets -D warnings` | clean |
| `cargo test --workspace` | pass — 168 tests (upstream's included) |
| `pnpm build`, `typecheck`, `format:check` | pass |
| `pnpm test` | 24 pass, 1 known root-only failure |

Browser smoke test on the Rust manager with the mock model: the prompt studio opened and ran a comparison; a story with fifth-edition rules was drafted and built; the game started with character sheets; a records download including traces succeeded (36 thoughts, 10 world-log lines, 192 trace records).

## Not checked / limits

- No real-model run after the picks.
- Saves written before upstream's journal format are migrated by upstream's code; not exercised here with a save that contains our storyline or ruleset state.
- Trace lines carry no world id, so after a world reset old traces are listed under the newest world in a download.
- Upstream's own wiki-less docs (`docs/README.md`) were taken as they are.
