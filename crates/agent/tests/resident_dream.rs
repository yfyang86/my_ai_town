use crate::support::*;
use agent::budget::Limits;
use agent::memory::{MemoryIndex, MemoryKind, MemoryStatus, MemoryStore};
use agent::runtime::{AgentRequest, ResidentRuntime};
use kernel::World;
use serde_json::json;

fn is_dream(q: &AgentRequest) -> bool {
    q.messages.iter().any(|m| m.content.contains("dream_task"))
}

/// 在床上睡一觉（床是这个测试自己放的），睡的时候照常轮询，不该有请求；minutes 后叫醒。
fn sleep(w: &mut World, r: &mut ResidentRuntime, id: &str, minutes: f64) {
    w.run_actions(
        "r1",
        id,
        w.actor("r1").unwrap().actions.revision,
        &serde_json::from_value::<Vec<protocol::ActionDraft>>(
            json!([{"intent":{"verb":"睡觉"},"prop_id":"bed"}]),
        )
        .unwrap(),
    )
    .unwrap();
    for _ in 0..(minutes / 10.0) as usize {
        w.step(10.0);
        assert!(!w.awake("r1"), "应该还睡着");
        assert!(r.poll(w, true).is_empty(), "睡着时不发请求");
    }
    w.interrupt_actions("r1", "睡醒了");
    w.step(1.0);
    assert!(w.awake("r1"));
}

/// 睡醒后轮到做梦的那个请求：醒来时有通知要先看（重要通知先响应，和整理轮次一个规矩），看过才轮到做梦。
fn dream_request(r: &mut ResidentRuntime, w: &mut World) -> Option<AgentRequest> {
    for _ in 0..3 {
        let q = next(r, w);
        if is_dream(&q) {
            return Some(q);
        }
        reply(r, w, &q, "observe", json!({}));
    }
    None
}

/// 下一个不是整理、也不是做梦的请求之前，把途中的整理轮次回掉；遇到做梦的请求就交回去。
fn next(r: &mut ResidentRuntime, w: &mut World) -> AgentRequest {
    loop {
        let q = request(r, w);
        if is_dream(&q) {
            return q;
        }
        let through = q.messages.iter().find_map(|m| {
            serde_json::from_str::<serde_json::Value>(&m.content)
                .ok()
                .and_then(|v| v["reflection_task"]["through"].as_u64())
        });
        let Some(through) = through else {
            return q;
        };
        reply(
            r,
            w,
            &q,
            "reflect",
            json!({"through_event":through,"summary":"整理完毕。","notes":[]}),
        );
    }
}

/// 睡一觉醒来做一次梦：请求里只有 dream 一件工具，材料是带编号的记忆；模型交回的认识记成新的记忆
/// （来源是它指的那几条，指到材料外面的不算）、以后按词查得到，放下的不再出现，梦留着；
/// 再睡一觉没什么新经历就不做梦；模型没把梦交回来时这一觉作罢，不算整理失败；
/// 梦的状态随存档走，没有这一项的旧存档照样能读。
#[test]
fn a_night_of_sleep_becomes_one_dream_with_insights_and_forgetting() {
    let mut w = world_with_props(vec![
        json!({"id":"bed","name":"床","locale":"loc_a","verbs":[{"verb":"睡觉","duration_minutes":480,"action":{"id":"sleep","label":"睡觉","duration_minutes":480,"completion":"sleep"}}]}),
    ]);
    w.enable_agent("r1").unwrap();
    for (i, text) in [
        "图书馆今天开放，晚上七点关门。",
        "新公告：图书馆明天维修关闭，后天再开放。",
        "集市周末有鱼卖，去晚了就没了。",
        "码头的灯坏了，夜里别走栈桥。",
        "酒馆新来了一位厨子。",
        "镇公所后天开会，谁都可以去听。",
    ]
    .iter()
    .enumerate()
    {
        read_board(&mut w, "r1", &format!("read{i}"), text);
    }
    let mut r = runtime(&mut w, Limits::default());
    let q = next(&mut r, &mut w);
    reply(&mut r, &mut w, &q, "observe", json!({}));
    assert!(!is_dream(&q), "没睡过觉不做梦");

    sleep(&mut w, &mut r, "nap1", 200.0);
    let q = dream_request(&mut r, &mut w).expect("睡醒后做一次梦");
    let names: Vec<_> = q
        .tools
        .iter()
        .map(|t| t["function"]["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, ["dream"]);
    let material = r
        .session("r1")
        .unwrap()
        .notebook
        .request
        .clone()
        .unwrap()
        .dream;
    assert!(material.len() >= 6, "{material:?}");
    let memory = &r.session("r1").unwrap().memory;
    let about = |words: &str| {
        material
            .iter()
            .copied()
            .find(|id| memory.records[(*id - 1) as usize].text.contains(words))
            .unwrap_or_else(|| panic!("材料里该有提到「{words}」的记忆"))
    };
    let (open, closed) = (about("公告read0"), about("公告read1"));
    // 读过的东西带着当时的正文，梦里才有东西可想
    assert!(q.messages[1].content.contains("图书馆明天维修关闭"));
    let routine = material
        .iter()
        .copied()
        .find(|id| {
            let m = &memory.records[(*id - 1) as usize];
            m.kind == MemoryKind::Episode && m.text.contains("睡觉")
        })
        .expect("材料里有一条日常见闻");
    let before = memory.records.len();
    reply(
        &mut r,
        &mut w,
        &q,
        "dream",
        json!({
            "insights": [
                {"text":"公告上的话隔天就可能变，去图书馆之前要再看一眼。","memory_ids":[open, closed, 9999]},
                {"text":"指到材料外面的不算。","memory_ids":[9999]}
            ],
            "let_go": [routine, 9999],
            "dream": "我在一座没有门的图书馆里找公告栏，纸上的字一行一行变成鱼游走了。"
        }),
    );
    let s = r.session("r1").unwrap();
    assert_eq!(s.memory.records.len(), before + 1, "只多一条认识");
    let insight = s.memory.records.last().unwrap();
    assert_eq!(insight.kind, MemoryKind::Insight);
    for id in [open, closed] {
        let source = &s.memory.records[(id - 1) as usize];
        assert!(source
            .event_ids
            .iter()
            .all(|e| insight.event_ids.contains(e)));
    }
    assert_eq!(
        s.memory.records[(routine - 1) as usize].status,
        MemoryStatus::Superseded
    );
    let dream = &s.memory.dream;
    assert!(!dream.pending);
    assert_eq!(dream.dreams.len(), 1);
    assert_eq!(dream.dreams[0].insights, [insight.memory_id]);
    assert_eq!(dream.dreams[0].let_go, 1);
    assert!(dream.dreams[0].text.contains("图书馆"));
    assert_eq!(s.notebook.failures, 0);
    assert!(s.memory.validate(&w, "r1").is_ok());
    // 侧栏里看得到这一觉的梦
    let view = r.view(&w, "r1").expect("有 Agent 会话");
    assert_eq!(view.dreams.len(), 1);
    assert_eq!((view.dreams[0].insights, view.dreams[0].let_go), (1, 1));
    // 玩法叫人交动作的通知过后不值得记：比一句日常见闻还不要紧
    let mut notice = s.memory.records[0].clone();
    notice.text = "【故事】轮到你说话：用 mode_action 交 speak".into();
    assert!(notice.importance() < s.memory.records[(routine - 1) as usize].importance());
    // 以后按词查，梦里想明白的排在最前面
    let mut index = MemoryIndex::new().unwrap();
    let found = index
        .search("r1", &s.memory, "去图书馆", w.minutes(), 3)
        .unwrap();
    assert_eq!(found[0].record.memory_id, insight.memory_id);
    // 梦随存档走；没有这一项的旧存档照样读
    let saved = serde_json::to_value(&s.memory).unwrap();
    let loaded: MemoryStore = serde_json::from_value(saved.clone()).unwrap();
    assert_eq!(loaded.dream, s.memory.dream);
    let mut old = saved;
    old.as_object_mut().unwrap().remove("dream");
    let loaded: MemoryStore = serde_json::from_value(old).unwrap();
    assert!(loaded.dream.dreams.is_empty() && !loaded.dream.pending);

    // 做完梦接着过日子；再睡一觉，没什么新经历，不做梦
    let q = next(&mut r, &mut w);
    assert!(!is_dream(&q));
    reply(&mut r, &mut w, &q, "observe", json!({}));
    sleep(&mut w, &mut r, "nap2", 150.0);
    assert!(
        dream_request(&mut r, &mut w).is_none(),
        "新记忆不够，这一觉不做梦"
    );
    assert!(!r.session("r1").unwrap().memory.dream.pending);

    // 又攒了些经历再睡：这回模型没把梦交回来，这一觉作罢，不算整理失败
    for (i, text) in [
        "码头今晚有船到。",
        "酒馆明天歇业一天。",
        "广场的钟修好了。",
        "集市搬到了河对岸。",
        "图书馆进了一批新书。",
        "镇上要来一位新医生。",
    ]
    .iter()
    .enumerate()
    {
        read_board(&mut w, "r1", &format!("again{i}"), text);
    }
    sleep(&mut w, &mut r, "nap3", 150.0);
    let q = dream_request(&mut r, &mut w).expect("攒够了经历，睡醒做梦");
    reply_with(&mut r, &mut w, &q, "……", vec![]);
    let s = r.session("r1").unwrap();
    assert!(!s.memory.dream.pending && s.notebook.request.is_none());
    assert_eq!((s.memory.dream.dreams.len(), s.notebook.failures), (1, 0));
    assert!(!is_dream(&next(&mut r, &mut w)));
}
