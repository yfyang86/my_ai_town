# Sprint 5 — report

## Shipped

**A word index for memory.** The agent's SQLite gained an FTS5 table. Text is tokenised by us — CJK runs into single characters and adjacent pairs, Latin and digits into words — so two-character Chinese words match. Queries are ranked with BM25.

**One score.** `0.5 × relevance + 0.2 × recency + 0.3 × importance`. Recency halves every two game days; importance comes from what a memory is (insight > experience > recognition > notices and speech > routine). Used by the `recall` tool when a text query is given and by prompt assembly to fill the slots that per-object lookups leave empty.

**Dreaming.** On waking from at least two hours of sleep:

- without a model: stale, already-reflected routine memories stop being recalled;
- with a model: one request with the most important new memories; the model returns insights (each citing its evidence), memories to let go, and a short dream. Insights are a new memory kind and rank highest in recall. Dreams are kept per day.

It rides on the existing reflection round, so request plumbing, budgets, retries and the built-in model's prompt format are untouched. Tool definitions sent in ordinary life and reflection rounds are byte-identical to before.

Also: story generation on the desktop manager now has a 300 s timeout (was 100 s).

## Tests

| Check | Result |
| --- | --- |
| `cargo clippy --workspace --all-targets -D warnings` | clean |
| `cargo test --workspace` | pass — 143 tests (+2) |
| `pnpm build`, `typecheck`, `format:check` | pass |
| `pnpm test` | 22 pass, 1 known root-only failure |

New regression tests:

- Index: a two-character query finds the one right memory; a relevant old memory outranks a barely related new one; at equal text the newer wins; an important old experience stays in the top two; superseded memories never appear; another resident's memories cannot be searched; `recall` with text uses the same ranking and follows changes to the store.
- Runtime: a resident sleeps → no requests while asleep → after acknowledging the wake-up notice, exactly one dream request offering only the `dream` tool and carrying the text of what was read → the insight is stored with its evidence and ranks first for a related query, the let-go memory is superseded, ids outside the material are ignored → dream state survives a save round-trip and an old save without it loads → a second sleep with little new experience plans no dream → a model that returns nothing drops the dream without a reflection failure.
- All 30 existing agent tests pass unchanged, including the built-in model's exact-prompt test.

## Real-model run (DeepSeek `deepseek-flash`)

A temporary key was provided during this sprint. It was used only from a scratch data directory outside the repository and appears in no commit.

| What | Result |
| --- | --- |
| Story draft, 6 characters, via `/story` | 65 s. Valid on first try: 6 places mapped to generator kinds, 6 characters each with goal and secret, 6 beats, 5 milestones |
| Same through the browser editor, 5 characters | 70 s to draft; built the town; setup finished |
| Playing that story with real agents | All 6 beats fired; **20 scene speeches** in character; 5 of 6 milestones reached — one by `beat`, one by `gather`, three by `declare` with the characters' own words; the story closed with its epilogue. One character volunteered his secret during the first scene |
| Dream, with one character's 90 real memories from that day | One `dream` call: 4 insights, every cited id valid, each grounded in the cited memories; 8 memories let go — mostly procedural "your turn to speak" notices; a 109-character dream that recombined the day's places and people. With an empty memory list the model returned no insights rather than inventing any |

This closes the "not verified" item from sprint 4.

## Defects found and fixed during the sprint

1. **Story generation could time out on desktop.** The real model needs 65–70 s; the Node manager's worker limit was 100 s. Raised to 300 s for this call.
2. **Dream material had no substance for things read.** A reading memory only says "finished reading X". The material now carries an excerpt of what was read.
3. **`reflect` could have been accepted during a dream round** and advanced the notebook. It is now refused there.
4. **Adding `Insight` would have changed the `recall` tool's schema** for every request. The kind is excluded from the schema's filter list so tool definitions stay identical.

## Found, not changed

- **The dream waits for the wake-up notice to be acknowledged**, so it is the second request after waking, not the first. Same rule as reflection; left as is.
- **Procedural notices crowd the dream material.** "Your turn to speak" notices score like any notice and took 5 of 16 slots in the real run. The model correctly let them go, but the slots would be better spent on what was actually said. Candidate for sprint 6.
- **Every beat got a scene** in the model-written stories although the prompt asks for scenes only at key confrontations. Playable, but chatty and costly. Candidate for a prompt tweak.
- In the development sandbox the game's HTTP client does not trust the sandbox proxy's certificate; calls were relayed through a local forwarder. Not a product issue.

## Not done

- Dreams and insights are not yet visible in the UI (sprint 6).
- The real-model dream was exercised with a request built from real memories, not by waiting for a resident to sleep through a night in a live town. The live path is covered by the runtime test with scripted replies.
