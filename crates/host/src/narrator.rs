//! 主持人的模型调用（开着第五版规则的故事里的临场裁定）：内核记下谁想做什么，
//! 这里替它问一次模型，把裁定交回去；题面和回复的读法在 agent::narrator，合不合规矩内核再查。
//! 一个线程、有上限的队列：裁定不抢村民请求的池子。
use crate::llm::{chat_completion, LlmTarget};
use crate::tasks::TaskPool;
use agent::completion::ModelError;
use protocol::{Ruling, RulingRequest};
use std::collections::BTreeSet;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};

pub type Ruled = (String, Result<Ruling, ModelError>);

pub struct Narrator {
    tasks: TaskPool,
    finished: SyncSender<Ruled>,
    results: Receiver<Ruled>,
    /// 已经发出去、还没交回内核的事
    sent: BTreeSet<String>,
}

impl Default for Narrator {
    fn default() -> Self {
        let (finished, results) = sync_channel(16);
        Self {
            tasks: TaskPool::new(1, 8),
            finished,
            results,
            sent: BTreeSet::new(),
        }
    }
}

impl Narrator {
    /// 这件事还没问过就问一次；target 为 None 时用固定的裁定（离线接线验收）。
    /// 队列满了这一拍不问，下一拍再来。
    pub fn ask(&mut self, request: RulingRequest, target: Option<LlmTarget>) {
        if self.sent.contains(&request.id) {
            return;
        }
        let id = request.id.clone();
        let finished = self.finished.clone();
        let job = move || {
            let reply = match &target {
                None => Ok(agent::narrator::mock_reply(&request)),
                Some(target) => chat_completion(target, &agent::narrator::prompt(&request)),
            };
            let ruling =
                reply.and_then(|text| agent::narrator::parse(&text).map_err(ModelError::Reply));
            // 主循环已经退出时没有人收结果，丢掉即可。
            let _ = finished.send((request.id, ruling));
        };
        if self.tasks.submit(job).is_ok() {
            self.sent.insert(id);
        }
    }

    /// 裁好了的事，交回内核
    pub fn finished(&mut self) -> Vec<Ruled> {
        let done: Vec<Ruled> = self.results.try_iter().collect();
        for (id, _) in &done {
            self.sent.remove(id);
        }
        done
    }
}
