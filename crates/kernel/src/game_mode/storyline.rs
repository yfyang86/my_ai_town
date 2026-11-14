//! 故事模式：整本剧本是开局时带进来的数据——角色（公开的身份、只有本人知道的目标和秘密）、
//! 到点发生的节拍（对所有人宣布一件事，单独叮嘱几个人，可以再开一场轮流说话的戏）、要追踪的里程碑。
//! 这里只管三件事：到点把节拍说出来、戏里按次序一次叫醒一个人、记下里程碑达没达成。
//! 不替谁做决定：没有人被挪动，去不去、说什么、宣不宣布都由角色自己定，故事可以走偏。
//! 里程碑有三种算法：某个节拍发生了；几个人同时在某个地点；有人用 declare 宣布它发生了并写上经过。
//! 到了收场的时刻，把达成和没达成的里程碑说一遍，一局结束。
//! 和执剑人之夜一样：只叫醒该做事的人，别人的话写进每个人的经历；期限按游戏分钟，挂在 boundary 上。
use super::combat::{self, Deed, Fight, Order, Outcome};
use super::srd5::{self, Srd5};
use super::Rules;
use crate::error::{KernelError, Result};
use crate::{World, DAY};
use protocol::{
    ActionDraft, EventInfo, EventSpeech, FactKind, GameModeAction, GameModeContext, GameModeView,
    RulesetChoice, RulesetView, StorylineBeatState, StorylineBeatView, StorylineCheck,
    StorylineContext, StorylineLine, StorylineMilestoneView, StorylineScript, StorylineSeat,
    StorylineView, StorylineWhen,
};
use protocol::{
    CheckWay, DeathCause, Edge, EventActor, Killing, Ruling, RulingKind, RulingRequest,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const CAST: std::ops::RangeInclusive<usize> = 2..=12;
const BEATS: std::ops::RangeInclusive<usize> = 1..=24;
const MILESTONES: usize = 12;
const DAYS: std::ops::RangeInclusive<u32> = 1..=14;
const TITLE_CHARS: usize = 40;
const PREMISE_CHARS: usize = 2000;
const TEXT_CHARS: usize = 600;
const DETAIL_CHARS: usize = 300;
const SCENE_MINUTES: std::ops::RangeInclusive<u32> = 1..=120;
const SCENE_CHARS: std::ops::RangeInclusive<u32> = 20..=600;
/// 宣布里程碑时写的经过、交动作时写的理由最多多少字
const EVIDENCE_CHARS: usize = 200;
/// 最后一个节拍过了以后、离收场不到这么久（游戏分钟）时提醒一遍：还有只能由自己宣布的里程碑的人，各自被单独叫到
const LAST_CALL_MINUTES: f64 = 120.0;
/// 到了收场的钟点还有人握着没宣布的里程碑：再等这么久（游戏分钟，按正常速度走），正在路上的宣布来得及算数
const GRACE_MINUTES: f64 = 20.0;
/// 主持人裁定时能选的难度
const RULING_DCS: [u32; 5] = [5, 10, 15, 20, 25];
/// 一个人每个游戏小时最多请主持人裁定几件事
const ASKS_PER_HOUR: usize = 3;
/// 主持人这么久没裁出来就作罢（游戏分钟）
const ASK_WAIT_MINUTES: f64 = 30.0;
const NOTE_CHARS: usize = 80;
/// 临场裁定在骰子记录里的名字留这么长
const ASK_TITLE_CHARS: usize = 24;
const REASON_CHARS: usize = 120;
/// 有「几个人同时在某地」的里程碑没达成时，隔这么久看一次
const WATCH_MINUTES: f64 = 5.0;
/// 节拍之间世界按正常速度的几倍走；戏里照常速
const FREE_TEMPO: f64 = 3.0;
/// 给模型看的戏里的话最多留多少段
const RECENT_LINES: usize = 20;
const EPS: f64 = 1e-6;
/// 一个节拍上最多几项检定，一项检定最多几种试法
const BEAT_CHECKS: usize = 6;
const CHECK_WAYS: usize = 4;
const CHECK_EDGES: usize = 4;
/// 战斗里轮到的角色有多久出手（游戏分钟）；到点没交算闪避。和戏里说话的期限一个量级：
/// 真模型想一回合要十几秒，5 分钟时一个人连着四回合没赶上
const FIGHT_TURN_MINUTES: f64 = 15.0;

fn clip(text: &str, n: usize) -> (String, bool) {
    let text = text.trim();
    if text.chars().count() <= n {
        return (text.to_string(), false);
    }
    (text.chars().take(n).collect(), true)
}

fn reason_of(action: &GameModeAction) -> Option<String> {
    action
        .reason
        .as_deref()
        .map(|r| clip(r, REASON_CHARS).0)
        .filter(|r| !r.is_empty())
}

fn clock(at: f64) -> String {
    let t = crate::time_of_day(at).round() as i64;
    format!("{:02}:{:02}", t / 60 % 24, t % 60)
}

fn bad(text: impl Into<String>) -> KernelError {
    KernelError::action(text)
}

fn is_id(id: &str) -> bool {
    (1..=32).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

/// 一段字：不能空，不能超长
fn text(what: &str, value: &str, max: usize) -> Result<()> {
    let n = value.trim().chars().count();
    if n == 0 {
        return Err(bad(format!("{what}不能空着")));
    }
    if n > max {
        return Err(bad(format!("{what}最多 {max} 字，现在 {n} 字")));
    }
    Ok(())
}

fn seats_in(what: &str, seats: &[u32], cast: usize) -> Result<()> {
    let mut seen = BTreeSet::new();
    for &s in seats {
        if s == 0 || s as usize > cast {
            return Err(bad(format!("{what}里的座号 {s} 不在 1–{cast} 之间")));
        }
        if !seen.insert(s) {
            return Err(bad(format!("{what}里的座号 {s} 写了两次")));
        }
    }
    Ok(())
}

/// 剧本查一遍：数量、字数、先后、互相指的东西都对得上才开局。
pub(super) fn check(script: &StorylineScript) -> Result<()> {
    text("标题", &script.title, TITLE_CHARS)?;
    text("前情", &script.premise, PREMISE_CHARS)?;
    let cast = script.cast.len();
    if !CAST.contains(&cast) {
        return Err(bad(format!(
            "角色要 {}–{} 个，剧本里有 {cast} 个",
            CAST.start(),
            CAST.end()
        )));
    }
    for (i, r) in script.cast.iter().enumerate() {
        let who = format!("{} 号角色的", i + 1);
        text(&format!("{who}身份"), &r.role, TITLE_CHARS)?;
        text(&format!("{who}目标"), &r.goal, TEXT_CHARS)?;
        if let Some(s) = &r.secret {
            text(&format!("{who}秘密"), s, TEXT_CHARS)?;
        }
    }
    if !BEATS.contains(&script.beats.len()) {
        return Err(bad(format!(
            "节拍要 {}–{} 个，剧本里有 {} 个",
            BEATS.start(),
            BEATS.end(),
            script.beats.len()
        )));
    }
    let mut ids = BTreeSet::new();
    let mut checks = BTreeSet::new();
    let mut fights = BTreeSet::new();
    // 检定的优势劣势指着的里程碑：等里程碑都看过了再对
    let mut edge_marks: Vec<(String, String)> = Vec::new();
    let mut last = 0u32;
    for b in &script.beats {
        let what = format!("节拍「{}」", b.id);
        if !is_id(&b.id) || !ids.insert(b.id.as_str()) {
            return Err(bad(format!(
                "{what}的编号要用小写字母、数字、连字符，而且不能重复"
            )));
        }
        text(&format!("{what}的标题"), &b.title, TITLE_CHARS)?;
        text(&format!("{what}的宣布"), &b.announce, TEXT_CHARS)?;
        if let Some(brief) = &b.brief {
            text(&format!("{what}的叮嘱"), brief, TEXT_CHARS)?;
        }
        if let Some(place) = &b.place {
            text(&format!("{what}的地点"), place, TITLE_CHARS)?;
        }
        if !DAYS.contains(&b.day) || b.minute >= DAY as u32 {
            return Err(bad(format!(
                "{what}的时刻不对：第 {}–{} 天，当天第 0–1439 分钟",
                DAYS.start(),
                DAYS.end()
            )));
        }
        let at = (b.day - 1) * DAY as u32 + b.minute;
        if at < last {
            return Err(bad(format!("{what}排在了前一个节拍之前：节拍要按先后排")));
        }
        last = at;
        seats_in(&format!("{what}的 who"), &b.who, cast)?;
        if let Some(scene) = &b.scene {
            if scene.speakers.is_empty() {
                return Err(bad(format!("{what}的戏里没有人说话")));
            }
            seats_in(&format!("{what}的戏"), &scene.speakers, cast)?;
            if !SCENE_MINUTES.contains(&scene.minutes) || !SCENE_CHARS.contains(&scene.chars) {
                return Err(bad(format!(
                    "{what}的戏：每人 {}–{} 分钟、{}–{} 字",
                    SCENE_MINUTES.start(),
                    SCENE_MINUTES.end(),
                    SCENE_CHARS.start(),
                    SCENE_CHARS.end()
                )));
            }
        }
        for c in &b.checks {
            let what = format!("检定「{}」", c.id);
            if !is_id(&c.id) || !checks.insert(c.id.as_str()) {
                return Err(bad(format!(
                    "{what}的编号要用小写字母、数字、连字符，而且不能重复"
                )));
            }
            text(&format!("{what}的标题"), &c.title, TITLE_CHARS)?;
            text(&format!("{what}的说明"), &c.detail, DETAIL_CHARS)?;
            seats_in(&format!("{what}的 seats"), &c.seats, cast)?;
            if !srd5::DCS.contains(&c.dc) {
                return Err(bad(format!(
                    "{what}的难度要在 {}–{} 之间",
                    srd5::DCS.start(),
                    srd5::DCS.end()
                )));
            }
            if c.ways.is_empty() || c.ways.len() > CHECK_WAYS {
                return Err(bad(format!("{what}要有 1–{CHECK_WAYS} 种试法")));
            }
            let mut keys = BTreeSet::new();
            for way in &c.ways {
                srd5::check_way(way)?;
                if !keys.insert(srd5::way_key(way)) {
                    return Err(bad(format!("{what}的试法写重了")));
                }
            }
            if let Some(against) = &c.against {
                seats_in(&format!("{what}对抗的人"), &[against.seat], cast)?;
                if against.ways.is_empty() || against.ways.len() > CHECK_WAYS {
                    return Err(bad(format!("{what}对抗的一方要有 1–{CHECK_WAYS} 种办法")));
                }
                against.ways.iter().try_for_each(srd5::check_way)?;
                if c.seats.len() == 1 && c.seats[0] == against.seat {
                    return Err(bad(format!("{what}不能自己和自己对抗")));
                }
                if c.passive {
                    return Err(bad(format!("{what}不能既是被动检定又是对抗")));
                }
            }
            if c.passive && c.repeat {
                return Err(bad(format!("{what}是被动检定，没有再试一说")));
            }
            if c.edges.len() > CHECK_EDGES {
                return Err(bad(format!("{what}的优势劣势最多 {CHECK_EDGES} 条")));
            }
            for edge in &c.edges {
                text(
                    &format!("{what}的优势劣势的原因"),
                    &edge.reason,
                    TITLE_CHARS,
                )?;
                seats_in(&format!("{what}的优势劣势"), &edge.seats, cast)?;
                if let Some(place) = &edge.place {
                    text(&format!("{what}的优势劣势的地点"), place, TITLE_CHARS)?;
                }
                if let Some(milestone) = &edge.milestone {
                    edge_marks.push((c.id.clone(), milestone.clone()));
                }
            }
        }
        if let Some(enc) = &b.encounter {
            combat::check(enc, cast)?;
            if !is_id(&enc.id) || !fights.insert(enc.id.as_str()) {
                return Err(bad(format!(
                    "遭遇战「{}」的编号要用小写字母、数字、连字符，而且不能重复",
                    enc.id
                )));
            }
        }
        if b.checks.len() > BEAT_CHECKS {
            return Err(bad(format!("{what}上的检定最多 {BEAT_CHECKS} 项")));
        }
    }
    if !DAYS.contains(&script.ends_day) || script.ends_minute >= DAY as u32 {
        return Err(bad("收场的时刻不对"));
    }
    if (script.ends_day - 1) * DAY as u32 + script.ends_minute <= last {
        return Err(bad("收场要排在最后一个节拍之后"));
    }
    if let Some(e) = &script.epilogue {
        text("收场白", e, TEXT_CHARS)?;
    }
    if script.milestones.len() > MILESTONES {
        return Err(bad(format!("里程碑最多 {MILESTONES} 个")));
    }
    let mut marks = BTreeSet::new();
    for m in &script.milestones {
        let what = format!("里程碑「{}」", m.id);
        if !is_id(&m.id) || !marks.insert(m.id.as_str()) {
            return Err(bad(format!(
                "{what}的编号要用小写字母、数字、连字符，而且不能重复"
            )));
        }
        text(&format!("{what}的标题"), &m.title, TITLE_CHARS)?;
        text(&format!("{what}的说明"), &m.detail, DETAIL_CHARS)?;
        match &m.when {
            StorylineWhen::Beat { beat } => {
                if !ids.contains(beat.as_str()) {
                    return Err(bad(format!("{what}指的节拍「{beat}」不在剧本里")));
                }
            }
            StorylineWhen::Gather {
                place,
                seats,
                count,
            } => {
                text(&format!("{what}的地点"), place, TITLE_CHARS)?;
                seats_in(&format!("{what}的 seats"), seats, cast)?;
                let among = if seats.is_empty() { cast } else { seats.len() };
                if *count == 0 || *count as usize > among {
                    return Err(bad(format!("{what}要到场的人数要在 1–{among} 之间")));
                }
            }
            StorylineWhen::Declare { seats } => seats_in(&format!("{what}的 seats"), seats, cast)?,
            StorylineWhen::Check { check } => {
                if !checks.contains(check.as_str()) {
                    return Err(bad(format!("{what}指的检定「{check}」不在剧本里")));
                }
            }
            StorylineWhen::Encounter { encounter } => {
                if !fights.contains(encounter.as_str()) {
                    return Err(bad(format!("{what}指的遭遇战「{encounter}」不在剧本里")));
                }
            }
        }
    }
    for (check, milestone) in edge_marks {
        if !marks.contains(milestone.as_str()) {
            return Err(bad(format!(
                "检定「{check}」的优势劣势指的里程碑「{milestone}」不在剧本里"
            )));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Seat {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub gone: bool,
}

/// 正在进行的一场戏
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    /// 哪个节拍的戏
    pub beat: usize,
    /// 还没轮到的人，按次序
    pub queue: Vec<usize>,
    pub turn: Option<usize>,
    /// 这个人说话的期限
    pub until: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reached {
    pub at: f64,
    #[serde(default)]
    pub by: Option<usize>,
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Storyline {
    pub seed: u32,
    pub script: StorylineScript,
    /// 剧本的第 1 天是世界的第几天（从 0 数）
    pub first_day: u32,
    pub seats: Vec<Seat>,
    /// 下一个要发生的节拍
    #[serde(default)]
    pub next: usize,
    #[serde(default)]
    pub scene: Option<Scene>,
    /// 剧本里写的地点 → 地图上的地点编号；找不到的不在里面
    #[serde(default)]
    pub places: BTreeMap<String, String>,
    #[serde(default)]
    pub lines: Vec<StorylineLine>,
    /// 达成了的里程碑
    #[serde(default)]
    pub reached: BTreeMap<String, Reached>,
    #[serde(default)]
    pub ended: bool,
    /// 收场前的提醒叫过了（见 last_call）
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub last_call: bool,
    /// 到点了、正在等最后几句宣布（见 GRACE_MINUTES）
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub closing: bool,
    /// 开着的规则套件（角色卡、骰子、检定的经过）；没开时存档里没有这一项
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules: Option<Srd5>,
}

impl Storyline {
    /// 按此刻在镇上的村民排座次（按编号，也就是入住的先后），人数要够剧本里的角色。
    /// 第一个节拍今天还来得及就从今天算第 1 天，否则从明天算。
    pub(super) fn open(
        world: &World,
        seed: u32,
        script: &StorylineScript,
        ruleset: Option<&RulesetChoice>,
    ) -> Result<Self> {
        check(script)?;
        let rules = match ruleset {
            Some(RulesetChoice::Srd5 { sheets }) => {
                Some(Srd5::open(sheets, script.cast.len(), seed)?)
            }
            None => None,
        };
        let mut ids: Vec<&String> = world
            .actors
            .iter()
            .filter(|(_, a)| !a.is_avatar() && a.present() && !a.needs.body.dead)
            .map(|(id, _)| id)
            .collect();
        ids.sort();
        if ids.len() < script.cast.len() {
            return Err(bad(format!(
                "「{}」要 {} 个角色，现在镇上只有 {} 个村民",
                script.title.trim(),
                script.cast.len(),
                ids.len()
            )));
        }
        ids.truncate(script.cast.len());
        let seats = ids
            .iter()
            .map(|id| Seat {
                id: (*id).clone(),
                name: world.actors[*id].name.clone(),
                gone: false,
            })
            .collect();
        let now = world.minutes();
        let today = (now / DAY).floor() as u32;
        let first = &script.beats[0];
        let in_time = first.day > 1 || crate::time_of_day(now) <= f64::from(first.minute);
        let mut wanted: BTreeSet<&str> = script
            .beats
            .iter()
            .filter_map(|b| b.place.as_deref())
            .collect();
        for m in &script.milestones {
            if let StorylineWhen::Gather { place, .. } = &m.when {
                wanted.insert(place);
            }
        }
        for check in script.beats.iter().flat_map(|b| &b.checks) {
            wanted.extend(check.edges.iter().filter_map(|e| e.place.as_deref()));
        }
        let places = wanted
            .into_iter()
            .filter_map(|p| {
                let key = p.trim();
                world
                    .topo
                    .locales
                    .values()
                    .find(|l| l.name.contains(key))
                    .map(|l| (p.to_string(), l.id.clone()))
            })
            .collect();
        Ok(Self {
            seed,
            script: script.clone(),
            first_day: if in_time { today } else { today + 1 },
            seats,
            next: 0,
            scene: None,
            places,
            lines: Vec::new(),
            reached: BTreeMap::new(),
            ended: false,
            last_call: false,
            closing: false,
            rules,
        })
    }

    // ---------- 钟点和地点 ----------

    fn at(&self, day: u32, minute: u32) -> f64 {
        f64::from(self.first_day + day - 1) * DAY + f64::from(minute)
    }
    fn beat_at(&self, i: usize) -> f64 {
        let b = &self.script.beats[i];
        self.at(b.day, b.minute)
    }
    fn end_at(&self) -> f64 {
        self.at(self.script.ends_day, self.script.ends_minute)
    }
    /// 地图上有这个地点就用它的名字，没有就照剧本里写的
    fn place_name(&self, world: &World, place: &str) -> String {
        self.places
            .get(place)
            .map(|l| world.topo.locale_name(l))
            .unwrap_or_else(|| place.trim().to_string())
    }
    /// 「第 2 天 17:00」；剧本只有一天时不写第几天
    fn when(&self, day: u32, minute: u32) -> String {
        let hm = format!("{:02}:{:02}", minute / 60, minute % 60);
        if self.script.ends_day > 1 {
            format!("第 {day} 天 {hm}")
        } else {
            hm
        }
    }
    fn tag(&self) -> String {
        format!("【{}】", self.script.title.trim())
    }

    // ---------- 座位 ----------

    fn label(&self, i: usize) -> String {
        format!("{} 号 {}", i + 1, self.seats[i].name)
    }
    fn seat_of(&self, actor: &str) -> Option<usize> {
        self.seats.iter().position(|s| s.id == actor)
    }
    fn everyone(&self) -> Vec<usize> {
        (0..self.seats.len()).collect()
    }
    /// 剧本里的座号表（空着是所有人）换成下标
    fn among(&self, seats: &[u32]) -> Vec<usize> {
        if seats.is_empty() {
            self.everyone()
        } else {
            seats.iter().map(|&s| s as usize - 1).collect()
        }
    }
    fn here(&self, world: &World, i: usize) -> bool {
        !self.seats[i].gone
            && world
                .actor(&self.seats[i].id)
                .is_some_and(|a| a.present() && !a.needs.body.dead)
    }

    // ---------- 告诉谁 ----------

    fn object(&self) -> Vec<String> {
        vec![format!("storyline:{}", self.seed)]
    }

    /// 写进这些人的经历，不叫醒。
    fn tell(&self, world: &mut World, who: &[usize], text: &str) {
        let now = world.minutes();
        for &i in who {
            if !self.here(world, i) {
                continue;
            }
            let id = self.seats[i].id.clone();
            world.actor_mut(&id).journal.fact(
                now,
                FactKind::Observation,
                text.to_string(),
                self.object(),
                None,
                None,
            );
        }
    }

    /// 叫醒一个人：正在私聊的先散场，免得只轮到交谈的回合、交不了玩法动作。
    fn call(&self, world: &mut World, i: usize, text: &str) {
        if !self.here(world, i) {
            return;
        }
        let id = self.seats[i].id.clone();
        if world.active_chat(&id).is_some() {
            world.interrupt_actions(&id, "故事里轮到你了");
        }
        world.notify(&id, text.to_string());
    }

    /// 短休：不在打、戏也散了的时候歇一阵，花一颗生命骰回点生命。
    fn short_rest(&mut self, world: &mut World, i: usize) -> Result<String> {
        if self.fighting() {
            return Err(bad("正打着，歇不了"));
        }
        if self.scene.is_some() {
            return Err(bad("戏还没散，等这场戏完了再歇"));
        }
        let now = world.minutes();
        let Some(rules) = &mut self.rules else {
            return Err(bad("这个故事没有开规则"));
        };
        let roll = rules.short_rest(i, now)?;
        let outcome = format!(
            "掷生命骰 {} 得 {}，回了 {} 点生命，现在 {}/{}，生命骰还剩 {} 颗",
            roll.formula,
            roll.dice[0],
            roll.total,
            rules.hp[i],
            rules.hit_points(i),
            rules.hit_dice_left(i)
        );
        let line = format!("{} 歇了一阵（短休）：{outcome}。", self.label(i));
        self.say(world, &line);
        Ok(outcome)
    }

    // ---------- 临场裁定（开着规则套件才有） ----------

    /// 角色想做一件剧本里没写成检定的事：记下来等主持人裁定。一人一次只能有一件在等，每小时有上限。
    fn improvise(
        &mut self,
        world: &mut World,
        i: usize,
        action: &GameModeAction,
    ) -> Result<String> {
        if self.fighting() {
            return Err(bad("正打着，先把这场架打完"));
        }
        if self.scene.is_some() {
            return Err(bad("戏还没散，等这场戏完了再说"));
        }
        let text = action
            .text
            .as_deref()
            .map(|t| clip(t, EVIDENCE_CHARS).0)
            .filter(|t| !t.is_empty())
            .ok_or_else(|| bad("想做什么：text 里写清楚你要做的事和怎么做"))?;
        let now = world.minutes();
        let Some(rules) = &mut self.rules else {
            return Err(bad("这个故事没有开规则"));
        };
        if rules.asks.iter().any(|a| a.seat == i) {
            return Err(bad("上一件事主持人还没裁定，等结果出来再说"));
        }
        rules.asked.retain(|(_, at)| now - at < 60.0);
        if rules.asked.iter().filter(|(seat, _)| *seat == i).count() >= ASKS_PER_HOUR {
            return Err(bad(format!(
                "这一个小时里你已经请主持人裁定了 {ASKS_PER_HOUR} 件事，过一阵再说"
            )));
        }
        rules.ask_seq += 1;
        rules.asked.push((i, now));
        rules.asks.push(srd5::Ask {
            id: format!("ask-{}", rules.ask_seq),
            seat: i,
            text,
            at: now,
        });
        Ok("交给主持人裁定了：要不要掷骰、掷什么、多难，结果出来会告诉你".into())
    }

    /// 主持人迟迟没裁出来的事作罢，告诉本人
    fn expire_asks(&mut self, world: &mut World) {
        let now = world.minutes();
        let Some(rules) = &mut self.rules else {
            return;
        };
        let (late, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut rules.asks)
            .into_iter()
            .partition(|a| now - a.at >= ASK_WAIT_MINUTES);
        rules.asks = kept;
        for ask in late {
            let text = format!(
                "{}主持人没有回应你想做的事（{}），这件事作罢，可以换个办法。",
                self.tag(),
                ask.text
            );
            self.call(world, ask.seat, &text);
        }
    }

    /// 主持人交回的裁定：只认单子里的选项。掷骰和成败都在这里。
    fn settle_ask(
        &mut self,
        world: &mut World,
        id: &str,
        ruling: Option<&Ruling>,
    ) -> Result<String> {
        let now = world.minutes();
        let Some(rules) = &mut self.rules else {
            return Err(bad("这个故事没有开规则"));
        };
        let Some(k) = rules.asks.iter().position(|a| a.id == id) else {
            return Err(bad("没有这件等裁定的事（也许已经作罢了）"));
        };
        let ask = rules.asks.remove(k);
        let seat = ask.seat;
        if !self.here(world, seat) {
            return Err(bad("这个人已经不在了"));
        }
        let void = |this: &Self, world: &mut World, why: String| {
            let text = format!(
                "{}主持人对你想做的事（{}）没有给出合规矩的裁定，这件事作罢，可以换个办法。",
                this.tag(),
                ask.text
            );
            this.call(world, seat, &text);
            Err(bad(why))
        };
        let Some(ruling) = ruling else {
            return void(self, world, "主持人没有裁出来".into());
        };
        let note = ruling
            .note
            .as_deref()
            .map(|n| clip(n, NOTE_CHARS).0)
            .filter(|n| !n.is_empty())
            .map(|n| format!("（{n}）"))
            .unwrap_or_default();
        // 这件事做成了没有：做成了的顺带提醒一句能宣布的里程碑
        let mut done = false;
        let outcome = match ruling.kind {
            RulingKind::Auto => {
                done = true;
                format!("不用掷骰，做到了{note}")
            }
            RulingKind::Impossible => format!("做不到{note}"),
            RulingKind::Roll => {
                let way = CheckWay {
                    ability: ruling.ability.clone().unwrap_or_default(),
                    skill: ruling.skill.clone(),
                    save: false,
                };
                if let Err(e) = srd5::check_way(&way) {
                    return void(self, world, format!("裁定不合规矩：{e}"));
                }
                let Some(dc) = ruling.dc.filter(|dc| RULING_DCS.contains(dc)) else {
                    return void(
                        self,
                        world,
                        "裁定不合规矩：难度只能是 5、10、15、20、25".into(),
                    );
                };
                let Some(rules) = &mut self.rules else {
                    return Err(bad("这个故事没有开规则"));
                };
                // 骰子记录里只留开头一截：想做的事可以写得很长
                let (short, cut) = clip(&ask.text, ASK_TITLE_CHARS);
                let what = format!("临场：{short}{}", if cut { "…" } else { "" });
                let roll = rules.improvise(seat, &what, &way, dc, ruling.edge, now);
                done = roll.success == Some(true);
                let thrown = match ruling.edge {
                    Some(Edge::Advantage) => {
                        format!(
                            "有优势，掷两次 d20 得 {} 和 {}，取高的",
                            roll.dice[0], roll.dice[1]
                        )
                    }
                    Some(Edge::Disadvantage) => {
                        format!(
                            "有劣势，掷两次 d20 得 {} 和 {}，取低的",
                            roll.dice[0], roll.dice[1]
                        )
                    }
                    None => format!("掷 d20 得 {}", roll.dice[0]),
                };
                format!(
                    "用{}，难度 {dc}{note}，{thrown}，{:+} 共 {}：{}",
                    roll.way,
                    roll.modifier,
                    roll.total,
                    if roll.success == Some(true) {
                        "成功"
                    } else {
                        "失败"
                    }
                )
            }
        };
        let line = format!(
            "{}临场：{} 想做一件事——{}。主持人裁定：{outcome}",
            self.tag(),
            self.label(seat),
            ask.text
        );
        self.tell(world, &self.everyone(), &line);
        world.log(EventInfo::default(), line.clone());
        self.call(world, seat, &line);
        if done {
            if let Some(hint) = self.declare_hint(seat) {
                self.tell(world, &[seat], &hint);
            }
        }
        Ok(outcome)
    }

    // ---------- 节拍和戏 ----------

    /// 节拍到点：所有人听到宣布，和它有关的人被叫醒、听到叮嘱；有戏就开戏。
    fn fire(&mut self, world: &mut World, i: usize) {
        self.next = i + 1;
        // 上一场打完的遭遇战留在画面上到这里为止
        if let Some(rules) = &mut self.rules {
            if rules.fight.as_ref().is_some_and(|f| f.outcome.is_some()) {
                rules.fight = None;
            }
        }
        let b = self.script.beats[i].clone();
        let place = b
            .place
            .as_deref()
            .map(|p| format!("（{}）", self.place_name(world, p)))
            .unwrap_or_default();
        let public = format!(
            "{}{}{place}：{}",
            self.tag(),
            b.title.trim(),
            b.announce.trim()
        );
        let who = self.among(&b.who);
        let others: Vec<usize> = self
            .everyone()
            .into_iter()
            .filter(|i| !who.contains(i))
            .collect();
        self.tell(world, &others, &public);
        let private = match b.brief.as_deref().map(str::trim) {
            Some(brief) if !brief.is_empty() => format!("{public}\n只对你说：{brief}"),
            _ => public.clone(),
        };
        for &w in &who {
            self.call(world, w, &private);
        }
        world.log(EventInfo::default(), public);
        let now = world.minutes();
        let done: Vec<String> = self
            .script
            .milestones
            .iter()
            .filter(|m| matches!(&m.when, StorylineWhen::Beat { beat } if *beat == b.id))
            .map(|m| m.id.clone())
            .collect();
        for id in done {
            self.reach(world, &id, now, None, None);
        }
        self.notice(world, i);
        self.begin_fight(world, i);
        if let Some(scene) = &b.scene {
            self.scene = Some(Scene {
                beat: i,
                queue: scene.speakers.iter().map(|&s| s as usize - 1).collect(),
                turn: None,
                until: now,
            });
            self.next_speaker(world);
        }
    }

    fn next_speaker(&mut self, world: &mut World) {
        let Some(mut scene) = self.scene.take() else {
            return;
        };
        let now = world.minutes();
        let b = &self.script.beats[scene.beat];
        let Some(rules) = b.scene.as_ref() else {
            return;
        };
        while !scene.queue.is_empty() {
            let i = scene.queue.remove(0);
            if !self.here(world, i) {
                continue;
            }
            scene.turn = Some(i);
            scene.until = now + f64::from(rules.minutes);
            let text = format!(
                "{}「{}」轮到你说话：用 mode_action 交 speak，text 写你要说的话（最多 {} 字）；不说交 pass。{} 分钟内不交算跳过。",
                self.tag(),
                b.title.trim(),
                rules.chars,
                rules.minutes
            );
            self.call(world, i, &text);
            self.scene = Some(scene);
            return;
        }
        // 都说完了：这场戏散了
    }

    /// 一个里程碑达成：记下来，告诉所有人（藏着的只告诉能宣布它的人）。
    fn reach(
        &mut self,
        world: &mut World,
        id: &str,
        at: f64,
        by: Option<usize>,
        text: Option<String>,
    ) {
        let Some(m) = self.script.milestones.iter().find(|m| m.id == id).cloned() else {
            return;
        };
        if self.reached.contains_key(id) {
            return;
        }
        let said = match (&by, &text) {
            (Some(i), Some(t)) => format!("（{}：{t}）", self.label(*i)),
            _ => String::new(),
        };
        let line = format!("{}里程碑达成：{}{said}", self.tag(), m.title.trim());
        let audience = match &m.when {
            StorylineWhen::Declare { seats } if m.secret => self.among(seats),
            _ => self.everyone(),
        };
        self.tell(world, &audience, &line);
        world.log(EventInfo::default(), line);
        self.reached
            .insert(id.to_string(), Reached { at, by, text });
        // 开着规则：里程碑记在谁名下，谁得经验
        if let Some(seat) = by {
            self.award(world, seat, srd5::MILESTONE_XP);
        }
    }

    /// 「几个人同时在某地」的里程碑：到齐了就算达成。地图上没有这个地点的永远到不齐。
    fn watch(&mut self, world: &mut World) {
        let now = world.minutes();
        let met: Vec<String> = self
            .script
            .milestones
            .iter()
            .filter(|m| !self.reached.contains_key(&m.id))
            .filter(|m| match &m.when {
                StorylineWhen::Gather {
                    place,
                    seats,
                    count,
                } => self.places.get(place).is_some_and(|locale| {
                    let there = self
                        .among(seats)
                        .into_iter()
                        .filter(|&i| {
                            self.here(world, i)
                                && world
                                    .actor(&self.seats[i].id)
                                    .is_some_and(|a| a.locus.locale == *locale)
                        })
                        .count();
                    there >= *count as usize
                }),
                _ => false,
            })
            .map(|m| m.id.clone())
            .collect();
        for id in met {
            self.reach(world, &id, now, None, None);
        }
    }

    fn watching(&self) -> bool {
        self.script.milestones.iter().any(|m| {
            matches!(m.when, StorylineWhen::Gather { .. }) && !self.reached.contains_key(&m.id)
        })
    }

    /// 收场：达成和没达成的里程碑说一遍，再念收场白。
    fn finish(&mut self, world: &mut World) {
        self.ended = true;
        self.scene = None;
        let (done, missed): (Vec<_>, Vec<_>) = self
            .script
            .milestones
            .iter()
            .filter(|m| !m.secret && self.tracked(m))
            .partition(|m| self.reached.contains_key(&m.id));
        let names = |list: &[&protocol::StorylineMilestone]| {
            list.iter()
                .map(|m| m.title.trim().to_string())
                .collect::<Vec<_>>()
                .join("、")
        };
        let mut text = format!("{}故事收场。", self.tag());
        if !done.is_empty() {
            text.push_str(&format!("达成：{}。", names(&done)));
        }
        if !missed.is_empty() {
            text.push_str(&format!("没有达成：{}。", names(&missed)));
        }
        if let Some(e) = self
            .script
            .epilogue
            .as_deref()
            .map(str::trim)
            .filter(|e| !e.is_empty())
        {
            text.push_str(e);
        }
        self.tell(world, &self.everyone(), &text);
        world.log(EventInfo::default(), text);
    }

    /// 到点的事做一件；做了返回 true。戏没散时后面的节拍等着。
    fn step(&mut self, world: &mut World) -> bool {
        if self.ended {
            return false;
        }
        let now = world.minutes() + EPS;
        // 遭遇战没打完时后面的事都等着；轮到的角色到点没交就替他闪避
        if self.fighting() {
            let until = self.rules.as_ref().map_or(0.0, |r| r.fight_until);
            if now < until {
                return false;
            }
            self.fight_timeout(world);
            return true;
        }
        if let Some(scene) = &self.scene {
            if now < scene.until {
                return false;
            }
            if let Some(i) = scene.turn {
                let text = format!("{}{} 没有说话（到点跳过）。", self.tag(), self.label(i));
                self.tell(world, &self.everyone(), &text);
                world.log(EventInfo::default(), text);
            }
            self.next_speaker(world);
            return true;
        }
        if self.next < self.script.beats.len() {
            if now < self.beat_at(self.next) {
                return false;
            }
            let i = self.next;
            self.fire(world, i);
            return true;
        }
        // 节拍都过了、快收场了：还握着只能由自己宣布的里程碑的人各提醒一遍
        if !self.last_call {
            if now < self.last_call_at() {
                return false;
            }
            self.remind_to_declare(world);
            return true;
        }
        if now < self.end_at() {
            return false;
        }
        // 到点了。还有人握着没宣布的里程碑：再等一小会儿，不然模型正写着的那句宣布刚好晚一步就不算了
        let waiting =
            (0..self.seats.len()).any(|i| self.here(world, i) && !self.declarable(i).is_empty());
        if waiting && !self.closing {
            self.closing = true;
            let text = format!(
                "{}收场的钟点到了。还有话要当众定下来的，这是最后的机会；{GRACE_MINUTES} 分钟后收场。",
                self.tag()
            );
            for i in 0..self.seats.len() {
                if self.here(world, i) && !self.declarable(i).is_empty() {
                    self.tell(world, &[i], &text);
                    self.call(world, i, &text);
                }
            }
            return true;
        }
        if waiting && now < self.end_at() + GRACE_MINUTES {
            return false;
        }
        self.finish(world);
        true
    }

    fn last_call_at(&self) -> f64 {
        self.end_at() - LAST_CALL_MINUTES
    }

    /// 收场前的提醒：由当事人自己宣布的里程碑，没人宣布就一直不算。模型常常事情做了却忘了宣布，
    /// 所以快收场时把还能宣布的那几项单独告诉本人一遍、叫醒他；宣不宣布还是他自己定。别人听不到。
    fn remind_to_declare(&mut self, world: &mut World) {
        self.last_call = true;
        let left = (self.end_at() - world.minutes()).max(0.0).round();
        for i in 0..self.seats.len() {
            let mine: Vec<String> = self
                .declarable(i)
                .iter()
                .map(|m| {
                    format!(
                        "「{}」（编号 {}）：{}",
                        m.title.trim(),
                        m.id,
                        m.detail.trim()
                    )
                })
                .collect();
            if mine.is_empty() || !self.here(world, i) {
                continue;
            }
            let text = format!(
                "{}故事还有 {left} 分钟收场。下面的事只有你能宣布，没人宣布就不算发生过：{}。\
用 mode_action 交 declare（target 填编号，text 写一句经过）就是当众把它定下来——\
本来就是要你当众说一句、表个态的事，交 declare 就是说这句话，不用另外再试；\
要先做成一件事的，做成了马上交，还差一步就先去做完；没成、也来不及成的不要交。",
                self.tag(),
                mine.join("；")
            );
            self.tell(world, &[i], &text);
            self.call(world, i, &text);
        }
    }

    /// 不在了的人：记下来；正轮到他说话就马上往下走。
    fn notice_losses(&mut self, world: &mut World) {
        if self.ended {
            return;
        }
        for i in 0..self.seats.len() {
            if self.seats[i].gone || self.here(world, i) {
                continue;
            }
            self.seats[i].gone = true;
            let text = format!("{}{} 不在了。", self.tag(), self.label(i));
            self.tell(world, &self.everyone(), &text);
            world.log(EventInfo::default(), text);
            if let Some(fight) = self.rules.as_mut().and_then(|r| r.fight.as_mut()) {
                if let Some(f) = fight.fighters.iter_mut().find(|f| f.seat == Some(i)) {
                    f.fled = true;
                }
            }
            if let Some(scene) = &mut self.scene {
                if scene.turn == Some(i) {
                    scene.turn = None;
                    scene.until = scene.until.min(world.minutes());
                }
            }
        }
    }

    // ---------- 给模型 ----------

    /// 这个里程碑此刻算不算数：靠检定达成的要开着规则套件才有，没开时哪里都不出现
    fn tracked(&self, m: &protocol::StorylineMilestone) -> bool {
        self.rules.is_some()
            || !matches!(
                m.when,
                StorylineWhen::Check { .. } | StorylineWhen::Encounter { .. }
            )
    }

    /// 这个人能宣布的、还没达成的里程碑
    fn declarable(&self, i: usize) -> Vec<&protocol::StorylineMilestone> {
        self.script
            .milestones
            .iter()
            .filter(|m| !self.reached.contains_key(&m.id))
            .filter(|m| {
                matches!(&m.when, StorylineWhen::Declare { seats } if self.among(seats).contains(&i))
            })
            .collect()
    }

    /// 事情做成了以后顺带提一句：手里还有能宣布的里程碑，这件事要是就是其中一项，接着宣布。
    /// 模型做成了事常常忘了宣布，里程碑就一直空着。
    fn declare_hint(&self, i: usize) -> Option<String> {
        let mine: Vec<String> = self
            .declarable(i)
            .iter()
            .map(|m| format!("「{}」（编号 {}）", m.title.trim(), m.id))
            .collect();
        (!mine.is_empty()).then(|| {
            format!(
                "{}你手里还有只有你能宣布的事：{}。刚才这件事要是就让其中一项成了，接着用 mode_action 交 declare（target 填编号），不宣布不算。",
                self.tag(),
                mine.join("、")
            )
        })
    }

    /// 此刻该做什么
    fn duty(&self, world: &World, i: usize) -> String {
        if self.ended {
            return "故事已经收场。".into();
        }
        if self.seats[i].gone {
            return "你已经不在这个故事里了。".into();
        }
        if let Some(scene) = &self.scene {
            let b = &self.script.beats[scene.beat];
            let chars = b.scene.as_ref().map_or(0, |s| s.chars);
            return match scene.turn {
                Some(t) if t == i => format!(
                    "「{}」轮到你说话：交 speak，text 写你要说的话（最多 {chars} 字）；不说交 pass。",
                    b.title.trim()
                ),
                Some(t) => format!(
                    "「{}」正在进行，现在是 {} 在说；轮到你时会叫你。",
                    b.title.trim(),
                    self.label(t)
                ),
                None => format!("「{}」正在进行。", b.title.trim()),
            };
        }
        // 最近一个发生了的节拍；和这个人有关的带上叮嘱
        match self.next.checked_sub(1) {
            Some(last) => {
                let b = &self.script.beats[last];
                let place = b
                    .place
                    .as_deref()
                    .map(|p| format!("，{}", self.place_name(world, p)))
                    .unwrap_or_default();
                let brief = b
                    .brief
                    .as_deref()
                    .map(str::trim)
                    .filter(|_| self.among(&b.who).contains(&i))
                    .map(|t| format!(" 只对你说：{t}"))
                    .unwrap_or_default();
                format!(
                    "「{}」（{}{place}）：{}{brief}",
                    b.title.trim(),
                    clock(self.beat_at(last)),
                    b.announce.trim()
                )
            }
            None => "故事还没开始，先照常过日子。".into(),
        }
    }

    fn next_line(&self, world: &World) -> Option<String> {
        if self.ended {
            return None;
        }
        let Some(b) = self.script.beats.get(self.next) else {
            return Some(format!(
                "{} 故事收场",
                self.when(self.script.ends_day, self.script.ends_minute)
            ));
        };
        let place = b
            .place
            .as_deref()
            .map(|p| format!("，{}", self.place_name(world, p)))
            .unwrap_or_default();
        Some(format!(
            "{}「{}」{place}",
            self.when(b.day, b.minute),
            b.title.trim()
        ))
    }

    fn milestone_lines(&self, i: usize) -> Vec<String> {
        let mine: BTreeSet<&str> = self.declarable(i).iter().map(|m| m.id.as_str()).collect();
        self.script
            .milestones
            .iter()
            .filter(|m| self.tracked(m) && (!m.secret || mine.contains(m.id.as_str())))
            .map(|m| {
                let state = if self.reached.contains_key(&m.id) {
                    "已达成"
                } else if self.ended {
                    "没有达成"
                } else {
                    "还没达成"
                };
                let how = if mine.contains(m.id.as_str()) {
                    format!("（发生了就由你用 declare 宣布，target 填 {}）", m.id)
                } else {
                    String::new()
                };
                format!("[{state}] {}：{}{how}", m.title.trim(), m.detail.trim())
            })
            .collect()
    }

    // ---------- 交动作 ----------

    fn speak(&mut self, world: &mut World, i: usize, action: &GameModeAction) -> Result<String> {
        let Some(scene) = self.scene.as_ref().filter(|s| s.turn == Some(i)) else {
            return Err(bad(format!("现在没轮到你说话：{}", self.duty(world, i))));
        };
        let b = self.script.beats[scene.beat].clone();
        if action.action == "pass" {
            if let Some(s) = &mut self.scene {
                s.turn = None;
            }
            self.next_speaker(world);
            return Ok("这次没有说话".into());
        }
        let raw = action.text.as_deref().unwrap_or("").trim();
        if raw.is_empty() {
            return Err(bad("text 不能为空：写上你要说的话"));
        }
        let limit = b.scene.as_ref().map_or(TEXT_CHARS, |s| s.chars as usize);
        let (text, clipped) = clip(raw, limit);
        let away = b
            .place
            .as_deref()
            .and_then(|p| self.places.get(p))
            .is_some_and(|locale| {
                world
                    .actor(&self.seats[i].id)
                    .is_none_or(|a| a.locus.locale != *locale)
            });
        let occasion = b.title.trim().to_string();
        self.lines.push(StorylineLine {
            seat: i as u32 + 1,
            occasion: occasion.clone(),
            text: text.clone(),
            at: world.minutes(),
            away,
            reason: reason_of(action),
        });
        let heard = format!(
            "【{occasion}】{}{}：{text}",
            self.label(i),
            if away { "（没到场）" } else { "" }
        );
        self.tell(world, &self.everyone(), &heard);
        let actor = &world.actors[&self.seats[i].id];
        let info = EventInfo::speech(
            &actor.id,
            &actor.name,
            EventSpeech {
                occasion,
                seat: Some(i as u32 + 1),
                text,
            },
        );
        world.log(info, heard);
        if let Some(s) = &mut self.scene {
            s.turn = None;
        }
        self.next_speaker(world);
        Ok(if clipped {
            format!("说完了；超出 {limit} 字的部分没有念出来")
        } else {
            "说完了，大家都听到了".into()
        })
    }

    // ---------- 检定（开着规则套件才有） ----------

    /// 这个人此刻能试的检定：节拍已经发生、轮得到他、还没人通过；不能再试的那种他还没试过。
    /// 被动检定不用人去试，不在里面；对抗的另一方不能试自己这一项。
    fn open_checks(&self, i: usize) -> Vec<&StorylineCheck> {
        let Some(rules) = &self.rules else {
            return vec![];
        };
        self.script.beats[..self.next]
            .iter()
            .flat_map(|b| &b.checks)
            .filter(|c| {
                !c.passive
                    && self.among(&c.seats).contains(&i)
                    && c.against.as_ref().is_none_or(|a| a.seat as usize - 1 != i)
                    && (c.repeat || !rules.has_tried(i, &c.id))
                    && !rules.passed.contains_key(&c.id)
            })
            .collect()
    }

    /// 这个人此刻试这项检定有优势还是劣势，连同原因：剧本里写的情形成立的、有人帮了一把的。
    /// 优势和劣势都有就互相抵掉。
    fn edge_for(
        &self,
        world: &World,
        i: usize,
        check: &StorylineCheck,
    ) -> (Option<Edge>, Vec<String>) {
        let mut reasons = Vec::new();
        let (mut up, mut down) = (false, false);
        for e in &check.edges {
            let mine = e.seats.is_empty() || e.seats.contains(&(i as u32 + 1));
            let reached = e
                .milestone
                .as_ref()
                .is_none_or(|m| self.reached.contains_key(m));
            let there = e.place.as_ref().is_none_or(|p| {
                self.places.get(p).is_some_and(|locale| {
                    world
                        .actor(&self.seats[i].id)
                        .is_some_and(|a| a.locus.locale == *locale)
                })
            });
            if mine && reached && there {
                match e.effect {
                    Edge::Advantage => up = true,
                    Edge::Disadvantage => down = true,
                }
                reasons.push(e.reason.trim().to_string());
            }
        }
        let helper = self
            .rules
            .as_ref()
            .and_then(|r| r.helped.get(&check.id))
            .and_then(|h| h.get(&i));
        if let Some(&helper) = helper {
            up = true;
            reasons.push(format!("{} 帮了一把", self.seats[helper].name));
        }
        let edge = match (up, down) {
            (true, false) => Some(Edge::Advantage),
            (false, true) => Some(Edge::Disadvantage),
            _ => None,
        };
        (edge, reasons)
    }

    /// 一项检定给这个人看的那一行：难度或对抗的是谁、能不能再试、此刻的优势劣势，和他每种试法的加值
    fn check_note(&self, world: &World, i: usize, check: &StorylineCheck) -> String {
        let mut notes = match &check.against {
            Some(a) if self.here(world, a.seat as usize - 1) => {
                format!(
                    "和 {} 对抗，掷得比对方高才算成",
                    self.label(a.seat as usize - 1)
                )
            }
            _ => format!("难度 {}", check.dc),
        };
        if check.repeat {
            notes.push_str("，没成可以再试");
        }
        match self.edge_for(world, i, check) {
            (Some(Edge::Advantage), why) => {
                notes.push_str(&format!("，此刻有优势（{}）", why.join("；")))
            }
            (Some(Edge::Disadvantage), why) => {
                notes.push_str(&format!("，此刻有劣势（{}）", why.join("；")))
            }
            _ => {}
        }
        match &self.rules {
            Some(rules) => rules.check_line(i, check, &notes),
            None => String::new(),
        }
    }

    /// target 指的是这个人此刻能试的哪一项检定
    fn pick_check(&self, i: usize, action: &GameModeAction) -> Result<StorylineCheck> {
        if self.rules.is_none() {
            return Err(bad("这个故事没有开规则，没有检定可试"));
        }
        let open = self.open_checks(i);
        if open.is_empty() {
            return Err(bad("现在没有你能试的检定"));
        }
        let raw = action.target.as_deref().map(str::trim).unwrap_or("");
        open.iter()
            .find(|c| c.id == raw || c.title.trim() == raw)
            .map(|c| (*c).clone())
            .ok_or_else(|| {
                let ids: Vec<&str> = open.iter().map(|c| c.id.as_str()).collect();
                bad(format!(
                    "认不出 target「{raw}」：你能试的是 {}",
                    ids.join("、")
                ))
            })
    }

    /// 试一项检定：内核掷 d20 加上这个人的加值，和难度比（对抗时和对方掷出来的比）；
    /// 经过对所有人公布，通过了的里程碑随之达成。
    fn attempt(&mut self, world: &mut World, i: usize, action: &GameModeAction) -> Result<String> {
        let check = self.pick_check(i, action)?;
        // 用哪种试法：with 里的 skill 或 ability；只有一种试法时可以不填
        let wanted = action
            .with
            .get("skill")
            .or_else(|| action.with.get("ability"))
            .map(|w| w.trim().to_string());
        let way = match wanted {
            Some(key) => check.ways.iter().find(|w| srd5::way_key(w) == key).cloned(),
            None if check.ways.len() == 1 => check.ways.first().cloned(),
            None => None,
        };
        let Some(way) = way else {
            let keys: Vec<String> = check.ways.iter().map(srd5::way_key).collect();
            return Err(bad(format!(
                "「{}」要说明用哪种办法：with 里填 skill 或 ability，能用的是 {}",
                check.title.trim(),
                keys.join("、")
            )));
        };
        let (edge, reasons) = self.edge_for(world, i, &check);
        // 对抗：对方还在场就由他用最拿手的办法应对；不在了就照难度算
        let rival: Option<(usize, CheckWay)> = check.against.as_ref().and_then(|a| {
            let other = a.seat as usize - 1;
            let way = self.rules.as_ref()?.best(other, &a.ways)?.clone();
            self.here(world, other).then_some((other, way))
        });
        let now = world.minutes();
        let roll = match &mut self.rules {
            Some(rules) => rules.attempt(
                i,
                &check,
                &way,
                edge,
                rival.as_ref().map(|(other, way)| (*other, way)),
                now,
            ),
            None => return Err(bad("这个故事没有开规则，没有检定可试")),
        };
        let passed = roll.success == Some(true);
        let verdict = if passed { "成功" } else { "失败" };
        let said = action
            .text
            .as_deref()
            .map(|t| clip(t, EVIDENCE_CHARS).0)
            .filter(|t| !t.is_empty())
            .map(|t| format!("（{t}）"))
            .unwrap_or_default();
        let thrown = match edge {
            Some(Edge::Advantage) => format!(
                "有优势（{}），掷两次 d20 得 {} 和 {}，取高的",
                reasons.join("；"),
                roll.dice[0],
                roll.dice[1]
            ),
            Some(Edge::Disadvantage) => format!(
                "有劣势（{}），掷两次 d20 得 {} 和 {}，取低的",
                reasons.join("；"),
                roll.dice[0],
                roll.dice[1]
            ),
            None => format!("掷 d20 得 {}", roll.dice[0]),
        };
        let against = match (&rival, roll.versus) {
            (Some((other, other_way)), Some(total)) => format!(
                "{} 用{}应对，共 {total}",
                self.label(*other),
                srd5::way_name(other_way)
            ),
            _ => format!("对难度 {}", check.dc),
        };
        let outcome = format!(
            "{thrown}，{:+} 共 {}，{against}：{verdict}",
            roll.modifier, roll.total
        );
        let line = format!(
            "{}检定：{} 试「{}」{said}，用{}，{outcome}",
            self.tag(),
            self.label(i),
            check.title.trim(),
            roll.way,
        );
        self.tell(world, &self.everyone(), &line);
        world.log(EventInfo::default(), line);
        if passed {
            self.pass(world, &check.id, i);
            if let Some(hint) = self.declare_hint(i) {
                self.tell(world, &[i], &hint);
            }
        }
        Ok(outcome)
    }

    /// 检定通过了：靠它达成的里程碑记在这个人名下
    fn pass(&mut self, world: &mut World, check: &str, i: usize) {
        let now = world.minutes();
        let done: Vec<String> = self
            .script
            .milestones
            .iter()
            .filter(|m| matches!(&m.when, StorylineWhen::Check { check: c } if c == check))
            .map(|m| m.id.clone())
            .collect();
        for id in done {
            self.reach(world, &id, now, Some(i), None);
        }
    }

    /// 帮一把：自己不试这项检定，让同在一处的另一个人下一次试它时有优势。
    fn help(&mut self, world: &mut World, i: usize, action: &GameModeAction) -> Result<String> {
        let check = self.pick_check(i, action)?;
        let raw = action.with.get("seat").map(|s| s.trim()).unwrap_or("");
        let digits: String = raw.chars().take_while(char::is_ascii_digit).collect();
        let other = digits
            .parse::<usize>()
            .ok()
            .filter(|n| (1..=self.seats.len()).contains(n))
            .map(|n| n - 1)
            .or_else(|| self.seats.iter().position(|s| s.name == raw))
            .ok_or_else(|| bad("帮谁：with 里填 seat（对方的座号）"))?;
        if other == i || !self.open_checks(other).iter().any(|c| c.id == check.id) {
            return Err(bad(format!(
                "{} 现在试不了「{}」，帮不上",
                self.label(other),
                check.title.trim()
            )));
        }
        let place = |k: usize| {
            world
                .actor(&self.seats[k].id)
                .map(|a| a.locus.locale.clone())
        };
        if place(i) != place(other) {
            return Err(bad(format!(
                "要和 {} 在同一个地方才帮得上",
                self.label(other)
            )));
        }
        if let Some(rules) = &mut self.rules {
            rules.tried.entry(check.id.clone()).or_default().push(i);
            rules
                .helped
                .entry(check.id.clone())
                .or_default()
                .insert(other, i);
        }
        let line = format!(
            "{}{} 帮 {} 一把：{} 下一次试「{}」有优势",
            self.tag(),
            self.label(i),
            self.label(other),
            self.seats[other].name,
            check.title.trim()
        );
        self.tell(world, &self.everyone(), &line);
        world.log(EventInfo::default(), line);
        Ok(format!("帮上了：{} 下一次试有优势", self.seats[other].name))
    }

    /// 节拍到点时的被动检定：不掷骰，10 加上最拿手的那种办法的加值，够难度的人自己留意到（只告诉他本人）；
    /// 留意到的人里数值最高的那个算通过了这项检定。
    fn notice(&mut self, world: &mut World, beat: usize) {
        let Some(rules) = &self.rules else {
            return;
        };
        let mut found: Vec<(String, usize, i32)> = Vec::new();
        for c in self.script.beats[beat].checks.iter().filter(|c| c.passive) {
            for i in self.among(&c.seats) {
                if !self.here(world, i) {
                    continue;
                }
                let Some(way) = rules.best(i, &c.ways) else {
                    continue;
                };
                let value = 10 + rules.bonus(i, way);
                if value >= c.dc as i32 {
                    found.push((c.id.clone(), i, value));
                }
            }
        }
        for check in self.script.beats[beat].checks.clone() {
            let mut who: Vec<(usize, i32)> = found
                .iter()
                .filter(|(id, _, _)| *id == check.id)
                .map(|(_, i, value)| (*i, *value))
                .collect();
            if who.is_empty() {
                continue;
            }
            for (i, _) in &who {
                let text = format!(
                    "{}你留意到了：{}（{}）。别人不一定看出来了。",
                    self.tag(),
                    check.title.trim(),
                    check.detail.trim()
                );
                self.call(world, *i, &text);
            }
            who.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            let (best, _) = who[0];
            let names: Vec<String> = who.iter().map(|(i, _)| self.label(*i)).collect();
            world.log(
                EventInfo::default(),
                format!(
                    "{}被动检定「{}」（难度 {}）：{} 留意到了",
                    self.tag(),
                    check.title.trim(),
                    check.dc,
                    names.join("、")
                ),
            );
            if let Some(rules) = &mut self.rules {
                rules.passed.entry(check.id.clone()).or_insert(best);
            }
            self.pass(world, &check.id, best);
        }
    }

    /// 长休：在世界里实实在在睡够一觉，醒来生命和法术位回满（只告诉本人）。
    fn rest(&mut self, world: &mut World) {
        if self.rules.is_none() || self.fighting() {
            return;
        }
        let now = world.minutes();
        for i in 0..self.seats.len() {
            if !self.here(world, i) {
                continue;
            }
            let asleep = !world.awake(&self.seats[i].id);
            let rested = self
                .rules
                .as_mut()
                .is_some_and(|r| r.note_sleep(i, asleep, now));
            if rested {
                let sheet = self
                    .rules
                    .as_ref()
                    .map(|r| r.sheet_line(i))
                    .unwrap_or_default();
                let id = self.seats[i].id.clone();
                world.actor_mut(&id).journal.fact(
                    now,
                    FactKind::Observation,
                    format!("{}好好睡了一觉：生命和法术位都回满了。{sheet}", self.tag()),
                    self.object(),
                    None,
                    None,
                );
            }
        }
    }

    // ---------- 遭遇战（开着规则套件才有） ----------

    /// 对所有人说一句、记进世界日志；不叫醒谁。
    fn say(&self, world: &mut World, line: &str) {
        let text = format!("{}{line}", self.tag());
        self.tell(world, &self.everyone(), &text);
        world.log(EventInfo::default(), text);
    }

    fn fighting(&self) -> bool {
        self.rules
            .as_ref()
            .and_then(|r| r.fight.as_ref())
            .is_some_and(|f| f.outcome.is_none())
    }

    /// 这个人在正打着的遭遇战里是第几个（fighters 的下标）
    fn fighter_of(&self, i: usize) -> Option<usize> {
        let fight = self.rules.as_ref()?.fight.as_ref()?;
        fight.fighters.iter().position(|f| f.seat == Some(i))
    }

    /// 节拍到点打起来：卷进来的、还在场的角色照角色卡参战，对手照剧本摆好，各掷先攻。
    fn begin_fight(&mut self, world: &mut World, beat: usize) {
        let Some(enc) = self.script.beats[beat].encounter.clone() else {
            return;
        };
        let seats: Vec<usize> = self
            .among(&enc.party)
            .into_iter()
            .filter(|&i| self.here(world, i))
            .collect();
        let names: Vec<String> = seats.iter().map(|&i| self.seats[i].name.clone()).collect();
        let Some(rules) = &mut self.rules else {
            return;
        };
        if seats.is_empty() {
            return;
        }
        let members = seats
            .iter()
            .zip(names)
            .filter(|(i, _)| rules.hp[**i] > 0)
            .map(|(&i, name)| rules.member(i, name))
            .collect();
        let fight = Fight::open(&enc, members, &mut rules.dice);
        let order: Vec<String> = fight
            .order
            .iter()
            .map(|&k| fight.fighters[k].name.clone())
            .collect();
        let foes: Vec<String> = fight
            .fighters
            .iter()
            .filter(|f| f.seat.is_none())
            .map(|f| format!("{}（{}）", f.name, fight.zones[f.zone]))
            .collect();
        let opening = format!(
            "遭遇战「{}」打起来了！场地：{}。对手：{}。先攻次序：{}。",
            fight.title,
            fight.zones.join(" — "),
            foes.join("、"),
            order.join("、")
        );
        rules.fight = Some(fight);
        rules.fight_until = 0.0;
        self.say(world, &opening);
        self.drive_fight(world);
    }

    /// 把战斗往下推：对手和动不了的人的回合当场走完，轮到一个能动的角色就叫醒他、等他交；打完了就收场。
    fn drive_fight(&mut self, world: &mut World) {
        let now = world.minutes();
        for _ in 0..400 {
            let Some(rules) = &mut self.rules else {
                return;
            };
            let Some(fight) = &mut rules.fight else {
                return;
            };
            let Some(i) = fight.up() else {
                return self.end_fight(world);
            };
            let (mut lines, can) = fight.start_turn(i, &mut rules.dice, &mut rules.rolls, now);
            let seat = fight.fighters[i].seat;
            let waiting = match seat {
                Some(seat) if can => Some(seat),
                _ => {
                    if can {
                        lines.extend(fight.foe_turn(i, &mut rules.dice, &mut rules.rolls, now));
                    }
                    fight.end_turn();
                    None
                }
            };
            let brief = waiting.map(|_| fight.brief(i));
            if waiting.is_some() {
                rules.fight_until = now + FIGHT_TURN_MINUTES;
            }
            rules.trim_rolls();
            for line in &lines {
                self.say(world, line);
            }
            if let (Some(seat), Some(brief)) = (waiting, brief) {
                let text = format!(
                    "{}轮到你出手！{}\n用 mode_action 交一项：attack 攻击（target 填对手的编号）、dodge 闪避、disengage 撤步、\
dash 冲刺、help 帮同伴（target 填他的座号）、flee 退出战斗、pass 不出手；要先走一步就在 with 里填 move（区域的名字或第几块）。\
{FIGHT_TURN_MINUTES:.0} 分钟内不交算闪避。",
                    self.tag(),
                    brief.join("\n")
                );
                self.call(world, seat, &text);
                return;
            }
        }
    }

    /// 打完了：胜负说一遍；倒下没死的缓过来剩 1 点生命，死了的在世界里真的死去；打赢了的里程碑达成。
    fn end_fight(&mut self, world: &mut World) {
        let now = world.minutes();
        let Some(rules) = &mut self.rules else {
            return;
        };
        rules.fight_until = 0.0;
        let (dead, woke) = rules.settle();
        let Some(fight) = &rules.fight else {
            return;
        };
        let outcome = fight.outcome;
        let (id, title) = (fight.id.clone(), fight.title.clone());
        let duel = id == "duel";
        let won = outcome == Some(Outcome::Won);
        // 决斗没有经验也没有里程碑；打赢了遭遇战的经验站着的人平分
        let standing: Vec<usize> = fight
            .fighters
            .iter()
            .filter(|f| !f.dead)
            .filter_map(|f| f.seat)
            .collect();
        let prize = if won && !duel && !standing.is_empty() {
            Srd5::fight_xp(fight) / standing.len() as u32
        } else {
            0
        };
        let mut line = if duel {
            let winner = fight
                .fighters
                .iter()
                .find(|f| f.active())
                .map(|f| f.name.clone());
            match winner {
                Some(name) => format!("{title}：分出了高下，{name} 站到了最后。"),
                None => format!("{title}：两个人都停了手。"),
            }
        } else {
            if won {
                rules.won.push(id.clone());
            }
            format!(
                "遭遇战「{title}」结束：{}。",
                match outcome {
                    Some(Outcome::Won) => "对手都倒下了，打赢了",
                    Some(Outcome::Lost) => "站着的人都倒下或退走了，打输了",
                    _ => "拖得太久，双方散开了",
                }
            )
        };
        if !woke.is_empty() {
            line.push_str(&format!("{} 缓了过来，剩 1 点生命。", woke.join("、")));
        }
        if prize > 0 {
            line.push_str(&format!("在场的每人得 {prize} 点经验。"));
        }
        self.say(world, &line);
        if prize > 0 {
            for seat in standing {
                self.award(world, seat, prize);
            }
        }
        for seat in dead {
            self.kill(world, seat, &title);
        }
        if won && !duel {
            let done: Vec<String> = self
                .script
                .milestones
                .iter()
                .filter(
                    |m| matches!(&m.when, StorylineWhen::Encounter { encounter } if *encounter == id),
                )
                .map(|m| m.id.clone())
                .collect();
            for m in done {
                self.reach(world, &m, now, None, None);
            }
        }
    }

    /// 给一个人经验；升级了就说一声，单独告诉他新的角色卡。
    fn award(&mut self, world: &mut World, seat: usize, xp: u32) {
        let Some(rules) = &mut self.rules else {
            return;
        };
        let Some(level) = rules.gain(seat, xp) else {
            return;
        };
        let sheet = rules.sheet_line(seat);
        self.say(world, &format!("{} 升到了 {level} 级。", self.label(seat)));
        let text = format!("{}你升到了 {level} 级。现在的角色卡：{sheet}", self.tag());
        self.call(world, seat, &text);
    }

    /// 两个角色动手（开着规则时对人动手只有这一条路）：同在一处、都还站着、没有别的架在打，
    /// 按规则里的战斗来，打到一方倒下为止。
    fn begin_duel(
        &mut self,
        world: &mut World,
        i: usize,
        action: &GameModeAction,
    ) -> Result<String> {
        let raw = action.target.as_deref().map(str::trim).unwrap_or("");
        let digits: String = raw.chars().take_while(char::is_ascii_digit).collect();
        let other = digits
            .parse::<usize>()
            .ok()
            .filter(|n| (1..=self.seats.len()).contains(n))
            .map(|n| n - 1)
            .or_else(|| self.seats.iter().position(|s| s.name == raw))
            .filter(|&k| k != i)
            .ok_or_else(|| bad("和谁动手：target 填对方的座号"))?;
        if self.fighting() {
            return Err(bad("有一场架正在打，打完再说"));
        }
        if self.scene.is_some() {
            return Err(bad("戏还没散，这会儿动不了手"));
        }
        let place = |k: usize| {
            world
                .actor(&self.seats[k].id)
                .map(|a| a.locus.locale.clone())
        };
        if !self.here(world, other) || place(i) != place(other) {
            return Err(bad(format!("{} 不在你跟前", self.label(other))));
        }
        let (a, b) = (self.seats[i].name.clone(), self.seats[other].name.clone());
        let Some(rules) = &mut self.rules else {
            return Err(bad("这个故事没有开规则"));
        };
        if rules.hp[i] <= 0 || rules.hp[other] <= 0 {
            return Err(bad("有一方已经站不起来了"));
        }
        let fight = Fight::duel(rules.member(i, a), rules.member(other, b), &mut rules.dice);
        let opening = format!(
            "{}！先攻次序：{}。打到一方倒下为止。",
            fight.title,
            fight
                .order
                .iter()
                .map(|&k| fight.fighters[k].name.clone())
                .collect::<Vec<_>>()
                .join("、")
        );
        rules.fight = Some(fight);
        rules.fight_until = 0.0;
        self.say(world, &opening);
        self.drive_fight(world);
        Ok("动上手了，轮到你时会叫你".into())
    }

    /// 不在打的时候施治疗的法术：对象要和你在同一个地方；医疗祷言照顾到同在一处的所有人。
    fn cast_outside(
        &mut self,
        world: &mut World,
        i: usize,
        action: &GameModeAction,
    ) -> Result<String> {
        if self.fighting() {
            return Err(bad("正打着，还没轮到你出手"));
        }
        let spell = action
            .with
            .get("spell")
            .map(|s| s.trim().to_string())
            .ok_or_else(|| bad("施哪个法术：with 里填 spell"))?;
        let slot = action
            .with
            .get("slot")
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(0);
        let place = |k: usize| {
            world
                .actor(&self.seats[k].id)
                .map(|a| a.locus.locale.clone())
        };
        let near: Vec<usize> = (0..self.seats.len())
            .filter(|&k| self.here(world, k) && place(k) == place(i))
            .collect();
        let raw = action.target.as_deref().map(str::trim).unwrap_or("");
        let digits: String = raw.chars().take_while(char::is_ascii_digit).collect();
        let one = digits
            .parse::<usize>()
            .ok()
            .map(|n| n.wrapping_sub(1))
            .or_else(|| self.seats.iter().position(|s| s.name == raw));
        let targets: Vec<usize> = match one {
            Some(k) if near.contains(&k) => {
                let mut list = vec![k];
                list.extend(near.iter().copied().filter(|&n| n != k));
                list
            }
            Some(_) => return Err(bad("对方不在你跟前")),
            None => near,
        };
        let Some(rules) = &mut self.rules else {
            return Err(bad("这个故事没有开规则"));
        };
        let name = rules
            .spells(i)
            .into_iter()
            .find(|s| s.id == spell)
            .map_or(spell.clone(), |s| s.name);
        let healed = rules.cast_heal(i, &spell, slot, &targets)?;
        let parts: Vec<String> = healed
            .iter()
            .map(|(k, n)| {
                format!(
                    "{} 回了 {n} 点（{}/{}）",
                    self.seats[*k].name,
                    rules.hp[*k],
                    rules.hit_points(*k)
                )
            })
            .collect();
        let line = format!("{} 施{name}：{}", self.label(i), parts.join("，"));
        self.say(world, &line);
        Ok(line)
    }

    /// 在战斗里死了的角色在世界里也死去：健康归零，按遇害结算。
    fn kill(&self, world: &mut World, i: usize, fight: &str) {
        let id = self.seats[i].id.clone();
        let Some(a) = world.actor(&id) else {
            return;
        };
        if !a.present() || a.needs.body.dead {
            return;
        }
        let was = a.needs.abilities();
        world.actor_mut(&id).needs.lose_health(100.0);
        world.reconcile_body(
            &id,
            was,
            fight,
            DeathCause {
                killed: Some(Killing {
                    by: EventActor {
                        id: format!("storyline:{}", self.seed),
                        name: fight.into(),
                    },
                    wound: "在战斗里倒下，没撑过来".into(),
                }),
                ..DeathCause::default()
            },
        );
    }

    /// 轮到的角色交的一回合。
    fn fight_turn(
        &mut self,
        world: &mut World,
        i: usize,
        action: &GameModeAction,
    ) -> Result<String> {
        let me = self.fighter_of(i).ok_or_else(|| bad("你不在这场战斗里"))?;
        let now = world.minutes();
        let said = action
            .text
            .as_deref()
            .map(|t| clip(t, EVIDENCE_CHARS).0)
            .filter(|t| !t.is_empty());
        let Some(rules) = &mut self.rules else {
            return Err(bad("这个故事没有开规则"));
        };
        let Some(fight) = &mut rules.fight else {
            return Err(bad("现在没有在打的遭遇战"));
        };
        if fight.up() != Some(me) {
            return Err(bad("还没轮到你出手"));
        }
        let here = fight.fighters[me].zone;
        let mut move_to =
            match action.with.get("move") {
                Some(raw) => Some(fight.zone(raw).ok_or_else(|| {
                    bad(format!("认不出区域「{raw}」：{}", fight.zones.join("、")))
                })?),
                None => None,
            };
        let target = || -> Result<usize> {
            let raw = action.target.as_deref().unwrap_or("");
            fight.find(raw).ok_or_else(|| {
                bad(format!(
                    "认不出 target「{raw}」：看 storyline.fight 里的编号"
                ))
            })
        };
        let deed = match action.action.as_str() {
            "attack" => Deed::Attack(target()?),
            "help" => Deed::Help(target()?),
            "dodge" => Deed::Dodge,
            "disengage" => Deed::Disengage,
            "flee" => Deed::Flee,
            "pass" => Deed::Pass,
            "second_wind" => Deed::SecondWind,
            "cast" => Deed::Cast {
                spell: action
                    .with
                    .get("spell")
                    .map(|s| s.trim().to_string())
                    .ok_or_else(|| bad("施哪个法术：with 里填 spell"))?,
                slot: action
                    .with
                    .get("slot")
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(0),
                target: action
                    .target
                    .as_deref()
                    .filter(|t| !t.trim().is_empty())
                    .map(|raw| {
                        fight.find(raw).ok_or_else(|| {
                            bad(format!("认不出 target「{raw}」：看 storyline.fight 里的编号"))
                        })
                    })
                    .transpose()?,
            },
            "dash" => {
                // 冲刺：with.move 填要到的那块，隔一块的中间那一步自动走
                let to = move_to.ok_or_else(|| bad("冲刺要在 with 里填 move：要到的区域"))?;
                move_to = match to.abs_diff(here) {
                    1 => None,
                    2 => Some((to + here) / 2),
                    _ => return Err(bad("冲刺最多走两步")),
                };
                Deed::Dash(to)
            }
            other => {
                return Err(bad(format!(
                "战斗里没有「{other}」：能交的是 attack、cast、dodge、disengage、dash、help、second_wind、flee、pass"
            )))
            }
        };
        let order = Order { move_to, deed };
        let mut lines = fight.party_turn(me, &order, &mut rules.dice, &mut rules.rolls, now)?;
        if let Some(said) = said {
            lines.insert(0, format!("{}：「{said}」", fight.fighters[me].name));
        }
        fight.end_turn();
        rules.fight_until = 0.0;
        rules.trim_rolls();
        for line in &lines {
            self.say(world, line);
        }
        self.drive_fight(world);
        Ok(lines.join("；"))
    }

    /// 轮到的角色到点没交：只顾着闪避。
    fn fight_timeout(&mut self, world: &mut World) {
        let now = world.minutes();
        let Some(rules) = &mut self.rules else {
            return;
        };
        let Some(fight) = &mut rules.fight else {
            return;
        };
        let Some(me) = fight.up() else {
            return;
        };
        let order = Order {
            move_to: None,
            deed: Deed::Dodge,
        };
        let name = fight.fighters[me].name.clone();
        let _ = fight.party_turn(me, &order, &mut rules.dice, &mut rules.rolls, now);
        fight.end_turn();
        rules.fight_until = 0.0;
        self.say(world, &format!("{name} 没来得及出手，只顾着闪避。"));
        self.drive_fight(world);
    }

    fn declare(&mut self, world: &mut World, i: usize, action: &GameModeAction) -> Result<String> {
        let open = self.declarable(i);
        if open.is_empty() {
            return Err(bad("现在没有要你宣布的里程碑"));
        }
        let raw = action.target.as_deref().map(str::trim).unwrap_or("");
        let Some(m) = open
            .iter()
            .find(|m| m.id == raw || m.title.trim() == raw)
            .map(|m| (*m).clone())
        else {
            let ids: Vec<&str> = open.iter().map(|m| m.id.as_str()).collect();
            return Err(bad(format!(
                "认不出 target「{raw}」：你能宣布的是 {}",
                ids.join("、")
            )));
        };
        let (text, _) = clip(action.text.as_deref().unwrap_or(""), EVIDENCE_CHARS);
        if text.is_empty() {
            return Err(bad("text 不能为空：写上发生了什么"));
        }
        let now = world.minutes();
        self.reach(world, &m.id, now, Some(i), Some(text));
        Ok(format!("已宣布「{}」达成", m.title.trim()))
    }
}

impl Rules for Storyline {
    const ID: &'static str = "storyline";

    fn boundary(&self, world: &World) -> f64 {
        if self.ended {
            return f64::INFINITY;
        }
        let now = world.minutes();
        let due = match &self.scene {
            _ if self.fighting() => self.rules.as_ref().map_or(now, |r| r.fight_until),
            Some(scene) => scene.until,
            None if self.next < self.script.beats.len() => self.beat_at(self.next),
            None if !self.last_call => self.last_call_at(),
            None if self.closing => self.end_at() + GRACE_MINUTES,
            None => self.end_at(),
        };
        let wait = (due - now).max(0.0);
        if self.watching() {
            wait.min(WATCH_MINUTES)
        } else {
            wait
        }
    }

    fn tempo(&self, _world: &World) -> f64 {
        if self.scene.is_some() || self.ended || self.fighting() || self.closing {
            1.0
        } else {
            FREE_TEMPO
        }
    }

    fn advance(&mut self, world: &mut World) {
        self.notice_losses(world);
        for _ in 0..64 {
            if !self.step(world) {
                break;
            }
        }
        if !self.ended {
            self.watch(world);
            self.rest(world);
            self.expire_asks(world);
        }
    }

    fn rulings(&self, world: &World) -> Vec<RulingRequest> {
        let Some(rules) = &self.rules else {
            return Vec::new();
        };
        rules
            .asks
            .iter()
            .filter(|a| self.here(world, a.seat))
            .map(|a| RulingRequest {
                id: a.id.clone(),
                actor: self.seats[a.seat].id.clone(),
                seat: a.seat as u32 + 1,
                name: self.seats[a.seat].name.clone(),
                role: self.script.cast[a.seat].role.trim().to_string(),
                sheet: rules.sheet_line(a.seat),
                place: world
                    .actor(&self.seats[a.seat].id)
                    .map(|actor| world.topo.locale_name(&actor.locus.locale))
                    .unwrap_or_default(),
                text: a.text.clone(),
            })
            .collect()
    }

    fn rule(&mut self, world: &mut World, id: &str, ruling: Option<&Ruling>) -> Result<String> {
        if self.ended {
            return Err(bad("故事已经收场了"));
        }
        self.settle_ask(world, id, ruling)
    }

    fn view(&self, world: &World) -> GameModeView {
        let seats = self
            .seats
            .iter()
            .enumerate()
            .map(|(i, s)| StorylineSeat {
                seat: i as u32 + 1,
                id: s.id.clone(),
                name: s.name.clone(),
                role: self.script.cast[i].role.trim().to_string(),
                gone: s.gone,
            })
            .collect();
        let acting = self.scene.as_ref().map(|s| s.beat);
        let beats = self
            .script
            .beats
            .iter()
            .enumerate()
            .map(|(i, b)| StorylineBeatView {
                id: b.id.clone(),
                title: b.title.trim().to_string(),
                at: self.beat_at(i),
                place: b.place.as_deref().map(|p| self.place_name(world, p)),
                state: if acting == Some(i) {
                    StorylineBeatState::Current
                } else if i < self.next {
                    StorylineBeatState::Done
                } else {
                    StorylineBeatState::Upcoming
                },
            })
            .collect();
        let milestones = self
            .script
            .milestones
            .iter()
            .filter(|m| self.tracked(m))
            .map(|m| {
                let r = self.reached.get(&m.id);
                StorylineMilestoneView {
                    id: m.id.clone(),
                    title: m.title.trim().to_string(),
                    detail: m.detail.trim().to_string(),
                    done: r.is_some(),
                    missed: self.ended && r.is_none(),
                    at: r.map(|r| r.at),
                    by: r.and_then(|r| r.by).map(|i| i as u32 + 1),
                    text: r.and_then(|r| r.text.clone()),
                }
            })
            .collect();
        let notice = if self.ended {
            "故事收场".to_string()
        } else if let Some(scene) = &self.scene {
            self.script.beats[scene.beat].title.trim().to_string()
        } else {
            match self.next_line(world) {
                Some(next) if self.next < self.script.beats.len() => format!("下一幕 {next}"),
                Some(next) => next,
                None => String::new(),
            }
        };
        GameModeView::Storyline(StorylineView {
            title: self.script.title.trim().to_string(),
            notice,
            seats,
            beats,
            milestones,
            turn: self
                .scene
                .as_ref()
                .and_then(|s| s.turn)
                .map(|t| t as u32 + 1),
            until: self
                .scene
                .as_ref()
                .filter(|s| s.turn.is_some())
                .map(|s| s.until),
            lines: self.lines.clone(),
            ended: self.ended,
            epilogue: self
                .script
                .epilogue
                .clone()
                .filter(|_| self.ended)
                .map(|e| e.trim().to_string()),
            ruleset: self.rules.as_ref().map(|r| RulesetView {
                id: "srd5".into(),
                sheets: (0..self.seats.len())
                    .map(|i| {
                        let line = r.sheet_line(i);
                        if self.seats[i].gone {
                            line.replacen("）·", "，已不在）·", 1)
                        } else {
                            line
                        }
                    })
                    .collect(),
                rolls: r.rolls.clone(),
                fight: r.fight.as_ref().map(Fight::view),
            }),
        })
    }

    fn context(&self, world: &World, actor: &str) -> Option<GameModeContext> {
        world.actor(actor)?;
        let i = self.seat_of(actor)?;
        let role = &self.script.cast[i];
        let cast: Vec<String> = (0..self.seats.len())
            .map(|k| {
                let mark = if k == i {
                    "，你"
                } else if self.seats[k].gone {
                    "，不在了"
                } else {
                    ""
                };
                format!(
                    "{} {}（{}{mark}）",
                    k + 1,
                    self.seats[k].name,
                    self.script.cast[k].role.trim()
                )
            })
            .collect();
        let fight = self
            .rules
            .as_ref()
            .and_then(|r| r.fight.as_ref())
            .filter(|f| f.outcome.is_none());
        let me = self.fighter_of(i);
        let fight_turn = fight.is_some_and(|f| me.is_some() && f.up() == me);
        let fight_lines = match (fight, me) {
            (Some(f), Some(me)) => {
                let mut lines = f.brief(me);
                lines.push(match f.up() {
                    Some(k) if k == me => "轮到你出手。要先走到隔壁那块再出手（比如走过去治人、走过去打），\
                                            就在同一个动作的 with 里填 move；dash 会把整个回合用在走路上。"
                        .to_string(),
                    Some(k) => format!("现在轮到 {}，轮到你时会叫你。", f.fighters[k].name),
                    None => String::new(),
                });
                lines
            }
            (Some(f), None) => vec![format!("「{}」正在打，你不在里面。", f.title)],
            _ => vec![],
        };
        let mut actions = Vec::new();
        if !self.ended && !self.seats[i].gone {
            if self.scene.as_ref().is_some_and(|s| s.turn == Some(i)) {
                actions.extend(["speak".to_string(), "pass".to_string()]);
            }
            if !self.declarable(i).is_empty() {
                actions.push("declare".into());
            }
            if !self.open_checks(i).is_empty() {
                actions.extend(["check".to_string(), "help".to_string()]);
            }
            if fight_turn {
                actions = [
                    "attack",
                    "dodge",
                    "disengage",
                    "dash",
                    "help",
                    "flee",
                    "pass",
                ]
                .map(str::to_string)
                .to_vec();
                let mine = fight.zip(me).map(|(f, k)| &f.fighters[k]);
                if mine.is_some_and(|f| !f.spells.is_empty()) {
                    actions.insert(1, "cast".into());
                }
                if mine.is_some_and(|f| f.second_wind) {
                    actions.push("second_wind".into());
                }
            } else if let Some(rules) = self.rules.as_ref().filter(|_| fight.is_none()) {
                actions.push("duel".into());
                if self.scene.is_none() && !rules.asks.iter().any(|a| a.seat == i) {
                    actions.push("attempt".into());
                }
                if rules.hp[i] > 0
                    && rules.hp[i] < rules.hit_points(i) as i32
                    && rules.hit_dice_left(i) > 0
                {
                    actions.push("rest".into());
                }
                if !rules.spells(i).is_empty() {
                    actions.push("cast".into());
                }
            }
        }
        let skip = self.lines.len().saturating_sub(RECENT_LINES);
        let today = self.lines[skip..]
            .iter()
            .map(|l| {
                let away = if l.away { "，没到场" } else { "" };
                format!(
                    "{}（{}{away}）：{}",
                    self.label(l.seat as usize - 1),
                    l.occasion,
                    l.text
                )
            })
            .collect();
        Some(GameModeContext::Storyline(StorylineContext {
            title: self.script.title.trim().to_string(),
            premise: self.script.premise.trim().to_string(),
            me: format!("{} · {}", self.label(i), role.role.trim()),
            goal: role.goal.trim().to_string(),
            secret: role.secret.as_deref().map(|s| s.trim().to_string()),
            cast: cast.join(" · "),
            now: self.duty(world, i),
            next: self.next_line(world),
            milestones: self.milestone_lines(i),
            today,
            actions,
            sheet: self.rules.as_ref().map(|r| r.sheet_line(i)),
            checks: match &self.rules {
                Some(_) if !self.ended && !self.seats[i].gone => self
                    .open_checks(i)
                    .into_iter()
                    .map(|c| self.check_note(world, i, c))
                    .collect(),
                _ => vec![],
            },
            fight: fight_lines,
        }))
    }

    /// 在座、还在的人都有这件工具，收场就没有了。说明对每个人都一样，好让请求前缀留在缓存里。
    fn tool(&self, _world: &World, actor: &str) -> Option<String> {
        let i = self.seat_of(actor)?;
        if self.ended || self.seats[i].gone {
            return None;
        }
        let base = "故事里交选择。只在观察里 storyline.actions 列出时能用，此刻发生了什么、该做什么看 storyline.now。\
action：speak 在轮到你的那场戏里说话（text 写你要说的话）、pass 这一场不说、\
declare 宣布一个里程碑达成了（target 填里程碑的编号，text 写发生了什么；只有 storyline.milestones 里写着由你宣布的才能交）。\
reason 写为什么这么选（只给旁观的人看）。\
故事只告诉你发生了什么、你想要什么；去哪里、找谁、说什么都由你自己定。";
        // 开着规则套件：多一种动作。说明对每个人都一样；各人能试什么、加值多少写在 storyline.checks 里
        Some(if self.rules.is_some() {
            format!(
                "{base}这个故事开着第五版规则：storyline.sheet 是你的角色卡，storyline.checks 是你此刻能试的检定。\
check 试一项检定（target 填检定的编号；有几种办法时在 with 里填 skill 或 ability 选一种，text 写你具体怎么做）：\
由骰子定成败——掷 d20 加上你这种办法的加值，不低于难度就成（写着对抗的，要掷得比对方高）；\
除了写着可以再试的，每项检定每人只能试一次，所以挑你拿手的办法，或者让更拿手的人去试。\
有优势时掷两次取高的，有劣势时取低的。help 帮一把（target 填检定的编号，with 里填 seat：对方的座号）：\
你自己不试这一项，和你在同一个地方的那个人下一次试它有优势。\
开着规则时不能直接对人推搡、抢、偷，要动手用 duel。\
剧本里打起来的遭遇战按先攻轮流出手：轮到你时 storyline.fight 写着战况和每个人的编号，\
attack 攻击（target 填对手的编号）、dodge 闪避（打你的有劣势）、disengage 撤步（走开不挨借机攻击）、\
dash 冲刺（with 里填 move：要到的区域）、help 帮同伴（target 填座号，他下一次攻击有优势）、flee 退出战斗、pass 不出手；\
cast 施法（with 里填 spell：法术的编号，要用高环的法术位再填 slot；target 填对象的编号，罩一块区域的法术指那块里的一个对手）、\
second_wind 回气（战士）；别的动作要先走一步就在 with 里填 move。近身要和对手在同一块区域；生命归零会倒下昏迷，可能死去。\
不在打的时候：cast 可以施治疗的法术（对象要在你跟前）；duel 和跟前的一个人动手（target 填座号），按同样的规矩打到一方倒下为止。\
rest 短休：受了伤、不在打的时候歇一阵，花一颗生命骰回点生命（一个小时一次，生命骰和等级一样多，用完要睡一觉才回来）。\
在床上睡够一觉，生命和法术位回满。\
attempt 做一件上面没列出来、成不成说不准的事（撬锁、翻墙、哄人、找东西……）：text 里写清楚要做什么、怎么做，\
主持人裁定要不要掷骰、用哪项属性和技能、多难，骰子替你掷，结果出来会告诉你；一次只能有一件在等，一个小时最多三件。"
            )
        } else {
            base.to_string()
        })
    }

    /// 开着规则套件时：对人动手只有决斗（mode_action duel）一条路，直接推搡、抢、偷的都拦下来
    fn judge(&self, _world: &World, _actor: &str, draft: &ActionDraft) -> Result<()> {
        if self.rules.is_some() && !self.ended && crate::force::of_draft(draft).is_some() {
            return Err(bad(
                "这个故事开着第五版规则：对人动手要按规则来，用 mode_action 交 duel（target 填对方的座号），不能直接推搡、抢、偷",
            ));
        }
        Ok(())
    }

    fn takes_params(&self) -> bool {
        self.rules.is_some()
    }

    fn act(&mut self, world: &mut World, actor: &str, action: &GameModeAction) -> Result<String> {
        let i = self.seat_of(actor).ok_or_else(|| bad("你不在这个故事里"))?;
        if self.ended {
            return Err(bad("故事已经收场了"));
        }
        if self.seats[i].gone {
            return Err(bad("你已经不在这个故事里了"));
        }
        // 正打着遭遇战、轮到这个人：交的是战斗里的一回合
        let my_turn = self.fighting()
            && self.fighter_of(i).is_some()
            && self
                .rules
                .as_ref()
                .and_then(|r| r.fight.as_ref())
                .and_then(Fight::up)
                == self.fighter_of(i);
        if my_turn {
            return self.fight_turn(world, i, action);
        }
        match action.action.as_str() {
            "speak" | "pass" => self.speak(world, i, action),
            "declare" => self.declare(world, i, action),
            "check" => self.attempt(world, i, action),
            "help" if self.rules.is_some() => self.help(world, i, action),
            "duel" if self.rules.is_some() => self.begin_duel(world, i, action),
            "cast" if self.rules.is_some() => self.cast_outside(world, i, action),
            "attempt" if self.rules.is_some() => self.improvise(world, i, action),
            "rest" if self.rules.is_some() => self.short_rest(world, i),
            other => Err(bad(format!(
                "故事里没有「{other}」这个动作：能交的是 speak、pass、declare{}",
                if self.rules.is_some() {
                    "、check、help、cast、duel、attempt、rest"
                } else {
                    ""
                }
            ))),
        }
    }
}
