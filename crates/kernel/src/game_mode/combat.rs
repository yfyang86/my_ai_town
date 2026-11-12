//! 第五版规则的战斗（System Reference Document 5.1，CC BY 4.0）：一场遭遇战里谁先动、轮到谁、怎么打、打成什么样。
//! 这里不碰世界：只管参战的人（角色和对手）的生命、位置和状态，掷骰、算命中和伤害、判胜负；
//! 叫醒谁、告诉谁、谁真的死了由故事模式（storyline.rs）照这里交回的经过去办。
//! 对手不是地图上的人，只在这场战斗里有：照简单的打法自己动，不调模型。
//!
//! 场地是排成一线的几块区域：近身要在同一块，远程哪里都够得着（身边有敌人时有劣势）；一回合走一步，
//! 冲刺走两步；从有敌人的区域走开，没先「撤步」的会挨一下借机攻击。
//! 生命归零的角色倒地昏迷，轮到时掷死亡豁免：三次成功稳住，三次失败死亡，掷出 20 醒过来剩 1 点；
//! 对手归零就倒下。数据（生物、武器、护甲）在 data/rules/5e/creatures.json，每项写着出处。
use super::dice::Dice;
use crate::error::{KernelError, Result};
use protocol::{DiceRoll, Edge, FightView, FighterView, StorylineEncounter};
use serde::{Deserialize, Serialize};

/// 打到这么多轮还没分出胜负就散了
const ROUNDS_MAX: u32 = 20;
const ZONES: std::ops::RangeInclusive<usize> = 2..=5;
const FOES: std::ops::RangeInclusive<usize> = 1..=8;
const NAME_CHARS: usize = 16;

fn bad(text: impl Into<String>) -> KernelError {
    KernelError::action(text)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attack {
    pub name: String,
    pub bonus: i32,
    /// 几颗几面的骰子
    pub dice: (u32, u32),
    pub plus: i32,
    #[serde(default)]
    pub ranged: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Creature {
    pub id: String,
    pub name: String,
    #[cfg_attr(not(test), allow(dead_code))]
    pub srd: String,
    pub ac: u32,
    pub hp: i32,
    pub dex: u32,
    #[serde(default = "ten")]
    pub con: u32,
    #[serde(default = "ten")]
    pub wis: u32,
    /// 人形生物：定身术只对它们有用
    #[serde(default)]
    pub humanoid: bool,
    pub attack: Attack,
    /// 一回合打几下（多重攻击）；不写是一下
    #[serde(default = "one")]
    pub attacks: u32,
}

fn one() -> u32 {
    1
}

fn ten() -> u32 {
    10
}

/// 法术怎么生效
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpellKind {
    /// 掷法术攻击对护甲等级
    Attack,
    /// 对方过豁免：没过吃全额，过了吃一半或不吃
    Save,
    /// 必中
    Auto,
    Heal,
    /// 祝福：最多三个自己人攻击和豁免多掷一颗 d4（专注）
    Bless,
    /// 定身：感知豁免没过就动弹不得（专注）
    Hold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reach {
    Ranged,
    /// 要碰到：同一块区域
    Touch,
    /// 罩住一块区域里所有的对手
    Zone,
    /// 在场的自己人都算（只能休整时用）
    Group,
}

/// 一个准备好的法术，施法者的加值已经算进去了
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spell {
    pub id: String,
    pub name: String,
    /// 环数；戏法是 0
    pub level: u32,
    pub kind: SpellKind,
    pub reach: Reach,
    #[serde(default)]
    pub dice: (u32, u32),
    /// 每次伤害或治疗另加的数（魔法飞弹每发 +1，治疗加施法属性调整值）
    #[serde(default)]
    pub plus: i32,
    #[serde(default)]
    pub save: Option<String>,
    #[serde(default)]
    pub half: bool,
    /// 一次几发
    #[serde(default)]
    pub rays: u32,
    /// 用高一环的法术位多几颗骰、多几发
    #[serde(default)]
    pub upcast_dice: u32,
    #[serde(default)]
    pub upcast_rays: u32,
    #[serde(default)]
    pub concentration: bool,
    /// 打中后下一次打这个对象的攻击有优势
    #[serde(default)]
    pub marks: bool,
    /// 要花十分钟：打着的时候施不了
    #[serde(default)]
    pub rest_only: bool,
    /// 法术攻击加值、豁免难度
    #[serde(default)]
    pub bonus: i32,
    #[serde(default)]
    pub dc: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Weapon {
    pub id: String,
    pub name: String,
    #[cfg_attr(not(test), allow(dead_code))]
    pub srd: String,
    pub dice: (u32, u32),
    /// 灵巧或远程：力量和敏捷里用高的那个
    pub finesse: bool,
    pub ranged: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Armor {
    pub id: String,
    pub name: String,
    #[cfg_attr(not(test), allow(dead_code))]
    pub srd: String,
    pub base: u32,
    /// 敏捷调整值最多加几点；没有上限是 null
    pub dex_cap: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct Gear {
    pub creatures: Vec<Creature>,
    pub weapons: Vec<Weapon>,
    pub armor: Vec<Armor>,
    pub shield_bonus: u32,
}

pub fn gear() -> &'static Gear {
    static DATA: std::sync::OnceLock<Gear> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../data/rules/5e/creatures.json"))
            .expect("data/rules/5e/creatures.json 随源码带着，格式是对的")
    })
}

/// 参战的一方：角色带着座位下标，对手没有
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fighter {
    pub name: String,
    #[serde(default)]
    pub seat: Option<usize>,
    pub hp: i32,
    pub max_hp: i32,
    pub ac: u32,
    pub zone: usize,
    pub attack: Attack,
    /// 敏捷调整值：定先攻
    pub dex: i32,
    #[serde(default)]
    pub initiative: i32,
    /// 这一轮在闪避：打他的有劣势
    #[serde(default)]
    pub dodging: bool,
    /// 这一回合撤步了：走开不挨借机攻击
    #[serde(default)]
    pub disengaged: bool,
    #[serde(default)]
    pub prone: bool,
    /// 有同伴帮了一把：下一次攻击有优势
    #[serde(default)]
    pub aided: bool,
    /// 这一轮的反应用掉了（借机攻击）
    #[serde(default)]
    pub reacted: bool,
    /// 死亡豁免成功、失败的次数
    #[serde(default)]
    pub saves: (u8, u8),
    #[serde(default)]
    pub stable: bool,
    #[serde(default)]
    pub dead: bool,
    /// 退出了这场战斗
    #[serde(default)]
    pub fled: bool,
    /// 哪一边：0 是角色（决斗里是先动手的），1 是对手
    #[serde(default)]
    pub side: u8,
    /// 是哪种生物（算经验用）；角色没有
    #[serde(default)]
    pub creature: Option<String>,
    #[serde(default)]
    pub humanoid: bool,
    #[serde(default)]
    pub level: u32,
    /// 法术位：下标 0 是一环，值是还剩几个
    #[serde(default)]
    pub slots: Vec<u32>,
    #[serde(default)]
    pub spells: Vec<Spell>,
    /// 敏捷、体质、感知豁免的加值
    #[serde(default)]
    pub save_mods: (i32, i32, i32),
    /// 专注着的法术和它罩着的人
    #[serde(default)]
    pub focus: Option<(String, Vec<usize>)>,
    #[serde(default)]
    pub blessed: bool,
    /// 被定身：谁定的、豁免难度
    #[serde(default)]
    pub held: Option<(usize, u32)>,
    /// 被指引之箭照亮：下一次打他的攻击有优势
    #[serde(default)]
    pub marked: bool,
    /// 偷袭多掷几颗 d6（游荡者）；这一回合用过没有
    #[serde(default)]
    pub sneak: u32,
    #[serde(default)]
    pub sneaked: bool,
    /// 一回合打几下（有多重攻击的生物）
    #[serde(default = "one")]
    pub attacks: u32,
    /// 回气还能用（战士，一场休息一次）
    #[serde(default)]
    pub second_wind: bool,
}

impl Fighter {
    fn party(&self) -> bool {
        self.seat.is_some()
    }
    /// 还站着、还在场
    pub fn active(&self) -> bool {
        !self.dead && !self.fled && self.hp > 0
    }
    /// 还算在这场战斗里（倒了没死也算）
    fn present(&self) -> bool {
        !self.dead && !self.fled
    }
    fn states(&self) -> Vec<String> {
        let mut s = Vec::new();
        if self.dead {
            s.push("死亡".to_string());
        } else if self.fled {
            s.push("已退出".to_string());
        } else if self.hp <= 0 {
            s.push(
                if self.stable {
                    "昏迷（稳住了）"
                } else {
                    "昏迷"
                }
                .to_string(),
            );
        } else {
            if self.prone {
                s.push("倒地".to_string());
            }
            if self.dodging {
                s.push("闪避".to_string());
            }
            if self.held.is_some() {
                s.push("被定身".to_string());
            }
            if self.blessed {
                s.push("受祝福".to_string());
            }
            if self.focus.is_some() {
                s.push("专注中".to_string());
            }
        }
        s
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Won,
    Lost,
    Drawn,
}

/// 一回合里做的那件事
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Deed {
    Attack(usize),
    /// 冲刺：再走一步到这块区域
    Dash(usize),
    Dodge,
    Disengage,
    Help(usize),
    Flee,
    Pass,
    /// 施法：哪个法术、用几环的法术位、对谁
    Cast {
        spell: String,
        slot: u32,
        target: Option<usize>,
    },
    /// 回气（战士）：回 1d10 + 等级点生命
    SecondWind,
}

/// 角色交的一回合：先走不走、做什么
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    pub move_to: Option<usize>,
    pub deed: Deed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fight {
    pub id: String,
    pub title: String,
    pub zones: Vec<String>,
    pub round: u32,
    /// 先攻次序（fighters 的下标）
    pub order: Vec<usize>,
    /// 轮到次序里的第几个
    pub at: usize,
    pub fighters: Vec<Fighter>,
    #[serde(default)]
    pub outcome: Option<Outcome>,
}

/// 角色参战时带的数：名字、座位、此刻的生命和上限、护甲、攻击、敏捷调整值
pub struct Member {
    pub name: String,
    pub seat: usize,
    pub hp: i32,
    pub max_hp: i32,
    pub ac: u32,
    pub attack: Attack,
    pub dex: i32,
    pub level: u32,
    pub slots: Vec<u32>,
    pub spells: Vec<Spell>,
    pub save_mods: (i32, i32, i32),
    pub sneak: u32,
    pub second_wind: bool,
}

fn fighter_of(m: Member, zone: usize, side: u8) -> Fighter {
    Fighter {
        name: m.name,
        seat: Some(m.seat),
        hp: m.hp,
        max_hp: m.max_hp,
        ac: m.ac,
        zone,
        attack: m.attack,
        dex: m.dex,
        side,
        humanoid: true,
        level: m.level,
        slots: m.slots,
        spells: m.spells,
        save_mods: m.save_mods,
        sneak: m.sneak,
        second_wind: m.second_wind,
        ..blank()
    }
}

/// 剧本里的一场遭遇战合不合规：区域数、对手数和种类、位置都对得上。
pub fn check(enc: &StorylineEncounter, cast: usize) -> Result<()> {
    let what = format!("遭遇战「{}」", enc.id);
    if enc.title.trim().is_empty() || enc.title.chars().count() > 40 {
        return Err(bad(format!("{what}的标题要 1–40 字")));
    }
    if !ZONES.contains(&enc.zones.len())
        || enc
            .zones
            .iter()
            .any(|z| z.trim().is_empty() || z.chars().count() > NAME_CHARS)
    {
        return Err(bad(format!(
            "{what}要分 {}–{} 块区域，每块有名字",
            ZONES.start(),
            ZONES.end()
        )));
    }
    if enc.party.iter().any(|s| *s == 0 || *s as usize > cast) {
        return Err(bad(format!("{what}卷进来的座号不在 1–{cast} 之间")));
    }
    if enc.party_zone as usize >= enc.zones.len() {
        return Err(bad(format!("{what}：角色开始的区域不在场地里")));
    }
    if !FOES.contains(&enc.foes.len()) {
        return Err(bad(format!(
            "{what}的对手要 {}–{} 个",
            FOES.start(),
            FOES.end()
        )));
    }
    for foe in &enc.foes {
        if !gear().creatures.iter().any(|c| c.id == foe.creature) {
            return Err(bad(format!(
                "{what}：规则里没有「{}」这种生物",
                foe.creature
            )));
        }
        if foe.zone as usize >= enc.zones.len() {
            return Err(bad(format!("{what}：对手的位置不在场地里")));
        }
        if foe
            .name
            .as_ref()
            .is_some_and(|n| n.trim().is_empty() || n.chars().count() > NAME_CHARS)
        {
            return Err(bad(format!("{what}：对手的名字要 1–{NAME_CHARS} 字")));
        }
    }
    Ok(())
}

fn roll_dice(dice: &mut Dice, (count, sides): (u32, u32)) -> Vec<u32> {
    (0..count).map(|_| dice.roll(sides)).collect()
}

impl Fight {
    /// 开打：对手照剧本摆好，角色站在开始的那块；各掷先攻（d20 + 敏捷调整值），高的先动，
    /// 一样时敏捷高的先、再一样角色先。
    pub fn open(enc: &StorylineEncounter, members: Vec<Member>, dice: &mut Dice) -> Self {
        let mut fighters: Vec<Fighter> = members
            .into_iter()
            .map(|m| fighter_of(m, enc.party_zone as usize, 0))
            .collect();
        let mut counts = std::collections::BTreeMap::new();
        for foe in &enc.foes {
            let Some(c) = gear().creatures.iter().find(|c| c.id == foe.creature) else {
                continue;
            };
            let n = counts.entry(c.id.clone()).or_insert(0u32);
            *n += 1;
            let many = enc.foes.iter().filter(|f| f.creature == c.id).count() > 1;
            let name = foe.name.clone().unwrap_or_else(|| {
                if many {
                    format!("{}{n}", c.name)
                } else {
                    c.name.clone()
                }
            });
            fighters.push(Fighter {
                name: name.trim().to_string(),
                seat: None,
                hp: c.hp,
                max_hp: c.hp,
                ac: c.ac,
                zone: foe.zone as usize,
                attack: c.attack.clone(),
                dex: (c.dex as i32 - 10).div_euclid(2),
                side: 1,
                creature: Some(c.id.clone()),
                humanoid: c.humanoid,
                attacks: c.attacks.max(1),
                save_mods: (
                    (c.dex as i32 - 10).div_euclid(2),
                    (c.con as i32 - 10).div_euclid(2),
                    (c.wis as i32 - 10).div_euclid(2),
                ),
                ..blank()
            });
        }
        for f in &mut fighters {
            f.initiative = dice.roll(20) as i32 + f.dex;
        }
        let mut order: Vec<usize> = (0..fighters.len()).collect();
        order.sort_by(|&a, &b| {
            let (x, y) = (&fighters[a], &fighters[b]);
            y.initiative
                .cmp(&x.initiative)
                .then(y.dex.cmp(&x.dex))
                .then(y.party().cmp(&x.party()))
                .then(a.cmp(&b))
        });
        Self {
            id: enc.id.clone(),
            title: enc.title.trim().to_string(),
            zones: enc.zones.iter().map(|z| z.trim().to_string()).collect(),
            round: 1,
            order,
            at: 0,
            fighters,
            outcome: None,
        }
    }

    /// 此刻轮到谁（fighters 的下标）；打完了就没有
    pub fn up(&self) -> Option<usize> {
        self.outcome.is_none().then(|| self.order[self.at])
    }

    fn hostile(&self, a: usize, b: usize) -> bool {
        self.fighters[a].side != self.fighters[b].side
    }

    /// 这个人的敌人里还站着的
    fn enemies(&self, i: usize) -> Vec<usize> {
        (0..self.fighters.len())
            .filter(|&k| self.hostile(i, k) && self.fighters[k].active())
            .collect()
    }

    /// 回合开始：闪避、撤步、反应清掉，倒地的站起来；昏迷的角色掷死亡豁免。
    /// 返回这一步的经过，和他这回合能不能动。
    pub fn start_turn(
        &mut self,
        i: usize,
        dice: &mut Dice,
        rolls: &mut Vec<DiceRoll>,
        at: f64,
    ) -> (Vec<String>, bool) {
        let mut lines = Vec::new();
        let f = &mut self.fighters[i];
        f.dodging = false;
        f.disengaged = false;
        f.reacted = false;
        if !f.present() {
            return (lines, false);
        }
        f.sneaked = false;
        if f.hp > 0 {
            f.prone = false;
            // 被定身的人轮到时动不了，回合末再过一次感知豁免
            if let Some((by, dc)) = f.held {
                let name = f.name.clone();
                // 这一回合动不了；回合末再过一次豁免，过了下一回合才能动
                if self.save(i, ("wis", dc), "定身", dice, rolls, at) {
                    self.fighters[i].held = None;
                    if self.fighters[by].focus.is_some() {
                        self.fighters[by].focus = None;
                    }
                    lines.push(format!("{name} 这一回合动不了，回合末挣脱了定身"));
                } else {
                    lines.push(format!("{name} 还被定着，动不了"));
                }
                return (lines, false);
            }
            return (lines, true);
        }
        if f.stable || !f.party() {
            return (lines, false);
        }
        let die = dice.roll(20);
        let name = f.name.clone();
        let verdict = match die {
            20 => {
                f.hp = 1;
                f.saves = (0, 0);
                f.prone = false;
                "掷出 20，醒了过来，剩 1 点生命".to_string()
            }
            1 => {
                f.saves.1 = (f.saves.1 + 2).min(3);
                format!("掷出 1，算两次失败（失败 {}/3）", f.saves.1)
            }
            d if d >= 10 => {
                f.saves.0 += 1;
                format!("成功（成功 {}/3）", f.saves.0)
            }
            _ => {
                f.saves.1 += 1;
                format!("失败（失败 {}/3）", f.saves.1)
            }
        };
        let mut line = format!("{name} 昏迷中掷死亡豁免：d20 得 {die}，{verdict}");
        if f.saves.1 >= 3 {
            f.dead = true;
            line.push_str("。没撑过去，死了");
        } else if f.saves.0 >= 3 {
            f.stable = true;
            line.push_str("。伤势稳住了");
        }
        rolls.push(DiceRoll {
            at,
            seat: f.seat.map_or(0, |s| s as u32 + 1),
            what: "死亡豁免".into(),
            way: "不加任何加值".into(),
            formula: "d20".into(),
            dice: vec![die],
            modifier: 0,
            total: die as i32,
            dc: Some(10),
            success: Some(die >= 10),
            edge: None,
            versus: None,
        });
        lines.push(line);
        let awake = self.fighters[i].hp > 0;
        (lines, awake)
    }

    /// 一次攻击：d20 加攻击加值对护甲等级，掷出 1 必不中、20 必中而且伤害骰加倍。
    /// 有优势：目标倒地而且是近身、或者有同伴帮了一把；有劣势：目标在闪避、或者身边有敌人还用远程。
    fn strike(
        &mut self,
        i: usize,
        target: usize,
        dice: &mut Dice,
        rolls: &mut Vec<DiceRoll>,
        at: f64,
        how: &str,
    ) -> String {
        let attack = self.fighters[i].attack.clone();
        let crowded = self
            .enemies(i)
            .iter()
            .any(|&k| self.fighters[k].zone == self.fighters[i].zone);
        let t = &self.fighters[target];
        let held = t.held.is_some();
        let up =
            (t.prone && !attack.ranged) || self.fighters[i].aided || t.hp <= 0 || t.marked || held;
        let down = t.dodging || (attack.ranged && crowded) || (t.prone && attack.ranged);
        let edge = match (up, down) {
            (true, false) => Some(Edge::Advantage),
            (false, true) => Some(Edge::Disadvantage),
            _ => None,
        };
        self.fighters[i].aided = false;
        self.fighters[target].marked = false;
        let first = dice.roll(20);
        let (thrown, kept) = match edge {
            None => (vec![first], first),
            Some(e) => {
                let second = dice.roll(20);
                let kept = if e == Edge::Advantage {
                    first.max(second)
                } else {
                    first.min(second)
                };
                (vec![first, second], kept)
            }
        };
        // 受祝福的攻击多掷一颗 d4
        let bless = if self.fighters[i].blessed {
            dice.roll(4) as i32
        } else {
            0
        };
        let total = kept as i32 + attack.bonus + bless;
        let ac = self.fighters[target].ac;
        // 掷出 20，或者近身打中动弹不得的人，都算重击
        let landed = kept != 1 && (kept == 20 || total >= ac as i32);
        let crit = kept == 20 || (landed && held && !attack.ranged);
        let hit = kept != 1 && (crit || total >= ac as i32);
        rolls.push(DiceRoll {
            at,
            seat: self.fighters[i].seat.map_or(0, |s| s as u32 + 1),
            what: format!(
                "{} 攻击 {}",
                self.fighters[i].name, self.fighters[target].name
            ),
            way: attack.name.clone(),
            formula: format!("d20{:+}", attack.bonus),
            dice: thrown.clone(),
            modifier: attack.bonus,
            total,
            dc: Some(ac),
            success: Some(hit),
            edge,
            versus: None,
        });
        let thrown_text = match edge {
            Some(Edge::Advantage) => format!("有优势，掷 {} 和 {} 取高的", thrown[0], thrown[1]),
            Some(Edge::Disadvantage) => {
                format!("有劣势，掷 {} 和 {} 取低的", thrown[0], thrown[1])
            }
            None => format!("掷 d20 得 {kept}"),
        };
        let head = format!(
            "{}{how}用{}打 {}：{thrown_text}，{:+} 共 {total}，对护甲 {ac}",
            self.fighters[i].name, attack.name, self.fighters[target].name, attack.bonus
        );
        if !hit {
            return format!("{head}：没打中");
        }
        let count = if crit {
            attack.dice.0 * 2
        } else {
            attack.dice.0
        };
        let mut damage = (roll_dice(dice, (count, attack.dice.1)).iter().sum::<u32>() as i32
            + attack.plus)
            .max(1);
        // 偷袭（游荡者）：有优势，或者目标身边有自己的同伴，每回合一次，多掷几颗 d6
        let flanked = (0..self.fighters.len()).any(|k| {
            k != i
                && !self.hostile(i, k)
                && self.fighters[k].active()
                && self.fighters[k].zone == self.fighters[target].zone
        });
        let mut sneak = String::new();
        let me = &self.fighters[i];
        if me.sneak > 0 && !me.sneaked && (edge == Some(Edge::Advantage) || flanked) {
            let extra = roll_dice(dice, (me.sneak * if crit { 2 } else { 1 }, 6))
                .iter()
                .sum::<u32>() as i32;
            damage += extra;
            sneak = format!("（偷袭 +{extra}）。");
            self.fighters[i].sneaked = true;
        }
        let after = format!(
            "{sneak}{}",
            self.hurt(target, damage, crit, dice, rolls, at)
        );
        format!(
            "{head}：{}，伤害 {damage}{}{after}",
            if crit { "重击" } else { "命中" },
            if after.starts_with('（') { "" } else { "。" }
        )
    }

    /// 伤害落到一个人身上：还醒着的扣生命，扣到 0 角色倒地昏迷、对手倒下；已经昏迷的算死亡豁免失败（重击两次）。
    /// 正专注着法术的人挨了打要过体质豁免（难度 10 和伤害一半里高的），没过法术就断了。返回这一下的结果。
    fn hurt(
        &mut self,
        target: usize,
        damage: i32,
        crit: bool,
        dice: &mut Dice,
        rolls: &mut Vec<DiceRoll>,
        at: f64,
    ) -> String {
        let t = &mut self.fighters[target];
        let name = t.name.clone();
        if t.hp <= 0 {
            t.saves.1 = (t.saves.1 + if crit { 2 } else { 1 }).min(3);
            t.stable = false;
            return if t.saves.1 >= 3 {
                t.dead = true;
                format!("{name} 没撑过去，死了")
            } else {
                format!("{name} 昏迷中又挨了一下（死亡豁免失败 {}/3）", t.saves.1)
            };
        }
        t.hp -= damage;
        if t.hp > 0 {
            let mut line = format!("{name} 还剩 {} 点生命", t.hp);
            if t.focus.is_some() {
                let dc = (damage / 2).max(10);
                let die = dice.roll(20);
                let total = die as i32 + t.save_mods.1;
                let kept = total >= dc;
                rolls.push(DiceRoll {
                    at,
                    seat: t.seat.map_or(0, |s| s as u32 + 1),
                    what: format!("{name} 维持专注"),
                    way: "体质豁免".into(),
                    formula: format!("d20{:+}", t.save_mods.1),
                    dice: vec![die],
                    modifier: t.save_mods.1,
                    total,
                    dc: Some(dc as u32),
                    success: Some(kept),
                    edge: None,
                    versus: None,
                });
                if !kept {
                    line.push_str(&format!("，{}", self.drop_focus(target)));
                }
            }
            return line;
        }
        t.hp = 0;
        let fell = if t.party() {
            t.prone = true;
            t.saves = (0, 0);
            t.stable = false;
            format!("{name} 倒下昏迷了")
        } else {
            t.dead = true;
            format!("{name} 倒下了")
        };
        if self.fighters[target].focus.is_some() {
            format!("{fell}，{}", self.drop_focus(target))
        } else {
            fell
        }
    }

    /// 专注断了：法术罩着的人身上的效果跟着没了。
    fn drop_focus(&mut self, caster: usize) -> String {
        let Some((spell, targets)) = self.fighters[caster].focus.take() else {
            return String::new();
        };
        for t in targets {
            if let Some(f) = self.fighters.get_mut(t) {
                f.blessed = false;
                if f.held.is_some_and(|(by, _)| by == caster) {
                    f.held = None;
                }
            }
        }
        let name = self.fighters[caster]
            .spells
            .iter()
            .find(|s| s.id == spell)
            .map_or(spell.clone(), |s| s.name.clone());
        format!("{} 的{name}断了", self.fighters[caster].name)
    }

    /// 回生命：不超过上限；昏迷的人醒过来（倒在地上，轮到时自己站起来）。
    fn heal(&mut self, target: usize, amount: i32) -> String {
        let t = &mut self.fighters[target];
        let was_down = t.hp <= 0;
        t.hp = (t.hp.max(0) + amount.max(1)).min(t.max_hp);
        if was_down {
            t.saves = (0, 0);
            t.stable = false;
        }
        format!(
            "{} 回了 {amount} 点生命，现在 {}/{}{}",
            t.name,
            t.hp,
            t.max_hp,
            if was_down { "，醒了过来" } else { "" }
        )
    }

    /// 一次豁免：d20 加这项豁免的加值，不低于难度就过。受祝福的再加 1d4。
    fn save(
        &mut self,
        who: usize,
        (ability, dc): (&str, u32),
        what: &str,
        dice: &mut Dice,
        rolls: &mut Vec<DiceRoll>,
        at: f64,
    ) -> bool {
        let f = &self.fighters[who];
        let bonus = match ability {
            "dex" => f.save_mods.0,
            "con" => f.save_mods.1,
            _ => f.save_mods.2,
        };
        let die = dice.roll(20);
        let extra = if f.blessed { dice.roll(4) as i32 } else { 0 };
        let total = die as i32 + bonus + extra;
        let ok = total >= dc as i32;
        rolls.push(DiceRoll {
            at,
            seat: f.seat.map_or(0, |s| s as u32 + 1),
            what: format!("{} 抵抗{what}", f.name),
            way: format!("{}豁免", super::srd5::ability_name(ability)),
            formula: format!("d20{bonus:+}"),
            dice: vec![die],
            modifier: bonus + extra,
            total,
            dc: Some(dc),
            success: Some(ok),
            edge: None,
            versus: None,
        });
        ok
    }

    /// 施一个法术。环位不够、够不着、对象不对的退回去，不耗环位。
    fn cast(
        &mut self,
        i: usize,
        (id, slot, target): (&str, u32, Option<usize>),
        dice: &mut Dice,
        rolls: &mut Vec<DiceRoll>,
        at: f64,
    ) -> Result<Vec<String>> {
        let Some(spell) = self.fighters[i].spells.iter().find(|s| s.id == id).cloned() else {
            let known: Vec<&str> = self.fighters[i]
                .spells
                .iter()
                .map(|s| s.id.as_str())
                .collect();
            return Err(bad(format!(
                "你没有准备「{id}」：准备了的是 {}",
                if known.is_empty() {
                    "（没有法术）".to_string()
                } else {
                    known.join("、")
                }
            )));
        };
        if spell.rest_only {
            return Err(bad(format!("{}要花十分钟，打着的时候施不了", spell.name)));
        }
        // 戏法不耗环位；别的法术用不低于它环数的环位
        let slot = if spell.level == 0 {
            0
        } else {
            slot.max(spell.level)
        };
        if slot > 0
            && self.fighters[i]
                .slots
                .get(slot as usize - 1)
                .is_none_or(|n| *n == 0)
        {
            return Err(bad(format!("{slot} 环的法术位用完了")));
        }
        let me = self.fighters[i].zone;
        let up = slot.saturating_sub(spell.level);
        let who = |fight: &Fight, want_foe: bool| -> Result<usize> {
            let t =
                target.ok_or_else(|| bad(format!("{}要指一个对象：target 填编号", spell.name)))?;
            let f = fight
                .fighters
                .get(t)
                .filter(|f| f.present())
                .ok_or_else(|| bad("这个对象不在场"))?;
            if fight.hostile(i, t) != want_foe {
                return Err(bad(format!(
                    "{}要对{}用",
                    spell.name,
                    if want_foe { "对手" } else { "自己人" }
                )));
            }
            if spell.reach == Reach::Touch && f.zone != me {
                return Err(bad(format!(
                    "{}要碰到对方：{} 在{}，先走到同一块区域",
                    spell.name, f.name, fight.zones[f.zone]
                )));
            }
            Ok(t)
        };
        let caster = self.fighters[i].name.clone();
        let mut lines = Vec::new();
        match spell.kind {
            SpellKind::Attack => {
                let t = who(self, true)?;
                let crowded = self.enemies(i).iter().any(|&k| self.fighters[k].zone == me);
                for _ in 0..spell.rays.max(1) {
                    if !self.fighters[t].present() {
                        break;
                    }
                    let up_edge = self.fighters[t].marked || self.fighters[t].held.is_some();
                    let down_edge =
                        self.fighters[t].dodging || (spell.reach == Reach::Ranged && crowded);
                    self.fighters[t].marked = false;
                    let edge = match (up_edge, down_edge) {
                        (true, false) => Some(Edge::Advantage),
                        (false, true) => Some(Edge::Disadvantage),
                        _ => None,
                    };
                    let first = dice.roll(20);
                    let (thrown, kept) = match edge {
                        None => (vec![first], first),
                        Some(e) => {
                            let second = dice.roll(20);
                            let kept = if e == Edge::Advantage {
                                first.max(second)
                            } else {
                                first.min(second)
                            };
                            (vec![first, second], kept)
                        }
                    };
                    let bless = if self.fighters[i].blessed {
                        dice.roll(4) as i32
                    } else {
                        0
                    };
                    let total = kept as i32 + spell.bonus + bless;
                    let ac = self.fighters[t].ac;
                    let crit = kept == 20;
                    let hit = kept != 1 && (crit || total >= ac as i32);
                    rolls.push(DiceRoll {
                        at,
                        seat: self.fighters[i].seat.map_or(0, |s| s as u32 + 1),
                        what: format!("{caster} 对 {} 施{}", self.fighters[t].name, spell.name),
                        way: "法术攻击".into(),
                        formula: format!("d20{:+}", spell.bonus),
                        dice: thrown,
                        modifier: spell.bonus + bless,
                        total,
                        dc: Some(ac),
                        success: Some(hit),
                        edge,
                        versus: None,
                    });
                    let head = format!(
                        "{caster} 对 {} 施{}：掷 d20 得 {kept}，共 {total}，对护甲 {ac}",
                        self.fighters[t].name, spell.name
                    );
                    if !hit {
                        lines.push(format!("{head}：没打中"));
                        continue;
                    }
                    let count = (spell.dice.0 + spell.upcast_dice * up) * if crit { 2 } else { 1 };
                    let damage = (roll_dice(dice, (count, spell.dice.1)).iter().sum::<u32>()
                        as i32
                        + spell.plus)
                        .max(1);
                    let after = self.hurt(t, damage, crit, dice, rolls, at);
                    if spell.marks && self.fighters[t].present() {
                        self.fighters[t].marked = true;
                    }
                    lines.push(format!(
                        "{head}：{}，伤害 {damage}。{after}",
                        if crit { "重击" } else { "命中" }
                    ));
                }
            }
            SpellKind::Auto => {
                let t = who(self, true)?;
                let darts = spell.rays.max(1) + spell.upcast_rays * up;
                let damage: i32 = (0..darts)
                    .map(|_| roll_dice(dice, spell.dice).iter().sum::<u32>() as i32 + spell.plus)
                    .sum();
                let after = self.hurt(t, damage, false, dice, rolls, at);
                lines.push(format!(
                    "{caster} 施{}，{darts} 发都打中 {}：伤害 {damage}。{after}",
                    spell.name, self.fighters[t].name
                ));
            }
            SpellKind::Save => {
                let ability = spell.save.clone().unwrap_or_else(|| "dex".into());
                let targets: Vec<usize> = if spell.reach == Reach::Zone {
                    // 罩住一块区域：自己这块或隔壁那块里所有的对手
                    let zone = match target {
                        Some(t) => self.fighters.get(t).map_or(me, |f| f.zone),
                        None => me,
                    };
                    if zone.abs_diff(me) > 1 {
                        return Err(bad(format!("{}只够得着你这块和隔壁的区域", spell.name)));
                    }
                    self.enemies(i)
                        .into_iter()
                        .filter(|&k| self.fighters[k].zone == zone)
                        .collect()
                } else {
                    vec![who(self, true)?]
                };
                if targets.is_empty() {
                    return Err(bad("那块区域里没有对手"));
                }
                let count = spell.dice.0 + spell.upcast_dice * up;
                let full = (roll_dice(dice, (count, spell.dice.1)).iter().sum::<u32>() as i32
                    + spell.plus)
                    .max(1);
                for t in targets {
                    let name = self.fighters[t].name.clone();
                    let saved = self.save(t, (&ability, spell.dc), &spell.name, dice, rolls, at);
                    let damage = match (saved, spell.half) {
                        (false, _) => full,
                        (true, true) => full / 2,
                        (true, false) => 0,
                    };
                    let verdict = if saved { "豁免过了" } else { "没躲开" };
                    if damage == 0 {
                        lines.push(format!(
                            "{caster} 的{}罩向 {name}：{verdict}，没受伤",
                            spell.name
                        ));
                    } else {
                        let after = self.hurt(t, damage, false, dice, rolls, at);
                        lines.push(format!(
                            "{caster} 的{}罩向 {name}（难度 {}）：{verdict}，伤害 {damage}。{after}",
                            spell.name, spell.dc
                        ));
                    }
                }
            }
            SpellKind::Heal => {
                let t = who(self, false)?;
                let amount = roll_dice(dice, (spell.dice.0 + spell.upcast_dice * up, spell.dice.1))
                    .iter()
                    .sum::<u32>() as i32
                    + spell.plus;
                lines.push(format!(
                    "{caster} 施{}：{}",
                    spell.name,
                    self.heal(t, amount)
                ));
            }
            SpellKind::Bless => {
                let allies: Vec<usize> = (0..self.fighters.len())
                    .filter(|&k| !self.hostile(i, k) && self.fighters[k].active())
                    .take(3)
                    .collect();
                if self.fighters[i].focus.is_some() {
                    lines.push(self.drop_focus(i));
                }
                for &k in &allies {
                    self.fighters[k].blessed = true;
                }
                let names: Vec<String> = allies
                    .iter()
                    .map(|&k| self.fighters[k].name.clone())
                    .collect();
                self.fighters[i].focus = Some((spell.id.clone(), allies));
                lines.push(format!(
                    "{caster} 施{}（专注）：{} 的攻击和豁免各多掷一颗 d4",
                    spell.name,
                    names.join("、")
                ));
            }
            SpellKind::Hold => {
                let t = who(self, true)?;
                if !self.fighters[t].humanoid {
                    return Err(bad(format!(
                        "{}只对人形生物有用，{} 不是",
                        spell.name, self.fighters[t].name
                    )));
                }
                if self.fighters[i].focus.is_some() {
                    lines.push(self.drop_focus(i));
                }
                let name = self.fighters[t].name.clone();
                if self.save(t, ("wis", spell.dc), &spell.name, dice, rolls, at) {
                    lines.push(format!(
                        "{caster} 对 {name} 施{}：{name} 挣脱了",
                        spell.name
                    ));
                } else {
                    self.fighters[t].held = Some((i, spell.dc));
                    self.fighters[i].focus = Some((spell.id.clone(), vec![t]));
                    lines.push(format!(
                        "{caster} 对 {name} 施{}（专注）：{name} 动弹不得，打他的有优势，近身打中都算重击",
                        spell.name
                    ));
                }
            }
        }
        if slot > 0 {
            self.fighters[i].slots[slot as usize - 1] -= 1;
        }
        Ok(lines)
    }

    /// 两个角色动手：就在眼前这一块，打到一方倒下为止（倒下的不再挨打，打完缓过来）。
    pub fn duel(a: Member, b: Member, dice: &mut Dice) -> Self {
        let title = format!("{} 和 {} 动手", a.name, b.name);
        let mut fighters: Vec<Fighter> = [a, b]
            .into_iter()
            .enumerate()
            .map(|(side, m)| fighter_of(m, 0, side as u8))
            .collect();
        for f in &mut fighters {
            f.initiative = dice.roll(20) as i32 + f.dex;
        }
        let order = if (fighters[1].initiative, fighters[1].dex)
            > (fighters[0].initiative, fighters[0].dex)
        {
            vec![1, 0]
        } else {
            vec![0, 1]
        };
        Self {
            id: "duel".into(),
            title,
            zones: vec!["跟前".into(), "几步外".into()],
            round: 1,
            order,
            at: 0,
            fighters,
            outcome: None,
        }
    }

    /// 走一步到隔壁那块：没撤步的话，原来那块里还站着、有近身攻击、反应没用掉的敌人各打一下借机攻击；
    /// 被打倒了就走不成。
    fn walk(
        &mut self,
        i: usize,
        to: usize,
        dice: &mut Dice,
        rolls: &mut Vec<DiceRoll>,
        at: f64,
    ) -> Vec<String> {
        let mut lines = Vec::new();
        let from = self.fighters[i].zone;
        if !self.fighters[i].disengaged {
            let threats: Vec<usize> = self
                .enemies(i)
                .into_iter()
                .filter(|&k| {
                    let f = &self.fighters[k];
                    f.zone == from && !f.reacted && !f.attack.ranged
                })
                .collect();
            for k in threats {
                self.fighters[k].reacted = true;
                lines.push(self.strike(k, i, dice, rolls, at, "趁他走开借机"));
                if !self.fighters[i].active() {
                    return lines;
                }
            }
        }
        self.fighters[i].zone = to;
        lines.push(format!(
            "{} 从{}走到{}",
            self.fighters[i].name, self.zones[from], self.zones[to]
        ));
        lines
    }

    /// 角色交的一回合。不合规矩的（够不着、走不到、没有这个人）退回去说明原因，这一回合不算用掉。
    pub fn party_turn(
        &mut self,
        i: usize,
        order: &Order,
        dice: &mut Dice,
        rolls: &mut Vec<DiceRoll>,
        at: f64,
    ) -> Result<Vec<String>> {
        let near = |a: usize, b: usize| a.abs_diff(b) == 1;
        let here = self.fighters[i].zone;
        if let Some(to) = order.move_to {
            if to >= self.zones.len() || !near(here, to) {
                return Err(bad(format!(
                    "一回合只能走到隔壁的区域：你在{}",
                    self.zones[here]
                )));
            }
        }
        let after = order.move_to.unwrap_or(here);
        match &order.deed {
            Deed::Attack(t) => {
                let Some(target) = self.fighters.get(*t) else {
                    return Err(bad("没有这个目标"));
                };
                if !self.hostile(i, *t) || !target.present() {
                    return Err(bad(format!("{} 不是还在场的对手", target.name)));
                }
                if !self.fighters[i].attack.ranged && target.zone != after {
                    return Err(bad(format!(
                        "{} 在{}，近身打要先走到同一块区域",
                        target.name, self.zones[target.zone]
                    )));
                }
            }
            Deed::Dash(to) => {
                if *to >= self.zones.len() || !near(after, *to) {
                    return Err(bad("冲刺是再走一步到隔壁的区域"));
                }
            }
            Deed::Help(a) => {
                let Some(ally) = self.fighters.get(*a) else {
                    return Err(bad("没有这个人"));
                };
                if *a == i || self.hostile(i, *a) || !ally.active() || ally.zone != after {
                    return Err(bad(format!(
                        "{} 不是和你在同一块区域、还站着的同伴",
                        ally.name
                    )));
                }
            }
            Deed::Flee => {
                if self
                    .enemies(i)
                    .iter()
                    .any(|&k| self.fighters[k].zone == after)
                {
                    return Err(bad("身边还有对手，脱不了身：先撤步走开，或者把他打倒"));
                }
            }
            Deed::SecondWind => {
                if !self.fighters[i].second_wind {
                    return Err(bad("回气用过了，要好好休息一晚才能再用"));
                }
            }
            Deed::Dodge | Deed::Disengage | Deed::Pass | Deed::Cast { .. } => {}
        }
        // 施法先试一遍：不合规矩的退回去，不走那一步、不耗环位
        if let Deed::Cast {
            spell,
            slot,
            target,
        } = &order.deed
        {
            let mut trial = self.clone();
            trial.fighters[i].zone = after;
            trial.cast(
                i,
                (spell, *slot, *target),
                &mut dice.clone(),
                &mut Vec::new(),
                at,
            )?;
        }
        let mut lines = Vec::new();
        // 撤步要在走之前生效
        if order.deed == Deed::Disengage {
            self.fighters[i].disengaged = true;
            lines.push(format!("{} 撤步", self.fighters[i].name));
        }
        if let Some(to) = order.move_to {
            lines.extend(self.walk(i, to, dice, rolls, at));
            if !self.fighters[i].active() {
                return Ok(lines);
            }
        }
        let name = self.fighters[i].name.clone();
        match &order.deed {
            Deed::Attack(t) => lines.push(self.strike(i, *t, dice, rolls, at, "")),
            Deed::Dash(to) => lines.extend(self.walk(i, *to, dice, rolls, at)),
            Deed::Dodge => {
                self.fighters[i].dodging = true;
                lines.push(format!("{name} 全力闪避"));
            }
            Deed::Help(a) => {
                self.fighters[*a].aided = true;
                lines.push(format!(
                    "{name} 帮 {} 牵制对手：他下一次攻击有优势",
                    self.fighters[*a].name
                ));
            }
            Deed::Flee => {
                self.fighters[i].fled = true;
                lines.push(format!("{name} 退出了战斗"));
            }
            Deed::Disengage => {}
            Deed::Pass => lines.push(format!("{name} 这一回合没有出手")),
            Deed::Cast {
                spell,
                slot,
                target,
            } => lines.extend(self.cast(i, (spell, *slot, *target), dice, rolls, at)?),
            Deed::SecondWind => {
                let f = &self.fighters[i];
                let amount = dice.roll(10) as i32 + f.level as i32;
                self.fighters[i].second_wind = false;
                lines.push(format!("{name} 回气：{}", self.heal(i, amount)));
            }
        }
        Ok(lines)
    }

    /// 对手的一回合，照简单的打法：远程的打还站着的角色里生命最少的；近身的，同一块里有人就打生命最少的，
    /// 没有就朝最近的角色走一步，走到了就打。
    pub fn foe_turn(
        &mut self,
        i: usize,
        dice: &mut Dice,
        rolls: &mut Vec<DiceRoll>,
        at: f64,
    ) -> Vec<String> {
        let mut lines = Vec::new();
        let weakest = |fight: &Fight, among: Vec<usize>| {
            among.into_iter().min_by_key(|&k| (fight.fighters[k].hp, k))
        };
        let targets = self.enemies(i);
        if targets.is_empty() {
            return lines;
        }
        if self.fighters[i].attack.ranged {
            if let Some(t) = weakest(self, targets) {
                lines.push(self.strike(i, t, dice, rolls, at, ""));
            }
            return lines;
        }
        let here = self.fighters[i].zone;
        let close: Vec<usize> = targets
            .iter()
            .copied()
            .filter(|&k| self.fighters[k].zone == here)
            .collect();
        if close.is_empty() {
            let Some(goal) = targets
                .iter()
                .map(|&k| self.fighters[k].zone)
                .min_by_key(|z| z.abs_diff(here))
            else {
                return lines;
            };
            let to = if goal > here { here + 1 } else { here - 1 };
            lines.extend(self.walk(i, to, dice, rolls, at));
            if !self.fighters[i].active() {
                return lines;
            }
        }
        let here = self.fighters[i].zone;
        let close: Vec<usize> = self
            .enemies(i)
            .into_iter()
            .filter(|&k| self.fighters[k].zone == here)
            .collect();
        let Some(mut t) = weakest(self, close) else {
            return lines;
        };
        // 多重攻击：打倒了一个就换同一块里还站着的下一个
        for _ in 0..self.fighters[i].attacks.max(1) {
            if self.fighters[t].hp <= 0 {
                let standing: Vec<usize> = self
                    .enemies(i)
                    .into_iter()
                    .filter(|&k| self.fighters[k].zone == here && self.fighters[k].hp > 0)
                    .collect();
                match weakest(self, standing) {
                    Some(next) => t = next,
                    None => break,
                }
            }
            lines.push(self.strike(i, t, dice, rolls, at, ""));
        }
        lines
    }

    /// 这一回合结束：看分没分出胜负，没有就轮到下一个；转完一圈是新的一轮。
    pub fn end_turn(&mut self) {
        let foes = self.fighters.iter().any(|f| f.side == 1 && f.active());
        let party = self.fighters.iter().any(|f| f.side == 0 && f.active());
        self.outcome = match (party, foes) {
            (_, false) => Some(Outcome::Won),
            (false, true) => Some(Outcome::Lost),
            _ => None,
        };
        if self.outcome.is_some() {
            return;
        }
        self.at += 1;
        if self.at >= self.order.len() {
            self.at = 0;
            self.round += 1;
            if self.round > ROUNDS_MAX {
                self.outcome = Some(Outcome::Drawn);
            }
        }
    }

    /// 这个人怎么称呼目标：角色用座号，对手用 f1、f2……（按名单里的次序）
    pub fn tag(&self, k: usize) -> String {
        match self.fighters[k].seat {
            Some(seat) => (seat + 1).to_string(),
            None => {
                let n = self.fighters[..=k].iter().filter(|f| !f.party()).count();
                format!("f{n}")
            }
        }
    }

    /// target 写的是谁：座号、f 加编号、或者名字
    pub fn find(&self, raw: &str) -> Option<usize> {
        let raw = raw.trim();
        (0..self.fighters.len()).find(|&k| self.tag(k) == raw || self.fighters[k].name == raw)
    }

    /// 区域写的是哪一块：名字，或者从 1 数的第几块
    pub fn zone(&self, raw: &str) -> Option<usize> {
        let raw = raw.trim();
        self.zones.iter().position(|z| z == raw).or_else(|| {
            raw.parse::<usize>()
                .ok()
                .filter(|n| (1..=self.zones.len()).contains(n))
                .map(|n| n - 1)
        })
    }

    /// 给轮到的角色看的战况：第几轮、每块区域里是谁（同伴写生命，对手只写伤得重不重）、自己的状况
    pub fn brief(&self, i: usize) -> Vec<String> {
        let me = &self.fighters[i];
        let mut lines = vec![format!(
            "{} · 第 {} 轮。你在{}，生命 {}/{}，护甲 {}，{}（{}{:+} 命中，伤害 {}d{}{:+}）",
            self.title,
            self.round,
            self.zones[me.zone],
            me.hp,
            me.max_hp,
            me.ac,
            me.attack.name,
            if me.attack.ranged {
                "远程 "
            } else {
                "近身 "
            },
            me.attack.bonus,
            me.attack.dice.0,
            me.attack.dice.1,
            me.attack.plus
        )];
        if !me.spells.is_empty() {
            let slots: Vec<String> = me
                .slots
                .iter()
                .enumerate()
                .map(|(k, n)| format!("{} 环剩 {n}", k + 1))
                .collect();
            let spells: Vec<String> = me
                .spells
                .iter()
                .filter(|s| !s.rest_only)
                .map(|s| {
                    format!(
                        "{}={}（{}）",
                        s.id,
                        s.name,
                        if s.level == 0 {
                            "戏法，不耗法术位".to_string()
                        } else {
                            format!("{} 环", s.level)
                        }
                    )
                })
                .collect();
            lines.push(format!(
                "法术位：{}。能施的法术：{}",
                if slots.is_empty() {
                    "没有".to_string()
                } else {
                    slots.join("，")
                },
                spells.join("；")
            ));
        }
        if me.second_wind {
            lines.push("回气还能用一次（second_wind：回 1d10 + 等级点生命）。".to_string());
        }
        for (z, zone) in self.zones.iter().enumerate() {
            let who: Vec<String> = (0..self.fighters.len())
                .filter(|&k| self.fighters[k].zone == z && self.fighters[k].present())
                .map(|k| {
                    let f = &self.fighters[k];
                    let shape = if f.party() {
                        format!("生命 {}/{}", f.hp.max(0), f.max_hp)
                    } else if f.hp * 2 <= f.max_hp {
                        "伤得重".to_string()
                    } else if f.hp < f.max_hp {
                        "受了伤".to_string()
                    } else {
                        "没受伤".to_string()
                    };
                    let states = f.states();
                    let extra = if states.is_empty() {
                        String::new()
                    } else {
                        format!("，{}", states.join("、"))
                    };
                    let side = if k == i {
                        "你"
                    } else if f.party() {
                        "同伴"
                    } else {
                        "对手"
                    };
                    format!("{}={}（{side}，{shape}{extra}）", self.tag(k), f.name)
                })
                .collect();
            lines.push(format!(
                "{}. {zone}：{}",
                z + 1,
                if who.is_empty() {
                    "没有人".to_string()
                } else {
                    who.join("；")
                }
            ));
        }
        lines
    }

    pub fn view(&self) -> FightView {
        FightView {
            title: self.title.clone(),
            round: self.round,
            zones: self.zones.clone(),
            fighters: self
                .fighters
                .iter()
                .map(|f| FighterView {
                    name: f.name.clone(),
                    seat: f.seat.map(|s| s as u32 + 1),
                    hp: f.hp.max(0),
                    max_hp: f.max_hp,
                    ac: f.ac,
                    zone: f.zone as u32,
                    state: f.states(),
                })
                .collect(),
            turn: self.up().map(|i| i as u32),
            outcome: self.outcome.map(|o| {
                match o {
                    Outcome::Won => "won",
                    Outcome::Lost => "lost",
                    Outcome::Drawn => "drawn",
                }
                .to_string()
            }),
        }
    }
}

fn blank() -> Fighter {
    Fighter {
        name: String::new(),
        seat: None,
        hp: 1,
        max_hp: 1,
        ac: 10,
        zone: 0,
        attack: Attack {
            name: String::new(),
            bonus: 0,
            dice: (1, 1),
            plus: 0,
            ranged: false,
        },
        dex: 0,
        initiative: 0,
        dodging: false,
        disengaged: false,
        prone: false,
        aided: false,
        reacted: false,
        saves: (0, 0),
        stable: false,
        dead: false,
        fled: false,
        side: 0,
        creature: None,
        humanoid: false,
        level: 1,
        slots: Vec::new(),
        spells: Vec::new(),
        save_mods: (0, 0, 0),
        focus: None,
        blessed: false,
        held: None,
        marked: false,
        sneak: 0,
        sneaked: false,
        attacks: 1,
        second_wind: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::EncounterFoe;

    fn encounter(foes: &[(&str, u32)]) -> StorylineEncounter {
        StorylineEncounter {
            id: "ambush".into(),
            title: "栈桥伏击".into(),
            zones: vec!["栈桥".into(), "货堆".into(), "船舱".into()],
            party: vec![],
            party_zone: 0,
            foes: foes
                .iter()
                .map(|(creature, zone)| EncounterFoe {
                    creature: (*creature).into(),
                    name: None,
                    zone: *zone,
                })
                .collect(),
        }
    }

    fn member(seat: usize, hp: i32, ac: u32, bonus: i32, dice: (u32, u32), ranged: bool) -> Member {
        Member {
            name: format!("角色{}", seat + 1),
            seat,
            hp,
            max_hp: hp,
            ac,
            attack: Attack {
                name: if ranged { "短弓" } else { "长剑" }.into(),
                bonus,
                dice,
                plus: 3,
                ranged,
            },
            dex: 1,
            level: 1,
            slots: vec![],
            spells: vec![],
            save_mods: (1, 1, 1),
            sneak: 0,
            second_wind: false,
        }
    }

    /// 四个一级角色：两个近身、两个远程
    fn party() -> Vec<Member> {
        vec![
            member(0, 12, 16, 5, (1, 8), false),
            member(1, 9, 14, 5, (1, 6), false),
            member(2, 10, 16, 4, (1, 6), false),
            member(3, 7, 12, 5, (1, 6), true),
        ]
    }

    /// 角色的打法（测试里代替模型）：远程的原地打生命最少的对手；近身的同一块有对手就打，没有就朝最近的走一步再打
    fn simple(fight: &Fight, i: usize) -> Order {
        let foes = fight.enemies(i);
        let me = &fight.fighters[i];
        let weakest = |among: &[usize]| among.iter().copied().min_by_key(|&k| fight.fighters[k].hp);
        if me.attack.ranged {
            return Order {
                move_to: None,
                deed: weakest(&foes).map_or(Deed::Pass, Deed::Attack),
            };
        }
        let close: Vec<usize> = foes
            .iter()
            .copied()
            .filter(|&k| fight.fighters[k].zone == me.zone)
            .collect();
        if let Some(t) = weakest(&close) {
            return Order {
                move_to: None,
                deed: Deed::Attack(t),
            };
        }
        let goal = foes
            .iter()
            .map(|&k| fight.fighters[k].zone)
            .min_by_key(|z| z.abs_diff(me.zone));
        let Some(goal) = goal else {
            return Order {
                move_to: None,
                deed: Deed::Pass,
            };
        };
        let to = if goal > me.zone {
            me.zone + 1
        } else {
            me.zone - 1
        };
        let there: Vec<usize> = foes
            .iter()
            .copied()
            .filter(|&k| fight.fighters[k].zone == to)
            .collect();
        Order {
            move_to: Some(to),
            deed: weakest(&there).map_or(Deed::Dodge, Deed::Attack),
        }
    }

    fn play(enc: &StorylineEncounter, seed: u32) -> Fight {
        play_with(enc, seed, party())
    }

    /// 会法术的人的打法：有同伴倒下就治，不然有法术位就放最厉害的，再不然放戏法
    fn caster(fight: &Fight, i: usize) -> Option<Order> {
        let me = &fight.fighters[i];
        let has_slot = |s: &Spell| s.level == 0 || me.slots.get(s.level as usize - 1) > Some(&0);
        let cast = |s: &Spell, target: usize| Order {
            move_to: None,
            deed: Deed::Cast {
                spell: s.id.clone(),
                slot: s.level,
                target: Some(target),
            },
        };
        let down = (0..fight.fighters.len()).find(|&k| {
            !fight.hostile(i, k) && fight.fighters[k].present() && fight.fighters[k].hp <= 0
        });
        let heal = me
            .spells
            .iter()
            .find(|s| s.kind == SpellKind::Heal && has_slot(s));
        if let (Some(k), Some(s)) = (down, heal) {
            return Some(cast(s, k));
        }
        let weakest = fight
            .enemies(i)
            .into_iter()
            .min_by_key(|&k| fight.fighters[k].hp)?;
        me.spells
            .iter()
            .filter(|s| s.kind != SpellKind::Heal && has_slot(s))
            .max_by_key(|s| s.level)
            .map(|s| cast(s, weakest))
    }

    fn play_with(enc: &StorylineEncounter, seed: u32, members: Vec<Member>) -> Fight {
        let mut dice = Dice::new(seed);
        let mut rolls = Vec::new();
        let mut fight = Fight::open(enc, members, &mut dice);
        while let Some(i) = fight.up() {
            let (_, can) = fight.start_turn(i, &mut dice, &mut rolls, 0.0);
            if can {
                if fight.fighters[i].party() {
                    let order = caster(&fight, i).unwrap_or_else(|| simple(&fight, i));
                    fight
                        .party_turn(i, &order, &mut dice, &mut rolls, 0.0)
                        .unwrap();
                } else {
                    fight.foe_turn(i, &mut dice, &mut rolls, 0.0);
                }
            }
            fight.end_turn();
        }
        fight
    }

    fn spell(id: &str, level: u32, kind: SpellKind, reach: Reach, dice: (u32, u32)) -> Spell {
        Spell {
            id: id.into(),
            name: id.into(),
            level,
            kind,
            reach,
            dice,
            plus: 0,
            save: None,
            half: false,
            rays: 0,
            upcast_dice: 0,
            upcast_rays: 0,
            concentration: false,
            marks: false,
            rest_only: false,
            bonus: 5,
            dc: 13,
        }
    }

    /// 法术的规矩：戏法不耗环位，别的耗一个、用完了退回去；必中的不掷攻击骰；摸得着才治得了，昏迷的人治了就醒；
    /// 专注的法术挨打后体质豁免没过就断，罩着的效果一起没；定身只对人形生物，被定住的轮到时动不了；
    /// 偷袭一回合只加一次；回气一次，用过就没有。
    #[test]
    fn spells_spend_slots_hold_concentration_and_class_features_fire_once() {
        let enc = encounter(&[("thug", 1), ("wolf", 1)]);
        let mut dice = Dice::new(11);
        let mut rolls = Vec::new();
        let mut members = party();
        let mut missile = spell("missile", 1, SpellKind::Auto, Reach::Ranged, (1, 4));
        missile.plus = 1;
        missile.rays = 3;
        let mut cure = spell("cure", 1, SpellKind::Heal, Reach::Touch, (1, 8));
        cure.plus = 3;
        let mut bless = spell("bless", 1, SpellKind::Bless, Reach::Ranged, (1, 4));
        bless.concentration = true;
        let mut hold = spell("hold", 2, SpellKind::Hold, Reach::Ranged, (0, 0));
        hold.concentration = true;
        members[2].slots = vec![3, 2];
        members[2].spells = vec![
            spell("bolt", 0, SpellKind::Attack, Reach::Ranged, (1, 10)),
            missile,
            cure,
            bless,
            hold,
        ];
        members[1].sneak = 1;
        members[0].second_wind = true;
        let mut fight = Fight::open(&enc, members, &mut dice);
        let (thug, wolf) = (4, 5);
        fight.fighters[thug].hp = 200;
        fight.fighters[thug].max_hp = 200;
        let cast = |spell: &str, slot: u32, target: Option<usize>| Order {
            move_to: None,
            deed: Deed::Cast {
                spell: spell.into(),
                slot,
                target,
            },
        };
        // 必中：三发各 d4+1，不掷攻击骰，耗一个一环位
        fight
            .party_turn(
                2,
                &cast("missile", 1, Some(thug)),
                &mut dice,
                &mut rolls,
                0.0,
            )
            .unwrap();
        let lost = 200 - fight.fighters[thug].hp;
        assert!((6..=15).contains(&lost), "{lost}");
        assert!(rolls.is_empty());
        assert_eq!(fight.fighters[2].slots, vec![2, 2]);
        // 戏法不耗环位；没准备的法术、用完的环位退回去，什么都不动
        fight
            .party_turn(2, &cast("bolt", 0, Some(thug)), &mut dice, &mut rolls, 0.0)
            .unwrap();
        assert_eq!((rolls.len(), &fight.fighters[2].slots), (1, &vec![2, 2]));
        assert!(fight
            .party_turn(
                2,
                &cast("fireball", 3, Some(thug)),
                &mut dice,
                &mut rolls,
                0.0
            )
            .is_err());
        fight.fighters[2].slots[0] = 0;
        let before = fight.fighters[thug].hp;
        assert!(fight
            .party_turn(
                2,
                &cast("missile", 1, Some(thug)),
                &mut dice,
                &mut rolls,
                0.0
            )
            .is_err());
        // 没有一环位了：用二环位施，多一发
        fight
            .party_turn(
                2,
                &cast("missile", 2, Some(thug)),
                &mut dice,
                &mut rolls,
                0.0,
            )
            .unwrap();
        assert_eq!(fight.fighters[2].slots, vec![0, 1]);
        assert!(fight.fighters[thug].hp < before);
        // 治疗要碰到：隔一块的治不了、不耗环位；同一块昏迷的同伴治了就醒
        fight.fighters[2].slots = vec![3, 2];
        fight.fighters[0].zone = 1;
        fight.fighters[0].hp = 0;
        fight.fighters[0].prone = true;
        assert!(fight
            .party_turn(2, &cast("cure", 1, Some(0)), &mut dice, &mut rolls, 0.0)
            .is_err());
        assert!(fight
            .party_turn(2, &cast("cure", 1, Some(thug)), &mut dice, &mut rolls, 0.0)
            .is_err());
        assert_eq!(fight.fighters[2].slots, vec![3, 2]);
        fight.fighters[0].zone = 0;
        let lines = fight
            .party_turn(2, &cast("cure", 1, Some(0)), &mut dice, &mut rolls, 0.0)
            .unwrap();
        assert!(lines[0].contains("醒了过来"), "{lines:?}");
        assert!((4..=11).contains(&fight.fighters[0].hp));
        // 祝福是专注：施法者挨了打、体质豁免没过，祝福就断
        fight
            .party_turn(2, &cast("bless", 1, None), &mut dice, &mut rolls, 0.0)
            .unwrap();
        assert!(fight.fighters[0].blessed && fight.fighters[2].blessed);
        fight.fighters[2].save_mods.1 = -30;
        let line = fight.hurt(2, 2, false, &mut dice, &mut rolls, 0.0);
        assert!(line.contains("断了"), "{line}");
        assert!(fight.fighters[2].focus.is_none());
        assert!(fight.fighters.iter().all(|f| !f.blessed));
        // 定身：狼不是人形；打手感知豁免没过就动不了，施法者专注一断就松开
        assert!(fight
            .party_turn(2, &cast("hold", 2, Some(wolf)), &mut dice, &mut rolls, 0.0)
            .is_err());
        fight.fighters[thug].save_mods.2 = -30;
        fight
            .party_turn(2, &cast("hold", 2, Some(thug)), &mut dice, &mut rolls, 0.0)
            .unwrap();
        assert_eq!(fight.fighters[thug].held, Some((2, 13)));
        let (_, can) = fight.start_turn(thug, &mut dice, &mut rolls, 0.0);
        assert!(!can);
        fight.hurt(2, 2, false, &mut dice, &mut rolls, 0.0);
        assert!(fight.fighters[thug].held.is_none());
        // 偷袭：目标身边有同伴，一回合里打中的头一下多掷 d6，第二下没有
        fight.fighters[0].zone = 1;
        fight.fighters[1].zone = 1;
        fight.fighters[thug].ac = 1;
        let mut hits = Vec::new();
        while hits.len() < 2 {
            let line = fight.strike(1, thug, &mut dice, &mut rolls, 0.0, "");
            if !line.contains("没打中") {
                hits.push(line);
            }
        }
        assert!(
            hits[0].contains("偷袭") && !hits[1].contains("偷袭"),
            "{hits:?}"
        );
        // 回气：回 1d10 + 等级，只有一次
        fight.fighters[0].hp = 1;
        let wind = Order {
            move_to: None,
            deed: Deed::SecondWind,
        };
        fight
            .party_turn(0, &wind, &mut dice, &mut rolls, 0.0)
            .unwrap();
        assert!((3..=12).contains(&fight.fighters[0].hp));
        assert!(fight
            .party_turn(0, &wind, &mut dice, &mut rolls, 0.0)
            .is_err());
        // 两个人动手：各算一边，站在同一块
        let duel = Fight::duel(party().remove(0), party().remove(1), &mut dice);
        assert!(duel.hostile(0, 1) && duel.fighters[0].zone == duel.fighters[1].zone);
        assert_eq!(duel.enemies(0), vec![1]);
    }

    /// 规则数据每一项都写着出处；剧本里不合规的遭遇战开不了。
    #[test]
    fn combat_data_cites_the_srd_and_bad_encounters_are_refused() {
        let g = gear();
        assert_eq!(g.creatures.len(), 10);
        assert!(g.creatures.iter().all(|c| c.srd.starts_with("Monsters > ")));
        // 打手有多重攻击：一回合两下
        let twice: Vec<&str> = g
            .creatures
            .iter()
            .filter(|c| c.attacks == 2)
            .map(|c| c.id.as_str())
            .collect();
        assert_eq!(twice, ["thug"]);
        assert!(g.weapons.iter().all(|w| !w.srd.is_empty() && w.dice.1 >= 1));
        assert!(g
            .armor
            .iter()
            .all(|a| a.srd.starts_with("Equipment > Armor")));
        assert!(check(&encounter(&[("bandit", 1)]), 4).is_ok());
        assert!(check(&encounter(&[("dragon", 1)]), 4).is_err());
        assert!(check(&encounter(&[("bandit", 7)]), 4).is_err());
        assert!(check(&encounter(&[]), 4).is_err());
        let mut one_zone = encounter(&[("bandit", 0)]);
        one_zone.zones.truncate(1);
        assert!(check(&one_zone, 4).is_err());
    }

    /// 一回合里的规矩：近身够不着的退回去、不算用掉；走开挨借机攻击，撤步了就不挨；闪避时打他的有劣势；
    /// 帮一把让同伴下一次攻击有优势，用掉就没了；身边有对手脱不了身；倒下的角色掷死亡豁免直到稳住或死去；
    /// 同一个种子打出来一模一样，存档读出来接着打也一样。
    #[test]
    fn a_turn_follows_reach_reactions_edges_and_death_saves() {
        let enc = encounter(&[("thug", 1)]);
        let mut dice = Dice::new(5);
        let mut rolls = Vec::new();
        let mut fight = Fight::open(&enc, party(), &mut dice);
        let foe = 4;
        assert_eq!(
            (fight.tag(0), fight.tag(foe)),
            ("1".to_string(), "f1".to_string())
        );
        assert_eq!(fight.find("f1"), Some(foe));
        assert_eq!(fight.zone("货堆"), Some(1));
        // 近身的够不着隔一块的对手；远程的够得着
        let reach = Order {
            move_to: None,
            deed: Deed::Attack(foe),
        };
        assert!(fight
            .party_turn(0, &reach, &mut dice, &mut rolls, 0.0)
            .is_err());
        assert!(rolls.is_empty(), "退回去的不掷骰");
        assert!(fight
            .party_turn(3, &reach, &mut dice, &mut rolls, 0.0)
            .is_ok());
        // 走到对手那块再打
        let close = Order {
            move_to: Some(1),
            deed: Deed::Attack(foe),
        };
        fight
            .party_turn(0, &close, &mut dice, &mut rolls, 0.0)
            .unwrap();
        assert_eq!(fight.fighters[0].zone, 1);
        // 身边有对手：脱不了身；直接走开挨一下借机攻击（对手的反应用掉了，第二个人走开就不挨）
        let flee = Order {
            move_to: None,
            deed: Deed::Flee,
        };
        assert!(fight
            .party_turn(0, &flee, &mut dice, &mut rolls, 0.0)
            .is_err());
        let back = Order {
            move_to: Some(0),
            deed: Deed::Dodge,
        };
        let before = rolls.len();
        let lines = fight
            .party_turn(0, &back, &mut dice, &mut rolls, 0.0)
            .unwrap();
        assert!(lines[0].contains("借机"), "{lines:?}");
        assert_eq!(rolls.len(), before + 1);
        assert!(fight.fighters[foe].reacted && fight.fighters[0].dodging);
        // 撤步再走：不挨打
        fight.fighters[1].zone = 1;
        fight.fighters[foe].reacted = false;
        let slip = Order {
            move_to: Some(0),
            deed: Deed::Disengage,
        };
        let before = rolls.len();
        fight
            .party_turn(1, &slip, &mut dice, &mut rolls, 0.0)
            .unwrap();
        assert_eq!(rolls.len(), before, "撤步了不挨借机攻击");
        // 闪避的人挨打有劣势；被帮了一把的人攻击有优势，用掉就没了
        fight.fighters[0].zone = 1;
        fight.fighters[0].hp = 12;
        fight.strike(foe, 0, &mut dice, &mut rolls, 0.0, "");
        assert_eq!(rolls.last().unwrap().edge, Some(Edge::Disadvantage));
        fight.fighters[2].zone = 1;
        let help = Order {
            move_to: None,
            deed: Deed::Help(0),
        };
        fight
            .party_turn(2, &help, &mut dice, &mut rolls, 0.0)
            .unwrap();
        fight.fighters[foe].hp = 500;
        fight
            .party_turn(0, &reach, &mut dice, &mut rolls, 0.0)
            .unwrap();
        assert_eq!(rolls.last().unwrap().edge, Some(Edge::Advantage));
        fight
            .party_turn(0, &reach, &mut dice, &mut rolls, 0.0)
            .unwrap();
        assert_eq!(rolls.last().unwrap().edge, None);

        // 倒下的角色：轮到时掷死亡豁免，直到稳住、死去或醒过来；醒不过来就不能动
        fight.fighters[1].hp = 0;
        fight.fighters[1].prone = true;
        let mut ended = false;
        for _ in 0..12 {
            let (lines, can) = fight.start_turn(1, &mut dice, &mut rolls, 0.0);
            let f = &fight.fighters[1];
            if f.dead || f.stable || f.hp > 0 {
                assert!(f.saves.0 >= 3 || f.saves.1 >= 3 || f.hp == 1, "{f:?}");
                assert_eq!(can, f.hp > 0);
                ended = true;
                break;
            }
            assert!(!can && lines[0].contains("死亡豁免"));
        }
        assert!(ended, "死亡豁免总会有个结果");

        // 同一个种子打出来一模一样；打到一半存下来读回去接着打也一样
        let small = encounter(&[("bandit", 1), ("bandit", 2)]);
        assert_eq!(play(&small, 42), play(&small, 42));
        let mut dice = Dice::new(42);
        let mut fight = Fight::open(&small, party(), &mut dice);
        let saved = (
            serde_json::to_string(&fight).unwrap(),
            serde_json::to_string(&dice).unwrap(),
        );
        let finish = |mut fight: Fight, mut dice: Dice| {
            let mut rolls = Vec::new();
            while let Some(i) = fight.up() {
                let (_, can) = fight.start_turn(i, &mut dice, &mut rolls, 0.0);
                if can && fight.fighters[i].party() {
                    let order = simple(&fight, i);
                    fight
                        .party_turn(i, &order, &mut dice, &mut rolls, 0.0)
                        .unwrap();
                } else if can {
                    fight.foe_turn(i, &mut dice, &mut rolls, 0.0);
                }
                fight.end_turn();
            }
            fight
        };
        let straight = finish(fight.clone(), dice.clone());
        fight = serde_json::from_str(&saved.0).unwrap();
        dice = serde_json::from_str(&saved.1).unwrap();
        assert_eq!(finish(fight, dice), straight);
    }

    /// 打一千场：四个一级角色对两个强盗几乎都赢，对四个强盗加一个打手输赢参半偏难，对八个打手几乎都输。
    /// 难度随对手多少变，说明伤害、命中和先攻都在起作用；没有一场打不完。
    #[test]
    fn a_thousand_fights_land_where_the_encounter_size_predicts() {
        let rate = |enc: &StorylineEncounter| {
            let wins = (1..=1000u32)
                .filter(|&seed| {
                    let fight = play(enc, seed);
                    assert!(fight.outcome.is_some() && fight.round <= ROUNDS_MAX + 1);
                    fight.outcome == Some(Outcome::Won)
                })
                .count();
            wins as f64 / 1000.0
        };
        let easy = rate(&encounter(&[("bandit", 1), ("bandit", 2)]));
        let hard = rate(&encounter(&[
            ("bandit", 1),
            ("bandit", 1),
            ("bandit", 2),
            ("bandit", 2),
            ("thug", 2),
        ]));
        let deadly = rate(&encounter(&[("thug", 1); 8]));
        assert!(easy > 0.95, "两个强盗：{easy}");
        assert!((0.15..=0.85).contains(&hard), "四个强盗加一个打手：{hard}");
        assert!(deadly < 0.05, "八个打手：{deadly}");
        assert!(easy > hard && hard > deadly);
        // 同一场难打的架，后排两个人换成牧师和法师（各两个一环位）：赢得更多，可也不是稳赢
        let casters = || {
            let mut members = party();
            let mut heal = spell("word", 1, SpellKind::Heal, Reach::Ranged, (1, 4));
            heal.plus = 3;
            let mut flame = spell("flame", 0, SpellKind::Save, Reach::Ranged, (1, 8));
            flame.save = Some("dex".into());
            let mut missile = spell("missile", 1, SpellKind::Auto, Reach::Ranged, (1, 4));
            missile.plus = 1;
            missile.rays = 3;
            members[2].slots = vec![2];
            members[2].spells = vec![heal, flame];
            members[3].slots = vec![2];
            members[3].spells = vec![
                missile,
                spell("bolt", 0, SpellKind::Attack, Reach::Ranged, (1, 10)),
            ];
            members
        };
        let enc = encounter(&[
            ("bandit", 1),
            ("bandit", 1),
            ("bandit", 2),
            ("bandit", 2),
            ("thug", 2),
        ]);
        let wins = (1..=1000u32)
            .filter(|&seed| play_with(&enc, seed, casters()).outcome == Some(Outcome::Won))
            .count();
        let magic = wins as f64 / 1000.0;
        assert!(magic > hard && magic < 0.98, "带法术 {magic}，不带 {hard}");
    }
}
