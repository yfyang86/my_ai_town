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
    /// 熟练的技能（SKILLS 里的编号）：职业给的几项加上背景给的两项
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
    /// 专精的技能（熟练加值算两次）：只有游荡者有，要在 skills 里
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expertise: Vec<String>,
    /// 手里的武器（data/rules/5e/creatures.json 的 weapons）；不写是拳脚
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weapon: Option<String>,
    /// 身上的甲；不写是没穿
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub armor: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub shield: bool,
}

/// 节拍上的一场遭遇战：在一个地方、分成几块相邻的区域（按次序排成一线），来的是哪些对手、卷进来的是哪些角色
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorylineEncounter {
    pub id: String,
    pub title: String,
    /// 区域的名字，相邻的挨着写：近身要在同一块，走一步挪到隔壁
    pub zones: Vec<String>,
    /// 卷进来的座号；空着是所有人
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub party: Vec<u32>,
    /// 角色从第几块区域开始（从 0 数）
    #[serde(default)]
    pub party_zone: u32,
    pub foes: Vec<EncounterFoe>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncounterFoe {
    /// 哪种生物（data/rules/5e/creatures.json）
    pub creature: String,
    /// 在故事里叫什么；不写用生物的名字加编号
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub zone: u32,
}

/// 名册里一场正在打或刚打完的遭遇战
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FightView {
    pub title: String,
    pub round: u32,
    pub zones: Vec<String>,
    pub fighters: Vec<FighterView>,
    /// 轮到谁（fighters 的下标）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn: Option<u32>,
    /// 打完了：won 对手都倒了，lost 角色都倒了或都退了，drawn 拖得太久散了
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FighterView {
    pub name: String,
    /// 角色的座号；对手没有
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seat: Option<u32>,
    pub hp: i32,
    pub max_hp: i32,
    pub ac: u32,
    pub zone: u32,
    /// 此刻的状态：倒地、昏迷、闪避、已退出、死亡……没有就空着
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state: Vec<String>,
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
    /// 没成可以再试；不写是每人只能试一次
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub repeat: bool,
    /// 被动检定：不掷骰，节拍到点时拿 10 + 加值和难度比，过了的人自己留意到（别人不知道）
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub passive: bool,
    /// 对抗：不和难度比，和这个人掷出来的比（他用自己最拿手的那种办法），平手算没成
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub against: Option<CheckContest>,
    /// 什么情形下有优势或劣势（掷两次取高或取低）；两样都有就互相抵掉
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<CheckEdge>,
}

/// 对抗的另一方：谁、能用哪几种办法
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckContest {
    pub seat: u32,
    pub ways: Vec<CheckWay>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Edge {
    Advantage,
    Disadvantage,
}

/// 一条优势或劣势：milestone 达成了、或者人在 place 时生效（都不写就一直生效）；seats 空着是对所有人
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckEdge {
    pub effect: Edge,
    /// 为什么：给试的人和旁观的人看
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub milestone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seats: Vec<u32>,
}

/// 试的办法：一项属性，可以再带一项技能（熟练的加熟练加值）；save 是这项属性的豁免（职业熟练的加熟练加值）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckWay {
    pub ability: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub save: bool,
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
    /// 有优势或劣势时：dice 里是两颗，取了高的或低的那颗
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge: Option<Edge>,
    /// 对抗时对方掷出来的总数
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub versus: Option<i32>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fight: Option<FightView>,
}
