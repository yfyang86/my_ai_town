# Game modes

A game mode is a rule set with its own goal, timetable and ending, layered on the ordinary town. At most one is open per world. It is saved with the world as `{"<id>": {...}}`.

## The contract (`crates/kernel/src/game_mode/mod.rs`)

```rust
trait Rules {
    const ID: &'static str;
    fn boundary(&self, world: &World) -> f64;          // minutes until the mode next needs to act
    fn advance(&mut self, world: &mut World);          // settle at world.minutes()
    fn rates(&self, …) {}                              // extra bodily cost for an actor
    fn view(&self, world: &World) -> GameModeView;     // broadcast with the roster
    fn context(&self, world: &World, actor: &str) -> Option<GameModeContext>; // the model's column
    fn tempo(&self, world: &World) -> f64 { 1.0 }      // speed multiplier, clamped 0.25–8
    fn tool(&self, world: &World, actor: &str) -> Option<String> { None }     // mode_action description
    fn act(&mut self, world, actor, action: &GameModeAction) -> Result<String>;
    fn clash(&mut self, world, actor, target, text) {} // someone used force on someone
    fn judge(&self, world, actor, draft: &ActionDraft) -> Result<()> { Ok(()) } // may refuse a core action
    fn takes_params(&self) -> bool { false }           // mode_action gets a `with` map
}
```

The core calls these and nothing else. The mode is taken out of the world while it runs, so it may read and write the world freely.

| Hook | Called from |
| --- | --- |
| `boundary`, `advance` | `kernel/src/simulation.rs` |
| `rates` | `kernel/src/physiology/` |
| `view` | `kernel/src/projection.rs` (roster) |
| `context` | `agent/src/observation.rs` — added under `GameModeContext::key()` |
| `tool`, `act` | `agent/src/tools.rs` — the `mode_action` tool |
| `tempo` | `host/src/runtime.rs` |
| `clash` | `kernel/src/force.rs`, prop harm in physiology |
| `judge` | `kernel/src/action_commands.rs`, before an agent's action group is queued (not for the player's avatar). One refusal rejects the whole group |
| `takes_params` | `agent/src/tools.rs` — adds `with` to the `mode_action` schema only when true |

What a mode does to the world today: write journal facts, send notices, interrupt actions, log (including speech), place and remove props, hurt or remove residents. It does not teleport anyone and has no generic way to forbid core actions.

## Wire shapes (`crates/protocol/src/game_mode/`)

| Type | Role |
| --- | --- |
| `GameModeChoice` | Client → host: which mode and its settings. Tagged by `kind`. Absent = close |
| `GameModeView` | Host → clients with every roster. Tagged by `kind` |
| `GameModeContext` | What one resident's model sees. Untagged; keyed by `key()` |
| `GameModeAction` | `{action, target?, text?, reason?, with?}` — shared by all modes; `with` is a map of short named strings, absent unless the open mode asks for it |

## The modes

`storyline` (故事模式) is described separately below; the three hand-coded modes compare like this:

| | `tide` 大逃杀 | `werewolf` 狼人杀 | `swordholder` 执剑人之夜 |
| --- | --- | --- | --- |
| Shape | Shrinking safe circle over four days, a boat at the end | Judge state machine: day talk and vote, night role actions | Judge state machine: one election day, handover, a ten-minute decision |
| Settings at open | `seed` | `seed`, `day_hours` | `seed`, `day_hours`, `rules` |
| `mode_action` | none | speak, vote, role actions | speak / pass, vote / abstain, appoint, press / hold |
| Touches bodies | wading costs energy | kills and exiles | no |
| Places | geometry of the walk map | named locales | locales found by name: 广场, 联合国/大厅, 天线 |

### 执剑人之夜 in detail

Seats follow move-in order: 1 holder, 2–3 candidates, 4 envoy, the rest voters (6–12 players).

`Waiting → Day → Gather → Speech → Vote → (Tiebreak) → Interval → Handover → Attack → Aftermath → Over`

| Constant | Value | Meaning |
| --- | --- | --- |
| `BELL` / `MEET` / `HANDOVER_AT` | 16:30 / 17:00 / 19:00 | Bell, speeches, handover |
| `SPEAK_MINUTES`, `VOTE_MINUTES` | 15, 15 | Deadline per speaker; voting window |
| `APPOINT_MINUTES`, `HANDOVER_MINUTES` | 10, 10 | Tie-break by the holder; handover remark |
| `ATTACK_DELAY`, `ATTACK_MINUTES` | 5, 10 | Droplet arrives after handover; time to decide |
| `STATEMENT_MINUTES` | 15 | Envoy's closing statement |
| `FREE_TEMPO` | 3× | Speed in phases without a deadline |
| Text limits | 100 / 200 / 400 chars | Remark / stance / address |

The judge advances on two conditions only: the person whose turn it is has acted, or the deadline passed. It wakes only the person who must act; everyone else gets the line written into their experience. Endings: `Broadcast` (pressed), `Failed` (held or timed out), `Peace` (the attack is switched off).

#### Rules as data

The constants above are defaults. `GameModeChoice::Swordholder.rules` (`SwordholderRules`, every field optional) overrides them per game:

- **Times** — `bell`, `meet`, `handover` (minute of the day).
- **Deadlines** — `speak_minutes`, `vote_minutes`, `appoint_minutes`, `handover_minutes`, `attack_delay`, `attack_minutes`, `statement_minutes`; `free_tempo`.
- **Switches** — `opening_remark`, `stances`, `handover_remark`, `attack`, `statement`.

`Timetable::resolve` fills defaults and validates once at open (ranges, bell ≤ meet < handover, the free day must fit before the bell); a bad value is refused and the previous mode stays. The resolved `Timetable` is saved with the game and omitted from the save when it equals the defaults, so an unedited game is byte-identical to one from before rules existed. Announcements keep their wording; only clock times, minute counts and the clauses for switched-off phases change.

The cast lives in `frontend/town-shared/src/prefabs/swordholder.ts` and can be edited per game in the setup screen.

### Storyline in detail

The one mode whose content is entirely data. `GameModeChoice::Storyline { seed, script }` carries a `StorylineScript`; the kernel validates it at open and keeps it in the save.

| Part | What it does |
| --- | --- |
| `cast[]` | By seat (move-in order). `role` is public; `goal` and `secret` reach only that seat's model |
| `beats[]` | At `day` / `minute`: everyone hears `announce`; seats in `who` are woken and also hear `brief`; an optional `scene` has `speakers` talk in turn, each with a deadline and a length cap |
| `milestones[]` | `beat` — a beat has run · `gather` — enough of the named seats are at a place at once (checked every 5 minutes) · `declare` — a named seat reports it with `mode_action declare` and a sentence of evidence. `secret` hides it from everyone who cannot declare it |
| `ends_day` / `ends_minute`, `epilogue` | The close: reached and missed milestones are announced |

It is deliberately soft. Nobody is moved; a beat says what happens and a resident decides what to do about it. A scene only records whether the speaker was at the place. The next beat waits while a scene is still running. Places are matched by substring against locale names; a place the map lacks is treated as description only.

`mode_action`: `speak` / `pass` on your scene turn, `declare` with `target` = milestone id.

Limits (refused at open with a reason): 2–12 seats, 1–24 beats in time order, ≤ 12 milestones, ids unique and resolvable, 14 days at most, text lengths capped.

The first script is hand-written: `frontend/town-shared/src/prefabs/lighthouse.ts` (灯塔之约).

## Rulesets

A **ruleset** is an optional layer of mechanics on the storyline mode: what characters attempt is resolved by rules and dice instead of by declaration. `GameModeChoice::Storyline.ruleset` selects one; without it the storyline is byte-for-byte what it was.

Only one exists: **fifth-edition rules** from SRD 5.1 (`RulesetChoice::Srd5`), in `kernel/src/game_mode/srd5.rs`. Attribution is in `NOTICE` and `licenses/SRD-5.1.txt`; only SRD material may be added, and the product calls it 「第五版规则」.

| Piece | Where | Notes |
| --- | --- | --- |
| Dice | `game_mode/dice.rs` | Seeded splitmix64 with rejection sampling; state is saved, so a reload continues the same sequence. The kernel is the only thing that rolls |
| Class data | `data/rules/5e/classes.json`, embedded at compile time | Hit die, saving-throw proficiencies, skill list and count, expertise count. Every entry cites its SRD section; a test enforces it |
| Sheet | `Srd5Sheet` | Class, level 1–3, six abilities, skills (class picks plus two from background), expertise (rogue). Validated at open against the class data. Hit points and all bonuses are derived, never typed in |
| Check | `StorylineCheck` on a beat | DC, who may try, one or more ways. A way is an ability with an optional skill, or a saving throw (`save`). Open once its beat has fired |
| Resolution | `Srd5::attempt` | d20 + ability modifier + proficiency (twice with expertise) vs. DC. One attempt per seat unless `repeat`; a passed check closes for everyone |
| Advantage / disadvantage | `CheckEdge` on a check | Applies when a milestone has been reached and/or the character is at a place, for the named seats. Two dice, keep the higher or lower; both at once cancel |
| Contest | `StorylineCheck.against` | The other seat responds with their best listed way, rolled by the kernel; the challenger must beat it, ties fail. Both rolls are logged |
| Passive check | `StorylineCheck.passive` | No roll: when the beat fires, 10 + best bonus vs. DC. Those who make it are told privately; the highest is credited |
| Help | `mode_action help` | `with.seat` names whom; the helper gives up their own attempt, must be in the same place, and the helped character's next attempt has advantage |
| Milestone | `StorylineWhen::Check` | Reached when someone passes; invisible when no ruleset is on |
| Roll log | `RulesetView.rolls` | Formula, dice, modifier, total, DC, outcome — last 40 |

`mode_action check`: `target` is the check id; `with.skill` or `with.ability` picks the way (optional when there is only one; a save's key is `<ability>_save`). Each resident's context lists the checks open to them with *their own* bonus for each way, so the model can choose its best approach or leave it to someone better.

With a ruleset on, `judge` refuses shoving, robbing and stealing: harming someone will go through combat rules. The plan for the rest (combat, magic, rest, progression, a model narrator) is in `Sprints/ruleset-5e/plan.md`.

## Adding a mode — checklist

1. `crates/kernel/src/game_mode/<mode>.rs` implementing `Rules`; add to `GameMode`, `each!`, `World::set_game_mode`.
2. `crates/protocol/src/game_mode/<mode>.rs`; add a variant to `GameModeView`, `GameModeChoice`, `GameModeContext` (+ `key()`).
3. `crates/host/src/world_commands.rs`: seed roll and the receipt text in `save_game_mode`.
4. `crates/protocol/tests/wire/samples.rs`: a roster sample and a choice sample.
5. `frontend/town-shared/src/protocol.ts`: the mirrored types.
6. `frontend/town-2d/src/map/hd2d/gameModes/<mode>.ts` implementing `GameModeLook`; add to `gameModeLooks`.
7. `frontend/town-shared/src/audio/gameModes/<mode>.ts`; add to `GAME_MODE_MUSIC`.
8. Optional: a prefab in `frontend/town-shared/src/prefabs/`.
9. A kernel test in `crates/kernel/tests/game_mode.rs` that plays a full game through contexts and actions, saves and restores mid-game, and ends with `no_trace` after closing.

The compiler and the wire test point at whatever is missing.
