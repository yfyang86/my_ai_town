//! 做梦：村民睡了一觉醒来，把睡前这段日子的记忆理一遍。
//! 不用模型也做的一步（`settle`）：很久以前、已经整理过的琐碎见闻不再出现在检索里（只改状态，不删）。
//! 连着模型时再做一步：把这段日子里要紧的记忆交给模型（走整理轮次，工具只有 dream），它交回
//! 几条提炼出来的认识（每条指明来自哪几条记忆）、可以放下的记忆、和几句梦。认识记成 `Insight`，
//! 要紧程度最高，以后按词、按对象都查得到；梦按天留着，给玩家看。一觉只做一次梦。
use super::{MemoryKind, MemoryRecord, MemoryStatus, MemoryStore};
use protocol::EvidenceSource;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// 睡够这么久（游戏分钟）才算一觉
pub const MIN_SLEEP_MINUTES: f64 = 120.0;
/// 上次做梦以后新记忆不到这么多条就不做梦
pub const MIN_MATERIAL: usize = 6;
/// 交给模型的记忆最多这么多条
pub const MATERIAL_MAX: usize = 16;
/// 没内容的琐碎见闻过了这么久（游戏分钟）、又已经整理过，就不再出现在检索里
const STALE_AFTER: f64 = 3.0 * 1440.0;
const STALE_IMPORTANCE: f64 = 0.2;
/// 留着的梦最多这么多天
const DREAMS_KEPT: usize = 30;
pub const INSIGHTS_MAX: usize = 4;
pub const INSIGHT_CHARS: usize = 200;
pub const INSIGHT_EVIDENCE_MAX: usize = 6;
pub const LET_GO_MAX: usize = 12;
pub const DREAM_CHARS: usize = 300;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DreamState {
    /// 睡下的时刻；醒着时没有
    #[serde(default)]
    pub asleep_since: Option<f64>,
    /// 睡醒了，等着做这一觉的梦
    #[serde(default)]
    pub pending: bool,
    /// 上次做梦时最新的记忆编号：之后的才是下一次的材料
    #[serde(default)]
    pub through_memory: u64,
    #[serde(default)]
    pub last_at: f64,
    /// 最近的梦，按先后
    #[serde(default)]
    pub dreams: Vec<DreamNote>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DreamNote {
    /// 哪一天醒来时做的（从 0 数）
    pub day: i64,
    pub at: f64,
    pub text: String,
    /// 这一觉提炼出的认识（记忆编号）
    #[serde(default)]
    pub insights: Vec<u64>,
    /// 这一觉放下的记忆有几条
    #[serde(default)]
    pub let_go: u64,
}

/// 模型用 dream 工具交回来的
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DreamUpdate {
    #[serde(default)]
    pub insights: Vec<DreamInsight>,
    #[serde(default)]
    pub let_go: Vec<u64>,
    #[serde(default)]
    pub dream: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DreamInsight {
    pub text: String,
    pub memory_ids: Vec<u64>,
}

impl DreamState {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// 存档里没做过梦、也没睡过的不写这一项
pub(super) fn untouched(state: &DreamState) -> bool {
    state.is_default()
}

impl MemoryStore {
    /// 每一拍告诉它村民此刻睡没睡着。刚醒、而且睡够了一觉时返回 true：先做不用模型的那一步，
    /// 再记下「该做梦了」。`reflected_through` 是整理轮次已经处理到的事件编号。
    pub fn note_sleep(&mut self, asleep: bool, now: f64, reflected_through: u64) -> bool {
        match (asleep, self.dream.asleep_since) {
            (true, None) => {
                self.dream.asleep_since = Some(now);
                false
            }
            (false, Some(since)) => {
                self.dream.asleep_since = None;
                if now - since < MIN_SLEEP_MINUTES {
                    return false;
                }
                self.settle(now, reflected_through);
                self.dream.pending = true;
                true
            }
            _ => false,
        }
    }

    /// 不用模型的那一步：没内容、不要紧、很久以前而且已经整理过的见闻，不再出现在检索里。
    /// 返回放下了几条。
    pub fn settle(&mut self, now: f64, reflected_through: u64) -> usize {
        self.retire(|r| {
            r.kind == MemoryKind::Episode
                && r.status == MemoryStatus::Active
                && !r.protected
                && now - r.at > STALE_AFTER
                && r.importance() <= STALE_IMPORTANCE
                && r.event_ids.iter().all(|id| *id <= reflected_through)
        })
        .len()
    }

    /// 这一次做梦的材料：上次做梦以后的记忆里最要紧的几条，按先后排。
    pub fn dream_material(&self) -> Vec<&MemoryRecord> {
        let mut fresh: Vec<&MemoryRecord> = self
            .records
            .iter()
            .filter(|r| {
                r.memory_id > self.dream.through_memory
                    && r.status != MemoryStatus::Superseded
                    && r.kind != MemoryKind::Insight
                    && !r.text.trim().is_empty()
            })
            .collect();
        fresh.sort_by(|a, b| {
            b.importance()
                .total_cmp(&a.importance())
                .then_with(|| b.at.total_cmp(&a.at))
        });
        fresh.truncate(MATERIAL_MAX);
        fresh.sort_by_key(|r| r.memory_id);
        fresh
    }

    /// 该不该做梦：睡醒了，而且新记忆够多。不够就把这一觉的梦免了。
    pub fn dream_due(&mut self) -> bool {
        if !self.dream.pending {
            return false;
        }
        if self.dream_material().len() < MIN_MATERIAL {
            self.skip_dream();
            return false;
        }
        true
    }

    /// 这一觉不做梦了（材料不够、模型没交、放不进窗口）：下一觉再说。
    pub fn skip_dream(&mut self) {
        self.dream.pending = false;
    }

    /// 模型交回来的梦：认识记成新的记忆（来源是它指的那几条材料），放下的改成已修正，梦留着。
    /// `material` 是这一次交给模型的记忆编号，指到别处的不算。
    pub fn apply_dream(
        &mut self,
        actor: &str,
        now: f64,
        day: i64,
        update: DreamUpdate,
        material: &[u64],
    ) -> Result<DreamNote, String> {
        if !self.dream.pending {
            return Err("现在没有在做梦".into());
        }
        let allowed: BTreeSet<u64> = material.iter().copied().collect();
        let mut insights = Vec::new();
        for insight in update.insights.into_iter().take(INSIGHTS_MAX) {
            let text: String = insight.text.trim().chars().take(INSIGHT_CHARS).collect();
            let evidence: Vec<&MemoryRecord> = insight
                .memory_ids
                .iter()
                .filter(|id| allowed.contains(id))
                .filter_map(|id| self.records.get((*id - 1) as usize))
                .take(INSIGHT_EVIDENCE_MAX)
                .collect();
            if text.is_empty() || evidence.is_empty() {
                continue;
            }
            let mut event_ids: Vec<u64> =
                evidence.iter().flat_map(|r| r.event_ids.clone()).collect();
            event_ids.sort_unstable();
            event_ids.dedup();
            let mut objects: Vec<String> =
                evidence.iter().flat_map(|r| r.objects.clone()).collect();
            objects.sort();
            objects.dedup();
            let record = MemoryRecord {
                event_ids,
                at: now,
                objects,
                source: EvidenceSource::Inferred,
                confidence: 0.7,
                text,
                ..MemoryRecord::blank(
                    actor,
                    MemoryKind::Insight,
                    format!("insight:{}", self.records.len() + 1),
                )
            };
            self.push(record);
            insights.push(self.records.len() as u64);
        }
        let gone: BTreeSet<u64> = update
            .let_go
            .into_iter()
            .filter(|id| allowed.contains(id))
            .take(LET_GO_MAX)
            .collect();
        let let_go = self
            .retire(|r| {
                gone.contains(&r.memory_id)
                    && r.kind == MemoryKind::Episode
                    && r.status == MemoryStatus::Active
                    && !r.protected
            })
            .len() as u64;
        let note = DreamNote {
            day,
            at: now,
            text: update.dream.trim().chars().take(DREAM_CHARS).collect(),
            insights,
            let_go,
        };
        self.dream.pending = false;
        self.dream.last_at = now;
        self.dream.through_memory = self.records.len() as u64;
        self.dream.dreams.push(note.clone());
        let extra = self.dream.dreams.len().saturating_sub(DREAMS_KEPT);
        self.dream.dreams.drain(..extra);
        // 记忆没变时也让存档知道梦变了
        self.revision += 1;
        Ok(note)
    }
}
