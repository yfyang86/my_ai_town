# Sprint 5 — Agent memory: a word index, ranking, and dreaming

## Where we start

- Memories are `MemoryRecord`s in the save. The SQLite "index" is in-memory, searched by substring, ordered by `protected, content, confidence, time`.
- Nothing ranks by relevance to what is happening, nothing fades, and nothing happens while a resident sleeps.

## Goal

Residents recall what is relevant, not just what is recent or tagged; and sleep does something — the day is consolidated into a few durable insights, clutter is retired, and a short dream is left behind.

## Design

### 1. A real word index (SQLite FTS5)

- FTS5 table beside `memories`, kept in step by `sync`.
- **Chinese-capable tokenisation done by us**: CJK runs become overlapping two-character tokens, Latin and digits become lower-cased words. The stored text and the query are tokenised the same way, so a two-character word matches (the built-in `trigram` tokenizer cannot match queries shorter than three characters, and `unicode61` treats a whole Chinese sentence as one token).
- Query terms are OR-ed and ranked with BM25.

### 2. One score

`score = relevance + recency + importance`

| Part | Source |
| --- | --- |
| relevance | BM25 of the query against the memory, normalised to 0–1 within the result set |
| recency | exponential decay with a half-life in game time (two days) |
| importance | 0–1 from what the record is: insights and repeated failures high, protected items high, notices and speech above routine observations, scaled by confidence |

Used by the `recall` tool when a text query is given, and by prompt assembly: besides the per-object lookups, the names of people, places and things in view plus open goal titles form a query, and the best-scoring memories fill the remaining slots.

### 3. Dreaming

When a resident has been asleep for a while and has enough new experience since the last dream:

- **Always (no model needed):** retire superseded clutter — exact repeats and stale low-importance routine episodes stop being recalled. Nothing is deleted; status changes to `superseded`.
- **When a model is connected:** one request per sleep with the day's experiences. The model answers through a `dream` tool: up to four **insights** (durable lessons or beliefs, each tied to the experiences that support it), memories to **let go**, and a few sentences of **dream**. Insights become memories of a new kind, `insight`; the dream text is kept per day.
- Counts against the resident's normal budget; skipped when the budget is exhausted. One dream per sleep.

### 4. Database

Stays SQLite, bundled, no service. The save remains the source of truth so that rollback and checkpointing stay atomic; the database is the query engine, rebuilt on start.

## Work

| Layer | Change |
| --- | --- |
| `crates/agent/src/memory/index.rs` | FTS5 table, tokeniser, scored recall |
| `crates/agent/src/memory.rs` | `importance`, local consolidation, `Insight` records |
| `crates/agent/src/runtime*` | Dream round: plan while asleep, request, apply; relevance uses scored recall |
| `crates/agent/src/prompts`, `tools` | `dream` tool and rules |
| `crates/protocol` | `MemoryKind::Insight`; dream text in the memory view |
| `frontend` | types only this sprint; UI in sprint 6 |

## Acceptance

- Index test: a two-character Chinese query finds the right memories; relevance outranks recency when the match is strong; an old routine memory loses to a fresh one at equal relevance; an important old memory still surfaces.
- Runtime test: a resident sleeps → exactly one dream request → insights stored and recalled next day, let-go memories no longer recalled, a second sleep without new experience plans no dream; with no model, local consolidation still runs.
- Old saves load (new fields default).
- With no dream ever run, prompts are unchanged except for the ordering of recalled memories.
- Full local gate green; PR merged into `dev`.

## Risks

- **Built-in model prompt format.** A new request kind must not change the existing life and reflection prompts. The dream round is a separate request, like reflection.
- **Cost.** One extra call per resident per sleep; bounded and budgeted.
- **Recall ordering changes behaviour.** Covered by keeping the per-object lookups first and filling only the remaining slots by score.
