//! 主持人（开着第五版规则的故事里）：角色想做一件剧本里没写成检定的事，问模型怎么裁。
//! 模型只能从一张单子里选——哪项属性、哪项技能、五档难度、有没有优势，或者不用掷、做不到；
//! 它不掷骰，也不定成败。这里只管写题面和读回复，合不合规矩由内核再查一遍，骰子也是内核掷的。
use crate::completion::{first_object, parse_unclosed};
use protocol::{Edge, Ruling, RulingKind, RulingRequest};
use serde_json::Value;

const NOTE_CHARS: usize = 80;

/// 给模型的题面
pub fn prompt(request: &RulingRequest) -> String {
    let words = kernel::game_mode::ruleset_words();
    let skills = words
        .skills
        .iter()
        .map(|(id, name, ability)| format!("{id} {name}（{ability}）"))
        .collect::<Vec<_>>()
        .join("、");
    format!(
        r#"你是一场桌面角色扮演的主持人，现在只做一件事：裁定一个角色想做的事要不要掷骰、掷什么、多难。
你不掷骰，也不决定成败；不要替角色做决定，不要续写故事。

角色：{name}（{role}）
角色卡：{sheet}
此刻在：{place}
他想做的事：{text}

只输出一个 JSON 对象，不要任何解释：
{{"kind": "roll", "ability": "str", "skill": "athletics", "dc": 15, "edge": null, "note": "一句话说明为什么这么裁，30 字以内"}}
- kind 只能是三种：roll 要掷一次检定；auto 这件事平常人都做得到，不用掷；impossible 这件事无论如何做不到。
  成不成说不准、失败了有后果的事才用 roll。
- ability 只能是：str 力量、dex 敏捷、con 体质、int 智力、wis 感知、cha 魅力。
- skill 从这张表里选一个最贴切的编号，都不贴切就写 null：{skills}。
- dc 只能是 5（很容易）、10（容易）、15（中等）、20（难）、25（很难）之一。照这件事本身有多难来定，不要每次都给同一档：
  寻常人多半做得到的是 10，要有点本事的是 15，行家也常失手的是 20。
- edge：眼前的情形明显对他有利写 "advantage"，明显不利写 "disadvantage"，否则写 null。
- kind 不是 roll 时，ability、skill、dc、edge 都写 null。"#,
        name = request.name,
        role = request.role,
        sheet = request.sheet,
        place = if request.place.is_empty() {
            "（不清楚）"
        } else {
            &request.place
        },
        text = request.text,
    )
}

fn word(value: &Value) -> Option<String> {
    let text = value.as_str()?.trim().to_ascii_lowercase();
    (!text.is_empty() && text != "null" && text != "none").then_some(text)
}

/// 读模型的回复：取第一个 JSON 对象。只把字段原样读出来，认不认得交给内核。
pub fn parse(reply: &str) -> Result<Ruling, String> {
    let invalid = || "主持人没有给出能读的裁定".to_string();
    let object = first_object(reply).ok_or_else(invalid)?;
    let raw = parse_unclosed(object.trim_end()).map_err(|_| invalid())?;
    let kind = match word(&raw["kind"]).as_deref() {
        Some("roll") => RulingKind::Roll,
        Some("auto") => RulingKind::Auto,
        Some("impossible") => RulingKind::Impossible,
        _ => return Err("主持人的裁定不是 roll、auto、impossible 之一".into()),
    };
    let rolled = kind == RulingKind::Roll;
    let edge = match word(&raw["edge"]).as_deref() {
        Some("advantage") => Some(Edge::Advantage),
        Some("disadvantage") => Some(Edge::Disadvantage),
        _ => None,
    };
    let note = raw["note"]
        .as_str()
        .map(|n| n.trim().chars().take(NOTE_CHARS).collect::<String>())
        .filter(|n| !n.is_empty());
    Ok(Ruling {
        kind,
        ability: word(&raw["ability"]).filter(|_| rolled),
        skill: word(&raw["skill"]).filter(|_| rolled),
        dc: raw["dc"]
            .as_u64()
            .or_else(|| raw["dc"].as_str()?.trim().parse().ok())
            .map(|dc| dc.min(99) as u32)
            .filter(|_| rolled),
        edge: edge.filter(|_| rolled),
        note,
    })
}

/// 假模型的裁定（离线接线验收用）：一律掷一次中等难度的力量（运动）检定
pub fn mock_reply(_request: &RulingRequest) -> String {
    r#"{"kind":"roll","ability":"str","skill":"athletics","dc":15,"edge":null,"note":"照规矩掷一次"}"#.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 题面里列着单子；回复读得宽（外面包着话、大小写、数字写成字符串都认），单子外的种类读不出来；
    /// 不掷骰的裁定不带属性和难度。认不认得技能、难度在不在五档里由内核查。
    #[test]
    fn narrator_prompt_lists_the_menu_and_replies_are_read_tolerantly() {
        let request = RulingRequest {
            id: "ask-1".into(),
            actor: "r1".into(),
            seat: 1,
            name: "阿塔".into(),
            role: "守塔人".into(),
            sheet: "战士 1 级".into(),
            place: "甲地".into(),
            text: "用肩膀撞开灯塔的门".into(),
        };
        let text = prompt(&request);
        for part in [
            "阿塔（守塔人）",
            "用肩膀撞开灯塔的门",
            "athletics 运动（str）",
            "5（很容易）",
        ] {
            assert!(text.contains(part), "{part}");
        }
        let ruling = parse(
            "好的。\n```json\n{\"kind\":\"Roll\",\"ability\":\"STR\",\"skill\":\"athletics\",\"dc\":\"15\",\"edge\":\"advantage\",\"note\":\"门轴锈了\"}\n```",
        )
        .unwrap();
        assert_eq!(
            (
                ruling.kind,
                ruling.ability.as_deref(),
                ruling.skill.as_deref(),
                ruling.dc,
                ruling.edge
            ),
            (
                RulingKind::Roll,
                Some("str"),
                Some("athletics"),
                Some(15),
                Some(Edge::Advantage)
            )
        );
        let auto =
            parse(r#"{"kind":"auto","ability":"str","dc":10,"skill":null,"edge":null}"#).unwrap();
        assert_eq!(
            (auto.kind, auto.ability, auto.dc),
            (RulingKind::Auto, None, None)
        );
        assert!(parse(r#"{"kind":"succeed"}"#).is_err());
        assert!(parse("我觉得他能做到").is_err());
        assert_eq!(parse(&mock_reply(&request)).unwrap().dc, Some(15));
    }
}
