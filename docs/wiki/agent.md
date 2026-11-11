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
| `completion.rs`, `completion/local.rs`, `builtin.rs` | OpenAI-compatible bodies and parsing; local Qwen via Ollama; built-in fine-tuned model template |
| `context/*` | Folding long histories, summaries, bounded views |
| `session.rs` | The saved session (history cursors, `memory`, `notebook`, budgets) |
| `notebook/` | Topic pages and day summaries written by reflection |
| `memory.rs`, `memory/index.rs` | Memory records and the query index |
| `view.rs`, `view/memory_page*` | Projections for the observer UI |

## Memory today

Two layers that are easy to confuse:

**`MemoryStore`** — `Vec<MemoryRecord>` inside the saved session. This is the durable data.

| Kind | Created | Key | Notes |
| --- | --- | --- | --- |
| `Episode` | `ingest`: one per journal event | `event:{id}` | Reported speech of others gets `Reported`, confidence 0.5; identical ones merge |
| `Recognition` | `consolidate`: last result of doing X | `last_result:{prop\|verb}:{verb}` | Replaces the previous one (`Superseded`) |
| `Experience` | `consolidate`: a failure | `failure:{obj}:{verb}:{reason}` | Starts `Candidate` 0.5; a repeat makes it `Active` 0.8; a later success unprotects it |

Nothing is ever deleted; records only become `Superseded`.

**`MemoryIndex`** — an *in-memory* SQLite database (`Connection::open_in_memory()`), rebuilt from the store after each start. Tables `memories` and `objects`. `recall` filters by substring (`instr(lower(text), lower(?))`), object, kind and time, and orders by `protected, content, confidence, at` — there is no word index, no relevance score and no decay.

**Into the prompt** (`runtime/relevance.rs`): the focus is the set of objects in view (event objects, people here, visible props, current locale, goal locales). For each focus object the two top memories are recalled, already-shown and not-yet-folded ones dropped, six kept. Notebook pages are scored by how many entity names they share with the context (+1 for same locale); the top three are attached.

## Notebook and reflection

Reflection is a separate request with its own tools (`reflect`, `read_content`, `context_read`). It triggers at most hourly (sim time) when the day rolled over, something important happened, or six new episodes piled up. It writes notebook pages (`people/…`, `places/…`, `lessons/…`, `self.md`) with history, and a day summary `summaries/Dnnn.md`. It does not write `MemoryRecord`s.

## Sleep

A resident sleeps by doing a timed prop action whose completion is `Sleep`; `World::awake(id)` is false meanwhile, and the runtime plans no turns. There is no sleep-time processing yet.

## Model calls

The agent builds `AgentRequest { actor_id, session_id, stamp, messages, tools, max_output_tokens }`. The host sends it (`host/src/agent_worker.rs`, `host/src/llm.rs`) and returns a `ModelReply`. One-off text calls use `completion::prompt_body` + `host::llm::chat_completion(target, prompt)`; examples are the persona generator (`template_prompt` / `parse_template`) and the town planner (`planner_prompt` / `parse_plan`).

## Tests

One binary, `crates/agent/tests/agent.rs`, fixtures in `tests/support/`. Memory recall: `tests/resident_interactions.rs`; notebook and reflection: `tests/resident_notebook.rs`.
