# Sprint 4 — report

## Shipped

**The story editor.** 「写一个故事」 on the selection page:

1. Paste a story or outline, pick 2–12 characters and 1–3 days.
2. The connected model returns a **story draft**: title, premise, worldview, style, places, a cast with personas plus role / goal / secret, beats, milestones, ending.
3. Review and edit every field; add or remove places, beats and milestones.
4. 建成小镇 — the draft is tidied and checked, a map is generated from its places, and the town is saved with the draft attached. Entering the town opens the usual setup screen; starting opens 故事模式 with that script.

How it is built:

- `crates/agent/src/story.rs` — prompt, tolerant parser, and `tidy`: slugs ids, clamps seats and times, sorts beats, clips text, maps place references onto the draft's place names, drops dangling milestones, moves the ending after the last beat — then validates with the kernel's own check. Idempotent, so drafts edited in the page go through the same function.
- `crates/host/src/story.rs` — one function for both managers; `town-map story` for the Node manager.
- `POST /api/towns/story` in both managers; `creation.scenario` accepted and capped at 64 KB in both.
- `frontend/town-shared/src/prefabs/scenario.ts` — draft → generator spec, storyline script, prefab.
- `frontend/town-2d/src/launcher/storyEditor.ts` — the dialog.

## Tests

| Check | Result |
| --- | --- |
| `cargo clippy --workspace --all-targets -D warnings` | clean |
| `cargo test --workspace` | pass — 141 tests (+1) |
| `pnpm build`, `typecheck`, `format:check` | pass |
| `pnpm test` | 22 pass, 1 known root-only failure |

New or extended regression tests:

- Agent: one deliberately sloppy model reply (prose and code fence around the JSON, duplicate and upper-case ids, beats out of order, seats out of range, half place names, a milestone pointing at a missing beat, an ending before the beats) becomes a draft the kernel accepts; tidying it again changes nothing; non-JSON and beat-less replies are refused.
- Node manager: `/story` from an outline (mock model), `/story` on an edited draft, refusals, `creation.scenario` round-trip, oversize refused.
- Rust manager: `/story` on a draft, refusal, `creation.scenario` saved and listed, oversize refused.

End-to-end in a real browser with the mock model: outline → draft (4 characters) → renamed the first beat → 建成小镇 → town listed with the edited draft → setup screen showed 4 seats → game started → the storyline HUD showed the renamed beat.

## Defects found and fixed during the sprint

1. **「写一个故事」 overflowed the card strip.** The slot stacked two full-height buttons. Both buttons now share one card height.
2. **`pnpm preview` ignored `TOWN_MOCK_LLM`.** Only the standalone manager honoured it, so the editor called a real endpoint in the UI check. `start-client.mjs` now passes the flag.
3. **Sprint 3's browser run was not actually on the mock model** for the same reason — residents had a provider that refused connections, and scenes advanced by deadline. The conclusion of that run (beats, scenes, ending) stands; the note in the sprint 3 report about "the mock model does not speak" was the wrong explanation.

## Not verified

**No real model was available in the development environment.** The prompt has never been answered by a real LLM here; only the mock reply and hand-written sloppy replies were exercised. The parser is built to absorb bad output, and nothing is built without the review screen, but prompt quality (how good the stories are, how often a model ignores the schema) needs a run with a real provider. First thing to check after merging.

## Known limits

- Looks are borrowed from built-in villager templates; the draft does not design appearances.
- The review screen edits text fields and lists; it has no timeline drag-and-drop.
- A draft holds at most 8 places besides homes and 12 characters.
