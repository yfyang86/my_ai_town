use crate::support::*;
use agent::tools;
use protocol::{
    Abilities, GameModeChoice, RulesetChoice, Srd5Sheet, StorylineBeat, StorylineRole,
    StorylineScript,
};
use serde_json::json;

fn script() -> StorylineScript {
    let role = |role: &str| StorylineRole {
        role: role.into(),
        goal: "把事情办成".into(),
        secret: None,
    };
    StorylineScript {
        title: "试规则".into(),
        premise: "两个人，一件事。".into(),
        cast: vec![role("甲"), role("乙")],
        beats: vec![StorylineBeat {
            id: "start".into(),
            title: "开场".into(),
            day: 1,
            minute: 480,
            place: None,
            who: vec![],
            announce: "开始了。".into(),
            brief: None,
            scene: None,
            checks: vec![],
            encounter: None,
        }],
        milestones: vec![],
        ends_day: 1,
        ends_minute: 600,
        epilogue: None,
    }
}

/// 玩法动作的工具定义：开着的玩法要另带参数时（故事开着规则套件）才多一项 with，说明里讲怎么填；
/// 同一个故事不开规则时，工具定义里没有 with，模型交来的 with 也照样认得（不认得的参数才拒收）。
#[test]
fn mode_action_takes_named_parameters_only_when_the_open_mode_asks_for_them() {
    let sheet = Srd5Sheet {
        class: "fighter".into(),
        level: 1,
        abilities: Abilities {
            str: 15,
            dex: 14,
            con: 13,
            int: 12,
            wis: 10,
            cha: 8,
        },
        skills: vec!["athletics".into()],
        expertise: vec![],
        weapon: None,
        armor: None,
        shield: false,
    };
    let mode_action = |ruleset: Option<RulesetChoice>| {
        let mut w = world();
        w.spawn("r2", "阿禾", "loc_a").unwrap();
        w.set_game_mode(Some(&GameModeChoice::Storyline {
            seed: Some(3),
            script: Box::new(script()),
            ruleset,
        }))
        .unwrap();
        tools::life_definitions(&w, "r1")
            .into_iter()
            .find(|d| d["function"]["name"] == "mode_action")
            .expect("开着故事就有玩法动作")
    };
    let plain = mode_action(None);
    assert!(plain["function"]["parameters"]["properties"]
        .get("with")
        .is_none());
    let ruled = mode_action(Some(RulesetChoice::Srd5 {
        sheets: vec![sheet.clone(), sheet],
    }));
    let with = &ruled["function"]["parameters"]["properties"]["with"];
    assert_eq!(with["type"], "object");
    assert_eq!(with["additionalProperties"]["type"], "string");
    assert!(ruled["function"]["description"]
        .as_str()
        .unwrap()
        .contains("with 里填 skill 或 ability"));
    // 别的都一样：只多了 with 这一项
    let mut same = ruled["function"]["parameters"].clone();
    same["properties"].as_object_mut().unwrap().remove("with");
    assert_eq!(same, plain["function"]["parameters"]);
    assert_eq!(
        json!(["action"]),
        plain["function"]["parameters"]["required"]
    );
}
