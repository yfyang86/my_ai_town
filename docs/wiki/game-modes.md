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
    fn rulings(&self, world) -> Vec<RulingRequest> { vec![] } // things waiting for the host's narrator model
    fn rule(&mut self, world, id, ruling: Option<&Ruling>) -> Result<String>; // the narrator's answer
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
| `rulings`, `rule` | `host/src/resident_driver.rs` (`narrate`), every tick. The kernel never calls a model; the host does and hands the answer back |

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

#### Combat (`game_mode/combat.rs`)

A beat may carry an **encounter**: a title, two to five named zones in a line, the creatures and where they stand, and which seats are drawn in. With a ruleset on, the fight starts when the beat fires and blocks later beats until it ends.

| Piece | Rule here |
| --- | --- |
| Data | `data/rules/5e/creatures.json`: ten creatures, nine weapons, four armours, each citing its SRD section |
| Combatants | Characters fight from their sheet (weapon, armour, shield → attack bonus, damage, AC). **Creatures exist only in the fight** — not on the map, no model calls; they follow simple tactics |
| Order | Initiative: d20 + Dexterity modifier, once, highest first |
| A turn | Optionally step to an adjacent zone, then one of: attack, cast, dodge, disengage, dash, help, second_wind, flee, pass |
| Reach | Melee needs the same zone; ranged reaches anywhere, with disadvantage if an enemy stands beside you |
| Leaving a zone | Each enemy there with a melee attack and an unused reaction makes an opportunity attack, unless you disengaged |
| Attack | d20 + bonus vs. AC; natural 1 misses, natural 20 hits and doubles the damage dice. Advantage against a prone target in melee or after an ally's help; disadvantage against a dodging target |
| At 0 hit points | A character falls unconscious and rolls a death save each turn: three successes stabilise, three failures kill, a 20 revives at 1. A creature at 0 is out |
| Ending | Won when no creature stands; lost when no character stands or all have fled; drawn after 20 rounds |
| Afterwards | Hit points carry on to the rest of the story; survivors at 0 come round with 1. **A character who dies in the fight dies in the world** (the kernel's ordinary death) |

A character's turn has a deadline of 15 game minutes — the same order as a scene turn; with 5 a real model missed four turns in a row. A timed-out turn is a dodge.

`combat.rs` does not touch the world: it takes dice and returns what happened as lines. `storyline.rs` announces them, wakes whoever is up, and applies deaths. That is why a thousand fights can be simulated in a unit test.

#### Magic, rest and growth

| Piece | Rule here |
| --- | --- |
| Data | `data/rules/5e/spells.json`: fourteen cleric and wizard spells (cantrips, 1st and 2nd level), slots by level, casting ability, experience thresholds and creature experience, second wind and sneak attack — each citing its SRD section |
| Prepared spells | `Srd5Sheet.spells`, validated at open: own class only, a level the character has slots for, at most eight |
| Kinds | `attack` (spell attack vs. AC), `save` (full damage on a failed save, half or none on a success; `zone` reach covers every enemy in a zone), `auto` (magic missile), `heal`, `bless`, `hold` |
| Derived | Spell attack = proficiency + casting modifier; save DC = 8 + the same. Never typed in |
| Slots | A cantrip costs none; anything else spends a slot of its level or higher (`with.slot`), and some spells gain dice or missiles from a higher slot. An illegal cast is refused before anything is rolled or spent |
| Concentration | One spell at a time. Taking damage forces a Constitution save (DC 10 or half the damage); failing or dropping to 0 ends the spell and what it granted |
| Healing | In a fight, `cast` on an ally; an unconscious character wakes. Outside a fight, `mode_action cast` heals someone in the same place; `prayer_of_healing` reaches everyone there and cannot be cast mid-fight |
| Class features | Fighter: second wind (1d10 + level, once per long rest). Rogue: sneak attack (extra d6s once a turn, with advantage or an ally beside the target) |
| Long rest | A real sleep in the world of at least six hours: on waking, hit points, slots and second wind are back, and the character is told. Tracked in `Storyline::rest` from `World::awake` |
| Experience | Winning an encounter splits its creatures' experience among the characters still alive; reaching a milestone in your own name gives 50. Levels 2 and 3 at 300 and 900: hit points rise, new slots are usable at once |
| Duel | `mode_action duel` with `target` a seat: two characters in the same place fight by the same turn rules until one drops. Nobody dies, the loser comes round with 1 hit point, no experience. This is the only way to use force on a person while a ruleset is on |

#### Narrator rulings

A character may try something the script has no check for: `mode_action attempt` with `text`. Not during a fight or a scene; one pending per character; three per game hour.

| Step | Where | What |
| --- | --- | --- |
| Ask | `Storyline::improvise` | The attempt is stored in the ruleset state (`Srd5.asks`) and survives a save |
| Call | `host::narrator`, `agent::narrator` | The host asks the *asking resident's own model* once, with a closed menu: `roll` (one of six abilities, optionally one of eighteen skills, DC 5 / 10 / 15 / 20 / 25, advantage / disadvantage / neither), `auto`, or `impossible`, plus a one-line note |
| Validate and roll | `Storyline::settle_ask` | Anything off the menu is discarded without a roll and the character is told to try another way. A legal `roll` is rolled by the kernel with the character's own bonus and logged like any check |
| Give up | `Storyline::expire_asks` | No answer in 30 game minutes: the attempt lapses and the character is told |

The narrator never rolls and never decides success. In mock-model mode the ruling is a fixed Strength (Athletics) DC 15.

Not yet: short rests and hit dice, loot and potions, spells that help a scripted check, action surge and channel divinity, conditions beyond prone / dodging / unconscious / held, hit points reflected in the kernel's health meter.

`mode_action check`: `target` is the check id; `with.skill` or `with.ability` picks the way (optional when there is only one; a save's key is `<ability>_save`). Each resident's context lists the checks open to them with *their own* bonus for each way, so the model can choose its best approach or leave it to someone better.

With a ruleset on, `judge` refuses shoving, robbing and stealing and points to `duel`. The plan for the rest (a model narrator, adventure drafting) is in `Sprints/ruleset-5e/plan.md`.

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
