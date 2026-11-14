//! 记录：世界进程把说过的话、想法、世界里发生的事顺手写进小镇目录，
//! 玩家事后能整份下载（管理服务的 `/api/towns/<小镇>/transcript` 照着这里的文件发）。
//!
//! 目录 `transcript/` 里按种类各一个文件，一直往后追加，每行一条 JSON（Agent 的轨迹不在这里另抄一份，见下）：
//! - `chat.jsonl`：台词（私聊里的一句）和玩法里的发言（议事、演说……）；
//! - `thought.jsonl`：村民每次做决定时的想法和它的来由；
//! - `event.jsonl`：世界日志（走动、活动、检定、战斗……），不含模型请求的台账行；
//!
//! 下载时的第四种「trace」是村民 Agent 的轨迹——发给模型的请求、模型的回复、工具调用和结果。它由
//! agent_trace.rs 写在小镇目录的 `logs/` 里，有大小上限，记不记完整题面看设定里的「日志」；这里只是读出来
//! （最多 TRACE_DOWNLOAD_BYTES，超了给最新的），每行是 `{"trace":…, "model_snapshot":…}`，没有下面那几项。
//!
//! 前三种每行都带 `kind`、现实时间 `at`（毫秒）、世界编号 `world`、世界里的分钟 `minutes`；重置世界后 `world` 会换。
//! 台词和想法从名册里新出现的那一条记下，和页面的动态同一个办法；进程刚起来时名册上已有的不补记。
//! 世界不存档（没有小镇目录）时不记。写不进去只记一次日志，世界照常跑。
use protocol::{RosterMember, ServerMsg};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// 小镇目录下放记录的子目录
pub const TRANSCRIPT_DIR: &str = "transcript";
/// 记录的种类；前三种也是 transcript/ 里的文件名（加 `.jsonl`），trace 读的是 logs/ 里的轨迹日志
pub const KINDS: [&str; 4] = ["chat", "thought", "event", "trace"];
/// 小镇目录下放轨迹日志的子目录（管理服务开世界进程时用 AI_TOWN_LOG_DIR 指到这里）
pub const LOGS_DIR: &str = "logs";
/// 一次下载最多给这么多轨迹
pub const TRACE_DOWNLOAD_BYTES: u64 = 64 << 20;

/// 记录本：模拟循环、广播和村民驱动共用的一份。
#[derive(Clone, Default)]
pub struct Transcript(Arc<Mutex<Option<Book>>>);

struct Book {
    dir: PathBuf,
    world: String,
    files: BTreeMap<&'static str, BufWriter<File>>,
    /// 每个人上一句记下的台词（哪场交谈的第几句）
    said: BTreeMap<String, (String, u64)>,
    /// 每个人上一条记下的想法（哪一刻的哪一段）
    thought: BTreeMap<String, (u64, String)>,
    /// 见过名册了：第一份名册只用来记住「已经有的」，不补记
    primed: bool,
    failed: bool,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

impl Book {
    fn write(&mut self, kind: &'static str, mut line: Value) {
        line["kind"] = json!(kind);
        line["at"] = json!(now_ms());
        line["world"] = json!(self.world);
        let result = (|| -> std::io::Result<()> {
            if !self.files.contains_key(kind) {
                fs::create_dir_all(&self.dir)?;
                let file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(self.dir.join(format!("{kind}.jsonl")))?;
                self.files.insert(kind, BufWriter::new(file));
            }
            let Some(out) = self.files.get_mut(kind) else {
                return Ok(());
            };
            serde_json::to_writer(&mut *out, &line)?;
            out.write_all(b"\n")?;
            out.flush()
        })();
        if let Err(error) = result {
            if !self.failed {
                eprintln!("[host] 记录写不进 {}：{error}", self.dir.display());
            }
            self.failed = true;
        }
    }

    fn member(&mut self, m: &RosterMember, minutes: f64, clock: &str) {
        if let Some(s) = &m.speech {
            let key = (s.chat_id.clone(), s.seq);
            if self.said.get(&m.id) != Some(&key) {
                self.said.insert(m.id.clone(), key);
                if self.primed {
                    // 交谈里的另外几个人（名册里写的是名字）
                    let with: Vec<&String> = m
                        .chat
                        .iter()
                        .flat_map(|c| &c.participants)
                        .filter(|who| **who != m.name && **who != m.id)
                        .collect();
                    self.write(
                        "chat",
                        json!({
                            "minutes": s.at, "clock": clock, "id": m.id, "name": m.name,
                            "chat_id": s.chat_id, "seq": s.seq, "with": with, "text": s.text,
                        }),
                    );
                }
            }
        }
        if let Some(t) = &m.thought {
            if t.text.trim().is_empty() {
                return;
            }
            let key = (t.sim_minutes.to_bits(), t.text.clone());
            if self.thought.get(&m.id) != Some(&key) {
                self.thought.insert(m.id.clone(), key);
                if self.primed {
                    self.write(
                        "thought",
                        json!({
                            "minutes": if t.sim_minutes > 0.0 { t.sim_minutes } else { minutes },
                            "clock": t.clock, "id": m.id, "name": m.name,
                            "trigger": t.trigger_info, "text": t.text,
                        }),
                    );
                }
            }
        }
    }
}

impl Transcript {
    /// 开始往这个小镇目录记；world 是此刻的世界编号。
    pub fn open(&self, town_dir: &Path, world: &str) {
        *self.0.lock().unwrap() = Some(Book {
            dir: town_dir.join(TRANSCRIPT_DIR),
            world: world.to_string(),
            files: BTreeMap::new(),
            said: BTreeMap::new(),
            thought: BTreeMap::new(),
            primed: false,
            failed: false,
        });
    }

    /// 世界重置了：之后的行记在新的世界编号下，旧世界名册上的话不算新的。
    pub fn rebind(&self, world: &str) {
        if let Some(book) = self.0.lock().unwrap().as_mut() {
            book.world = world.to_string();
            book.said.clear();
            book.thought.clear();
            book.primed = false;
        }
    }

    /// 每条广播过一遍：世界日志记一行（带「一段话」的另记进台词），名册里新出现的台词和想法各记一行。
    pub fn note(&self, msg: &ServerMsg) {
        let mut guard = self.0.lock().unwrap();
        let Some(book) = guard.as_mut() else {
            return;
        };
        match msg {
            ServerMsg::Event {
                sim_minutes,
                clock,
                text,
                info,
            } => {
                if info.kind == protocol::EventKind::Model {
                    return;
                }
                let actor = info.actor.as_ref();
                if let Some(speech) = &info.speech {
                    book.write(
                        "chat",
                        json!({
                            "minutes": sim_minutes, "clock": clock,
                            "id": actor.map(|a| &a.id), "name": actor.map(|a| &a.name),
                            "occasion": speech.occasion, "seat": speech.seat, "text": speech.text,
                        }),
                    );
                }
                book.write(
                    "event",
                    json!({
                        "minutes": sim_minutes, "clock": clock, "text": text,
                        "event": info.kind, "id": actor.map(|a| &a.id), "name": actor.map(|a| &a.name),
                    }),
                );
            }
            ServerMsg::Roster {
                sim_minutes,
                clock,
                locales,
                ..
            } => {
                for m in locales.iter().flat_map(|l| &l.members) {
                    book.member(m, *sim_minutes, clock);
                }
                book.primed = true;
            }
            _ => {}
        }
    }
}

/// 管理服务发下载用：小镇目录里选中的几种记录，每种是它那个文件的原文（每行一条 JSON）。
/// 没记过的种类给空串；wanted 为空是全部，里面不认得的种类不理。
pub fn read(town_dir: &Path, wanted: &[&str]) -> std::io::Result<Value> {
    let mut kinds = serde_json::Map::new();
    for kind in KINDS {
        if !wanted.is_empty() && !wanted.contains(&kind) {
            continue;
        }
        if kind == "trace" {
            let text = crate::agent_trace::recent(&town_dir.join(LOGS_DIR), TRACE_DOWNLOAD_BYTES)?;
            kinds.insert(kind.into(), json!(text));
            continue;
        }
        let path = town_dir.join(TRANSCRIPT_DIR).join(format!("{kind}.jsonl"));
        let text = match fs::read(path) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error),
        };
        kinds.insert(kind.into(), json!(text));
    }
    Ok(json!({ "kinds": kinds }))
}
