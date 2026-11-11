# Sprint 2 — report

## Shipped

**执剑人之夜 is configurable.**

- `SwordholderRules` on the wire (`GameModeChoice::Swordholder.rules`): three clock times, seven deadlines, free-phase speed, and five switches — opening remark, voter stances, handover remark, droplet attack, closing statement.
- Kernel resolves and validates the rules once at open, saves them with the game, and omits them when they equal the defaults. An unedited game produces the same save and the same announcements as before.
- New ending `peace` when the attack is switched off; banner, ending card and music handle it.
- Setup screen: collapsed 「规则」 section (time pickers, minute fields, switches, 恢复默认) and 「人设」 section (per-seat name, intro, personality, situation, drives — this game only).
- Prefab options are data (`field` path instead of an `apply` function); `PrefabRule` describes the rule rows. This is the format the storyline editor will emit.

**Scripted towns are optional.**

- `GET/POST /api/towns/prefabs` in both managers, stored in `<root>/config/launcher.json`.
- 「显示哪些」 beside the 剧本 group on the selection page. Hiding only removes the card; a game already played is untouched.

## Tests

| Check | Result |
| --- | --- |
| `cargo clippy --workspace --all-targets -D warnings` | clean |
| `cargo test --workspace` | pass — 139 tests (+1) |
| `pnpm build`, `typecheck`, `format:check` | pass |
| `pnpm test` | 22 pass, 1 known root-only failure (`readonly.test.mjs`, see sprint 1) |

New or extended regression tests:

- Kernel: a full game with re-timed phases and four phases switched off ends in `peace` before noon; rules survive a mid-vote save/restore; reversed times and a free day that does not fit are refused; a default game writes no `rules` into the save.
- Wire: a `set_game_mode` sample carrying every rule, type-checked against `protocol.ts`.
- Node manager and Rust manager: hide list round-trips, duplicates collapse, a malformed id is refused.

End-to-end in a real browser against the mock model: toggled a scripted town off and on; opened 执剑人之夜, moved the bell/speeches/handover to 10:00/10:10/11:00, switched the attack off, renamed seat 2, started the game. The saved world carried exactly those rules and the renamed resident.

## Defects found and fixed during the sprint

1. **The show/hide control was unclickable.** Built as a popover above the card strip; the strip scrolls horizontally and clipped it. Rebuilt inline in the group row.
2. **Toggling a scripted town rebuilt the whole 3D preview.** It called the full refresh. Now only the list is redrawn, and the selection moves only if the selected town was the one hidden.
3. **A test edit landed in the wrong game mode** (the werewolf test also matched the search string). Caught by the compiler.

## Not done / moved

- Rules for 狼人杀 and 大逃杀 are not exposed. The mechanism (`PrefabRule`, a `rules` object on the choice) is generic; those two modes keep their fixed timetables until someone needs them changed.
- The cast editor changes text only; looks are still the prefab's.

## Process note

GitHub Actions do not run on this fork — PR #1 never received a check. From this sprint on, the local gate is the merge gate and each PR states its result.
