//! 规则套件在线上的形状：故事模式可以另外开一套规则，角色做的事由规则和骰子定成败。
//! 现在只有一套：第五版规则（System Reference Document 5.1，Wizards of the Coast LLC，CC BY 4.0，
//! 署名见仓库的 NOTICE）。骰子只在内核里掷，模型和页面都只看得到结果。
use serde::{Deserialize, Serialize};

/// 六项属性的编号，按规则书的次序
pub const ABILITIES: [&str; 6] = ["str", "dex", "con", "int", "wis", "cha"];

/// 十八项技能和各自用的属性
pub const SKILLS: [(&str, &str); 18] = [
    ("acrobatics", "dex"),
    ("animal_handling", "wis"),
    ("arcana", "int"),
    ("athletics", "str"),
    ("deception", "cha"),
    ("history", "int"),
    ("insight", "wis"),
    ("intimidation", "cha"),
    ("investigation", "int"),
    ("medicine", "wis"),
    ("nature", "int"),
    ("perception", "wis"),
    ("performance", "cha"),
    ("persuasion", "cha"),
    ("religion", "int"),
    ("sleight_of_hand", "dex"),
    ("stealth", "dex"),
    ("survival", "wis"),
];

/// 开哪一套规则、带什么设置
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RulesetChoice {
    /// 第五版规则：sheets 是角色卡，按座号排，和剧本里的角色一一对应
    Srd5 { sheets: Vec<Srd5Sheet> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Abilities {
    pub str: u32,
    pub dex: u32,
    pub con: u32,
    pub int: u32,
    pub wis: u32,
    pub cha: u32,
}

/// 一张角色卡。现在管检定用得到的几项；生命、护甲、法术位随后面的规则加进来。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Srd5Sheet {
    /// 职业：fighter、rogue、cleric、wizard
    pub class: String,
    /// 等级 1–3
    pub level: u32,
    pub abilities: Abilities,
    /// 熟练的技能（SKILLS 里的编号）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
}

/// 节拍上的一项检定：到了这个节拍就可以试，难度是 dc，可以用 ways 里的任一种办法
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorylineCheck {
    pub id: String,
    pub title: String,
    /// 试的是什么、成了会怎样：给能试的人看
    pub detail: String,
    pub dc: u32,
    /// 能试的座号；空着是所有人
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seats: Vec<u32>,
    pub ways: Vec<CheckWay>,
}

/// 试的办法：一项属性，可以再带一项技能（熟练的加熟练加值）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckWay {
    pub ability: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill: Option<String>,
}

/// 掷的一次骰：谁、为了什么、怎么算的、结果
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiceRoll {
    pub at: f64,
    pub seat: u32,
    /// 为了什么掷（检定的标题）
    pub what: String,
    /// 用的办法，写给人看（「敏捷·巧手」）
    pub way: String,
    /// 算式，如 d20+5
    pub formula: String,
    /// 每颗骰子掷出的点数
    pub dice: Vec<u32>,
    pub modifier: i32,
    pub total: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dc: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub success: Option<bool>,
}

/// 名册里规则套件的那一块：每个座位的角色卡写成一行、最近掷的骰
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RulesetView {
    /// 哪一套规则：srd5
    pub id: String,
    /// 按座号排，每人一行
    pub sheets: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rolls: Vec<DiceRoll>,
}
