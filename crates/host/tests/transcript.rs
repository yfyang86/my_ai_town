//! 记录：世界跑一段，小镇目录的 transcript/ 里就有世界日志、想法和村民 Agent 的轨迹，
//! 每行带种类、现实时间、世界编号和世界里的分钟；管理服务下载时读的就是这几份原文。
//! 重置世界后新的行记在新的世界编号下。
use crate::support::*;
use serde_json::{json, Value};
use std::cell::Cell;

fn lines(town: &Town, kind: &str) -> Vec<Value> {
    let dir = town.save().parent().unwrap().to_path_buf();
    let read = host::transcript::read(&dir, &[kind]).unwrap();
    let kinds = read["kinds"].as_object().unwrap();
    assert_eq!(kinds.len(), 1, "只给要的那一种");
    kinds[kind]
        .as_str()
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn a_running_world_writes_down_events_thoughts_and_agent_traces() {
    let mut town = Town::new();
    town.args = map_args(MAP_2D);
    let mut ws = town.start();
    join_id(&mut ws, "tpl_a");
    join_id(&mut ws, "tpl_b");
    let world_id = reply(&mut ws, json!({"type":"get_population"}), "population")["population"]
        ["world_id"]
        .clone();
    send(
        &mut ws,
        json!({"type":"set_speed","request_id":"go","speed":20}),
    );
    // 跑到村民回过一次话为止
    let (rosters, replied) = (Cell::new(0), Cell::new(false));
    let received = received_until(&mut ws, |m| {
        rosters.set(rosters.get() + usize::from(m["type"] == "roster"));
        let reply = m["type"] == "event" && m["text"].as_str().is_some_and(|t| t.contains("响应"));
        replied.set(replied.get() || reply);
        rosters.get() >= 10 && replied.get()
    });
    // 入住的那两条世界日志在；之后页面收到的世界日志（不算模型台账行）一条不少
    let events = lines(&town, "event");
    assert!(events.len() >= 2, "{events:?}");
    for text in received
        .iter()
        .filter(|m| m["type"] == "event" && m["kind"] != "model")
        .filter_map(|m| m["text"].as_str())
    {
        assert!(events.iter().any(|e| e["text"] == text), "没记下：{text}");
    }
    assert!(events
        .iter()
        .all(|e| e["text"] != "" && !e["text"].as_str().unwrap().contains("Agent 请求")));
    for line in &events {
        assert_eq!(line["kind"], "event");
        assert_eq!(line["world"], world_id);
        assert!(line["at"].is_u64() && line["minutes"].is_number() && line["clock"].is_string());
    }
    // 轨迹里有发给模型的请求和回复；想法是名册上新出现的那一条
    let traces = lines(&town, "trace");
    let kinds = |event: &str| {
        traces
            .iter()
            .filter(|t| t["trace"]["event"] == event)
            .count()
    };
    assert!(
        kinds("request") >= 1 && kinds("response") >= 1,
        "{}",
        traces.len()
    );
    let thoughts = lines(&town, "thought");
    assert!(!thoughts.is_empty());
    assert!(thoughts
        .iter()
        .all(|t| t["name"].is_string() && !t["text"].as_str().unwrap().is_empty()));
    assert!(lines(&town, "chat").is_empty(), "假模型不聊天");

    // 重置世界：之后的行记在新的世界编号下，旧的还在
    let before = events.len();
    send(&mut ws, json!({"type":"reset_world","request_id":"again"}));
    received_until(&mut ws, |m| m["type"] == "world_reset");
    join_id(&mut ws, "tpl_a");
    let after = lines(&town, "event");
    assert!(after.len() > before);
    assert_eq!(after[..before], events[..]);
    assert_ne!(after.last().unwrap()["world"], world_id);
}
