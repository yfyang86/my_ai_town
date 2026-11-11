//! 玩法（大逃杀的潮水圈、狼人杀、执剑人之夜……）在线上的形状。一个世界同时最多开一个玩法；没开时名册、设定里都没有
//! 玩法的字段，给模型的观察里也没有那一栏。加一个玩法：在这个目录写它自己的类型，再在下面三个枚举里
//! 各加一项（内核那边见 kernel 的 game_mode）。
mod ruleset;
mod storyline;
mod swordholder;
mod tide;
mod werewolf;

pub use ruleset::{
    Abilities, CheckContest, CheckEdge, CheckWay, DiceRoll, Edge, EncounterFoe, FightView,
    FighterView, RulesetChoice, RulesetView, Srd5Sheet, StorylineCheck, StorylineEncounter,
    ABILITIES, SKILLS,
};
pub use storyline::{
    StorylineBeat, StorylineBeatState, StorylineBeatView, StorylineContext, StorylineLine,
    StorylineMilestone, StorylineMilestoneView, StorylineRole, StorylineScene, StorylineScript,
    StorylineSeat, StorylineView, StorylineWhen,
};
pub use swordholder::{
    SwordholderBallot, SwordholderContext, SwordholderDecision, SwordholderEnding, SwordholderLine,
    SwordholderPhase, SwordholderRole, SwordholderRules, SwordholderSeat, SwordholderView,
};
pub use tide::{
    TideCircle, TideContext, TideFlare, TideNews, TideNewsKind, TidePhase, TidePlayer, TideView,
};
pub use werewolf::{
    WerewolfBallot, WerewolfCheck, WerewolfContext, WerewolfLine, WerewolfNight, WerewolfOut,
    WerewolfPhase, WerewolfRole, WerewolfSeat, WerewolfSide, WerewolfView,
};

use serde::{Deserialize, Serialize};

/// 名册里随时间广播的玩法状态，前端按 kind 认出是哪个玩法。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GameModeView {
    Tide(TideView),
    Werewolf(WerewolfView),
    Swordholder(SwordholderView),
    Storyline(StorylineView),
}

/// 打开哪个玩法、带什么设置；SetGameMode 不带它就是关掉。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GameModeChoice {
    /// 潮水圈：seed 定下这一局的圈；不给时 host 随机取一个
    Tide {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seed: Option<u32>,
    },
    /// 狼人杀：seed 定下谁拿什么身份；day_hours 是白天自由活动几小时（天亮到 16:30 敲钟）
    Werewolf {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seed: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        day_hours: Option<u32>,
    },
    /// 执剑人之夜：seed 定下两位候选人演说和选民表态的先后、平票时抽签；
    /// day_hours 是白天自由活动几小时（天亮到敲钟）；rules 是改过的钟点、期限和关掉的环节，不带就照原来的
    Swordholder {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seed: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        day_hours: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rules: Option<SwordholderRules>,
    },
    /// 故事模式：整本剧本带进来；seed 定下骰子。ruleset 是可选的规则套件（角色卡、检定……），
    /// 不带时故事里做什么都不掷骰，和没有规则套件时一字不差
    Storyline {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seed: Option<u32>,
        script: Box<StorylineScript>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ruleset: Option<RulesetChoice>,
    },
}

/// 给模型看的一栏：观察里的键名是 `key()`，值就是这一项本身。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GameModeContext {
    Tide(TideContext),
    Werewolf(WerewolfContext),
    Swordholder(SwordholderContext),
    Storyline(StorylineContext),
}

impl GameModeContext {
    pub fn key(&self) -> &'static str {
        match self {
            GameModeContext::Tide(_) => "tide",
            GameModeContext::Werewolf(_) => "werewolf",
            GameModeContext::Swordholder(_) => "swordholder",
            GameModeContext::Storyline(_) => "storyline",
        }
    }
}

/// 玩法动作：模型用 mode_action 工具交的一项选择（投票、发言、夜里的身份行动……）。
/// 能做哪些、参数怎么填由开着的玩法说明。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameModeAction {
    pub action: String,
    /// 对谁：座号或名字
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// 要说的话
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// 为什么这么选：只给旁观的人看
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// 这项动作另外要的几个参数（用哪项技能、哪个法术……），开着的玩法要才有；没有时线上不出现
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub with: std::collections::BTreeMap<String, String>,
}
