# Changelog relative to mewamew/my_ai_town

This file records how this fork differs from the original [mewamew/my_ai_town](https://github.com/mewamew/my_ai_town). It is a synchronization snapshot, not a replacement for the player-facing release notes or the detailed commit history.

## Snapshot: 2026-11-14

- Fork `main`: [`216f5943`](https://github.com/yfyang86/my_ai_town/commit/216f5943), `feat: 更新模型目录并支持自定义模型`.
- Upstream `main`: [`203afb9d`](https://github.com/mewamew/my_ai_town/commit/203afb9d), `Merge PR #169 with Frostlight integration fixes`.
- Common ancestor: [`c6e44309`](https://github.com/mewamew/my_ai_town/commit/c6e44309), dated 2026-11-10.

The branches contain cherry-picked and reimplemented changes, so raw ahead/behind counts do not represent counts of unique features. Earlier shared history is not repeated here.

### Latest fork change: provider catalog and custom models

Commit [`216f5943`](https://github.com/yfyang86/my_ai_town/commit/216f5943) adds changes not currently in upstream:

- Replaces retired DeepSeek presets with `deepseek-flash` and `deepseek-v4-pro`, and makes Flash the shipped default.
- Updates the Kimi, GLM, MiniMax, and Alibaba Bailian catalogs from their current official documentation; checks Xiaomi MiMo and Volcengine Ark entries without changing their valid model IDs.
- Lets users add and remove custom model IDs per provider, choose a provider default, and edit the model during connection and onboarding.
- Persists user-added candidates in `providers.json` and shares them with town-creation, default-model, and resident-model selectors.
- Keeps existing provider configuration files readable and keeps API keys only in `credentials.json`.
- Adds catalog and persistence regressions, including a real manager save/read cycle and a headless-Chrome add/save/select flow with the external connection test mocked.

Validation for this change: Rust formatting, clippy, and workspace tests; frontend build, typecheck, formatting, and all 27 JavaScript tests; the headless-Chrome flow described above.

### Other fork-led changes since the common ancestor

These feature lines have no corresponding upstream commit subject in the comparison snapshot:

- Story mode and its scenario editor, including beats, scenes, milestones, and the Lighthouse scenario.
- Swordholder scheduling and scenario visibility controls.
- Word-search memory retrieval, ranked memory synthesis, dreams, and dream visibility in the sidebar.
- Optional fifth-edition rules: character data, dice and checks, combat, spells, rests, experience, duels, narrator rulings, and scenario-authored encounters.
- Generated pixel art for game modes and scenario-specific assets.
- Downloadable conversation, thought, world-event, and model-trace records.
- Onboarding continuation after a page refresh.
- Prompt Studio and updated default story-writing guidance.
- Milestone announcement reminders, encounter scaling for joining characters, mixed milestone completion methods, and time for announcements.
- Post-synchronization cleanup for bounded trace reads, transcript downloads, and the single-manager architecture.
- Iteration plans, developer wiki pages, and sprint reports documenting this work.

### Confirmed upstream cherry-picks

`git log --cherry-mark` identifies the following upstream/local pairs as patch-equivalent. They are synchronization, not fork-only features:

- `202b88ef` → `bc7ea3c9`: version 0.5.0.
- `197f74f6` → `15538f12`: restore saves while a resident is guiding someone.
- `d381e66e` → `4155ea2b`: stop precompressing frontend build output.
- `4fb182c7` → `23b52912`: allow `TOWN_MANAGER_BINARY` in `start-client`.
- `c97f7a5c` → `a7c7955c`: manager concurrency fixes for model settings and opening towns.
- `7d87ed54` → `c6eb9a15`: remember provider model names and apply resident models to others.
- `71649d25` → `8540838d`: seal recordings interrupted by a terminated world process.
- `b6e513d5` → `fe3a1929`: skip incomplete shadow resources on the first HD-2D frame.

### Same-topic changes with different patches

The following upstream and local commits share a subject or goal but are not patch-equivalent. They must be reviewed as divergent implementations rather than blindly cherry-picked again:

- Handshake timeout handling: upstream `b17df191`, local `b6f54fd0`.
- Smaller roster broadcasts: upstream `f188658d`, local `3e7a3fda`.
- Append-only history and background persistence: upstream `3961e18e`, local `18a371f1`.
- Bounded Agent trace logs: upstream `d05612e8`, local `f5611403`.
- A single Rust manager for development and preview: upstream `7ecbc41f`, local `51042eac`.
- Stable town/script ordering: upstream `0bbb7ea8`, local `bd83c091`.

### Upstream work not synchronized in this snapshot

The visible upstream-only feature series is Frostlight; no corresponding local Frostlight implementation was found:

- [`d4e99fea`](https://github.com/mewamew/my_ai_town/commit/d4e99fea): Frostlight ring-town survival, map, presentation, and music controls.
- [`7e7513ae`](https://github.com/mewamew/my_ai_town/commit/7e7513ae): Frostlight player guide.
- [`78da48a6`](https://github.com/mewamew/my_ai_town/commit/78da48a6): Frostlight survival and saving documentation.
- [`203afb9d`](https://github.com/mewamew/my_ai_town/commit/203afb9d): PR #169 integration fixes.

This section describes commit presence, not a promise that every same-topic patch above is identical in behavior. Review the divergent implementations before further synchronization.

## Refreshing this comparison

```sh
git fetch upstream --prune
git merge-base main upstream/main
git log --left-right --cherry-mark --oneline main...upstream/main
```

Update the snapshot hashes and the synchronized/unsynchronized sections after fetching a newer upstream or cherry-picking more work.
