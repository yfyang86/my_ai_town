# Sprint 13 — report

## Shipped

### `main` merged

`origin/main` (generated pixel tiles for mode HUDs, restructured README) is merged into the working line. Two conflicts, both documentation: `README.md` (kept `main`'s structure, re-added the 「第五版规则」 section in its style) and `docs/wiki/game-modes.md` (both sides kept).

### Town log and download

- **Always recorded.** The world process appends to `towns/<id>/transcript/` while it runs:

  | File | Content |
  | --- | --- |
  | `chat.jsonl` | every line of a private chat (with whom) and every mode speech (occasion, seat) |
  | `thought.jsonl` | each resident's thought at a decision, with its trigger |
  | `event.jsonl` | the world log — movement, activities, checks, fights; not the model-ledger lines |
  | `trace.jsonl` | the agent trace: requests sent to the model, replies, tool calls and results |

  Every line has `kind`, wall time `at`, `world` id and sim `minutes`. A reset continues in the same files under a new world id.
- **Download.** Menu → 「记录」: tick the kinds (traces are off by default — they are large), pick JSON, JSONL or readable text, download. Works while the world runs or is paused, and covers what happened while no page was open.
- **Route.** `GET /api/towns/<id>/transcript?kinds=…` on both managers returns the raw files.
- It is user data in the town folder and goes when the town is deleted.

### Setup with many characters

The setup screen waited 15 s per step; with twelve characters on a slow page a join reply was processed too late and the start failed at resident 9. The wait is now 45 s. (The world answered each join in about 0.2 s.)

## Tests

| Check | Result |
| --- | --- |
| `cargo clippy --workspace --all-targets -D warnings` | clean |
| `cargo test --workspace` | pass — 159 tests (+1) |
| `pnpm build`, `typecheck`, `format:check` | pass |
| `pnpm test` | 22 pass, 1 known root-only failure |

- **New host test:** a world run with the mock model writes down every world-log line the page received, at least one request and one response trace, and thoughts; no chat (the mock does not talk); after a reset, old lines are untouched and new ones carry a new world id.
- **Manager tests** (Node and Rust) gained a check of the route.
- **Browser:** the dialog downloaded JSON, text and JSONL; all three parsed.

## Real-model run (DeepSeek `deepseek-flash`) — 「丁香一日杀局」

**Draft.** The owner's wuxia outline, 12 characters, one day, rules on: one model call of about two minutes produced 12 valid sheets (fighters, rogues, two clerics, a wizard), four checks, one encounter (the tea-shed ambush: a thug and a cultist against three of the cast), six beats and five milestones. Nothing needed correcting.

**Play.** Ran to the end in about 7 minutes at ×8.

- The ambush opened the day: 程小青 was dropped by a critical hit, rolled a 20 on a death save and got up; 金碎心 used second wind, went down, and came round after the fight; 诸葛太平 finished it. 41 experience each.
- 40 rolls, 12 narrator calls, three short rests chosen by the model.
- Four of five milestones; the last (「银雕已离镇」) was the one left to a character to declare.
- 449 model calls, about 5.9 M tokens.

**Records downloaded from that game:** 131 chat lines, 67 thoughts, 226 world-log lines, 2,530 trace records (436 requests and 436 responses among them); 0.2 MB without traces, 22 MB with.

## Defects found and fixed

1. **Game start failed at resident 9 of 12** (above).
2. **Chat lines listed the speaker among the people spoken to.** The roster names participants by name, not id.
3. **Downloads were named `download`** with a Chinese file name in this browser. File names are now ASCII (`records-<town id>-<time>.json`); the town's name is inside the file.
4. The dialog reported characters as megabytes; it now reports bytes.

## Observed, not changed

- **A half-finished start cannot be resumed after reloading the page**: the setup screen only opens on a fresh town. The run above was completed by sending the remaining joins and the mode over a raw connection.
- 程小青 missed three fight turns in a row at ×8 with twelve residents thinking at once. Fight turns have a 15 game-minute deadline; at normal speed that is eight times longer in real time.
- Traces are large (22 MB for one game day with twelve residents) and the log is never trimmed.
- Downloading through a Blob link was tested in desktop Chromium only, not in the Android, macOS or Windows shells.
- The readable-text format abbreviates trace records; use JSON or JSONL for full traces.
