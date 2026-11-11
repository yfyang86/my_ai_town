//! 故事模式的线上形状：整本剧本是数据（角色、到点发生的节拍、要追踪的里程碑），由开局时带进来。
//! 座号从 1 数（按入住的先后）；剧本里的钟点是「第几天（从 1 数）的第几分钟」，名册里的时刻是世界分钟。
//! 名册里的 `StorylineView` 是上帝视角；给模型的 `StorylineContext` 只有这个人自己该知道的事。
use serde::{Deserialize, Serialize};

/// 一本剧本：开局时查一遍（见 kernel 的 storyline.rs），随这一局存进存档。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorylineScript {
    pub title: String,
    /// 每个角色都知道的前情
    pub premise: String,
    /// 角色，按座号排
    pub cast: Vec<StorylineRole>,
    /// 到点发生的事，按先后排
    pub beats: Vec<StorylineBeat>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub milestones: Vec<StorylineMilestone>,
    /// 故事在第几天的第几分钟收场
    pub ends_day: u32,
    pub ends_minute: u32,
    /// 收场时说的一段
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epilogue: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorylineRole {
    /// 在故事里是谁，名册上公开
    pub role: String,
    /// 想要什么：只有本人知道
    pub goal: String,
    /// 不能让别人知道的事：只有本人知道
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
}

/// 一个节拍：到点对所有人宣布一件事，单独叮嘱几个人，可以再开一场轮流说话的戏。
/// 不替谁做决定：去不去、说什么都由角色自己定。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorylineBeat {
    pub id: String,
    pub title: String,
    pub day: u32,
    pub minute: u32,
    /// 在哪里：地点名字里的几个字（地图上找不到时只当说明）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    /// 和哪几个座号有关（叫醒他们）；空着是所有人
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub who: Vec<u32>,
    /// 所有人都听到的一段
    pub announce: String,
    /// 只对 who 里的人说的一段
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brief: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene: Option<StorylineScene>,
    /// 这个节拍开出来的检定（要开着规则套件才有用；没开时不出现）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<super::StorylineCheck>,
    /// 这个节拍打起来的一场遭遇战（要开着规则套件；没开时不打）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encounter: Option<super::StorylineEncounter>,
}

/// 一场戏：这几个座号按次序一次一人说话，每人有期限和字数上限
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorylineScene {
    pub speakers: Vec<u32>,
    pub minutes: u32,
    pub chars: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorylineMilestone {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub when: StorylineWhen,
    /// 只有能宣布它的人知道有这一项（旁观的人照样看得到）
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub secret: bool,
}

/// 里程碑怎么算达成
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StorylineWhen {
    /// 这个节拍发生了
    Beat { beat: String },
    /// 这几个座号（空着是所有人）里至少 count 个同时在这个地点
    Gather {
        place: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        seats: Vec<u32>,
        count: u32,
    },
    /// 这几个座号（空着是所有人）里有人用 declare 宣布它发生了，带上经过
    Declare {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        seats: Vec<u32>,
    },
    /// 有人通过了这项检定（要开着规则套件）
    Check { check: String },
    /// 这场遭遇战打赢了（要开着规则套件）
    Encounter { encounter: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StorylineSeat {
    pub seat: u32,
    pub id: String,
    pub name: String,
    pub role: String,
    /// 不在了：去世或离开了这里
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub gone: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorylineBeatState {
    Upcoming,
    /// 发生了，它的那场戏还在进行
    Current,
    Done,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StorylineBeatView {
    pub id: String,
    pub title: String,
    /// 世界分钟
    pub at: f64,
    /// 地点的名字
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    pub state: StorylineBeatState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StorylineMilestoneView {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub done: bool,
    /// 收场时还没达成
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub missed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<f64>,
    /// 谁宣布的
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<u32>,
    /// 宣布时写的经过
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// 一场戏里说的一段话
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StorylineLine {
    pub seat: u32,
    /// 哪个节拍的戏（节拍的标题）
    pub occasion: String,
    pub text: String,
    pub at: f64,
    /// 说的时候人不在这场戏的地点
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub away: bool,
    /// 交这段话时写的打算，只给旁观的人看
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// 名册里随时间广播的故事（上帝视角），前端按它画横幅、时间线、里程碑和正在进行的那场戏
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StorylineView {
    pub title: String,
    /// 此刻到了哪一步，横幅上的一句
    pub notice: String,
    pub seats: Vec<StorylineSeat>,
    pub beats: Vec<StorylineBeatView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub milestones: Vec<StorylineMilestoneView>,
    /// 这场戏轮到谁说
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn: Option<u32>,
    /// 他说话的期限
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<StorylineLine>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ended: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epilogue: Option<String>,
    /// 开着规则套件时：角色卡和掷过的骰
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ruleset: Option<super::RulesetView>,
}

/// 给模型看的故事：自己是谁、想要什么、此刻发生了什么、接下来是什么
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorylineContext {
    pub title: String,
    pub premise: String,
    pub me: String,
    pub goal: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
    pub cast: String,
    pub now: String,
    /// 下一个节拍：只有时刻、标题和地点
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub milestones: Vec<String>,
    /// 戏里大家说过的话
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub today: Vec<String>,
    /// 此刻能用 mode_action 交的动作
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<String>,
    /// 开着规则套件时：自己的角色卡
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheet: Option<String>,
    /// 开着规则套件时：此刻自己能试的检定
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<String>,
    /// 正在打的遭遇战：第几轮、各块区域里是谁、自己的状况和能做的事
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fight: Vec<String>,
}
