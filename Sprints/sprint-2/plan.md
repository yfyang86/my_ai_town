# Sprint 2 — Configurable scenarios

## Goal

执剑人之夜 stops being a fixed script: its timetable, phases and cast are data the player can edit before a game, and every optional part can be switched off.

## Design

**Rules as data.** `GameModeChoice::Swordholder` gains an optional `rules` object. Every field is optional and falls back to today's constant, so an absent `rules` plays exactly the current game and old saves load unchanged.

| Field | Default | Effect |
| --- | --- | --- |
| `bell`, `meet`, `handover` | 16:30, 17:00, 19:00 | Minutes into the day |
| `speak_minutes`, `vote_minutes`, `appoint_minutes`, `handover_minutes` | 15, 15, 10, 10 | Deadlines |
| `attack_delay`, `attack_minutes`, `statement_minutes` | 5, 10, 15 | The droplet and the closing statement |
| `free_tempo` | 3 | Speed in phases without a deadline |
| `opening_remark` | on | The holder's line before the speeches |
| `stances` | on | Voters speak before voting |
| `handover_remark` | on | The holder's line at handover |
| `attack` | on | Off: the game ends at handover with a new ending, `peace` |
| `statement` | on | The envoy's closing statement |

Validation happens once at open (order of times, ranges); a bad value is refused with a reason and the previous mode stays.

**Declarative prefab options.** `PrefabOption.apply` (a function) is replaced by data: `{field, kind: 'number' | 'boolean' | 'choice', path}` so an option can target `rules.*`. This is the shape the storyline editor will emit in sprint 4.

**Setup screen.** A collapsed 「规则」 section for prefabs that declare advanced options: time pickers, minute steppers, switches. A 「恢复默认」 button.

**Editable cast.** In the lineup, each seat's persona (name, intro, personality, situation, drives) can be edited before the game starts; edits apply to this game only.

**Prefab visibility.** A manager-side setting lists hidden prefab ids, so a scripted town can be removed from the selection page and brought back. Both managers.

## Acceptance

- Kernel test: a full game with re-timed phases and `stances`, `opening_remark` off; a game with `attack` off ends in `peace`; default game transcript unchanged; mid-game save/restore keeps the rules.
- Wire: sample with `rules`; `protocol.ts` mirrors it.
- With no mode open, saves and prompts are unchanged (`no_trace`).
- Manager test: hide and show a prefab.
- Full gate green; PR merged into `dev`.

## Risks

- The built-in model was trained on today's wording — announcements keep their wording; only clock times and minute counts are substituted.
- `GameModeChoice` derives `Eq`; rules therefore use integer minutes and a tenths-based tempo.
