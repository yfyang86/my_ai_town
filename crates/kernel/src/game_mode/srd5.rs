//! 第五版规则（System Reference Document 5.1，Wizards of the Coast LLC，CC BY 4.0）在内核里的那一半：
//! 角色卡、调整值和熟练加值、属性检定。故事模式开着它时，剧本里的检定由这里掷骰定成败。
//! 现在只有检定；战斗、法术、休息和升级随后面几期加进来，都放在这个文件旁边。
use super::dice::Dice;
use crate::error::{KernelError, Result};
use protocol::{Abilities, CheckWay, DiceRoll, Srd5Sheet, StorylineCheck, ABILITIES, SKILLS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const CLASSES: [(&str, &str); 4] = [
    ("fighter", "战士"),
    ("rogue", "游荡者"),
    ("cleric", "牧师"),
    ("wizard", "法师"),
];
const LEVELS: std::ops::RangeInclusive<u32> = 1..=3;
const SCORES: std::ops::RangeInclusive<u32> = 3..=20;
pub(super) const DCS: std::ops::RangeInclusive<u32> = 1..=30;
/// 名册里留着的掷骰记录最多这么多条
const ROLLS_KEPT: usize = 40;

fn bad(text: impl Into<String>) -> KernelError {
    KernelError::action(text)
}

pub(super) fn ability_name(id: &str) -> &'static str {
    match id {
        "str" => "力量",
        "dex" => "敏捷",
        "con" => "体质",
        "int" => "智力",
        "wis" => "感知",
        "cha" => "魅力",
        _ => "",
    }
}

pub(super) fn skill_name(id: &str) -> &'static str {
    match id {
        "acrobatics" => "特技",
        "animal_handling" => "驯兽",
        "arcana" => "奥秘",
        "athletics" => "运动",
        "deception" => "欺瞒",
        "history" => "历史",
        "insight" => "洞悉",
        "intimidation" => "威吓",
        "investigation" => "调查",
        "medicine" => "医药",
        "nature" => "自然",
        "perception" => "察觉",
        "performance" => "表演",
        "persuasion" => "游说",
        "religion" => "宗教",
        "sleight_of_hand" => "巧手",
        "stealth" => "隐匿",
        "survival" => "求生",
        _ => "",
    }
}

fn score(abilities: &Abilities, id: &str) -> u32 {
    match id {
        "str" => abilities.str,
        "dex" => abilities.dex,
        "con" => abilities.con,
        "int" => abilities.int,
        "wis" => abilities.wis,
        _ => abilities.cha,
    }
}

/// 属性调整值：（属性值 − 10）÷ 2，向下取整
pub(super) fn modifier(score: u32) -> i32 {
    (score as i32 - 10).div_euclid(2)
}

/// 熟练加值：1–4 级 +2
fn proficiency(level: u32) -> i32 {
    2 + (level.saturating_sub(1) / 4) as i32
}

/// 一种试法合不合规：属性要认得，带了技能的技能也要认得
pub(super) fn check_way(way: &CheckWay) -> Result<()> {
    if !ABILITIES.contains(&way.ability.as_str()) {
        return Err(bad(format!("不认得属性「{}」", way.ability)));
    }
    if let Some(skill) = &way.skill {
        if !SKILLS.iter().any(|(id, _)| id == skill) {
            return Err(bad(format!("不认得技能「{skill}」")));
        }
    }
    Ok(())
}

/// 一种试法写给人看：「敏捷·巧手」「力量」
pub(super) fn way_name(way: &CheckWay) -> String {
    match &way.skill {
        Some(skill) => format!("{}·{}", ability_name(&way.ability), skill_name(skill)),
        None => ability_name(&way.ability).to_string(),
    }
}

/// 一种试法的编号：带技能的是技能，不带的是属性；模型在 with 里填它
pub(super) fn way_key(way: &CheckWay) -> &str {
    way.skill.as_deref().unwrap_or(&way.ability)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Srd5 {
    pub sheets: Vec<Srd5Sheet>,
    pub dice: Dice,
    #[serde(default)]
    pub rolls: Vec<DiceRoll>,
    /// 每项检定谁试过了（座位下标）：一人只能试一次
    #[serde(default)]
    pub tried: BTreeMap<String, Vec<usize>>,
    /// 通过了的检定 → 谁通过的
    #[serde(default)]
    pub passed: BTreeMap<String, usize>,
}

impl Srd5 {
    /// 角色卡查一遍：张数和角色对得上，职业、等级、属性值、技能都在规则里。
    pub(super) fn open(sheets: &[Srd5Sheet], cast: usize, seed: u32) -> Result<Self> {
        if sheets.len() != cast {
            return Err(bad(format!(
                "角色卡要和角色一样多：剧本里 {cast} 个角色，给了 {} 张",
                sheets.len()
            )));
        }
        for (i, sheet) in sheets.iter().enumerate() {
            let who = format!("{} 号的角色卡", i + 1);
            if !CLASSES.iter().any(|(id, _)| *id == sheet.class) {
                return Err(bad(format!("{who}：不认得职业「{}」", sheet.class)));
            }
            if !LEVELS.contains(&sheet.level) {
                return Err(bad(format!(
                    "{who}：等级要在 {}–{} 之间",
                    LEVELS.start(),
                    LEVELS.end()
                )));
            }
            if ABILITIES
                .iter()
                .any(|a| !SCORES.contains(&score(&sheet.abilities, a)))
            {
                return Err(bad(format!(
                    "{who}：属性值要在 {}–{} 之间",
                    SCORES.start(),
                    SCORES.end()
                )));
            }
            let mut seen = std::collections::BTreeSet::new();
            for skill in &sheet.skills {
                if !SKILLS.iter().any(|(id, _)| id == skill) || !seen.insert(skill) {
                    return Err(bad(format!("{who}：技能「{skill}」不认得或写了两次")));
                }
            }
        }
        Ok(Self {
            sheets: sheets.to_vec(),
            dice: Dice::new(seed),
            rolls: Vec::new(),
            tried: BTreeMap::new(),
            passed: BTreeMap::new(),
        })
    }

    /// 这个人用这种试法的加值：属性调整值，熟练这项技能的再加熟练加值
    fn bonus(&self, seat: usize, way: &CheckWay) -> i32 {
        let sheet = &self.sheets[seat];
        let proficient = way
            .skill
            .as_ref()
            .is_some_and(|skill| sheet.skills.contains(skill));
        modifier(score(&sheet.abilities, &way.ability))
            + if proficient {
                proficiency(sheet.level)
            } else {
                0
            }
    }

    /// 角色卡写成一行：「战士 1 级 · 力量 16(+3) 敏捷 12(+1) …… · 熟练：运动、威吓」
    pub(super) fn sheet_line(&self, seat: usize) -> String {
        let sheet = &self.sheets[seat];
        let class = CLASSES
            .iter()
            .find(|(id, _)| *id == sheet.class)
            .map_or("", |(_, name)| name);
        let scores: Vec<String> = ABILITIES
            .iter()
            .map(|a| {
                let s = score(&sheet.abilities, a);
                format!("{} {s}({:+})", ability_name(a), modifier(s))
            })
            .collect();
        let skills: Vec<&str> = sheet.skills.iter().map(|s| skill_name(s)).collect();
        let trained = if skills.is_empty() {
            String::new()
        } else {
            format!(
                " · 熟练（+{}）：{}",
                proficiency(sheet.level),
                skills.join("、")
            )
        };
        format!("{class} {} 级 · {}{trained}", sheet.level, scores.join(" "))
    }

    /// 一项检定对这个人写成一行：每种试法带着他自己的加值
    pub(super) fn check_line(&self, seat: usize, check: &StorylineCheck) -> String {
        let ways: Vec<String> = check
            .ways
            .iter()
            .map(|w| {
                format!(
                    "{}（{} {:+}）",
                    way_key(w),
                    way_name(w),
                    self.bonus(seat, w)
                )
            })
            .collect();
        format!(
            "{}：{}（{}）难度 {}，可以用 {}",
            check.id,
            check.title.trim(),
            check.detail.trim(),
            check.dc,
            ways.join(" 或 ")
        )
    }

    pub(super) fn has_tried(&self, seat: usize, check: &str) -> bool {
        self.tried.get(check).is_some_and(|t| t.contains(&seat))
    }

    /// 掷一次检定：d20 + 加值，和难度比。记下这一掷，返回它。
    pub(super) fn attempt(
        &mut self,
        seat: usize,
        check: &StorylineCheck,
        way: &CheckWay,
        at: f64,
    ) -> DiceRoll {
        let die = self.dice.roll(20);
        let bonus = self.bonus(seat, way);
        let total = die as i32 + bonus;
        let success = total >= check.dc as i32;
        let roll = DiceRoll {
            at,
            seat: seat as u32 + 1,
            what: check.title.trim().to_string(),
            way: way_name(way),
            formula: format!("d20{bonus:+}"),
            dice: vec![die],
            modifier: bonus,
            total,
            dc: Some(check.dc),
            success: Some(success),
        };
        self.tried.entry(check.id.clone()).or_default().push(seat);
        if success {
            self.passed.entry(check.id.clone()).or_insert(seat);
        }
        self.rolls.push(roll.clone());
        let extra = self.rolls.len().saturating_sub(ROLLS_KEPT);
        self.rolls.drain(..extra);
        roll
    }
}
