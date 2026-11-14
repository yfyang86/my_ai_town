//! 提示词调校：写故事、生成人物这两处给模型的提示词，玩家可以在设定里改。三种用法：
//! - 默认：随应用带的那一份——写故事的默认带着「讲究一点」那几段（见 default_modules）；
//! - 风格：在默认的任务说明和输出格式之间插进几段——身份、背景、文风、例子、不要什么、交稿前自查，
//!   输出格式不动，所以回复照样读得了；
//! - 专业：整份模板自己写（任务说明连输出格式），占位符由这里换成实际的数。
//!
//! 模板写到「输入之前」为止：提纲、一句话描述，还有开着规则套件时的那一段，都由调用的一方接在后面。
//! 设置存在应用配置目录的 prompts.json（见 host::prompts）。
use serde::{Deserialize, Serialize};

/// 风格模式里每一段最多这么长；专业模式的整份模板最多这么长
pub const MODULE_MAX_CHARS: usize = 2000;
pub const PRO_MAX_CHARS: usize = 16000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// 故事编辑器：提纲 → 故事草稿
    Story,
    /// 村民库：一句话 → 人物
    Character,
}

impl Target {
    pub fn of(id: &str) -> Option<Self> {
        match id {
            "story" => Some(Self::Story),
            "character" => Some(Self::Character),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Default,
    Style,
    Pro,
}

/// 风格模式的几段。空着的不写进提示词。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Modules {
    /// 模型扮演谁：「你是……」
    #[serde(default)]
    pub role: String,
    /// 题材、时代、世界是什么样的
    #[serde(default)]
    pub background: String,
    /// 文风和气质
    #[serde(default)]
    pub style: String,
    /// 可以参考的例子
    #[serde(default)]
    pub example: String,
    /// 不要这样写
    #[serde(default)]
    pub nogos: String,
    /// 交稿前对照检查的几条
    #[serde(default)]
    pub rubrics: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tuning {
    #[serde(default)]
    pub mode: Mode,
    #[serde(default)]
    pub modules: Modules,
    /// 专业模式的整份模板；空着就是默认的那一份
    #[serde(default)]
    pub pro: String,
    /// 风格模式是从哪一套预设改起的（页面记着用，不影响提示词）
    #[serde(default)]
    pub preset: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptSettings {
    #[serde(default)]
    pub story: Tuning,
    #[serde(default)]
    pub character: Tuning,
}

impl PromptSettings {
    pub fn of(&self, target: Target) -> &Tuning {
        match target {
            Target::Story => &self.story,
            Target::Character => &self.character,
        }
    }
    pub fn tidy(self) -> Self {
        Self {
            story: self.story.tidy(),
            character: self.character.tidy(),
        }
    }
}

fn clip(text: &str, max: usize) -> String {
    text.trim().chars().take(max).collect()
}

/// 默认的整份模板：任务说明、默认带的那几段、输出格式
pub fn default_template(target: Target) -> String {
    styled(target, &default_modules(target))
}

/// 默认模式带的几段。写故事带「讲究一点」：不换题材，只要求人物具体、目标相撞、节拍逼人做选择
/// ——是对模型的要求，不保证每次都做到，但真模型比下来排出的人物和冲突更扎实。生成人物不带。
/// 页面风格模式里的「讲究一点」预设（frontend/town-shared/src/prompts/presets.ts）和这里是同一份文字。
pub fn default_modules(target: Target) -> Modules {
    match target {
        Target::Character => Modules::default(),
        Target::Story => Modules {
            role: "你是一位擅长群戏的编剧，习惯先想清楚每个人要什么、怕什么，再排事情".into(),
            background: String::new(),
            style: "人物介绍写具体的习惯和物件，不写「善良」「聪明」这类评语。\
节拍的宣布像镇上真会传开的一句话，有时间、地点和一件看得见的事。"
                .into(),
            example:
                "人物：「老周，五十三，修了半辈子钟，自己从不戴表；欠着码头一笔钱，谁也没告诉。」\n\
节拍宣布：「晌午，钟楼的钟多敲了一下。敲钟的不是老周。」"
                    .into(),
            nogos: "不要让角色的目标互不相干。\n\
不要用「神秘」「复杂」「内心挣扎」这种空词代替具体的事。\n\
不要把结局写进节拍里。\n\
不要让所有人都通情达理。"
                .into(),
            rubrics: "每个角色的目标至少和另一个角色的目标直接相撞。\n\
每个秘密都和某个节拍或某个别人的目标有关，说破了会改变一个人的选择。\n\
任意两个角色之间，都说得出他们为什么要找对方谈。\n\
每个节拍都逼着至少一个人当场做选择，而不只是通知一件事。\n\
里程碑的达成办法要有几种：有到点就算的，有几个人凑到一处的，有当事人自己宣布的；自己宣布的不超过一半，彼此不冲突，可以都成。"
                .into(),
        },
    }
}

/// 任务说明和输出格式之间插进这几段；身份换掉任务说明的头一句（「你是……。」），后面的任务照旧
fn styled(target: Target, m: &Modules) -> String {
    let (head, contract) = parts(target);
    let head = match (m.role.trim(), head.split_once('。')) {
        ("", _) | (_, None) => head.to_string(),
        (role, Some((_, rest))) => format!("{}。{rest}", role.trim_end_matches(['。', '.'])),
    };
    let blocks: String = [
        ("【背景】", &m.background),
        ("【文风和气质】", &m.style),
        ("【可以参考的例子】只学味道，不要照抄", &m.example),
        ("【不要这样写】", &m.nogos),
        ("【交稿前逐条对照，不合的改到合为止】", &m.rubrics),
    ]
    .iter()
    .filter(|(_, text)| !text.trim().is_empty())
    .map(|(label, text)| format!("{label}\n{}\n", text.trim()))
    .collect();
    if blocks.is_empty() {
        format!("{head}{contract}")
    } else {
        format!("{}\n{blocks}{contract}", head.trim_end())
    }
}

/// 模板里的占位符和各自换成什么（给页面写说明用）
pub fn placeholders(target: Target) -> &'static [(&'static str, &'static str)] {
    match target {
        Target::Story => &[
            ("CAST", "角色数"),
            ("DAYS", "天数"),
            ("PRESET_IDS", "能选的小镇风格"),
            ("KINDS", "能用的地点种类"),
        ],
        Target::Character => &[
            ("TAKEN_NAMES", "已经有人用了的名字"),
            ("PLACE_NAMES", "这张地图上的地点"),
            ("LOOK_OPTIONS", "长相能选的选项"),
        ],
    }
}

/// 默认模板拆成两半：任务说明（风格模式在它后面插东西）、输出格式
fn parts(target: Target) -> (&'static str, &'static str) {
    let (text, mark) = match target {
        Target::Story => (crate::story::TEMPLATE, "\n只输出一个 JSON 对象"),
        Target::Character => (crate::completion::TEMPLATE_PROMPT, "只输出 JSON，不要解释"),
    };
    let at = text.find(mark).unwrap_or(text.len());
    text.split_at(at)
}

impl Tuning {
    pub fn tidy(mut self) -> Self {
        let m = &mut self.modules;
        for part in [
            &mut m.role,
            &mut m.background,
            &mut m.style,
            &mut m.example,
            &mut m.nogos,
            &mut m.rubrics,
        ] {
            *part = clip(part, MODULE_MAX_CHARS);
        }
        self.pro = self.pro.trim_end().chars().take(PRO_MAX_CHARS).collect();
        self.preset = clip(&self.preset, 40);
        self
    }

    /// 这份设置下的整份模板（占位符还没换）。默认模式就是随应用带的那一份。
    pub fn template(&self, target: Target) -> String {
        match self.mode {
            Mode::Default => default_template(target),
            Mode::Pro if self.pro.trim().is_empty() => default_template(target),
            Mode::Pro => format!("{}\n", self.pro.trim_end()),
            Mode::Style => styled(target, &self.modules),
        }
    }

    /// 专业模式的模板少了要紧的东西时提醒一句（照样能存）
    pub fn warnings(&self, target: Target) -> Vec<String> {
        if self.mode != Mode::Pro || self.pro.trim().is_empty() {
            return Vec::new();
        }
        let mut notes = Vec::new();
        let needs: &[(&str, &str)] = match target {
            Target::Story => &[
                ("JSON", "没有要求只输出 JSON，回复多半读不了"),
                (
                    "\"cast\"",
                    "没有 \"cast\" 这一项的写法，模型不知道角色怎么交",
                ),
                (
                    "\"beats\"",
                    "没有 \"beats\" 这一项的写法，故事至少要有一个节拍",
                ),
                ("CAST", "没有用占位符 CAST，模型不知道要几个角色"),
                ("KINDS", "没有用占位符 KINDS，模型不知道能用哪些地点种类"),
            ],
            Target::Character => &[
                ("JSON", "没有要求只输出 JSON，回复多半读不了"),
                ("\"name\"", "没有 \"name\" 这一项的写法"),
                (
                    "LOOK_OPTIONS",
                    "没有用占位符 LOOK_OPTIONS，人物会用默认的长相",
                ),
            ],
        };
        for (mark, note) in needs {
            if !self.pro.contains(mark) {
                notes.push((*note).to_string());
            }
        }
        notes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 默认模式的模板就是随应用带的那一份；风格模式在任务说明和输出格式之间插填了的几段、身份换掉头一句，
    /// 输出格式原样在后面；专业模式用自己写的整份，空着退回默认，少了要紧的东西有提醒；太长的截掉。
    #[test]
    fn tuned_templates_keep_the_output_contract_unless_the_whole_template_is_rewritten() {
        for target in [Target::Story, Target::Character] {
            let plain = default_template(target);
            let (head, contract) = parts(target);
            assert!(!head.is_empty() && !contract.is_empty());
            assert_eq!(Tuning::default().template(target), plain);
            let styled = Tuning {
                mode: Mode::Style,
                modules: Modules {
                    role: "你是古龙".into(),
                    style: "短句，留白。".into(),
                    nogos: "不要大团圆。".into(),
                    ..Modules::default()
                },
                ..Tuning::default()
            };
            let text = styled.template(target);
            assert!(text.starts_with("你是古龙。"), "{text}");
            assert!(text.ends_with(contract));
            let style = text.find("【文风和气质】\n短句，留白。").unwrap();
            let nogos = text.find("【不要这样写】\n不要大团圆。").unwrap();
            assert!(style < nogos && nogos < text.find(contract).unwrap());
            assert!(!text.contains("【背景】"), "空着的段不写");
            // 一段都没填的风格模式：只有任务说明和输出格式，默认带的那几段也不带
            let empty = Tuning {
                mode: Mode::Style,
                ..Tuning::default()
            };
            assert_eq!(empty.template(target), format!("{head}{contract}"));
            assert!(plain.ends_with(contract));
            let pro = Tuning {
                mode: Mode::Pro,
                pro: "只写一句话。".into(),
                ..Tuning::default()
            };
            assert_eq!(pro.template(target), "只写一句话。\n");
            assert!(pro.warnings(target).len() >= 3);
            let blank = Tuning {
                mode: Mode::Pro,
                ..Tuning::default()
            };
            assert_eq!(blank.template(target), plain);
            let full = Tuning {
                mode: Mode::Pro,
                pro: plain.clone(),
                ..Tuning::default()
            };
            assert!(
                full.warnings(target).is_empty(),
                "{:?}",
                full.warnings(target)
            );
            for (mark, _) in placeholders(target) {
                assert!(plain.contains(mark), "默认模板里该有占位符 {mark}");
            }
        }
        // 写故事的默认带着「讲究一点」那几段；生成人物的默认什么都不带
        let story = default_template(Target::Story);
        assert!(story.starts_with("你是一位擅长群戏的编剧"));
        assert!(story.contains(
            "【交稿前逐条对照，不合的改到合为止】\n每个角色的目标至少和另一个角色的目标直接相撞。"
        ));
        let (head, contract) = parts(Target::Character);
        assert_eq!(
            default_template(Target::Character),
            format!("{head}{contract}")
        );
        let long = Tuning {
            mode: Mode::Style,
            modules: Modules {
                style: "风".repeat(MODULE_MAX_CHARS + 50),
                ..Modules::default()
            },
            pro: "字".repeat(PRO_MAX_CHARS + 50),
            ..Tuning::default()
        }
        .tidy();
        assert_eq!(long.modules.style.chars().count(), MODULE_MAX_CHARS);
        assert_eq!(long.pro.chars().count(), PRO_MAX_CHARS);
    }
}
