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
    pub attack: Attack,
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
            .map(|m| Fighter {
                name: m.name,
                seat: Some(m.seat),
                hp: m.hp,
                max_hp: m.max_hp,
                ac: m.ac,
                zone: enc.party_zone as usize,
                attack: m.attack,
                dex: m.dex,
                ..blank()
            })
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
        self.fighters[a].party() != self.fighters[b].party()
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
        if f.hp > 0 {
            f.prone = false;
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
        let up = (t.prone && !attack.ranged) || self.fighters[i].aided || t.hp <= 0;
        let down = t.dodging || (attack.ranged && crowded) || (t.prone && attack.ranged);
        let edge = match (up, down) {
            (true, false) => Some(Edge::Advantage),
            (false, true) => Some(Edge::Disadvantage),
            _ => None,
        };
        self.fighters[i].aided = false;
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
        let total = kept as i32 + attack.bonus;
        let ac = self.fighters[target].ac;
        let crit = kept == 20;
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
        let damage = (roll_dice(dice, (count, attack.dice.1)).iter().sum::<u32>() as i32
            + attack.plus)
            .max(1);
        let t = &mut self.fighters[target];
        let name = t.name.clone();
        let after = if t.hp <= 0 {
            // 已经昏迷的再挨打：算死亡豁免失败，重击算两次
            t.saves.1 = (t.saves.1 + if crit { 2 } else { 1 }).min(3);
            t.stable = false;
            if t.saves.1 >= 3 {
                t.dead = true;
                format!("{name} 没撑过去，死了")
            } else {
                format!("{name} 昏迷中又挨了一下（死亡豁免失败 {}/3）", t.saves.1)
            }
        } else {
            t.hp -= damage;
            if t.hp > 0 {
                format!("{name} 还剩 {} 点生命", t.hp)
            } else if t.party() {
                t.hp = 0;
                t.prone = true;
                t.saves = (0, 0);
                t.stable = false;
                format!("{name} 倒下昏迷了")
            } else {
                t.hp = 0;
                t.dead = true;
                format!("{name} 倒下了")
            }
        };
        format!(
            "{head}：{}，伤害 {damage}。{after}",
            if crit { "重击" } else { "命中" }
        )
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
            Deed::Dodge | Deed::Disengage | Deed::Pass => {}
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
        if let Some(t) = weakest(self, close) {
            lines.push(self.strike(i, t, dice, rolls, at, ""));
        }
        lines
    }

    /// 这一回合结束：看分没分出胜负，没有就轮到下一个；转完一圈是新的一轮。
    pub fn end_turn(&mut self) {
        let foes = self.fighters.iter().any(|f| !f.party() && f.active());
        let party = self.fighters.iter().any(|f| f.party() && f.active());
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
        let mut dice = Dice::new(seed);
        let mut rolls = Vec::new();
        let mut fight = Fight::open(enc, party(), &mut dice);
        while let Some(i) = fight.up() {
            let (_, can) = fight.start_turn(i, &mut dice, &mut rolls, 0.0);
            if can {
                if fight.fighters[i].party() {
                    let order = simple(&fight, i);
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

    /// 规则数据每一项都写着出处；剧本里不合规的遭遇战开不了。
    #[test]
    fn combat_data_cites_the_srd_and_bad_encounters_are_refused() {
        let g = gear();
        assert_eq!(g.creatures.len(), 10);
        assert!(g.creatures.iter().all(|c| c.srd.starts_with("Monsters > ")));
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
    }
}
