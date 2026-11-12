# Sprint 13 — Town log and download, large-cast playtest

Requested by the owner after `main` was updated.

## Goal

1. Bring `main`'s new commits (generated pixel art for mode HUDs, restructured README) into the working line.
2. **Record and download** everything said, thought and done in a town, and the agent traces, as JSON or another format.
3. Keep testing with a twelve-character story (the owner's wuxia outline), rules on.

## Scope

- Merge `origin/main`; resolve conflicts.
- Host: an always-on per-town log in `towns/<id>/transcript/` — `chat`, `thought`, `event`, `trace`, one JSONL file each.
- Both managers: `GET /api/towns/<id>/transcript?kinds=…`.
- Page: menu → 「记录」: choose kinds and format (JSON / JSONL / text), download.
- Real-model run: draft the outline with 12 characters and rules, play it, download the records.

## Acceptance

- Host test: a running world writes events, thoughts and traces; a reset starts a new world id in the same files.
- Both manager tests serve the route.
- Browser: three formats download and parse.
- Full local gate; PR into `dev`.
