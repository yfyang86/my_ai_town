//! 故事模式：整本剧本是开局时带进来的数据——角色（公开的身份、只有本人知道的目标和秘密）、
//! 到点发生的节拍（对所有人宣布一件事，单独叮嘱几个人，可以再开一场轮流说话的戏）、要追踪的里程碑。
//! 这里只管三件事：到点把节拍说出来、戏里按次序一次叫醒一个人、记下里程碑达没达成。
//! 不替谁做决定：没有人被挪动，去不去、说什么、宣不宣布都由角色自己定，故事可以走偏。
//! 里程碑有三种算法：某个节拍发生了；几个人同时在某个地点；有人用 declare 宣布它发生了并写上经过。
//! 到了收场的时刻，把达成和没达成的里程碑说一遍，一局结束。
//! 和执剑人之夜一样：只叫醒该做事的人，别人的话写进每个人的经历；期限按游戏分钟，挂在 boundary 上。
use super::Rules;
use crate::error::{KernelError, Result};
use crate::{World, DAY};
use protocol::{
    EventInfo, EventSpeech, FactKind, GameModeAction, GameModeContext, GameModeView,
    StorylineBeatState, StorylineBeatView, StorylineContext, StorylineLine, StorylineMilestoneView,
    StorylineScript, StorylineSeat, StorylineView, StorylineWhen,
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
const REASON_CHARS: usize = 120;
/// 有「几个人同时在某地」的里程碑没达成时，隔这么久看一次
const WATCH_MINUTES: f64 = 5.0;
/// 节拍之间世界按正常速度的几倍走；戏里照常速
const FREE_TEMPO: f64 = 3.0;
/// 给模型看的戏里的话最多留多少段
const RECENT_LINES: usize = 20;
const EPS: f64 = 1e-6;

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
fn check(script: &StorylineScript) -> Result<()> {
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
}

impl Storyline {
    /// 按此刻在镇上的村民排座次（按编号，也就是入住的先后），人数要够剧本里的角色。
    /// 第一个节拍今天还来得及就从今天算第 1 天，否则从明天算。
    pub(super) fn open(world: &World, seed: u32, script: &StorylineScript) -> Result<Self> {
        check(script)?;
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

    // ---------- 节拍和戏 ----------

    /// 节拍到点：所有人听到宣布，和它有关的人被叫醒、听到叮嘱；有戏就开戏。
    fn fire(&mut self, world: &mut World, i: usize) {
        self.next = i + 1;
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
            .filter(|m| !m.secret)
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
        if now < self.end_at() {
            return false;
        }
        self.finish(world);
        true
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
            if let Some(scene) = &mut self.scene {
                if scene.turn == Some(i) {
                    scene.turn = None;
                    scene.until = scene.until.min(world.minutes());
                }
            }
        }
    }

    // ---------- 给模型 ----------

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
            .filter(|m| !m.secret || mine.contains(m.id.as_str()))
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
            Some(scene) => scene.until,
            None if self.next < self.script.beats.len() => self.beat_at(self.next),
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
        if self.scene.is_some() || self.ended {
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
        }
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
        let mut actions = Vec::new();
        if !self.ended && !self.seats[i].gone {
            if self.scene.as_ref().is_some_and(|s| s.turn == Some(i)) {
                actions.extend(["speak".to_string(), "pass".to_string()]);
            }
            if !self.declarable(i).is_empty() {
                actions.push("declare".into());
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
        }))
    }

    /// 在座、还在的人都有这件工具，收场就没有了。说明对每个人都一样，好让请求前缀留在缓存里。
    fn tool(&self, _world: &World, actor: &str) -> Option<String> {
        let i = self.seat_of(actor)?;
        if self.ended || self.seats[i].gone {
            return None;
        }
        Some(
            "故事里交选择。只在观察里 storyline.actions 列出时能用，此刻发生了什么、该做什么看 storyline.now。\
action：speak 在轮到你的那场戏里说话（text 写你要说的话）、pass 这一场不说、\
declare 宣布一个里程碑达成了（target 填里程碑的编号，text 写发生了什么；只有 storyline.milestones 里写着由你宣布的才能交）。\
reason 写为什么这么选（只给旁观的人看）。\
故事只告诉你发生了什么、你想要什么；去哪里、找谁、说什么都由你自己定。"
                .to_string(),
        )
    }

    fn act(&mut self, world: &mut World, actor: &str, action: &GameModeAction) -> Result<String> {
        let i = self.seat_of(actor).ok_or_else(|| bad("你不在这个故事里"))?;
        if self.ended {
            return Err(bad("故事已经收场了"));
        }
        if self.seats[i].gone {
            return Err(bad("你已经不在这个故事里了"));
        }
        match action.action.as_str() {
            "speak" | "pass" => self.speak(world, i, action),
            "declare" => self.declare(world, i, action),
            other => Err(bad(format!(
                "故事里没有「{other}」这个动作：能交的是 speak、pass、declare"
            ))),
        }
    }
}
