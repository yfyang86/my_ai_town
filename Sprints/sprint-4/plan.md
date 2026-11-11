# Sprint 4 — Storyline editor

## Goal

Type a story or an outline; get a playable town. When a model is connected, the editor asks it to set up the whole harness — characters, events, milestones, places and style — shows it for review and editing, then builds the town.

## Flow

1. Selection page → 「写一个故事」 (enabled when a model is connected; otherwise it says how to connect one).
2. Paste a story or outline (≤ 6000 chars), pick the number of characters (2–12) and the number of days (1–3).
3. `POST /api/towns/story` → the model returns a **story draft**:
   - `title`, `premise`, `worldview`
   - `style`: environment preset (from the generator's list), climate, whether needs are on
   - `places[]`: kind (from the generator's place kinds) and name
   - `cast[]`: persona fields (name, age, gender, occupation, intro, tags, personality, speech, background, drives) + `role`, `goal`, `secret`
   - `beats[]`, `milestones[]`, ending — the sprint 3 script
4. Review screen: every field editable; beats and milestones can be added, removed, re-timed; validation messages inline (same limits as the kernel).
5. 「建成小镇」 → generator spec from `places` → `/preview` → `/save` with `creation.scenario` = the draft.
6. First entry to the town opens the ordinary setup screen, fed by a prefab built from the scenario — so 规则/人设/models/needs work as for any scripted town.

## Work

| Layer | Change |
| --- | --- |
| `crates/agent/src/completion.rs` | `story_prompt(outline, cast, days, kinds, presets)` and `parse_story(reply)`: tolerant JSON extraction, then normalisation (ids slugged, seats clamped, beats sorted, texts clipped) so a slightly-off reply still yields a valid draft |
| `crates/host` | `town-map story` worker mode; mock mode returns a fixed draft |
| `crates/manager` | `POST /story` in-process; `creation.scenario` accepted and size-capped |
| `scripts/towns` | `POST /story` via the worker; `creation.scenario` accepted and size-capped |
| `frontend/town-shared/src/prefabs` | `scenario.ts`: scenario → `Prefab` (spec, cast, script); `prefabOf` falls back to the town's scenario |
| `frontend/town-2d/src/launcher` | Editor dialog: outline → draft → review → build |

## Acceptance

- Rust: parser test — a clean reply, a reply wrapped in prose and code fences, and a reply with out-of-range values all normalise to drafts the kernel accepts; garbage is refused with a message.
- Both managers: `/story` with the mock model returns a draft; `/save` round-trips `creation.scenario`; an oversized scenario is refused.
- Browser, mock model: outline → draft → edit one beat → build → town opens with the setup screen → game starts → storyline HUD shows the edited beat.
- Full local gate green; PR merged into `dev`.

## Risks

- **Model output quality varies.** Mitigation: strict schema in the prompt, one worked example, tolerant parsing, and the review screen — nothing is built without the user seeing it.
- **Place names must match beats.** The normaliser rewrites each beat's place to the nearest generated place name, or drops it.
- **Prompt size.** Outline capped at 6000 chars; reply capped by the transport's 256 KB limit.
