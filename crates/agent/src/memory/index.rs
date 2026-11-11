//! SQLite 是可重建的查询索引；存档中的世界事实和 MemoryStore 才是恢复依据。
//! 两张普通表按对象、时间、种类查；一张 FTS5 表按词查：中文没有空格，入库前自己切词（见 `words`），
//! 查的时候把问的话照样切开、词之间取「或」，用 BM25 排相关度，再和新旧、要紧程度合成一个分（见 `search`）。
use super::{MemoryRecord, MemoryStore, RecallQuery, TEXT_PREVIEW_CHARS};
use crate::{tools::limits, AgentError};
use rusqlite::{params, Connection};
use std::collections::BTreeMap;

/// 索引失败时保留是哪一步出错。
fn sql(step: &str) -> impl Fn(rusqlite::Error) -> AgentError + '_ {
    move |e| AgentError::Memory(format!("记忆索引{step}失败：{e}"))
}

/// 新旧的半衰期（游戏分钟）：两天前的事新旧分减半
const HALF_LIFE: f64 = 2.0 * 1440.0;
/// 合成分里相关度、新旧、要紧程度各占多少
const RELEVANCE_WEIGHT: f64 = 0.5;
const RECENCY_WEIGHT: f64 = 0.2;
const IMPORTANCE_WEIGHT: f64 = 0.3;
/// 按词查时先取这么多条候选再合成分
const CANDIDATES: usize = 64;

fn cjk(c: char) -> bool {
    matches!(c as u32, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x3040..=0x30FF | 0xAC00..=0xD7AF)
}

/// 切词：一串汉字切成单字和相邻两字（「灯塔要卖」→ 灯 塔 要 卖 灯塔 塔要 要卖），字母数字连成一个词并转小写，
/// 别的都当分隔。两个字的词（人名、地名多是两个字）因此查得到；SQLite 自带的 trigram 要三个字才查得到。
/// `query` 时一串两个字以上的汉字只出两字的词（单字太泛），单独一个字才出单字。
pub(crate) fn words(text: &str, query: bool) -> Vec<String> {
    let mut out = Vec::new();
    let mut run: Vec<char> = Vec::new();
    let mut word = String::new();
    let flush_run = |run: &mut Vec<char>, out: &mut Vec<String>| {
        if !query || run.len() == 1 {
            out.extend(run.iter().map(|c| c.to_string()));
        }
        out.extend(run.windows(2).map(|w| w.iter().collect::<String>()));
        run.clear();
    };
    for c in text.chars() {
        if cjk(c) {
            if !word.is_empty() {
                out.push(std::mem::take(&mut word));
            }
            run.push(c);
        } else if c.is_alphanumeric() {
            flush_run(&mut run, &mut out);
            word.extend(c.to_lowercase());
        } else {
            flush_run(&mut run, &mut out);
            if !word.is_empty() {
                out.push(std::mem::take(&mut word));
            }
        }
    }
    flush_run(&mut run, &mut out);
    if !word.is_empty() {
        out.push(word);
    }
    out
}

/// 一条记忆和它的分
pub struct Scored {
    pub record: MemoryRecord,
    pub score: f64,
    /// 0–1：和问的话有多相关
    pub relevance: f64,
}

pub struct MemoryIndex {
    db: Connection,
    revisions: BTreeMap<String, u64>,
    /// 村民 → 序号：FTS 表的行号是「序号 << 32 | 记忆编号」
    actors: BTreeMap<String, i64>,
}
impl MemoryIndex {
    pub fn new() -> Result<Self, AgentError> {
        let db = Connection::open_in_memory().map_err(sql("打开"))?;
        db.execute_batch("CREATE TABLE memories(
                actor TEXT NOT NULL, id INTEGER NOT NULL, kind TEXT NOT NULL, at REAL NOT NULL,
                text TEXT NOT NULL, protected INTEGER NOT NULL, confidence REAL NOT NULL,
                status TEXT NOT NULL, content INTEGER NOT NULL, PRIMARY KEY(actor,id));
            CREATE TABLE objects(actor TEXT NOT NULL,memory_id INTEGER NOT NULL,object TEXT NOT NULL,PRIMARY KEY(actor,memory_id,object));
            CREATE INDEX by_actor_time ON memories(actor,at);
            CREATE INDEX by_actor_kind ON memories(actor,kind);
            CREATE INDEX by_object ON objects(actor,object,memory_id);
            CREATE VIRTUAL TABLE memory_words USING fts5(words, tokenize='unicode61');")
            .map_err(sql("建表"))?;
        Ok(Self {
            db,
            revisions: BTreeMap::new(),
            actors: BTreeMap::new(),
        })
    }

    fn sync(&mut self, actor: &str, store: &MemoryStore) -> Result<(), AgentError> {
        let previous = self.revisions.get(actor).copied().unwrap_or(0);
        if previous == store.revision {
            return Ok(());
        }
        let next = self.actors.len() as i64 + 1;
        let ordinal = *self.actors.entry(actor.into()).or_insert(next);
        let tx = self.db.transaction().map_err(sql("开启事务"))?;
        for record in store.records.iter().filter(|r| r.revision > previous) {
            let row = ordinal << 32 | record.memory_id as i64;
            tx.execute("DELETE FROM memory_words WHERE rowid=?1", params![row])
                .map_err(sql("清理词"))?;
            tx.execute(
                "INSERT INTO memory_words(rowid, words) VALUES (?1,?2)",
                params![row, words(&record.text, false).join(" ")],
            )
            .map_err(sql("写入词"))?;
            let kind = crate::labels::wire_name(&record.kind);
            let status = crate::labels::wire_name(&record.status);
            tx.execute(
                "INSERT OR REPLACE INTO memories VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![
                    actor,
                    record.memory_id,
                    kind,
                    record.at,
                    record.text,
                    record.protected,
                    record.confidence,
                    status,
                    record.has_content()
                ],
            )
            .map_err(sql("写入"))?;
            tx.execute(
                "DELETE FROM objects WHERE actor=?1 AND memory_id=?2",
                params![actor, record.memory_id],
            )
            .map_err(sql("清理对象"))?;
            for object in &record.objects {
                tx.execute(
                    "INSERT OR IGNORE INTO objects VALUES (?1,?2,?3)",
                    params![actor, record.memory_id, object],
                )
                .map_err(sql("写入对象"))?;
            }
        }
        tx.commit().map_err(sql("提交"))?;
        self.revisions.insert(actor.into(), store.revision);
        Ok(())
    }

    pub fn recall(
        &mut self,
        actor: &str,
        store: &MemoryStore,
        query: &RecallQuery,
    ) -> Result<Vec<MemoryRecord>, AgentError> {
        if store.records.iter().any(|record| record.actor_id != actor) {
            return Err(AgentError::Query("不能检索其他村民的私有记忆".into()));
        }
        if query
            .since
            .into_iter()
            .chain(query.until)
            .any(|at| !at.is_finite() || at < 0.0)
            || query
                .text
                .as_ref()
                .is_some_and(|t| t.chars().count() > limits::QUERY_CHARS)
            || query
                .object
                .as_ref()
                .is_some_and(|t| t.len() > limits::RECALL_OBJECT_BYTES)
        {
            return Err(AgentError::Query("记忆查询的时间或文本超出范围".into()));
        }
        self.sync(actor, store)?;
        let limit = query.effective_limit();
        // 带了要找的话：先按词查、按合成分排（「此刻」取最新一条记忆的时刻）；词上一条都对不上时，
        // 下面照原样按整段文字包含来找
        if let Some(text) = query.text.as_deref().filter(|t| !t.trim().is_empty()) {
            let now = store.records.iter().map(|r| r.at).fold(0.0, f64::max);
            let found: Vec<MemoryRecord> = self
                .search(actor, store, text, now, CANDIDATES)?
                .into_iter()
                .map(|s| s.record)
                .filter(|r| {
                    query.kind.is_none_or(|k| r.kind == k)
                        && query.since.is_none_or(|at| r.at >= at)
                        && query.until.is_none_or(|at| r.at <= at)
                        && query.object.as_ref().is_none_or(|o| r.objects.contains(o))
                })
                .take(limit)
                .collect();
            if !found.is_empty() {
                return Ok(found);
            }
        }
        let kind = query.kind.map(|k| crate::labels::wire_name(&k));
        let mut statement = self.db.prepare("SELECT m.id FROM memories m WHERE actor=?1
            AND length(trim(text)) > 0 AND status != 'superseded' AND (?2 IS NULL OR instr(lower(text),lower(?2)) > 0)
            AND (?3 IS NULL OR kind=?3) AND (?4 IS NULL OR at>=?4) AND (?5 IS NULL OR at<=?5)
            AND (?6 IS NULL OR EXISTS(SELECT 1 FROM objects o WHERE o.actor=m.actor AND o.memory_id=m.id AND o.object=?6))
            ORDER BY protected DESC, content DESC, confidence DESC, at DESC, id DESC LIMIT ?7").map_err(sql("准备查询"))?;
        let ids = statement
            .query_map(
                params![
                    actor,
                    query.text,
                    kind,
                    query.since,
                    query.until,
                    query.object,
                    // 同一件事的多条记录只保留一条，多取几倍再去重。
                    (limit * 4) as i64
                ],
                |row| row.get::<_, u64>(0),
            )
            .map_err(sql("查询"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql("读取结果"))?;
        let mut result = Vec::new();
        for record in super::distinct_records(
            ids.iter().map(|id| &store.records[(*id - 1) as usize]),
            limit,
        ) {
            let mut record = record.clone();
            record.text = record.text.chars().take(TEXT_PREVIEW_CHARS).collect();
            result.push(record);
        }
        Ok(result)
    }

    /// 按词查：和 `text` 相关的记忆，按「相关度 + 新旧 + 要紧程度」的合成分从高到低。
    /// 不带已经修正掉的；问的话切不出词时什么都不返回。
    pub fn search(
        &mut self,
        actor: &str,
        store: &MemoryStore,
        text: &str,
        now: f64,
        limit: usize,
    ) -> Result<Vec<Scored>, AgentError> {
        if store.records.iter().any(|record| record.actor_id != actor) {
            return Err(AgentError::Query("不能检索其他村民的私有记忆".into()));
        }
        self.sync(actor, store)?;
        let mut terms = words(text, true);
        terms.sort();
        terms.dedup();
        // 问得太长时只留前面一批词，免得一条查询拖太久
        terms.truncate(48);
        let Some(&ordinal) = self.actors.get(actor) else {
            return Ok(vec![]);
        };
        if terms.is_empty() {
            return Ok(vec![]);
        }
        let matching = terms
            .iter()
            .map(|t| format!("\"{t}\""))
            .collect::<Vec<_>>()
            .join(" OR ");
        let mut statement = self
            .db
            .prepare(
                "SELECT rowid, -bm25(memory_words) FROM memory_words
                 WHERE memory_words MATCH ?1 AND rowid >= ?2 AND rowid < ?3
                 ORDER BY bm25(memory_words) LIMIT ?4",
            )
            .map_err(sql("准备按词查询"))?;
        let hits = statement
            .query_map(
                params![
                    matching,
                    ordinal << 32,
                    (ordinal + 1) << 32,
                    CANDIDATES as i64
                ],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?)),
            )
            .map_err(sql("按词查询"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql("读取结果"))?;
        let best = hits.iter().map(|(_, bm)| *bm).fold(0.0_f64, f64::max);
        let mut scored: Vec<Scored> = hits
            .into_iter()
            .filter_map(|(row, bm)| {
                let record = store.records.get(((row & 0xffff_ffff) - 1) as usize)?;
                if record.status == super::MemoryStatus::Superseded || record.text.trim().is_empty()
                {
                    return None;
                }
                let relevance = if best > 0.0 {
                    (bm / best).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let score = RELEVANCE_WEIGHT * relevance
                    + RECENCY_WEIGHT * recency(record.at, now)
                    + IMPORTANCE_WEIGHT * record.importance();
                Some(Scored {
                    record: record.clone(),
                    score,
                    relevance,
                })
            })
            .collect();
        scored.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| b.record.memory_id.cmp(&a.record.memory_id))
        });
        let keep: std::collections::BTreeSet<u64> =
            super::distinct_records(scored.iter().map(|s| &s.record), limit)
                .into_iter()
                .map(|r| r.memory_id)
                .collect();
        scored.retain(|s| keep.contains(&s.record.memory_id));
        for s in &mut scored {
            s.record.text = s.record.text.chars().take(TEXT_PREVIEW_CHARS).collect();
        }
        Ok(scored)
    }
}

/// 新旧分：刚发生的是 1，每过一个半衰期减半
pub(crate) fn recency(at: f64, now: f64) -> f64 {
    0.5_f64.powf((now - at).max(0.0) / HALF_LIFE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{MemoryKind, MemoryStatus};
    use protocol::FactKind;

    fn store(items: &[(MemoryKind, f64, &str, Option<FactKind>, bool)]) -> MemoryStore {
        let mut store = MemoryStore::default();
        for (i, (kind, at, text, fact, protected)) in items.iter().enumerate() {
            let mut r = MemoryRecord::blank("r1", *kind, format!("k{i}"));
            r.memory_id = i as u64 + 1;
            r.event_ids = vec![i as u64 + 1];
            r.at = *at;
            r.text = (*text).to_string();
            r.fact_kind = *fact;
            r.protected = *protected;
            store.revision += 1;
            r.revision = store.revision;
            store.records.push(r);
        }
        store
    }

    /// 按词查：两个字的中文词查得到（人名、地名多是两个字）；相关的旧事排在不相关的新事前面；
    /// 一样相关时新的在前；要紧的旧经验没被新见闻挤掉；修正掉的不出来；别人的记忆查不了。
    #[test]
    fn word_search_ranks_by_relevance_recency_and_importance() {
        use MemoryKind::*;
        let mut s = store(&[
            (
                Episode,
                100.0,
                "在码头酒馆听老莫说，镇长去年来赊过好几回账",
                Some(FactKind::Speech),
                false,
            ),
            (Episode, 5000.0, "去集市买了面包", None, false),
            (
                Episode,
                5100.0,
                "广场上贴了公告：老灯塔要卖给外乡人",
                Some(FactKind::Notice),
                false,
            ),
            (
                Experience,
                200.0,
                "再次做「修灯」前，应先检查相关条件。失败证据：没有灯油。",
                None,
                true,
            ),
            (Episode, 5200.0, "镇长今天没露面", None, false),
            (Episode, 300.0, "路过集市", None, false),
            (Episode, 5250.0, "路过集市", None, false),
        ]);
        s.records[4].status = MemoryStatus::Superseded;
        let now = 5300.0;
        let mut index = MemoryIndex::new().unwrap();
        let ids = |found: Vec<Scored>| found.iter().map(|f| f.record.memory_id).collect::<Vec<_>>();

        assert_eq!(
            words("镇长说Hello 2次", false),
            ["镇", "长", "说", "镇长", "长说", "hello", "2", "次"]
        );
        assert_eq!(words("镇长 灯", true), ["镇长", "灯"]);

        // 两个字的词：只有那条旧的对得上（修正掉的那条不出来）
        assert_eq!(ids(index.search("r1", &s, "镇长", now, 6).unwrap()), [1]);
        // 相关的旧事排在只沾一点边的新事前面
        let found = ids(index.search("r1", &s, "镇长赊账 集市", now, 6).unwrap());
        assert_eq!(found[0], 1, "{found:?}");
        // 一样的话，新的在前
        assert_eq!(
            ids(index.search("r1", &s, "路过集市", now, 2).unwrap()),
            [7, 6]
        );
        // 要紧的旧经验还在前两条里
        let found = ids(index.search("r1", &s, "灯", now, 2).unwrap());
        assert!(found.contains(&4), "{found:?}");
        // 问的话切不出词：什么都没有
        assert!(index.search("r1", &s, "，。！", now, 6).unwrap().is_empty());

        // recall 带了要找的话就按词查、按分排；记忆变了索引跟着变
        let query = RecallQuery {
            text: Some("灯塔".into()),
            ..Default::default()
        };
        assert_eq!(index.recall("r1", &s, &query).unwrap()[0].memory_id, 3);
        s.records[2].status = MemoryStatus::Superseded;
        s.revision += 1;
        s.records[2].revision = s.revision;
        assert!(index.recall("r1", &s, &query).unwrap().is_empty());
        assert!(index.search("r2", &s, "灯塔", now, 6).is_err());
    }
}
