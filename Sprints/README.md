# Sprints

Six sprints, one folder each. Every sprint runs the same loop:

**plan → dev → test & fix → plan next → commit → PR to `dev` → merge when CI is green**

- `sprint-N/plan.md` — goal, scope, acceptance checks, written before any code.
- `sprint-N/report.md` — what shipped, test results, defects found and fixed, what moved to the next sprint.
- Work happens on the sprint branch, is merged into `dev` by pull request, and the next sprint restarts from `dev`.
- `main` is untouched until all six sprints are done.

## Roadmap

| Sprint | Theme | Outcome |
| --- | --- | --- |
| 1 | Understand & document | Developer notes and repo wiki under `docs/wiki/`, `AGENTS.md` amended, this roadmap |
| 2 | Configurable scenarios | 执剑人之夜's timetable, phases and cast become data (`rules`), editable in the setup screen; every phase can be switched off or re-timed |
| 3 | Storyline mode (runtime) | A generic `storyline` game mode: scheduled beats, tracked milestones, soft goals; kernel + protocol + look + music |
| 4 | Storyline editor | Story/outline → LLM → `{characters, events, milestones, style}` harness, reviewed and edited in the page, then played; endpoints in both managers |
| 5 | Agent memory | SQLite FTS5 word index (Chinese-capable), importance · recency · relevance ranking, "dreaming" while asleep |
| 6 | Memory UI, hardening, release | Dreams visible in the memory page, cross-feature regression, docs and wiki brought up to date, `dev` ready for `main` |

**Status (2026-11-11):** all six sprints are merged into `dev` and from there into `main`; each `sprint-N/report.md` says what shipped, how it was checked, and what was left.

## Second roadmap: fifth-edition rules (sprints 7–12)

An optional ruleset on the storyline mode, built from SRD 5.1. The plan, scope and decisions are in `ruleset-5e/plan.md`.

| Sprint | Theme | State |
| --- | --- | --- |
| 7 | Ruleset framework, dice, scripted checks | merged into `dev` |
| 8 | Classes, saves, contests, advantage, passive checks, help | merged into `dev` |
| 9 | Combat: initiative, turns, attacks, death saves | merged into `dev` |
| 10 | Magic, long rest, experience and levels, duels | merged into `dev` |
| 11 | Model narrator, adventure drafting in the story editor | planned |
| 12 | Playtest, balance, cost report, release notes | planned |

## Decisions taken with the product owner (2026-11-11)

1. Docs live in the repo: `docs/wiki/` and `Sprints/`; `AGENTS.md` is amended to allow exactly these.
2. Feature 1 means **data-driven scenarios**, and that data is what the storyline editor writes.
3. Memory database is **bundled SQLite + FTS5** — no extra service or model, works offline on every platform.
4. Git flow: `dev` created from `main`; each sprint is a PR into `dev`, merged once CI passes.
5. Storyline strength: **scheduled beats + soft goals** — events fire on time, milestones are tracked, agents still decide.
6. Platforms: shared frontend; new manager endpoints exist in both the Node and the Rust manager. Only the desktop/web path can be run in the development environment; phone and desktop shells are compile-checked.
