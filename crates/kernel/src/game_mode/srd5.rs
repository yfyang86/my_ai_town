//! 第五版规则（System Reference Document 5.1，Wizards of the Coast LLC，CC BY 4.0）在内核里的那一半：
//! 角色卡、调整值和熟练加值、属性检定。故事模式开着它时，剧本里的检定由这里掷骰定成败。
//! 职业的数据（生命骰、豁免熟练、能挑的技能）在 data/rules/5e/classes.json，编译时带进来。
//! 现在有检定的全套：技能和属性检定、豁免、优势劣势、对抗、被动检定、帮一把、专精；
//! 战斗、法术、休息和升级随后面几期加进来。
use super::combat::{self, Attack, Fight, Member, Reach, Spell, SpellKind};
use super::dice::Dice;
use crate::error::{KernelError, Result};
use protocol::{Abilities, CheckWay, DiceRoll, Edge, Srd5Sheet, StorylineCheck, ABILITIES, SKILLS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 一个职业：生命骰、哪两项豁免熟练、能从哪些技能里挑几项、能专精几项
#[derive(Debug, Clone, Deserialize)]
pub struct Class {
    pub id: String,
    pub name: String,
    /// 出自规则书的哪一节（数据测试查每一项都写了出处）
    #[cfg_attr(not(test), allow(dead_code))]
    pub srd: String,
    pub hit_die: u32,
    pub saves: Vec<String>,
    pub skill_count: usize,
    pub skills: Vec<String>,
    pub expertise: usize,
}

#[derive(Debug, Deserialize)]
struct ClassData {
    classes: Vec<Class>,
    /// 背景另给的技能熟练数
    background_skills: usize,
}

/// 规则书里的一个法术（没算施法者的加值）
#[derive(Debug, Clone, Deserialize)]
pub struct SpellData {
    pub id: String,
    pub name: String,
    #[cfg_attr(not(test), allow(dead_code))]
    pub srd: String,
    pub level: u32,
    pub classes: Vec<String>,
    pub kind: SpellKind,
    pub reach: Reach,
    #[serde(default)]
    pub dice: (u32, u32),
    #[serde(default)]
    pub plus: i32,
    /// 治疗量加施法属性的调整值
    #[serde(default)]
    pub add_mod: bool,
    #[serde(default)]
    pub save: Option<String>,
    #[serde(default)]
    pub half: bool,
    #[serde(default)]
    pub rays: u32,
    #[serde(default)]
    pub upcast_dice: u32,
    #[serde(default)]
    pub upcast_rays: u32,
    #[serde(default)]
    pub concentration: bool,
    #[serde(default)]
    pub marks: bool,
    #[serde(default)]
    pub rest_only: bool,
}

#[derive(Debug, Deserialize)]
struct Slots {
    /// 每一级各环有几个法术位
    by_level: Vec<Vec<u32>>,
}
#[derive(Debug, Deserialize)]
struct Experience {
    /// 升到各级要的经验
    levels: Vec<u32>,
    creatures: BTreeMap<String, u32>,
}
#[derive(Debug, Deserialize)]
struct Feature {
    #[serde(default)]
    dice_by_level: Vec<u32>,
}
#[derive(Debug, Deserialize)]
struct Features {
    sneak_attack: Feature,
}
#[derive(Debug, Deserialize)]
struct Magic {
    slots: Slots,
    casting: BTreeMap<String, String>,
    spells: Vec<SpellData>,
    experience: Experience,
    features: Features,
}

fn magic() -> &'static Magic {
    static DATA: std::sync::OnceLock<Magic> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../data/rules/5e/spells.json"))
            .expect("data/rules/5e/spells.json 随源码带着，格式是对的")
    })
}

/// 准备的法术最多这么多个
const SPELLS_MAX: usize = 8;
/// 睡够这么久（游戏分钟）算一次长休
const LONG_REST_MINUTES: f64 = 360.0;
/// 达成一个里程碑给的经验
pub(super) const MILESTONE_XP: u32 = 50;

fn data() -> &'static ClassData {
    static DATA: std::sync::OnceLock<ClassData> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../data/rules/5e/classes.json"))
            .expect("data/rules/5e/classes.json 随源码带着，格式是对的")
    })
}

fn class(id: &str) -> Option<&'static Class> {
    data().classes.iter().find(|c| c.id == id)
}
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
        if way.save {
            return Err(bad("一种试法不能既是技能又是豁免"));
        }
    }
    Ok(())
}

/// 一种试法写给人看：「敏捷·巧手」「体质豁免」「力量」
pub(super) fn way_name(way: &CheckWay) -> String {
    match &way.skill {
        Some(skill) => format!("{}·{}", ability_name(&way.ability), skill_name(skill)),
        None if way.save => format!("{}豁免", ability_name(&way.ability)),
        None => ability_name(&way.ability).to_string(),
    }
}

/// 一种试法的编号：带技能的是技能，豁免是「属性_save」，都不是就是属性；模型在 with 里填它
pub(super) fn way_key(way: &CheckWay) -> String {
    match &way.skill {
        Some(skill) => skill.clone(),
        None if way.save => format!("{}_save", way.ability),
        None => way.ability.clone(),
    }
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
    /// 有人帮一把的检定 → 被帮的人 → 帮他的人：被帮的人下一次试这项检定有优势，用掉就没了
    #[serde(default)]
    pub helped: BTreeMap<String, BTreeMap<usize, usize>>,
    /// 每个座位此刻的生命值
    #[serde(default)]
    pub hp: Vec<i32>,
    /// 正在打、或者刚打完还留在画面上的遭遇战
    #[serde(default)]
    pub fight: Option<Fight>,
    /// 战斗里轮到的角色出手的期限（世界分钟）；没在等谁时是 0
    #[serde(default)]
    pub fight_until: f64,
    /// 打赢了的遭遇战
    #[serde(default)]
    pub won: Vec<String>,
    /// 每个座位各环还剩几个法术位（下标 0 是一环）
    #[serde(default)]
    pub slots: Vec<Vec<u32>>,
    /// 每个座位的经验
    #[serde(default)]
    pub xp: Vec<u32>,
    /// 回气用掉了没有（战士）
    #[serde(default)]
    pub winded: Vec<bool>,
    /// 每个座位睡下的时刻；醒着时没有
    #[serde(default)]
    pub asleep_since: Vec<Option<f64>>,
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
            let Some(class) = class(&sheet.class) else {
                return Err(bad(format!("{who}：不认得职业「{}」", sheet.class)));
            };
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
            // 职业给几项（从职业的单子里挑）、背景再给两项（哪项都行）
            let allowed = class.skill_count + data().background_skills;
            let outside = sheet
                .skills
                .iter()
                .filter(|s| !class.skills.contains(s))
                .count();
            if outside > data().background_skills {
                return Err(bad(format!(
                    "{who}：{}的职业技能里没有的最多 {} 项（靠背景得来），写了 {outside} 项",
                    class.name,
                    data().background_skills
                )));
            }
            if sheet.skills.len() > allowed {
                return Err(bad(format!(
                    "{who}：{}最多熟练 {allowed} 项技能（职业 {} 项、背景 {} 项），写了 {} 项",
                    class.name,
                    class.skill_count,
                    data().background_skills,
                    sheet.skills.len()
                )));
            }
            let gear = combat::gear();
            if sheet
                .weapon
                .as_ref()
                .is_some_and(|w| !gear.weapons.iter().any(|x| x.id == *w))
                || sheet
                    .armor
                    .as_ref()
                    .is_some_and(|a| !gear.armor.iter().any(|x| x.id == *a))
            {
                return Err(bad(format!("{who}：规则里没有这件武器或护甲")));
            }
            // 准备的法术：要是这个职业的、环数用得了的
            let top = Self::slot_table(&sheet.class, sheet.level).len() as u32;
            let mut prepared = std::collections::BTreeSet::new();
            for id in &sheet.spells {
                let known = magic().spells.iter().find(|s| s.id == *id);
                let fits =
                    known.is_some_and(|s| s.classes.contains(&sheet.class) && s.level <= top);
                if !fits || !prepared.insert(id) {
                    return Err(bad(format!(
                        "{who}：{} {} 级准备不了法术「{id}」，或者写了两次",
                        class.name, sheet.level
                    )));
                }
            }
            if sheet.spells.len() > SPELLS_MAX {
                return Err(bad(format!("{who}：准备的法术最多 {SPELLS_MAX} 个")));
            }
            if sheet.expertise.len() > class.expertise
                || sheet.expertise.iter().any(|e| !sheet.skills.contains(e))
            {
                return Err(bad(format!(
                    "{who}：{}能专精 {} 项，而且要是已经熟练的技能",
                    class.name, class.expertise
                )));
            }
        }
        let mut rules = Self {
            sheets: sheets.to_vec(),
            dice: Dice::new(seed),
            rolls: Vec::new(),
            tried: BTreeMap::new(),
            passed: BTreeMap::new(),
            helped: BTreeMap::new(),
            hp: Vec::new(),
            fight: None,
            fight_until: 0.0,
            won: Vec::new(),
            slots: Vec::new(),
            xp: vec![0; cast],
            winded: vec![false; cast],
            asleep_since: vec![None; cast],
        };
        rules.hp = (0..cast).map(|i| rules.hit_points(i) as i32).collect();
        rules.slots = sheets
            .iter()
            .map(|s| Self::slot_table(&s.class, s.level))
            .collect();
        // 剧本里写的等级就是起点：经验从这一级的门槛算起
        for (i, sheet) in sheets.iter().enumerate() {
            rules.xp[i] = magic()
                .experience
                .levels
                .get(sheet.level as usize - 1)
                .copied()
                .unwrap_or(0);
        }
        Ok(rules)
    }

    /// 这个人用这种试法的加值：属性调整值；熟练这项技能、或者职业熟练这项豁免的再加熟练加值；专精的加两次
    pub(super) fn bonus(&self, seat: usize, way: &CheckWay) -> i32 {
        let sheet = &self.sheets[seat];
        let times = match &way.skill {
            Some(skill) if sheet.expertise.contains(skill) => 2,
            Some(skill) if sheet.skills.contains(skill) => 1,
            None if way.save
                && class(&sheet.class).is_some_and(|c| c.saves.contains(&way.ability)) =>
            {
                1
            }
            _ => 0,
        };
        modifier(score(&sheet.abilities, &way.ability)) + times * proficiency(sheet.level)
    }

    /// 这个人用这几种试法里最拿手的一种
    pub(super) fn best<'a>(&self, seat: usize, ways: &'a [CheckWay]) -> Option<&'a CheckWay> {
        ways.iter().max_by_key(|w| self.bonus(seat, w))
    }

    /// 生命值上限：1 级是生命骰的最大值加体质调整值，以后每级加生命骰的平均值（向上取整）加体质调整值
    pub(super) fn hit_points(&self, seat: usize) -> u32 {
        let sheet = &self.sheets[seat];
        let die = class(&sheet.class).map_or(8, |c| c.hit_die) as i32;
        let con = modifier(sheet.abilities.con);
        let later = (sheet.level as i32 - 1) * (die / 2 + 1 + con).max(1);
        (die + con + later).max(1) as u32
    }

    /// 护甲等级：甲的底数加敏捷调整值（中甲最多加 2，重甲不加），拿盾再加 2
    pub(super) fn armor_class(&self, seat: usize) -> u32 {
        let sheet = &self.sheets[seat];
        let gear = combat::gear();
        let armor = gear
            .armor
            .iter()
            .find(|a| Some(&a.id) == sheet.armor.as_ref())
            .or_else(|| gear.armor.iter().find(|a| a.id == "none"));
        let dex = modifier(sheet.abilities.dex);
        let (base, dex) = match armor {
            Some(a) => (a.base as i32, a.dex_cap.map_or(dex, |cap| dex.min(cap))),
            None => (10, dex),
        };
        (base
            + dex
            + if sheet.shield {
                gear.shield_bonus as i32
            } else {
                0
            })
        .max(1) as u32
    }

    /// 手里武器的攻击：近身用力量，灵巧或远程的用力量和敏捷里高的；命中加熟练加值，伤害加属性调整值
    pub(super) fn attack(&self, seat: usize) -> Attack {
        let sheet = &self.sheets[seat];
        let gear = combat::gear();
        let weapon = gear
            .weapons
            .iter()
            .find(|w| Some(&w.id) == sheet.weapon.as_ref())
            .or_else(|| gear.weapons.iter().find(|w| w.id == "unarmed"));
        let (strength, dex) = (modifier(sheet.abilities.str), modifier(sheet.abilities.dex));
        match weapon {
            Some(w) => {
                let ability = if w.ranged {
                    dex
                } else if w.finesse {
                    strength.max(dex)
                } else {
                    strength
                };
                Attack {
                    name: w.name.clone(),
                    bonus: ability + proficiency(sheet.level),
                    dice: w.dice,
                    plus: ability,
                    ranged: w.ranged,
                }
            }
            None => Attack {
                name: "拳脚".into(),
                bonus: strength + proficiency(sheet.level),
                dice: (1, 1),
                plus: strength,
                ranged: false,
            },
        }
    }

    /// 这个职业这一级各环有几个法术位；不会施法的没有
    fn slot_table(class: &str, level: u32) -> Vec<u32> {
        if !magic().casting.contains_key(class) {
            return Vec::new();
        }
        magic()
            .slots
            .by_level
            .get(level as usize - 1)
            .cloned()
            .unwrap_or_default()
    }

    /// 施法属性的调整值（牧师感知、法师智力）；不会施法的是 0
    fn casting_mod(&self, seat: usize) -> i32 {
        let sheet = &self.sheets[seat];
        magic()
            .casting
            .get(&sheet.class)
            .map_or(0, |a| modifier(score(&sheet.abilities, a)))
    }

    /// 这个人准备的法术，带上他自己的加值：法术攻击是熟练加值加施法属性，豁免难度是 8 加这两样
    pub(super) fn spells(&self, seat: usize) -> Vec<Spell> {
        let sheet = &self.sheets[seat];
        let ability = self.casting_mod(seat);
        let bonus = ability + proficiency(sheet.level);
        sheet
            .spells
            .iter()
            .filter_map(|id| magic().spells.iter().find(|s| s.id == *id))
            .map(|s| Spell {
                id: s.id.clone(),
                name: s.name.clone(),
                level: s.level,
                kind: s.kind,
                reach: s.reach,
                dice: s.dice,
                plus: s.plus + if s.add_mod { ability } else { 0 },
                save: s.save.clone(),
                half: s.half,
                rays: s.rays,
                upcast_dice: s.upcast_dice,
                upcast_rays: s.upcast_rays,
                concentration: s.concentration,
                marks: s.marks,
                rest_only: s.rest_only,
                bonus,
                dc: (8 + bonus).max(1) as u32,
            })
            .collect()
    }

    /// 豁免的加值：敏捷、体质、感知
    fn save_mods(&self, seat: usize) -> (i32, i32, i32) {
        let way = |ability: &str| CheckWay {
            ability: ability.into(),
            skill: None,
            save: true,
        };
        (
            self.bonus(seat, &way("dex")),
            self.bonus(seat, &way("con")),
            self.bonus(seat, &way("wis")),
        )
    }

    /// 这个角色参战时带的数
    pub(super) fn member(&self, seat: usize, name: String) -> Member {
        let sheet = &self.sheets[seat];
        let sneak = if sheet.class == "rogue" {
            magic()
                .features
                .sneak_attack
                .dice_by_level
                .get(sheet.level as usize - 1)
                .copied()
                .unwrap_or(1)
        } else {
            0
        };
        Member {
            name,
            seat,
            hp: self.hp[seat],
            max_hp: self.hit_points(seat) as i32,
            ac: self.armor_class(seat),
            attack: self.attack(seat),
            dex: modifier(sheet.abilities.dex),
            level: sheet.level,
            slots: self.slots.get(seat).cloned().unwrap_or_default(),
            spells: self.spells(seat),
            save_mods: self.save_mods(seat),
            sneak,
            second_wind: sheet.class == "fighter"
                && !self.winded.get(seat).copied().unwrap_or(false),
        }
    }

    /// 打完一场把角色的状况记回来：生命（倒下没死的缓过来剩 1 点）、法术位、回气用没用
    pub(super) fn settle(&mut self) -> (Vec<usize>, Vec<String>) {
        let (mut dead, mut woke) = (Vec::new(), Vec::new());
        let Some(fight) = &mut self.fight else {
            return (dead, woke);
        };
        for f in &mut fight.fighters {
            let Some(seat) = f.seat else {
                continue;
            };
            if f.dead {
                self.hp[seat] = 0;
                dead.push(seat);
                continue;
            }
            if f.hp <= 0 {
                f.hp = 1;
                f.prone = false;
                woke.push(f.name.clone());
            }
            self.hp[seat] = f.hp;
            if let Some(slots) = self.slots.get_mut(seat) {
                *slots = f.slots.clone();
            }
            if self.sheets[seat].class == "fighter" {
                self.winded[seat] = !f.second_wind;
            }
        }
        (dead, woke)
    }

    /// 给这个人加经验；够了就升级（最高 3 级）：生命上限涨多少现在的生命就涨多少，法术位照新等级补上。
    /// 返回升到了几级，没升是 None。
    pub(super) fn gain(&mut self, seat: usize, xp: u32) -> Option<u32> {
        self.xp[seat] += xp;
        let levels = &magic().experience.levels;
        let earned = levels.iter().filter(|need| self.xp[seat] >= **need).count() as u32;
        let now = self.sheets[seat].level;
        if earned <= now {
            return None;
        }
        let before = self.hit_points(seat) as i32;
        self.sheets[seat].level = earned.min(*LEVELS.end());
        let after = self.hit_points(seat) as i32;
        if self.hp[seat] > 0 {
            self.hp[seat] += after - before;
        }
        let table = Self::slot_table(&self.sheets[seat].class, self.sheets[seat].level);
        if let Some(slots) = self.slots.get_mut(seat) {
            let had = slots.clone();
            *slots = table
                .iter()
                .enumerate()
                .map(|(k, full)| {
                    // 新得的法术位马上能用，已经用掉的不回来
                    let old_full = Self::slot_table(&self.sheets[seat].class, now)
                        .get(k)
                        .copied()
                        .unwrap_or(0);
                    had.get(k).copied().unwrap_or(0) + full.saturating_sub(old_full)
                })
                .collect();
        }
        Some(self.sheets[seat].level)
    }

    /// 打赢一场遭遇战值多少经验（对手的经验加起来）
    pub(super) fn fight_xp(fight: &Fight) -> u32 {
        fight
            .fighters
            .iter()
            .filter_map(|f| f.creature.as_ref())
            .map(|c| magic().experience.creatures.get(c).copied().unwrap_or(0))
            .sum()
    }

    /// 每一拍告诉它这个人睡没睡着。睡够了一觉醒来算一次长休：生命回满、法术位回满、回气能再用。
    /// 返回这一下是不是刚歇好。
    pub(super) fn note_sleep(&mut self, seat: usize, asleep: bool, now: f64) -> bool {
        match (asleep, self.asleep_since[seat]) {
            (true, None) => {
                self.asleep_since[seat] = Some(now);
                false
            }
            (false, Some(since)) => {
                self.asleep_since[seat] = None;
                if now - since < LONG_REST_MINUTES || self.hp[seat] <= 0 {
                    return false;
                }
                self.hp[seat] = self.hit_points(seat) as i32;
                self.slots[seat] =
                    Self::slot_table(&self.sheets[seat].class, self.sheets[seat].level);
                self.winded[seat] = false;
                true
            }
            _ => false,
        }
    }

    /// 不在打的时候施一个治疗的法术：对象是同在一处的自己人（座位下标），返回每个人回了多少。
    pub(super) fn cast_heal(
        &mut self,
        seat: usize,
        id: &str,
        slot: u32,
        targets: &[usize],
    ) -> Result<Vec<(usize, i32)>> {
        let spell = self
            .spells(seat)
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| bad(format!("你没有准备「{id}」")))?;
        if spell.kind != SpellKind::Heal {
            return Err(bad(format!("{}只在战斗里用得上", spell.name)));
        }
        let slot = slot.max(spell.level);
        if self.slots[seat]
            .get(slot as usize - 1)
            .is_none_or(|n| *n == 0)
        {
            return Err(bad(format!("{slot} 环的法术位用完了")));
        }
        let targets: Vec<usize> = match spell.reach {
            Reach::Group => targets.to_vec(),
            _ => targets.iter().copied().take(1).collect(),
        };
        if targets.is_empty() {
            return Err(bad(format!("{}要指一个对象", spell.name)));
        }
        self.slots[seat][slot as usize - 1] -= 1;
        let count = spell.dice.0 + spell.upcast_dice * (slot - spell.level);
        let mut healed = Vec::new();
        for t in targets {
            let amount = (0..count)
                .map(|_| self.dice.roll(spell.dice.1))
                .sum::<u32>() as i32
                + spell.plus;
            let max = self.hit_points(t) as i32;
            let before = self.hp[t];
            self.hp[t] = (before.max(0) + amount.max(1)).min(max);
            healed.push((t, self.hp[t] - before));
        }
        Ok(healed)
    }

    pub(super) fn trim_rolls(&mut self) {
        let extra = self.rolls.len().saturating_sub(ROLLS_KEPT);
        self.rolls.drain(..extra);
    }

    /// 角色卡写成一行：「战士 1 级 · 力量 16(+3) 敏捷 12(+1) …… · 熟练：运动、威吓」
    pub(super) fn sheet_line(&self, seat: usize) -> String {
        let sheet = &self.sheets[seat];
        let class = class(&sheet.class);
        let saves: Vec<&str> = class
            .map(|c| c.saves.iter().map(|a| ability_name(a)).collect())
            .unwrap_or_default();
        let class = class.map_or("", |c| c.name.as_str());
        let scores: Vec<String> = ABILITIES
            .iter()
            .map(|a| {
                let s = score(&sheet.abilities, a);
                format!("{} {s}({:+})", ability_name(a), modifier(s))
            })
            .collect();
        let skills: Vec<String> = sheet
            .skills
            .iter()
            .map(|s| {
                let mark = if sheet.expertise.contains(s) {
                    "（专精）"
                } else {
                    ""
                };
                format!("{}{mark}", skill_name(s))
            })
            .collect();
        let trained = if skills.is_empty() {
            String::new()
        } else {
            format!(
                " · 熟练（+{}）：{}",
                proficiency(sheet.level),
                skills.join("、")
            )
        };
        let attack = self.attack(seat);
        let now = self.hp.get(seat).copied().unwrap_or(0).max(0);
        let spells = self.spells(seat);
        let magic = if spells.is_empty() {
            String::new()
        } else {
            let slots: Vec<String> = self.slots[seat]
                .iter()
                .enumerate()
                .map(|(k, n)| format!("{} 环 {n}", k + 1))
                .collect();
            let names: Vec<String> = spells
                .iter()
                .map(|s| format!("{}={}", s.id, s.name))
                .collect();
            format!(
                " · 法术位：{} · 准备的法术：{}",
                slots.join("、"),
                names.join("、")
            )
        };
        format!(
            "{class} {} 级（经验 {}）· 生命 {now}/{} · 护甲 {}（{}{}）· {}（{:+} 命中，{}d{}{:+}）· {} · 豁免熟练：{}{trained}{magic}",
            sheet.level,
            self.xp.get(seat).copied().unwrap_or(0),
            self.hit_points(seat),
            self.armor_class(seat),
            combat::gear()
                .armor
                .iter()
                .find(|a| Some(&a.id) == sheet.armor.as_ref())
                .map_or("没穿甲", |a| a.name.as_str()),
            if sheet.shield { "、盾" } else { "" },
            attack.name,
            attack.bonus,
            attack.dice.0,
            attack.dice.1,
            attack.plus,
            scores.join(" "),
            saves.join("、")
        )
    }

    /// 一项检定对这个人写成一行：每种试法带着他自己的加值
    pub(super) fn check_line(&self, seat: usize, check: &StorylineCheck, notes: &str) -> String {
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
            "{}：{}（{}）{notes}，可以用 {}",
            check.id,
            check.title.trim(),
            check.detail.trim(),
            ways.join(" 或 ")
        )
    }

    pub(super) fn has_tried(&self, seat: usize, check: &str) -> bool {
        self.tried.get(check).is_some_and(|t| t.contains(&seat))
    }

    /// 掷 d20：有优势或劣势时掷两颗，取高的或低的那颗。返回（掷出的每一颗，取的那一颗）
    fn d20(&mut self, edge: Option<Edge>) -> (Vec<u32>, u32) {
        let first = self.dice.roll(20);
        match edge {
            None => (vec![first], first),
            Some(edge) => {
                let second = self.dice.roll(20);
                let kept = match edge {
                    Edge::Advantage => first.max(second),
                    Edge::Disadvantage => first.min(second),
                };
                (vec![first, second], kept)
            }
        }
    }

    /// 掷一次检定：d20 + 加值，和难度比；对抗时和对方掷出来的比（rival 是对方的座位和他用的办法），
    /// 平手算没成。记下这一掷（对抗时对方那一掷也记），返回试的这个人的那一掷。
    pub(super) fn attempt(
        &mut self,
        seat: usize,
        check: &StorylineCheck,
        way: &CheckWay,
        edge: Option<Edge>,
        rival: Option<(usize, &CheckWay)>,
        at: f64,
    ) -> DiceRoll {
        let (dice, kept) = self.d20(edge);
        let bonus = self.bonus(seat, way);
        let total = kept as i32 + bonus;
        let versus = rival.map(|(other, other_way)| {
            let (other_dice, other_kept) = self.d20(None);
            let other_bonus = self.bonus(other, other_way);
            let other_total = other_kept as i32 + other_bonus;
            self.record(DiceRoll {
                at,
                seat: other as u32 + 1,
                what: format!("{}（应对）", check.title.trim()),
                way: way_name(other_way),
                formula: format!("d20{other_bonus:+}"),
                dice: other_dice,
                modifier: other_bonus,
                total: other_total,
                dc: None,
                success: Some(other_total >= total),
                edge: None,
                versus: Some(total),
            });
            other_total
        });
        let success = match versus {
            Some(other) => total > other,
            None => total >= check.dc as i32,
        };
        let roll = DiceRoll {
            at,
            seat: seat as u32 + 1,
            what: check.title.trim().to_string(),
            way: way_name(way),
            formula: format!("d20{bonus:+}"),
            dice,
            modifier: bonus,
            total,
            dc: versus.is_none().then_some(check.dc),
            success: Some(success),
            edge,
            versus,
        };
        self.tried.entry(check.id.clone()).or_default().push(seat);
        if let Some(helpers) = self.helped.get_mut(&check.id) {
            helpers.remove(&seat);
        }
        if success {
            self.passed.entry(check.id.clone()).or_insert(seat);
        }
        self.record(roll.clone());
        roll
    }

    fn record(&mut self, roll: DiceRoll) {
        self.rolls.push(roll);
        let extra = self.rolls.len().saturating_sub(ROLLS_KEPT);
        self.rolls.drain(..extra);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 规则数据每一项都写着出自规则书的哪一节，只有计划里定下的四个职业；技能和豁免都是规则里有的。
    /// 法术每一个都写着出处；角色卡上只能准备本职业、够得着环数的法术；
    /// 经验够了就升级：生命上限和法术位照新等级算，新得的法术位马上能用，用掉的不回来。
    #[test]
    fn spell_data_cites_the_srd_and_levels_follow_experience() {
        for s in &magic().spells {
            assert!(s.srd.starts_with("Spells > "), "{} 没写出处", s.id);
            assert!(s.level <= 2);
            assert!(s.classes.iter().all(|c| c == "cleric" || c == "wizard"));
        }
        let card = |class: &str, spells: &[&str]| Srd5Sheet {
            class: class.into(),
            level: 1,
            abilities: Abilities {
                str: 13,
                dex: 8,
                con: 14,
                int: 10,
                wis: 16,
                cha: 12,
            },
            skills: vec![],
            expertise: vec![],
            weapon: None,
            armor: None,
            shield: false,
            spells: spells.iter().map(|s| s.to_string()).collect(),
        };
        let bad = |s: Srd5Sheet| Srd5::open(&[s], 1, 1).is_err();
        assert!(bad(card("fighter", &["cure_wounds"])));
        assert!(bad(card("wizard", &["cure_wounds"])));
        assert!(bad(card("cleric", &["fireball"])));
        assert!(bad(card("cleric", &["hold_person"])), "一级够不着二环");
        let mut rules = Srd5::open(&[card("cleric", &["cure_wounds", "bless"])], 1, 1).unwrap();
        assert_eq!((rules.hit_points(0), rules.slots[0].clone()), (10, vec![2]));
        // 治疗：1d8 加感知调整值 3，耗一个一环位
        rules.hp[0] = 1;
        let healed = rules.cast_heal(0, "cure_wounds", 1, &[0]).unwrap();
        assert!((4..=11).contains(&healed[0].1));
        assert_eq!(rules.slots[0], vec![1]);
        assert!(rules.cast_heal(0, "bless", 1, &[0]).is_err());
        rules.hp[0] = 10;
        rules.slots[0][0] = 0;
        assert!(rules.cast_heal(0, "cure_wounds", 1, &[0]).is_err());
        assert_eq!(rules.gain(0, 299), None);
        assert_eq!(rules.gain(0, 1), Some(2));
        assert_eq!((rules.hit_points(0), rules.hp[0]), (17, 17));
        assert_eq!(rules.slots[0], vec![1]);
        assert_eq!(rules.gain(0, 600), Some(3));
        assert_eq!(rules.slots[0], vec![2, 2]);
        assert_eq!(rules.gain(0, 5000), None, "最高三级");
        assert!(rules.sheet_line(0).contains("3 级（经验 5900）"));
        // 睡不够六个小时不算长休；睡够了生命和法术位回满
        rules.hp[0] = 3;
        assert!(!rules.note_sleep(0, true, 0.0));
        assert!(!rules.note_sleep(0, false, 200.0));
        assert!(!rules.note_sleep(0, true, 300.0));
        assert!(rules.note_sleep(0, false, 300.0 + LONG_REST_MINUTES));
        assert_eq!((rules.hp[0], rules.slots[0].clone()), (24, vec![4, 2]));
    }

    /// 生命值和加值照规则算：战士 1 级体质 14 是 12 点，3 级是 28 点；游荡者专精的技能熟练加值算两次；
    /// 职业熟练的豁免加熟练加值，不熟练的只有属性调整值。
    #[test]
    fn class_data_cites_the_srd_and_derived_values_follow_the_rules() {
        let ids: Vec<&str> = data().classes.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["fighter", "rogue", "cleric", "wizard"]);
        for c in &data().classes {
            assert!(c.srd.starts_with("Classes > "), "{} 没写出处", c.id);
            assert!([6, 8, 10, 12].contains(&c.hit_die));
            assert_eq!(c.saves.len(), 2);
            assert!(c.saves.iter().all(|a| ABILITIES.contains(&a.as_str())));
            assert!(c
                .skills
                .iter()
                .all(|s| SKILLS.iter().any(|(id, _)| id == s)));
            assert!(c.skill_count <= c.skills.len());
        }
        let abilities = |str, dex, con, int, wis, cha| Abilities {
            str,
            dex,
            con,
            int,
            wis,
            cha,
        };
        let sheet = |class: &str, level, a, skills: &[&str], expertise: &[&str]| Srd5Sheet {
            class: class.into(),
            level,
            abilities: a,
            skills: skills.iter().map(|s| s.to_string()).collect(),
            expertise: expertise.iter().map(|s| s.to_string()).collect(),
            weapon: None,
            armor: None,
            shield: false,
            spells: vec![],
        };
        let fighter = abilities(16, 12, 14, 10, 13, 8);
        let rogue = abilities(8, 16, 12, 13, 10, 14);
        let rules = Srd5::open(
            &[
                sheet("fighter", 1, fighter, &["athletics"], &[]),
                sheet("fighter", 3, fighter, &["athletics"], &[]),
                sheet("rogue", 1, rogue, &["stealth", "deception"], &["stealth"]),
            ],
            3,
            1,
        )
        .unwrap();
        assert_eq!((rules.hit_points(0), rules.hit_points(1)), (12, 28));
        assert_eq!(rules.hit_points(2), 9);
        let way = |ability: &str, skill: Option<&str>, save| CheckWay {
            ability: ability.into(),
            skill: skill.map(str::to_owned),
            save,
        };
        // 游荡者：隐匿专精 +3+2+2，欺瞒熟练 +2+2，敏捷豁免熟练 +3+2，力量豁免不熟练 −1
        assert_eq!(rules.bonus(2, &way("dex", Some("stealth"), false)), 7);
        assert_eq!(rules.bonus(2, &way("cha", Some("deception"), false)), 4);
        assert_eq!(rules.bonus(2, &way("dex", None, true)), 5);
        assert_eq!(rules.bonus(2, &way("str", None, true)), -1);
        assert_eq!(way_key(&way("con", None, true)), "con_save");
        // 不合规则的角色卡：战士不能专精；职业单子外的技能超过背景给的两项；技能总数超了
        let bad = |s: Srd5Sheet| Srd5::open(&[s], 1, 1).is_err();
        assert!(bad(sheet(
            "fighter",
            1,
            fighter,
            &["athletics"],
            &["athletics"]
        )));
        assert!(bad(sheet(
            "fighter",
            1,
            fighter,
            &["arcana", "stealth", "religion"],
            &[]
        )));
        assert!(bad(sheet(
            "wizard",
            1,
            fighter,
            &["arcana", "history", "insight", "investigation", "medicine"],
            &[]
        )));
        assert!(bad(sheet("rogue", 1, rogue, &["stealth"], &["deception"])));

        // 优势和劣势确实改了成算：加值 0 对难度 11，平常一半成，有优势约四分之三，有劣势约四分之一
        let check = StorylineCheck {
            id: "odds".into(),
            title: "试手气".into(),
            detail: "".into(),
            dc: 11,
            seats: vec![],
            ways: vec![],
            repeat: true,
            passive: false,
            against: None,
            edges: vec![],
        };
        let flat = way("int", None, false);
        let mut rules = Srd5::open(&[sheet("fighter", 1, fighter, &[], &[])], 1, 77).unwrap();
        let mut rate = |edge| {
            let hits = (0..20_000)
                .filter(|_| rules.attempt(0, &check, &flat, edge, None, 0.0).success == Some(true))
                .count();
            hits as f64 / 20_000.0
        };
        let (plain, up, down) = (
            rate(None),
            rate(Some(Edge::Advantage)),
            rate(Some(Edge::Disadvantage)),
        );
        assert!((plain - 0.5).abs() < 0.02, "{plain}");
        assert!((up - 0.75).abs() < 0.02, "{up}");
        assert!((down - 0.25).abs() < 0.02, "{down}");
    }
}
