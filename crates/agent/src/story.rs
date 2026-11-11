//! 故事编辑器：把一段故事或提纲交给模型，要回一份「故事草稿」——角色（人设加上在故事里的身份、目标、秘密）、
//! 地点和风格、节拍、里程碑、收场。草稿里剧本那一半就是故事模式（kernel 的 storyline.rs）开局时带的剧本。
//! 模型写的东西不一定守规矩：`tidy` 把它理一遍（编号改成合规的、座号和钟点收进范围、节拍按先后排、
//! 字数截到上限、指不到东西的引用去掉），再交给内核查一遍，过了才交给页面。页面改过的草稿走同一条路。
use crate::completion::{first_object, parse_unclosed, ModelError};
use protocol::StorylineScript;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

pub const OUTLINE_MAX_CHARS: usize = 6000;
pub const CAST: std::ops::RangeInclusive<u64> = 2..=12;
pub const DAYS: std::ops::RangeInclusive<u64> = 1..=3;
const PLACES_MAX: usize = 8;
const BEATS_MAX: usize = 24;
const MILESTONES_MAX: usize = 12;

/// 页面给的可选项：地点种类（编号、名字）和风格（编号、名字）
pub struct Choices {
    pub kinds: Vec<(String, String)>,
    pub presets: Vec<(String, String)>,
}

fn pairs(value: &Value, id: &str) -> Vec<(String, String)> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| {
            let key = v[id].as_str()?.trim();
            let name = v["name"].as_str()?.trim();
            (!key.is_empty() && key.len() <= 40 && name.chars().count() <= 40)
                .then(|| (key.to_string(), name.to_string()))
        })
        .take(40)
        .collect()
}

impl Choices {
    /// 请求里的 kinds（{kind,name}）和 presets（{id,name}）；住处不用模型排，按角色数自动给
    pub fn of(body: &Value) -> Result<Self, String> {
        let kinds: Vec<_> = pairs(&body["kinds"], "kind")
            .into_iter()
            .filter(|(k, _)| k != "home")
            .collect();
        let presets = pairs(&body["presets"], "id");
        if kinds.is_empty() || presets.is_empty() {
            return Err("缺少可用的地点种类或风格".into());
        }
        Ok(Self { kinds, presets })
    }
}

const PROMPT: &str = r#"你是一个故事编排者。下面是用户给的一段故事或提纲，请把它改编成一座小镇里由 CAST 个 AI 角色演出、历时 DAYS 天的群戏。
角色都是自己拿主意的 AI：你只安排「到点发生什么」「每个人想要什么」，不要替角色写台词，也不要规定结局，让故事可以走偏。

只输出一个 JSON 对象，不要任何解释，结构如下（钟点 minute 是当天第几分钟，0–1439；day 从 1 数）：
{
  "title": "故事名，12 字以内",
  "premise": "所有角色都知道的前情，200 字以内",
  "worldview": "这座小镇是什么样的地方、此刻的处境，300 字以内",
  "style": {"preset": "PRESET_IDS 之一", "needs": false},
  "places": [{"kind": "地点种类编号", "name": "这个地点在故事里的名字，8 字以内"}],
  "cast": [{
    "name": "名字", "age": "年龄数字", "gender": "男或女", "occupation": "职业",
    "intro": "一句话简介", "tags": ["三四个性格词"], "personality": "性格，80 字以内",
    "speech": "说话方式", "background": "经历，80 字以内", "drives": "平时想要什么",
    "role": "在故事里的身份（公开）", "goal": "这个故事里想达成什么（只有本人知道）",
    "secret": "不能让别人知道的事（没有就不写这一项）"
  }],
  "beats": [{
    "id": "英文小写编号", "title": "节拍名", "day": 1, "minute": 540,
    "place": "发生在哪个地点的名字（没有就不写）",
    "who": [1, 2],
    "announce": "所有人都听到的一段：发生了什么",
    "brief": "只对 who 里的人说的一段（没有就不写）",
    "scene": {"speakers": [1, 2], "minutes": 15, "chars": 300}
  }],
  "milestones": [{
    "id": "英文小写编号", "title": "里程碑名", "detail": "什么算达成",
    "when": {"kind": "beat", "beat": "节拍编号"},
    "secret": false
  }],
  "ends_day": DAYS, "ends_minute": 1260,
  "epilogue": "收场时念的一句"
}
规则：
- cast 正好 CAST 个，座号按数组次序从 1 数；who、speakers、seats 里都填座号。
- places 用 3–8 个，kind 只能从这张表里选：KINDS。每个角色的住处会自动安排，不要写住宅。
- beats 用 3–8 个，按时间先后排，安排在白天和傍晚（360–1320 分钟）；scene 是一场轮流说话的戏，speakers 按次序每人说一段，重要的当面交锋才用，没有就不写这一项。who 不写表示所有人。
- milestones 用 3–6 个，when 有三种：{"kind":"beat","beat":节拍编号} 这个节拍发生了；{"kind":"gather","place":地点名字,"seats":[座号],"count":人数} 这些人里至少几个同时到了这个地点；{"kind":"declare","seats":[座号]} 由这些人里的某一个宣布它发生了。结局性的事（谁赢了、谁认了错、买卖成没成）用 declare，交给当事人自己宣布。secret 为 true 时只有能宣布它的人知道。
- 收场要排在最后一个节拍之后。
- 用和提纲相同的语言写。

提纲：
"#;

/// 给模型的提示词
pub fn prompt(outline: &str, cast: u64, days: u64, choices: &Choices) -> String {
    let join = |list: &[(String, String)]| {
        list.iter()
            .map(|(id, name)| format!("{id} {name}"))
            .collect::<Vec<_>>()
            .join("、")
    };
    let ids = choices
        .presets
        .iter()
        .map(|(id, name)| format!("{id}（{name}）"))
        .collect::<Vec<_>>()
        .join("、");
    format!(
        "{}{}",
        PROMPT
            .replace("CAST", &cast.to_string())
            .replace("DAYS", &days.to_string())
            .replace("PRESET_IDS", &ids)
            .replace("KINDS", &join(&choices.kinds)),
        outline.trim()
    )
}

fn clip(value: &Value, max: usize) -> String {
    let text = match value {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    };
    text.chars().take(max).collect()
}

fn number(value: &Value) -> Option<u64> {
    match value {
        Value::Number(n) => n.as_f64().map(|f| f.max(0.0).round() as u64),
        Value::String(s) => s
            .trim()
            .parse::<f64>()
            .ok()
            .map(|f| f.max(0.0).round() as u64),
        _ => None,
    }
}

/// 编号：留小写字母、数字、连字符；空了或重了就按位置起一个
fn slug(raw: &Value, fallback: String, used: &mut BTreeSet<String>) -> String {
    let mut id: String = clip(raw, 64)
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c == '_' || c == ' ' { '-' } else { c })
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
        .take(32)
        .collect();
    if id.is_empty() || used.contains(&id) {
        id = fallback;
    }
    let mut n = 2;
    let base = id.clone();
    while used.contains(&id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    used.insert(id.clone());
    id
}

/// 座号表：在 1–cast 里、不重复
fn seats(value: &Value, cast: usize) -> Vec<u64> {
    let mut seen = BTreeSet::new();
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(number)
        .filter(|&s| s >= 1 && s as usize <= cast && seen.insert(s))
        .collect()
}

/// 剧本里写的地点对到草稿里的地点名字上（谁包含谁都算）；对不上就没有
fn place(value: &Value, names: &[String]) -> Option<String> {
    let raw = clip(value, 40);
    if raw.is_empty() {
        return None;
    }
    names
        .iter()
        .find(|n| **n == raw)
        .or_else(|| {
            names
                .iter()
                .find(|n| n.contains(&raw) || raw.contains(n.as_str()))
        })
        .cloned()
}

/// 把一份草稿理成合规的：模型的回复和页面改过的草稿都走这里。返回的草稿里剧本那一半内核一定收。
pub fn tidy(raw: &Value, choices: &Choices) -> Result<Value, String> {
    if !raw.is_object() {
        return Err("故事草稿不是一个对象".into());
    }
    let title = clip(&raw["title"], 40);
    if title.is_empty() {
        return Err("故事没有名字".into());
    }
    // 角色
    let cast: Vec<Value> = raw["cast"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c.is_object())
        .take(*CAST.end() as usize)
        .enumerate()
        .map(|(i, c)| {
            let name = Some(clip(&c["name"], 12))
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| format!("角色{}", i + 1));
            let occupation = clip(&c["occupation"], 40);
            let drives = clip(&c["drives"], 300);
            let role = Some(clip(&c["role"], 40))
                .filter(|r| !r.is_empty())
                .or_else(|| Some(occupation.clone()).filter(|o| !o.is_empty()))
                .unwrap_or_else(|| "镇上的人".into());
            let goal = Some(clip(&c["goal"], 600))
                .filter(|g| !g.is_empty())
                .or_else(|| Some(drives.clone()).filter(|d| !d.is_empty()))
                .unwrap_or_else(|| "照自己的想法过好这几天".into());
            let tags: Vec<String> = c["tags"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|t| clip(t, 8))
                .filter(|t| !t.is_empty())
                .take(5)
                .collect();
            let mut one = json!({
                "name": name,
                "age": clip(&c["age"], 3),
                "gender": clip(&c["gender"], 4),
                "occupation": occupation,
                "intro": clip(&c["intro"], 80),
                "tags": tags,
                "personality": clip(&c["personality"], 300),
                "speech": clip(&c["speech"], 200),
                "background": clip(&c["background"], 300),
                "drives": drives,
                "role": role,
                "goal": goal,
            });
            let secret = clip(&c["secret"], 600);
            if !secret.is_empty() {
                one["secret"] = json!(secret);
            }
            one
        })
        .collect();
    let people = cast.len();
    if (people as u64) < *CAST.start() {
        return Err(format!("故事至少要 {} 个角色", CAST.start()));
    }
    // 地点：种类要在页面给的表里
    let mut names = Vec::new();
    let places: Vec<Value> = raw["places"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let kind = clip(&p["kind"], 40);
            let (kind, default) = choices.kinds.iter().find(|(k, _)| *k == kind)?;
            let name = Some(clip(&p["name"], 12))
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| default.clone());
            if names.contains(&name) {
                return None;
            }
            names.push(name.clone());
            Some(json!({ "kind": kind, "name": name }))
        })
        .take(PLACES_MAX)
        .collect();
    let preset = clip(&raw["style"]["preset"], 40);
    let preset = choices
        .presets
        .iter()
        .find(|(id, _)| *id == preset)
        .unwrap_or(&choices.presets[0])
        .0
        .clone();
    // 节拍：钟点收进范围，按先后排
    let days = number(&raw["days"]).unwrap_or(14).clamp(1, 14);
    let mut used = BTreeSet::new();
    let mut renamed = Vec::new();
    let mut beats: Vec<(u64, Value)> = raw["beats"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|b| b.is_object())
        .take(BEATS_MAX)
        .enumerate()
        .filter_map(|(i, b)| {
            let announce = clip(&b["announce"], 600);
            if announce.is_empty() {
                return None;
            }
            let id = slug(&b["id"], format!("beat-{}", i + 1), &mut used);
            renamed.push((clip(&b["id"], 64), id.clone()));
            let day = number(&b["day"]).unwrap_or(1).clamp(1, days);
            let minute = number(&b["minute"]).unwrap_or(540).min(1439);
            let title = Some(clip(&b["title"], 40))
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| format!("第 {} 幕", i + 1));
            let mut one = Map::new();
            one.insert("id".into(), json!(id));
            one.insert("title".into(), json!(title));
            one.insert("day".into(), json!(day));
            one.insert("minute".into(), json!(minute));
            if let Some(p) = place(&b["place"], &names) {
                one.insert("place".into(), json!(p));
            }
            let who = seats(&b["who"], people);
            if !who.is_empty() && who.len() < people {
                one.insert("who".into(), json!(who));
            }
            one.insert("announce".into(), json!(announce));
            let brief = clip(&b["brief"], 600);
            if !brief.is_empty() {
                one.insert("brief".into(), json!(brief));
            }
            let speakers = seats(&b["scene"]["speakers"], people);
            if !speakers.is_empty() {
                one.insert(
                    "scene".into(),
                    json!({
                        "speakers": speakers,
                        "minutes": number(&b["scene"]["minutes"]).unwrap_or(15).clamp(1, 120),
                        "chars": number(&b["scene"]["chars"]).unwrap_or(300).clamp(20, 600),
                    }),
                );
            }
            Some(((day - 1) * 1440 + minute, Value::Object(one)))
        })
        .collect();
    if beats.is_empty() {
        return Err("故事里至少要有一个节拍".into());
    }
    beats.sort_by_key(|(at, _)| *at);
    let last = beats.last().map_or(0, |(at, _)| *at);
    let beats: Vec<Value> = beats.into_iter().map(|(_, b)| b).collect();
    // 里程碑：指不到节拍或地点的去掉
    let mut marks = BTreeSet::new();
    let milestones: Vec<Value> = raw["milestones"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| m.is_object())
        .take(MILESTONES_MAX)
        .enumerate()
        .filter_map(|(i, m)| {
            let title = clip(&m["title"], 40);
            if title.is_empty() {
                return None;
            }
            let w = &m["when"];
            let when = match w["kind"].as_str()? {
                "beat" => {
                    let wanted = clip(&w["beat"], 64);
                    let id = renamed
                        .iter()
                        .find(|(old, new)| *old == wanted || *new == wanted)
                        .map(|(_, new)| new.clone())?;
                    json!({ "kind": "beat", "beat": id })
                }
                "gather" => {
                    let at = place(&w["place"], &names)?;
                    let among = seats(&w["seats"], people);
                    let pool = if among.is_empty() { people } else { among.len() };
                    let count = number(&w["count"]).unwrap_or(2).clamp(1, pool as u64);
                    let mut when = json!({ "kind": "gather", "place": at, "count": count });
                    if !among.is_empty() {
                        when["seats"] = json!(among);
                    }
                    when
                }
                "declare" => {
                    let among = seats(&w["seats"], people);
                    let mut when = json!({ "kind": "declare" });
                    if !among.is_empty() {
                        when["seats"] = json!(among);
                    }
                    when
                }
                _ => return None,
            };
            let secret = m["secret"].as_bool().unwrap_or(false) && when["kind"] == "declare";
            let mut one = json!({
                "id": slug(&m["id"], format!("mark-{}", i + 1), &mut marks),
                "title": title,
                "detail": Some(clip(&m["detail"], 300)).filter(|d| !d.is_empty()).unwrap_or_else(|| title.clone()),
                "when": when,
            });
            if secret {
                one["secret"] = json!(true);
            }
            Some(one)
        })
        .collect();
    // 收场：排在最后一个节拍之后
    let mut end = number(&raw["ends_day"])
        .zip(number(&raw["ends_minute"]))
        .map(|(d, m)| (d.clamp(1, 14) - 1) * 1440 + m.min(1439))
        .unwrap_or(0);
    if end <= last {
        end = (last + 60).min(14 * 1440 - 1);
    }
    if end <= last {
        return Err("最后一个节拍排得太晚，后面排不下收场".into());
    }
    let premise = Some(clip(&raw["premise"], 2000))
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| title.clone());
    let mut draft = json!({
        "title": title,
        "premise": premise,
        "worldview": clip(&raw["worldview"], 2000),
        "style": { "preset": preset, "needs": raw["style"]["needs"].as_bool().unwrap_or(false) },
        "places": places,
        "cast": cast,
        "beats": beats,
        "milestones": milestones,
        "ends_day": end / 1440 + 1,
        "ends_minute": end % 1440,
    });
    let epilogue = clip(&raw["epilogue"], 600);
    if !epilogue.is_empty() {
        draft["epilogue"] = json!(epilogue);
    }
    // 交给内核查一遍：这里理过还不合规的，说明原因
    kernel::game_mode::check_storyline(&script(&draft)?).map_err(|e| e.to_string())?;
    Ok(draft)
}

/// 草稿里剧本那一半：开故事模式时带的就是它
pub fn script(draft: &Value) -> Result<StorylineScript, String> {
    let cast: Vec<Value> = draft["cast"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| {
            let mut role = json!({ "role": c["role"], "goal": c["goal"] });
            if c["secret"].is_string() {
                role["secret"] = c["secret"].clone();
            }
            role
        })
        .collect();
    let mut script = json!({
        "title": draft["title"],
        "premise": draft["premise"],
        "cast": cast,
        "beats": draft["beats"],
        "milestones": draft["milestones"],
        "ends_day": draft["ends_day"],
        "ends_minute": draft["ends_minute"],
    });
    if draft["epilogue"].is_string() {
        script["epilogue"] = draft["epilogue"].clone();
    }
    serde_json::from_value(script).map_err(|e| format!("剧本格式不对：{e}"))
}

/// 模型的回复：取出第一个 JSON 对象，理成合规的草稿
pub fn parse(reply: &str, days: u64, choices: &Choices) -> Result<Value, ModelError> {
    let invalid = || ModelError::Reply("模型未返回有效 JSON，请改一改提纲后重试".into());
    let object = first_object(reply).ok_or_else(invalid)?;
    let mut raw = parse_unclosed(object.trim_end()).map_err(|_| invalid())?;
    raw["days"] = json!(days);
    tidy(&raw, choices).map_err(|e| ModelError::Reply(format!("模型写的故事用不了：{e}")))
}

/// 假模型的回复（测试用）：一个固定的小故事，角色数照要的给
pub fn mock_reply(outline: &str, cast: u64, choices: &Choices) -> String {
    let people: Vec<Value> = (1..=cast)
        .map(|n| {
            json!({
                "name": format!("角色{n}"), "age": "30", "gender": if n % 2 == 0 { "女" } else { "男" },
                "occupation": "居民", "intro": "镇上的人", "tags": ["稳重"],
                "personality": "稳重", "speech": "平常", "background": "一直住在镇上", "drives": "过好日子",
                "role": format!("第 {n} 位当事人"), "goal": "把事情说清楚",
            })
        })
        .collect();
    let kind = &choices.kinds[0];
    let title: String = outline.trim().chars().take(8).collect();
    json!({
        "title": if title.is_empty() { "假故事".to_string() } else { title },
        "premise": "镇上出了一件事，大家要在今天说清楚。",
        "worldview": "一座安静的小镇。",
        "style": { "preset": choices.presets[0].0, "needs": false },
        "places": [{ "kind": kind.0, "name": "会场" }],
        "cast": people,
        "beats": [
            { "id": "open", "title": "开场", "day": 1, "minute": 540, "announce": "事情传开了。" },
            { "id": "talk", "title": "当面说", "day": 1, "minute": 720, "place": "会场", "who": [1, 2],
              "announce": "大家到会场当面说。", "brief": "轮到你时说清楚。",
              "scene": { "speakers": [1, 2], "minutes": 10, "chars": 200 } },
        ],
        "milestones": [
            { "id": "talked", "title": "当面说过了", "detail": "开过会", "when": { "kind": "beat", "beat": "talk" } },
            { "id": "settled", "title": "事情了结", "detail": "有人宣布了结", "when": { "kind": "declare", "seats": [1] } },
        ],
        "ends_day": 1, "ends_minute": 900,
        "epilogue": "天黑了。",
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choices() -> Choices {
        Choices {
            kinds: vec![
                ("plaza".into(), "广场".into()),
                ("cafe".into(), "咖啡馆".into()),
            ],
            presets: vec![("coastal-village".into(), "海岸乡镇".into())],
        }
    }

    /// 模型的回复常常不守规矩：前后带话和代码围栏、编号带大写和空格、节拍没按先后排、座号越界、
    /// 地点只写了半个名字、里程碑指着不存在的节拍、收场排在节拍前面。理过以后是内核收的剧本；
    /// 不是 JSON、没有节拍的直接说用不了。
    #[test]
    fn a_sloppy_model_reply_becomes_a_script_the_kernel_accepts() {
        let reply = r#"好的，下面是编排：
```json
{"title":"  茶馆风波 ","premise":"茶馆要拆。","worldview":"老街。",
 "style":{"preset":"no-such","needs":"yes"},
 "places":[{"kind":"cafe","name":"老茶馆"},{"kind":"castle","name":"城堡"},{"kind":"plaza","name":""}],
 "cast":[{"name":"阿福","role":"掌柜","goal":"保住茶馆","secret":"欠着房租"},
         {"name":"小周","occupation":"房东","drives":"收回铺面"},
         {"name":"","age":41}],
 "beats":[
   {"id":"Final Vote","title":"表决","day":1,"minute":"1000","place":"茶馆","who":[1,2,9],
    "announce":"开始表决。","scene":{"speakers":[2,2,7,1],"minutes":999,"chars":5}},
   {"id":"Final Vote","title":"","day":1,"minute":500,"announce":"贴了告示。"},
   {"id":"x","title":"空的","day":1,"minute":600,"announce":"  "}],
 "milestones":[
   {"id":"M 1","title":"表决过了","when":{"kind":"beat","beat":"Final Vote"}},
   {"id":"m2","title":"指错了","when":{"kind":"beat","beat":"nowhere"}},
   {"id":"m3","title":"茶馆聚首","when":{"kind":"gather","place":"茶馆","seats":[1,2,3],"count":9}},
   {"id":"m4","title":"城堡聚首","when":{"kind":"gather","place":"城堡","count":2}},
   {"id":"m5","title":"认账","secret":true,"when":{"kind":"declare","seats":[1,8]}}],
 "ends_day":1,"ends_minute":300}
```
希望你喜欢。"#;
        let draft = parse(reply, 1, &choices()).unwrap();
        assert_eq!(draft["title"], "茶馆风波");
        assert_eq!(
            draft["style"],
            json!({"preset": "coastal-village", "needs": false})
        );
        assert_eq!(
            draft["places"],
            json!([{"kind": "cafe", "name": "老茶馆"}, {"kind": "plaza", "name": "广场"}])
        );
        assert_eq!(draft["cast"][1]["role"], "房东");
        assert_eq!(draft["cast"][1]["goal"], "收回铺面");
        assert_eq!(draft["cast"][2]["name"], "角色3");
        // 节拍按先后排，重了的编号另起，空的宣布不要；戏里的人去重、去越界，期限和字数收进范围
        let beats = draft["beats"].as_array().unwrap();
        assert_eq!(beats.len(), 2);
        assert_eq!(
            (&beats[0]["id"], &beats[0]["minute"]),
            (&json!("beat-2"), &json!(500))
        );
        assert_eq!(beats[1]["id"], "final-vote");
        assert_eq!(beats[1]["place"], "老茶馆");
        assert_eq!(beats[1]["who"], json!([1, 2]));
        assert_eq!(
            beats[1]["scene"],
            json!({"speakers": [2, 1], "minutes": 120, "chars": 20})
        );
        // 里程碑：指错节拍、指着没有的地点的去掉；人数收进范围；越界的座号去掉
        let ids: Vec<&str> = draft["milestones"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["m-1", "m3", "m5"]);
        assert_eq!(draft["milestones"][0]["when"]["beat"], "final-vote");
        assert_eq!(draft["milestones"][1]["when"]["count"], 3);
        assert_eq!(draft["milestones"][2]["when"]["seats"], json!([1]));
        assert_eq!(draft["milestones"][2]["secret"], true);
        // 收场挪到最后一个节拍之后
        assert_eq!(
            (&draft["ends_day"], &draft["ends_minute"]),
            (&json!(1), &json!(1060))
        );
        // 理过的草稿再理一遍不变：页面改完送回来走的是同一条路
        assert_eq!(tidy(&draft, &choices()).unwrap(), draft);
        assert_eq!(
            script(&draft).unwrap().cast[0].secret.as_deref(),
            Some("欠着房租")
        );

        assert!(parse("我写不出来。", 1, &choices()).is_err());
        assert!(parse(
            r#"{"title":"空","cast":[{"name":"a"},{"name":"b"}],"beats":[]}"#,
            1,
            &choices()
        )
        .is_err());
        // 假模型的回复也走得通
        let mock = mock_reply("一个测试故事", 4, &choices());
        assert_eq!(
            parse(&mock, 1, &choices()).unwrap()["cast"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
    }
}
