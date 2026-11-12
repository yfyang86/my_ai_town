//! 故事草稿里第五版规则的那一半：每个角色一张角色卡、节拍上的检定和遭遇战、靠它们达成的里程碑。
//! 模型写的东西宽进严出：认不得的职业、技能、武器、法术、生物在这里换成默认的或者去掉，
//! 每一处都留一句说明（draft.notes），不让一个错处废掉整份草稿；最后每一项都交给内核查过。
use super::{clip, number, seats, slug};
use kernel::game_mode::{check_sheet, check_storyline, ruleset_words, RulesetWords};
use protocol::{Srd5Sheet, StorylineCheck, StorylineEncounter, StorylineScript};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

const KEYS: [&str; 6] = ["str", "dex", "con", "int", "wis", "cha"];
/// 标准属性值：六个数各用一次
const ARRAY: [u64; 6] = [15, 14, 13, 12, 10, 8];
const CHECKS_PER_BEAT: usize = 3;
const CHECKS_MAX: usize = 8;
const ENCOUNTERS_MAX: usize = 2;
const FOES_MAX: usize = 6;
const SPELLS_MAX: usize = 6;

/// 模型没说哪项属性要紧时，各职业按这个先后分那六个数
fn priority(class: &str) -> [&'static str; 6] {
    match class {
        "rogue" => ["dex", "int", "cha", "con", "wis", "str"],
        "cleric" => ["wis", "con", "str", "cha", "int", "dex"],
        "wizard" => ["int", "dex", "con", "wis", "cha", "str"],
        _ => ["str", "con", "dex", "wis", "cha", "int"],
    }
}

fn strings(value: &Value) -> Vec<String> {
    let mut seen = BTreeSet::new();
    value
        .as_array()
        .into_iter()
        .flatten()
        .map(|v| clip(v, 40).to_ascii_lowercase().replace([' ', '-'], "_"))
        .filter(|s| !s.is_empty() && seen.insert(s.clone()))
        .collect()
}

fn known(list: &[(String, String)], value: &Value) -> Option<String> {
    let id = clip(value, 40)
        .to_ascii_lowercase()
        .replace([' ', '-'], "_");
    list.iter()
        .find(|(known, name)| *known == id || *name == id)
        .map(|(known, _)| known.clone())
}

/// 一张角色卡：职业认不得的当战士；属性按模型给的高低把标准的六个数重新分一遍；
/// 技能、武器、甲、法术只留规则里有、这个职业能用的。
fn sheet(raw: &Value, words: &RulesetWords) -> Value {
    let wanted = clip(&raw["class"], 20).to_ascii_lowercase();
    let class = words
        .classes
        .iter()
        .find(|c| c.0 == wanted || c.1 == wanted)
        .unwrap_or(&words.classes[0]);
    let (id, _, class_skills, count, caster) = class;
    let level = number(&raw["level"]).unwrap_or(1).clamp(1, 3);
    let order = priority(id);
    let mut ranked: Vec<&str> = KEYS.to_vec();
    ranked.sort_by_key(|key| {
        let given = number(&raw["abilities"][key]).unwrap_or(0);
        let place = order.iter().position(|k| k == key).unwrap_or(6);
        (std::cmp::Reverse(given), place)
    });
    let abilities: Map<String, Value> = ranked
        .iter()
        .zip(ARRAY)
        .map(|(key, score)| (key.to_string(), json!(score)))
        .collect();
    let picked: Vec<String> = strings(&raw["skills"])
        .into_iter()
        .filter(|s| words.skills.iter().any(|(id, _, _)| id == s))
        .collect();
    let mut skills: Vec<String> = picked
        .iter()
        .filter(|s| class_skills.contains(s))
        .take(*count)
        .cloned()
        .collect();
    for s in class_skills {
        if skills.len() >= *count {
            break;
        }
        if !skills.contains(s) {
            skills.push(s.clone());
        }
    }
    let background: Vec<String> = picked
        .iter()
        .filter(|s| !skills.contains(s))
        .take(2)
        .cloned()
        .collect();
    skills.extend(background);
    let mut one = json!({ "class": id, "level": level, "abilities": abilities, "skills": skills });
    let expertise: Vec<String> = strings(&raw["expertise"])
        .into_iter()
        .filter(|s| id == "rogue" && skills.contains(s))
        .take(2)
        .collect();
    if !expertise.is_empty() {
        one["expertise"] = json!(expertise);
    }
    if let Some(weapon) = known(&words.weapons, &raw["weapon"]) {
        one["weapon"] = json!(weapon);
    }
    if let Some(armor) = known(&words.armor, &raw["armor"]) {
        one["armor"] = json!(armor);
    }
    if raw["shield"].as_bool() == Some(true) {
        one["shield"] = json!(true);
    }
    if *caster {
        let top = if level >= 3 { 2 } else { 1 };
        let usable = |s: &&(String, String, u32, Vec<String>)| s.3.contains(id) && s.2 <= top;
        let wanted = strings(&raw["spells"]);
        let mut spells: Vec<String> = wanted
            .iter()
            .filter(|w| words.spells.iter().filter(usable).any(|s| s.0 == **w))
            .take(SPELLS_MAX)
            .cloned()
            .collect();
        if spells.is_empty() {
            // 没挑的给一份默认的：戏法加两个一环法术
            let of = |level: u32, n: usize| {
                words
                    .spells
                    .iter()
                    .filter(usable)
                    .filter(move |s| s.2 == level)
                    .take(n)
                    .map(|s| s.0.clone())
            };
            spells = of(0, 2).chain(of(1, 2)).collect();
        }
        one["spells"] = json!(spells);
    }
    one
}

/// 理过还不合规则的角色卡一步步退：去掉专精和装备，再不行给一张什么都不带的战士。
fn sound_sheet(raw: &Value, seat: usize, words: &RulesetWords, notes: &mut Vec<String>) -> Value {
    let ok = |v: &Value| {
        serde_json::from_value::<Srd5Sheet>(v.clone())
            .map_err(|e| e.to_string())
            .and_then(|s| check_sheet(&s).map_err(|e| e.to_string()))
    };
    let full = sheet(raw, words);
    if raw.is_object()
        && !raw["class"].is_null()
        && full["class"] != json!(clip(&raw["class"], 20).to_ascii_lowercase())
    {
        notes.push(format!(
            "{seat} 号的角色卡：规则里没有「{}」这个职业，改成了{}",
            clip(&raw["class"], 20),
            words.classes[0].1
        ));
    }
    let Err(why) = ok(&full) else {
        return full;
    };
    let mut plain = full.clone();
    for key in ["expertise", "weapon", "armor", "shield"] {
        plain.as_object_mut().map(|o| o.remove(key));
    }
    if ok(&plain).is_ok() {
        notes.push(format!(
            "{seat} 号的角色卡不合规则（{why}），去掉了专精和装备"
        ));
        return plain;
    }
    notes.push(format!(
        "{seat} 号的角色卡不合规则（{why}），换成了一张默认的"
    ));
    sheet(&Value::Null, words)
}

/// 一种试法：技能认得的话属性不对就用它本来的属性；只有属性的要是六项之一
fn way(raw: &Value, words: &RulesetWords) -> Option<Value> {
    let ability = clip(&raw["ability"], 8).to_ascii_lowercase();
    let ability = KEYS.contains(&ability.as_str()).then_some(ability);
    let skill = strings(&json!([raw["skill"]])).into_iter().next();
    let skill = skill.and_then(|s| {
        words
            .skills
            .iter()
            .find(|(id, name, _)| *id == s || *name == s)
    });
    match (skill, ability) {
        (Some((id, _, base)), ability) => {
            Some(json!({ "ability": ability.unwrap_or_else(|| base.clone()), "skill": id }))
        }
        (None, Some(ability)) if raw["save"].as_bool() == Some(true) => {
            Some(json!({ "ability": ability, "save": true }))
        }
        (None, Some(ability)) => Some(json!({ "ability": ability })),
        (None, None) => None,
    }
}

fn ways(raw: &Value, words: &RulesetWords) -> Vec<Value> {
    let mut list: Vec<Value> = Vec::new();
    for w in raw.as_array().into_iter().flatten() {
        if let Some(w) = way(w, words) {
            if !list.contains(&w) && list.len() < 4 {
                list.push(w);
            }
        }
    }
    list
}

fn check(
    raw: &Value,
    people: usize,
    words: &RulesetWords,
    used: &mut BTreeSet<String>,
) -> Option<Value> {
    let title = clip(&raw["title"], 40);
    let ways = ways(&raw["ways"], words);
    if title.is_empty() || ways.is_empty() {
        return None;
    }
    let id = slug(&raw["id"], format!("check-{}", used.len() + 1), used);
    let mut one = json!({
        "id": id,
        "title": title,
        "detail": Some(clip(&raw["detail"], 300)).filter(|d| !d.is_empty()).unwrap_or_else(|| title.clone()),
        "dc": number(&raw["dc"]).unwrap_or(13).clamp(5, 25),
        "ways": ways,
    });
    let among = seats(&raw["seats"], people);
    if !among.is_empty() && among.len() < people {
        one["seats"] = json!(among);
    }
    for flag in ["repeat", "passive"] {
        if raw[flag].as_bool() == Some(true) {
            one[flag] = json!(true);
        }
    }
    // 对抗：对方的座号和他能用的办法都要对得上
    let rival = seats(&json!([raw["against"]["seat"]]), people);
    let answers = self::ways(&raw["against"]["ways"], words);
    if let (Some(seat), false) = (rival.first(), answers.is_empty()) {
        one["against"] = json!({ "seat": seat, "ways": answers });
    }
    Some(one)
}

fn encounter(
    raw: &Value,
    people: usize,
    words: &RulesetWords,
    used: &mut BTreeSet<String>,
) -> Option<Value> {
    let mut zones: Vec<String> = Vec::new();
    for z in raw["zones"].as_array().into_iter().flatten() {
        let name = clip(z, 8);
        if !name.is_empty() && !zones.contains(&name) && zones.len() < 5 {
            zones.push(name);
        }
    }
    if zones.len() < 2 {
        zones = vec!["近处".into(), "远处".into()];
    }
    let last = zones.len() as u64 - 1;
    let foes: Vec<Value> = raw["foes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| {
            let creature = known(&words.creatures, &f["creature"])?;
            let mut one = json!({ "creature": creature, "zone": number(&f["zone"]).unwrap_or(last).min(last) });
            let name = clip(&f["name"], 8);
            if !name.is_empty() {
                one["name"] = json!(name);
            }
            Some(one)
        })
        .take(FOES_MAX)
        .collect();
    if foes.is_empty() {
        return None;
    }
    let title = Some(clip(&raw["title"], 40))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "一场架".into());
    let mut one = json!({
        "id": slug(&raw["id"], format!("fight-{}", used.len() + 1), used),
        "title": title,
        "zones": zones,
        "party_zone": number(&raw["party_zone"]).unwrap_or(0).min(last),
        "foes": foes,
    });
    let party = seats(&raw["party"], people);
    if !party.is_empty() && party.len() < people {
        one["party"] = json!(party);
    }
    Some(one)
}

/// 草稿理好了平常的那一半之后：开着规则就把角色卡、检定、遭遇战理好挂上去，没开就都拿掉。
/// 节拍上的 checks、encounter 和里程碑的 check、encounter 进来时还是模型的原样。
pub(super) fn settle(draft: &mut Value, raw: &Value, on: bool) -> Result<(), String> {
    let people = draft["cast"].as_array().map_or(0, Vec::len);
    // 先把节拍上的原样摘下来，剩下的是一本不带规则的剧本
    let mut extras: Vec<(Value, Value)> = Vec::new();
    for beat in draft["beats"].as_array_mut().into_iter().flatten() {
        let Some(beat) = beat.as_object_mut() else {
            continue;
        };
        extras.push((
            beat.remove("checks").unwrap_or(Value::Null),
            beat.remove("encounter").unwrap_or(Value::Null),
        ));
    }
    let needs_rules = |m: &Value| matches!(m["when"]["kind"].as_str(), Some("check" | "encounter"));
    draft.as_object_mut().map(|d| d.remove("notes"));
    if !on {
        if let Some(list) = draft["milestones"].as_array_mut() {
            list.retain(|m| !needs_rules(m));
        }
        draft.as_object_mut().map(|d| d.remove("rules"));
        return Ok(());
    }
    let words = ruleset_words();
    let mut notes: Vec<String> = Vec::new();
    // 角色卡：改过的草稿在 rules.sheets 里，模型刚写的在各人的 sheet 里
    let people_raw: Vec<&Value> = raw["cast"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c.is_object())
        .collect();
    let sheets: Vec<Value> = (0..people)
        .map(|i| {
            let edited = &raw["rules"]["sheets"][i];
            let source = if edited.is_object() {
                edited
            } else {
                people_raw.get(i).map_or(&Value::Null, |c| &c["sheet"])
            };
            sound_sheet(source, i + 1, &words, &mut notes)
        })
        .collect();
    // 检定和遭遇战：一项一项放进剧本里让内核查，不合规的去掉
    // 试的时候用的剧本不带靠检定、遭遇战达成的里程碑：它们指的那一项这时还没挂上去
    let mut bare = draft.clone();
    if let Some(list) = bare["milestones"].as_array_mut() {
        list.retain(|m| !needs_rules(m));
    }
    let base: StorylineScript = super::script(&bare)?;
    let mut used = BTreeSet::new();
    let mut renamed: Vec<(String, String)> = Vec::new();
    let (mut checks_kept, mut fights_kept) = (0, 0);
    for (i, (raw_checks, raw_fight)) in extras.iter().enumerate() {
        let title = clip(&draft["beats"][i]["title"], 40);
        let mut checks: Vec<Value> = Vec::new();
        for c in raw_checks.as_array().into_iter().flatten() {
            if checks.len() >= CHECKS_PER_BEAT || checks_kept >= CHECKS_MAX {
                break;
            }
            let Some(one) = check(c, people, &words, &mut used) else {
                notes.push(format!(
                    "节拍「{title}」上有一项检定没写清楚试什么、怎么试，去掉了"
                ));
                continue;
            };
            let mut probe = base.clone();
            let sound = serde_json::from_value::<StorylineCheck>(one.clone())
                .map_err(|e| e.to_string())
                .and_then(|typed| {
                    probe.beats[i].checks = vec![typed];
                    check_storyline(&probe).map_err(|e| e.to_string())
                });
            match sound {
                Ok(()) => {
                    renamed.push((clip(&c["id"], 64), clip(&one["id"], 64)));
                    checks.push(one);
                    checks_kept += 1;
                }
                Err(why) => notes.push(format!(
                    "检定「{}」不合规则（{why}），去掉了",
                    clip(&one["title"], 40)
                )),
            }
        }
        if !checks.is_empty() {
            draft["beats"][i]["checks"] = json!(checks);
        }
        if raw_fight.is_object() {
            if fights_kept >= ENCOUNTERS_MAX {
                notes.push(format!("遭遇战太多，节拍「{title}」上的那场去掉了"));
                continue;
            }
            let Some(one) = encounter(raw_fight, people, &words, &mut used) else {
                notes.push(format!(
                    "节拍「{title}」上的遭遇战没有规则里有的对手，去掉了"
                ));
                continue;
            };
            let mut probe = base.clone();
            let sound = serde_json::from_value::<StorylineEncounter>(one.clone())
                .map_err(|e| e.to_string())
                .and_then(|typed| {
                    probe.beats[i].encounter = Some(typed);
                    check_storyline(&probe).map_err(|e| e.to_string())
                });
            match sound {
                Ok(()) => {
                    renamed.push((clip(&raw_fight["id"], 64), clip(&one["id"], 64)));
                    draft["beats"][i]["encounter"] = one;
                    fights_kept += 1;
                }
                Err(why) => notes.push(format!(
                    "遭遇战「{}」不合规则（{why}），去掉了",
                    clip(&one["title"], 40)
                )),
            }
        }
    }
    // 靠检定、遭遇战达成的里程碑：指的那一项还在才留
    if let Some(list) = draft["milestones"].as_array_mut() {
        list.retain_mut(|m| {
            let Some(kind) = m["when"]["kind"].as_str().map(str::to_owned) else {
                return false;
            };
            if kind != "check" && kind != "encounter" {
                return true;
            }
            let wanted = clip(&m["when"][kind.as_str()], 64);
            match renamed
                .iter()
                .find(|(old, new)| *old == wanted || *new == wanted)
            {
                Some((_, new)) => {
                    m["when"][kind.as_str()] = json!(new);
                    true
                }
                None => {
                    notes.push(format!(
                        "里程碑「{}」指的检定或遭遇战不在了，去掉了",
                        clip(&m["title"], 40)
                    ));
                    false
                }
            }
        });
    }
    draft["rules"] = json!({ "kind": "srd5", "sheets": sheets });
    if !notes.is_empty() {
        draft["notes"] = json!(notes);
    }
    Ok(())
}

/// 提示词里规则的那一段：能用的职业、技能、武器、甲、法术、生物都列出来，模型只能从里面挑
pub(super) fn prompt() -> String {
    let words = ruleset_words();
    let pairs = |list: &[(String, String)]| {
        list.iter()
            .map(|(id, name)| format!("{id} {name}"))
            .collect::<Vec<_>>()
            .join("、")
    };
    let classes = words
        .classes
        .iter()
        .map(|(id, name, skills, count, _)| {
            format!("{id} {name}（从 {} 里挑 {count} 项）", skills.join("/"))
        })
        .collect::<Vec<_>>()
        .join("；");
    let skills = words
        .skills
        .iter()
        .map(|(id, name, ability)| format!("{id} {name}（{ability}）"))
        .collect::<Vec<_>>()
        .join("、");
    let spells = words
        .spells
        .iter()
        .filter(|s| s.2 <= 1)
        .map(|(id, name, level, classes)| {
            format!("{id} {name}（{}，{level} 环）", classes.join("/"))
        })
        .collect::<Vec<_>>()
        .join("、");
    format!(
        r#"这个故事开着第五版规则：成不成说不准的事要掷骰。除了上面的结构，再写三样东西。
一、cast 里每个人多一项 "sheet"：
  {{"class": "职业编号", "abilities": {{"str": 15, "dex": 13, "con": 14, "int": 8, "wis": 12, "cha": 10}}, "skills": ["技能编号"], "weapon": "武器编号", "armor": "甲的编号", "shield": false, "spells": ["法术编号"]}}
  - class 按这个人在故事里是谁来挑，只能是：{classes}。另外任选两项技能当出身。
  - abilities 把 15、14、13、12、10、8 六个数各用一次分给六项属性（str 力量、dex 敏捷、con 体质、int 智力、wis 感知、cha 魅力）。
  - 技能表：{skills}。
  - weapon 从这里挑，不带就不写：{weapons}。armor 从这里挑，不穿就不写：{armor}。
  - spells 只有 cleric 和 wizard 写，从本职业能用的里挑 3–5 个：{spells}。
二、节拍可以带 "checks"（一个节拍最多 3 项，整个故事 3–6 项）：这个节拍发生之后角色可以试的、成不成说不准的事。
  {{"id": "英文小写编号", "title": "试什么，12 字以内", "detail": "什么算成", "dc": 13, "seats": [1, 2], "ways": [{{"ability": "cha", "skill": "persuasion"}}]}}
  - dc 用 10（容易）、13（中等）、15（偏难）、18（难）；seats 是能试的座号，不写表示谁都能试；ways 写一到三种办法，skill 可以不写。
三、最多一两个节拍可以带 "encounter"：一场按回合打的架，故事里真要动手时才写。
  {{"id": "英文小写编号", "title": "这场架的名字", "zones": ["两到四块相邻的地方的名字"], "party": [1, 2], "party_zone": 0, "foes": [{{"creature": "生物编号", "name": "在故事里叫什么", "zone": 1}}]}}
  - creature 只能从这里挑：{creatures}。对手不要多：一级的角色两三个人对两三个弱的对手就够凶险了，打输了会死人。
milestones 的 when 另有两种：{{"kind": "check", "check": "检定编号"}} 有人通过了这项检定；{{"kind": "encounter", "encounter": "遭遇战编号"}} 这场架打赢了。
"#,
        weapons = pairs(&words.weapons),
        armor = pairs(&words.armor),
        creatures = pairs(&words.creatures),
    )
}
