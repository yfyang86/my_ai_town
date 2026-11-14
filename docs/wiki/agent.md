# Agent runtime and memory (`crates/agent`)

`ResidentRuntime` turns the world into model requests and model replies into world actions. It has no network and no threads; the host calls `poll` and ships the requests.

## A turn

`poll_at` walks sessions round-robin. For each: `memory.ingest` → resolve pending tools → if the session wants a request and the resident is awake, `plan_round` picks:

1. **Chat** — an ongoing conversation needs a reply.
2. **Reflection** — the notebook is due (`notebook::plan_reflection`).
3. **Life** — the ordinary turn, built from `observation::observe`.

| Module | Role |
| --- | --- |
| `runtime.rs`, `runtime/*` | Loop, prompt input, reply handling, retries, recovery, request ledger |
| `observation.rs` | What this resident perceives, as JSON; the open game mode's context is merged in here |
| `tools.rs`, `tools/*` | Tool table and execution: actions, `mode_action`, `recall`, `memory_search`, `memory_read`, `reflect`, … |
| `prompts.rs`, `prompts/*.txt` | Fixed prompt texts |
| `story.rs` | Story editor: prompt for a story draft, tolerant parsing and normalisation (`tidy`), conversion to a `StorylineScript` |
| `story/rules.rs` | The fifth-edition half of a draft: a sheet per character, checks and encounters on beats. The prompt lists every class, skill, weapon, armour, spell and creature the kernel knows (`kernel::game_mode::ruleset_words`). Each item is probed against the kernel on its own; what fails is dropped or replaced by a default and recorded in `draft.notes`. An encounter is also weighed against the party drawn into it (`fit`): creature experience may total at most 25 per character level, the heaviest foes are removed until it does, and a lone foe that is still too heavy is swapped for the heaviest that fits |
| `tuning.rs` | Prompt tuning for the two generation prompts (story draft, character). `Tuning {mode, modules, pro}`: `default` is the shipped template — for stories that includes the 讲究一点 blocks (`default_modules`: concrete habits and objects, colliding goals, beats that force a choice), for characters nothing extra; `style` inserts labelled blocks (role, background, style, example, no-gos, rubrics) between the task description and the output contract; `pro` replaces the whole template (placeholders `CAST`, `DAYS`, `PRESET_IDS`, `KINDS` / `TAKEN_NAMES`, `PLACE_NAMES`, `LOOK_OPTIONS` are substituted by the callers; `warnings` flags a template that lost the contract). The templates stop before the input: the outline, the one-line brief and the ruleset addendum are appended by `story::prompt` and `completion::template_prompt_with` |
| `narrator.rs` | Prompt and tolerant reply parser for narrator rulings (see game-modes → Narrator rulings) |
| `completion.rs`, `completion/local.rs`, `builtin.rs` | OpenAI-compatible bodies and parsing; local Qwen via Ollama; built-in fine-tuned model template |
| `context/*` | Folding long histories, summaries, bounded views |
| `session.rs` | The saved session (history cursors, `memory`, `notebook`, budgets) |
| `notebook/` | Topic pages and day summaries written by reflection |
| `memory.rs`, `memory/index.rs` | Memory records and the query index |
| `view.rs`, `view/memory_page*` | Projections for the observer UI |

## Memory

Two layers that are easy to confuse:

**`MemoryStore`** — `Vec<MemoryRecord>` inside the saved session. This is the durable data. (On disk the host splits the records into `towns/<id>/residents/<resident>/.state/records/Dnnn.jsonl`; the rest of the store stays in `world.json`.)

| Kind | Created | Key | Notes |
| --- | --- | --- | --- |
| `Episode` | `ingest`: one per journal event | `event:{id}` | Reported speech of others gets `Reported`, confidence 0.5; identical ones merge |
| `Recognition` | `consolidate`: last result of doing X | `last_result:{prop\|verb}:{verb}` | Replaces the previous one (`Superseded`) |
| `Experience` | `consolidate`: a failure | `failure:{obj}:{verb}:{reason}` | Starts `Candidate` 0.5; a repeat makes it `Active` 0.8; a later success unprotects it |
| `Insight` | a dream (below) | `insight:{n}` | `Inferred`, confidence 0.7; cites the memories it came from |

Nothing is ever deleted; records only become `Superseded`.

**`MemoryIndex`** — an *in-memory* SQLite database, rebuilt from the store after each start and kept in step by `sync`. The save stays the source of truth so rollback and checkpointing remain atomic; SQLite is the query engine.

- Tables `memories` and `objects` answer lookups by object, kind and time.
- **`memory_words`** is an FTS5 table for word search. Chinese has no spaces, so text is tokenised by us (`words`): a run of CJK characters becomes its single characters plus every adjacent pair (灯塔要卖 → 灯 塔 要 卖 灯塔 塔要 要卖); Latin and digits become lower-cased words. Queries are tokenised the same way (pairs only for runs of two or more), OR-ed, and ranked with BM25. Two-character words — most names and places — therefore match, which SQLite's own `trigram` tokenizer cannot do.

### One score

`search(actor, store, text, now, limit)` ranks by

`0.5 × relevance + 0.2 × recency + 0.3 × importance`

| Part | Meaning |
| --- | --- |
| relevance | BM25, normalised to 0–1 within the result set |
| recency | halves every two game days |
| importance | `MemoryRecord::importance()`: insight 0.9, experience 0.7, recognition 0.5, notices / speech / readings 0.5, other episodes 0.35 or 0.2; +0.2 if protected; scaled by confidence. Mode instructions ("your turn… use `mode_action`") score 0.1 — they say how to operate, not what happened |

Used in two places:

- **The `recall` tool** — with a `text` query it searches by words and returns by score; if no word matches it falls back to the old substring match. The tool's schema is unchanged.
- **Prompt assembly** (`runtime/relevance.rs`) — per-object lookups come first as before; remaining slots (six in total) are filled by a word search for the names of people, places and things in view plus open goal titles, keeping only results with relevance ≥ 0.35.

### Dreaming (`memory/dream.rs`)

The runtime tells the store every poll whether the resident is asleep. On waking after at least two game hours:

1. **`settle` — no model needed.** Routine episodes with no content, older than three days and already reflected, become `Superseded`.
2. **A dream is due** if at least six memories are new since the last dream. It is planned like a reflection round (same request kind, budget, retries, and the rule that unacknowledged notices come first) but with its own system prompt (`prompts/resident_dream.txt`), its own material — the sixteen most important new memories, with ids, plus the text of anything read — and a single tool, `dream`.
3. The model returns up to four **insights** (each citing memory ids from the material), memories to **let go**, and a few sentences of **dream**. Insights become `Insight` records carrying the evidence's events and objects, so both word and object lookups find them. Let-go applies only to unprotected episodes from the material. The dream text is kept per day in `MemoryStore.dream.dreams` (last 30).

The resident's detail view (`ResidentDetailView.dreams`, sent only to a page that is watching that resident) carries the five most recent dreams; the info panel shows them under 「梦」.

One dream per sleep. If the model does not deliver, that sleep's dream is dropped without counting as a reflection failure. A store that has never slept serialises exactly as before.

## Notebook and reflection

Reflection is a separate request with its own tools (`reflect`, `read_content`, `context_read`). It triggers at most hourly (sim time) when the day rolled over, something important happened, or six new episodes piled up. It writes notebook pages (`people/…`, `places/…`, `lessons/…`, `self.md`) with history, and a day summary `summaries/Dnnn.md`. It does not write `MemoryRecord`s.

## Sleep

A resident sleeps by doing a timed prop action whose completion is `Sleep`; `World::awake(id)` is false meanwhile, and the runtime plans no turns. Waking triggers the dream described above.

## Model calls

The agent builds `AgentRequest { actor_id, session_id, stamp, messages, tools, max_output_tokens }`. The host sends it (`host/src/agent_worker.rs`, `host/src/llm.rs`) and returns a `ModelReply`. One-off text calls use `completion::prompt_body` + `host::llm::chat_completion(target, prompt)`; examples are the persona generator (`template_prompt` / `parse_template`) and the town planner (`planner_prompt` / `parse_plan`).

## Tests

One binary, `crates/agent/tests/agent.rs`, fixtures in `tests/support/`. Memory recall: `tests/resident_interactions.rs`; notebook and reflection: `tests/resident_notebook.rs`.
