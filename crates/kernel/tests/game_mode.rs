use crate::common;
use kernel::{
    avatar::{Area, Spot, Surface, WalkMap},
    navigation::space::Terrain,
    needs::NeedValues,
    Event, World,
};
use protocol::{
    FactKind, GameModeAction, GameModeChoice, GameModeContext, GameModeView, GroupStatus,
    PhysiologySwitches, StorylineBeat, StorylineBeatState, StorylineMilestone, StorylineRole,
    StorylineScene, StorylineScript, StorylineView, StorylineWhen, SwordholderEnding,
    SwordholderPhase, SwordholderRole, SwordholderRules, SwordholderView, TideCircle, TideFlare,
    TideNewsKind, TidePhase, TideView, WerewolfOut, WerewolfPhase, WerewolfRole, WerewolfSide,
    WerewolfView,
};
use serde_json::json;
use std::{collections::BTreeMap, sync::Arc};

const HOUR: f64 = 60.0;
const DAY: f64 = 1440.0;
const GOAL: [f64; 2] = [24.0, 18.0];

/// 一座圆岛：最高处在 (24, 18)，离它约 14.6 米以外是海。甲站在最高处，乙站在离它 12 米的东岸。
fn island() -> World {
    island_with(&[("a", "甲", "loc_a"), ("b", "乙", "loc_b")])
}

/// 同一座岛，住进这些人：loc_a 的落脚点在最高处，loc_b 的在东岸。
fn island_with(people: &[(&str, &str, &str)]) -> World {
    island_on(common::tiny_topology(), people)
}

fn island_on(topo: kernel::Topology, people: &[(&str, &str, &str)]) -> World {
    let mut w = common::world_with(topo, 7.0 * HOUR, people);
    let (columns, rows) = (41, 41);
    let heights = (0..rows)
        .flat_map(|z| {
            (0..columns)
                .map(move |x| 2.0 - (f64::from(x) - GOAL[0]).hypot(f64::from(z) - GOAL[1]) / 7.5)
        })
        .collect();
    let spot = |locale: &str, position| Spot {
        locale: locale.into(),
        space: "town_outdoor".into(),
        position,
    };
    w.walk_map = Some(Arc::new(WalkMap {
        version: 1,
        svg_units_per_metre: 10.,
        metres_per_minute: 12.,
        outdoor_space: "town_outdoor".into(),
        surfaces: ["loc_a", "loc_b", "edge_ab"]
            .into_iter()
            .map(|locale| Surface {
                locale: locale.into(),
                space: "town_outdoor".into(),
                area: Area::Rect {
                    min: [0., 0.],
                    max: [40., 40.],
                },
            })
            .collect(),
        stations: BTreeMap::from([
            ("st_a".into(), spot("loc_a", GOAL)),
            ("st_b".into(), spot("loc_b", [36., 18.])),
        ]),
        terrain: Some(Terrain {
            origin: [0., 0.],
            step: 1.0,
            columns: columns as usize,
            rows: rows as usize,
            heights,
        }),
        sea_level: 0.0,
        ..Default::default()
    }));
    // 只留精力在变，五天里没人吃喝睡也不会死，方便看潮水多扣的那份。
    w.set_physiology(PhysiologySwitches {
        fullness: false,
        energy: true,
        water: false,
        health: false,
        weather: false,
    });
    w
}

fn logs(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .map(|e| {
            let Event::Log { text, .. } = e;
            text.clone()
        })
        .collect()
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn tide(w: &World) -> Option<TideView> {
    match w.game_mode_view() {
        Some(GameModeView::Tide(v)) => Some(v),
        _ => None,
    }
}

/// 没开玩法时存档和名册里都没有玩法的字段，和没有玩法这回事时一样。
fn no_trace(w: &World) {
    let snapshot = serde_json::to_value(w.snapshot()).unwrap();
    assert!(snapshot.get("game_mode").is_none());
    let roster = serde_json::to_value(w.roster(1.0)).unwrap();
    assert!(roster.get("game_mode").is_none());
}

fn rested(w: &mut World) {
    for id in ["a", "b"] {
        w.adjust_body(
            id,
            NeedValues {
                fullness: None,
                energy: Some(90.0),
                water: None,
                health: None,
            },
        )
        .unwrap();
    }
}

#[test]
fn tide_rings_shrink_onto_the_high_ground_and_drain_whoever_stands_in_water() {
    let mut w = island();
    assert!(w.game_mode().is_none());
    no_trace(&w);
    // 第 1 天 07:00 打开：今天的 06:00 已过，第 1 圈第 2 天天亮公布。
    w.set_game_mode(Some(&GameModeChoice::Tide { seed: Some(7) }))
        .unwrap();
    assert_eq!(w.game_mode(), Some("tide"));
    let v = tide(&w).unwrap();
    assert_eq!((v.phase, v.ring, v.rings), (TidePhase::Waiting, 0, 5));
    let mut previous: TideCircle = v.edge;
    assert!(dist(previous.center, GOAL) + 0.1 < previous.radius);

    // 第 2 天 08:00：第 1 圈已公布，所有人都收到公告，公告里写着还剩谁；第一只木箱冲上来。
    let events = w.step(25.0 * HOUR);
    assert!(logs(&events).iter().any(
        |t| t.starts_with("【潮水】第 1 圈已公布") && t.ends_with("岛上还剩 2 个人：甲、乙。")
    ));
    let crate_lines = |events: &[Event]| {
        logs(events)
            .iter()
            .filter(|t| t.contains("潮水冲上来一只木箱"))
            .count()
    };
    let mut crates = crate_lines(&events);
    let v = tide(&w).unwrap();
    assert_eq!((v.phase, v.ring), (TidePhase::Announced, 1));
    let first = v.next.unwrap();

    // 15:00 正在涨：水边在两圈之间；存档读档后接着涨到同一个位置。
    w.step(7.0 * HOUR);
    let v = tide(&w).unwrap();
    assert_eq!(v.phase, TidePhase::Rising);
    assert!(v.edge.radius < previous.radius && v.edge.radius > first.radius);
    let mut restored = island();
    restored.restore_snapshot(w.snapshot()).unwrap();
    assert_eq!(tide(&restored), tide(&w));
    w = restored;

    // 旧存档没有时间表：照当时每天一圈接着涨，第 3 天 13:00 还在涨第 2 圈。
    let mut old = serde_json::to_value(w.snapshot()).unwrap();
    old["game_mode"]["tide"]
        .as_object_mut()
        .unwrap()
        .remove("timetable")
        .unwrap();
    let mut daily = island();
    daily
        .restore_snapshot(serde_json::from_value(old).unwrap())
        .unwrap();
    daily.step(2.0 * DAY + 13.0 * HOUR - daily.minutes());
    let v = tide(&daily).unwrap();
    assert_eq!((v.phase, v.ring), (TidePhase::Rising, 2));

    // 后两天上午、下午各收一圈。每一圈涨到时水边停在那一圈：都在上一圈里、都包住最高处；中午涨到的那一刻
    // 下一圈接着公布。第 1 到第 3 圈公布时各冲上来一只木箱，菜刀那箱在前两箱里。
    let mut lines = Vec::new();
    for (k, day, hour) in [
        (1, 1.0, 18.0),
        (2, 2.0, 12.0),
        (3, 2.0, 18.0),
        (4, 3.0, 12.0),
    ] {
        let events = w.step(day * DAY + hour * HOUR + 30.0 - w.minutes());
        crates += crate_lines(&events);
        lines = logs(&events);
        let v = tide(&w).unwrap();
        let expected = if hour < 18.0 {
            (TidePhase::Announced, k + 1)
        } else {
            (TidePhase::Holding, k)
        };
        assert_eq!((v.phase, v.ring), expected);
        assert!(dist(v.edge.center, previous.center) + v.edge.radius <= previous.radius + 1e-6);
        // 终点一带（14 米，圈小时四分之三个半径）一直在圈里：圈心离终点不远于剩下的那点
        let slack = (v.edge.radius - 14.0).max(v.edge.radius * 0.25);
        assert!(dist(v.edge.center, GOAL) <= slack + 1e-6);
        previous = v.edge;
        if k == 1 {
            let Some(GameModeContext::Tide(context)) = w.game_mode_context("a") else {
                panic!("甲的模型该看到潮水这一栏");
            };
            assert_eq!(context.notice, "潮水停在第 1 圈；明天 06:00 公布下一圈。");
        }
        if k == 2 {
            let snapshot = serde_json::to_value(w.snapshot()).unwrap();
            assert!(snapshot["placed_props"]
                .as_object()
                .unwrap()
                .values()
                .any(|p| p["definition"]["id"] == "tide:crate-kitchen_knife"));
        }
    }
    assert_eq!(crates, 3);
    // 第 4 天中午第 4 圈涨到，最后一圈公布，船停到最高处，只坐一个人。
    assert!(lines.iter().any(|t| t
        .starts_with("【潮水】最后一圈已公布：今天 14:00 开始涨，18:00 前水会漫过全岛。")
        && t.contains("只坐得下一个人")));

    // 第 4 圈只剩山顶一小块：乙站在水里，比站在山顶的甲每小时多掉 6 点精力。
    rested(&mut w);
    w.step(HOUR);
    let energy = |w: &World, id: &str| w.actor(id).unwrap().needs.energy.value;
    let (a, b) = (energy(&w, "a"), energy(&w, "b"));
    assert!((a - b - 6.0).abs() < 0.5, "甲 {a} 乙 {b}");
    let Some(GameModeContext::Tide(context)) = w.game_mode_context("b") else {
        panic!("开着潮水圈时乙的模型该看到潮水这一栏");
    };
    assert!(context.standing.unwrap().starts_with("你站在潮水里"));

    // 乙在水里的棚子上睡觉：几分钟内就被泡醒，通知里写着往哪边走能回到圈里
    let hut: kernel::props::Definition = serde_json::from_value(json!({
        "id":"core:island-shelter","version":1,"name":"棚子","shelter":true,
        "actions":[{"id":"sleep","label":"在棚子里睡觉","duration_minutes":480,"completion":"sleep","rest":true}]
    }))
    .unwrap();
    w.place_prop_near(
        serde_json::from_value(json!({
            "instance":{"id":"hut","definition":hut.reference(),"locale":"loc_b"},
            "definition":hut
        }))
        .unwrap(),
        Some(&protocol::PlaceNear {
            space: "town_outdoor".into(),
            position: [36., 18.],
        }),
    )
    .unwrap();
    common::run(
        &mut w,
        "b",
        "nap",
        json!({"prop_id":"hut","intent":{"verb":"在棚子里睡觉"}}),
    )
    .unwrap();
    w.step(10.0);
    assert_eq!(
        w.action_group("b", "nap").unwrap().status,
        GroupStatus::Interrupted
    );
    let woken = w
        .actor("b")
        .unwrap()
        .journal
        .events_after(0)
        .iter()
        .any(|e| {
            e.kind == FactKind::Notice && e.text.contains("潮水漫上来，把你泡醒了：你站在潮水里")
        });
    assert!(woken, "乙该被泡醒");

    // 没人去登船，第 4 天 18:00 水漫过全岛，船带走离它最近的甲。
    let events = w.step(3.0 * DAY + 18.0 * HOUR - w.minutes());
    let lines = logs(&events);
    assert!(lines.iter().any(|t| t == "【潮水】水漫过了全岛。"));
    assert!(lines
        .iter()
        .any(|t| t == "【潮水】甲离船最近，被船带走了，是这一局唯一活着离开的人。"));
    let v = tide(&w).unwrap();
    assert_eq!((v.phase, v.edge.radius), (TidePhase::Over, 0.0));
    assert_eq!(v.edge.center, GOAL);
    assert_eq!(v.winner.as_deref(), Some("a"));
    assert!(!w.actor("a").unwrap().present());

    // 关掉以后不留痕迹
    w.set_game_mode(None).unwrap();
    assert!(w.game_mode().is_none());
    no_trace(&w);
}

/// 乙站的地方只有导航空间知道、行走地图里没有：开局布置的棚子、石屋和船的操作位都是这样，只在导航覆盖里。
/// 在那儿照样算站在水里，睡着了照样被泡醒。第二到第五局这些地方一直不算在水里，泡醒一次也没发生。
#[test]
fn tide_wakes_a_sleeper_whose_spot_only_navigation_knows() {
    let topo = common::topology(&common::two_places_json(|data| {
        data["props"].as_array_mut().unwrap().push(json!({
            "id":"hut","name":"棚子","locale":"loc_b",
            "verbs":[{"verb":"在棚子里睡觉","duration_minutes":480,
              "action":{"id":"sleep","label":"在棚子里睡觉","duration_minutes":480,"completion":"sleep","rest":true,"during":[],"conditions":[]}}]
        }));
    }));
    let mut w = island_on(topo, &[("a", "甲", "loc_a"), ("b", "乙", "loc_b")]);
    let mut map = (**w.walk_map.as_ref().unwrap()).clone();
    map.stations.remove("st_b");
    w.walk_map = Some(Arc::new(map));
    w.topo.navigation = Some(
        serde_json::from_value(json!({
            "navigation_revision": 1, "floors": [], "props": [], "routes": [],
            "stations": [{"id": "st_b", "locale": "loc_b", "position": [36.0, 0.4, 18.0]}]
        }))
        .unwrap(),
    );
    w.set_game_mode(Some(&GameModeChoice::Tide { seed: Some(7) }))
        .unwrap();
    // 第 3 天傍晚第 3 圈涨到，离最高处 12 米的乙早已在水里
    w.step(2.0 * DAY + 18.5 * HOUR - w.minutes());
    let Some(GameModeContext::Tide(context)) = w.game_mode_context("b") else {
        panic!("乙的模型该看到潮水这一栏");
    };
    assert!(context.standing.unwrap().starts_with("你站在潮水里"));
    common::run(
        &mut w,
        "b",
        "nap",
        json!({"prop_id":"hut","intent":{"verb":"在棚子里睡觉"}}),
    )
    .unwrap();
    w.step(10.0);
    assert_eq!(
        w.action_group("b", "nap").unwrap().status,
        GroupStatus::Interrupted
    );
    assert!(w
        .actor("b")
        .unwrap()
        .journal
        .events_after(0)
        .iter()
        .any(|e| {
            e.kind == FactKind::Notice && e.text.contains("潮水漫上来，把你泡醒了：你站在潮水里")
        }));
}

const KNIFE: &str = r#"{"id":"core:kitchen_knife","version":1,"name":"菜刀","description":"一把厚背菜刀，刀口很利。","portable":true,
 "actions":[{"id":"stab","label":"持刀袭击","duration_minutes":3,"completion":"activity",
  "on_complete":[{"kind":"harm","amount":100,"wound":"胸口一处刀伤"}],"recipient":true}]}"#;

#[test]
fn tide_hands_the_boat_to_the_last_one_standing_and_keeps_the_fights_in_its_news() {
    // 三个人都在最高处；只看健康和精力。
    let people = [
        ("a", "甲", "loc_a"),
        ("b", "乙", "loc_a"),
        ("c", "丙", "loc_a"),
    ];
    let mut w = island_with(&people);
    w.set_physiology(PhysiologySwitches {
        fullness: false,
        energy: true,
        water: false,
        health: true,
        weather: false,
    });
    w.set_game_mode(Some(&GameModeChoice::Tide { seed: Some(7) }))
        .unwrap();
    // 开局每人一张纸条，模型一直看得到目标
    let events = w.step(1.0);
    assert!(logs(&events)
        .iter()
        .any(|t| t.starts_with("【纸条】") && t.contains("最后一圈在第 4 天 12:00 公布")));
    let Some(GameModeContext::Tide(context)) = w.game_mode_context("c") else {
        panic!("丙的模型该看到潮水这一栏");
    };
    assert!(context.goal.starts_with("只有一个人能活着离开"));
    assert!(context
        .goal
        .ends_with("眼下岛上还活着 3 个人：甲、乙、丙。"));
    assert_eq!(tide(&w).unwrap().players.len(), 3);

    // 甲拿地上的菜刀刺死乙：战报里有这一刀，名册里写着乙怎么死的
    let knife: kernel::props::Definition = serde_json::from_str(KNIFE).unwrap();
    w.place_prop(
        serde_json::from_value(json!({
            "instance":{"id":"knife","definition":knife.reference(),"locale":"loc_a"},
            "definition":knife
        }))
        .unwrap(),
    )
    .unwrap();
    let stab = |w: &mut World, key: &str, whom: &str| {
        common::run(
            w,
            "a",
            key,
            json!({"prop_id":"knife","intent":{"verb":"持刀袭击"},"recipient":whom}),
        )
        .unwrap();
        common::finish(w, "a", key);
        assert_eq!(
            w.action_group("a", key).unwrap().status,
            GroupStatus::Succeeded
        );
    };
    stab(&mut w, "stab-b", "b");
    w.step(5.0);
    let v = tide(&w).unwrap();
    assert!(v
        .news
        .iter()
        .any(|n| n.kind == TideNewsKind::Clash && n.text == "甲持刀袭击乙，健康 −100，乙倒下了"));
    let fell = |v: &TideView, id: &str| {
        v.players
            .iter()
            .find(|p| p.id == id)
            .and_then(|p| p.fell.clone())
    };
    assert_eq!(fell(&v, "b").as_deref(), Some("被甲杀死（胸口一处刀伤）"));
    assert!(v.winner.is_none());

    // 甲又刺死丙：只剩甲，船提前开来接走他，这一局结束，读档后结果还在
    stab(&mut w, "stab-c", "c");
    let events = w.step(5.0);
    assert!(logs(&events)
        .iter()
        .any(|t| t == "【潮水】岛上只剩甲一个人了，船提前开了过来。甲是这一局唯一活着离开的人。"));
    let v = tide(&w).unwrap();
    assert_eq!((v.phase, v.winner.as_deref()), (TidePhase::Over, Some("a")));
    assert!(fell(&v, "c").is_some());
    assert!(!w.actor("a").unwrap().present());
    assert_eq!(w.game_mode_tempo(), 1.0);
    let mut restored = island_with(&people);
    restored.restore_snapshot(w.snapshot()).unwrap();
    assert_eq!(tide(&restored), tide(&w));
}

/// 船只认信号弹：第 1 圈冲上来的铁皮匣撬开就拿到，换了人公告全岛、记进战报，题面写着在谁身上；
/// 没带的人登不了船，登船途中被人抢就登不成，拿着的人登上去赢、信号弹打出去用掉。
#[test]
fn tide_boat_takes_only_whoever_carries_the_flare() {
    let people = [
        ("a", "甲", "loc_a"),
        ("b", "乙", "loc_a"),
        ("c", "丙", "loc_a"),
    ];
    let mut w = island_with(&people);
    let rest = |w: &mut World| {
        for id in ["a", "b", "c"] {
            w.adjust_body(
                id,
                NeedValues {
                    fullness: None,
                    energy: Some(90.0),
                    water: None,
                    health: None,
                },
            )
            .unwrap();
        }
    };
    let flare_line = |w: &World, id: &str| {
        let Some(GameModeContext::Tide(context)) = w.game_mode_context(id) else {
            panic!("开着潮水圈时模型该看到潮水这一栏");
        };
        context.flare.unwrap_or_default()
    };
    w.set_game_mode(Some(&GameModeChoice::Tide { seed: Some(7) }))
        .unwrap();
    let events = w.step(1.0);
    assert!(logs(&events)
        .iter()
        .any(|t| t.starts_with("【纸条】") && t.contains("船上的人只认信号弹")));

    // 第 2 天 06:00 第 1 圈公布，铁皮匣跟着木箱冲上来；甲撬开拿到信号弹，全岛都知道
    let events = w.step(DAY + 6.0 * HOUR + 1.0 - w.minutes());
    assert!(logs(&events)
        .iter()
        .any(|t| t.contains("潮水还冲上来一只铁皮匣")));
    assert_eq!(tide(&w).unwrap().flare, Some(TideFlare::Case));
    rest(&mut w);
    common::run(
        &mut w,
        "a",
        "open",
        json!({"prop_id":"tide-flare-case","intent":{"verb":"撬开铁皮匣"}}),
    )
    .unwrap();
    common::finish(&mut w, "a", "open");
    let events = w.step(5.0);
    assert!(logs(&events)
        .iter()
        .any(|t| t == "【潮水】甲撬开了铁皮匣，拿到了信号弹。"));
    let v = tide(&w).unwrap();
    assert_eq!(v.flare, Some(TideFlare::Held { id: "a".into() }));
    assert!(v
        .news
        .iter()
        .any(|n| n.kind == TideNewsKind::Flare && n.text == "甲撬开了铁皮匣，拿到了信号弹。"));
    assert!(flare_line(&w, "a").starts_with("信号弹在你身上"));
    assert_eq!(flare_line(&w, "b"), "信号弹在甲身上，甲此刻在「甲地」。");

    // 第 4 天 12:00 船来了。丙没带信号弹，登不了船
    w.step(3.0 * DAY + 12.0 * HOUR + 1.0 - w.minutes());
    rest(&mut w);
    let board = |w: &mut World, who: &str, key: &str| {
        common::run(
            w,
            who,
            key,
            json!({"prop_id":"tide-boat","intent":{"verb":"登船"}}),
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
    };
    let refused = board(&mut w, "c", "board-c").unwrap_err();
    assert!(refused.contains("需要随身带着信号弹×1"), "{refused}");

    // 甲在登船，乙当面抢他的信号弹：不管抢没抢到，甲的登船都被打断；乙抢到为止
    board(&mut w, "a", "board-a").unwrap();
    w.step(1.0);
    let flare = w
        .topo
        .props()
        .iter()
        .find(|p| p.definition.as_deref() == Some("tide:flare@1"))
        .unwrap()
        .id
        .clone();
    for i in 0..12 {
        let key = format!("rob-{i}");
        common::run(
            &mut w,
            "b",
            &key,
            json!({"intent":{"verb":"抢"},"recipient":"a","prop_id":flare}),
        )
        .unwrap();
        common::finish(&mut w, "b", &key);
        if w.inventory().carried_by(&flare, "b") {
            break;
        }
    }
    assert_eq!(
        w.action_group("a", "board-a").unwrap().status,
        GroupStatus::Interrupted
    );
    assert!(w.inventory().carried_by(&flare, "b"), "乙该抢到了信号弹");
    let events = w.step(5.0);
    assert!(logs(&events)
        .iter()
        .any(|t| t == "【潮水】信号弹从甲身上到了乙身上。"));
    // 战报里那一下抢记成信号弹的事，不重复记换手
    let news = tide(&w).unwrap().news;
    assert!(news
        .iter()
        .any(|n| n.kind == TideNewsKind::Flare && n.text == "乙从甲手里抢走了信号弹"));
    assert!(!news.iter().any(|n| n.text.starts_with("信号弹从甲身上")));
    let refused = board(&mut w, "a", "board-a2").unwrap_err();
    assert!(refused.contains("需要随身带着信号弹×1"), "{refused}");

    // 乙登上去：赢了，信号弹打了出去
    board(&mut w, "b", "board-b").unwrap();
    common::finish(&mut w, "b", "board-b");
    let events = w.step(5.0);
    assert!(logs(&events)
        .iter()
        .any(|t| t == "【潮水】乙在船边打出信号弹，登上了船，是这一局唯一活着离开的人。"));
    let v = tide(&w).unwrap();
    assert_eq!((v.phase, v.winner.as_deref()), (TidePhase::Over, Some("b")));
    assert!(w
        .topo
        .props()
        .iter()
        .all(|p| p.withdrawn || p.definition.as_deref() != Some("tide:flare@1")));
    let mut restored = island_with(&people);
    restored.restore_snapshot(w.snapshot()).unwrap();
    assert_eq!(tide(&restored), tide(&w));
}

fn werewolf(w: &World) -> WerewolfView {
    match w.game_mode_view() {
        Some(GameModeView::Werewolf(v)) => v,
        other => panic!("开着的该是狼人杀：{other:?}"),
    }
}

/// 改过规则的一局：10:00 敲钟、10:10 演说、11:00 交接，现任执剑人的两句话、选民表态、水滴来袭和最后的宣布都关掉。
/// 只剩两位候选人演说和投票，交接完就收场（结局是 peace）；投票中途存档读档，规则跟着存档走。
/// 钟点排反了、白天排不下时开不了，原来的世界不受影响。
#[test]
fn swordholder_night_follows_edited_rules_and_skips_switched_off_phases() {
    let ids: Vec<String> = (1..=6).map(|n| format!("r{n}")).collect();
    let names = ["罗辑", "程心", "维德", "智子", "艾AA", "老秦"];
    let people: Vec<(&str, &str, &str)> = ids
        .iter()
        .zip(names)
        .map(|(id, name)| (id.as_str(), name, "loc_a"))
        .collect();
    let fresh = || {
        let mut w = common::world_with(common::tiny_topology(), 7.0 * HOUR, &people);
        w.set_physiology(PhysiologySwitches {
            fullness: false,
            energy: false,
            water: false,
            health: false,
            weather: false,
        });
        w
    };
    let open = |w: &mut World, day_hours: u32, rules: SwordholderRules| {
        w.set_game_mode(Some(&GameModeChoice::Swordholder {
            seed: Some(5),
            day_hours: Some(day_hours),
            rules: Some(rules),
        }))
    };
    let mut w = fresh();
    let backwards = SwordholderRules {
        bell: Some(700),
        meet: Some(650),
        ..Default::default()
    };
    assert!(open(&mut w, 2, backwards).is_err());
    let early = SwordholderRules {
        bell: Some(300),
        meet: Some(310),
        ..Default::default()
    };
    assert!(open(&mut w, 8, early).is_err());
    assert!(w.game_mode().is_none());

    let rules = SwordholderRules {
        bell: Some(600),
        meet: Some(610),
        handover: Some(660),
        speak_minutes: Some(5),
        opening_remark: Some(false),
        stances: Some(false),
        handover_remark: Some(false),
        attack: Some(false),
        statement: Some(false),
        ..Default::default()
    };
    open(&mut w, 2, rules).unwrap();
    // 08:00 天亮：议程照改过的钟点说，没有关掉的环节
    w.step(1.1 * HOUR);
    let told = w.actor("r5").unwrap().journal.events_after(0);
    let agenda = told.iter().find(|e| e.text.contains("身份是")).unwrap();
    assert!(agenda.text.contains("10:00 敲钟"), "{}", agenda.text);
    assert!(agenda.text.contains("11:00 在"), "{}", agenda.text);
    assert!(!agenda.text.contains("表态") && !agenda.text.contains("先说一句"));

    let mut reloaded = false;
    for _ in 0..600 {
        let v = swordholder(&w);
        if v.phase == SwordholderPhase::Over {
            break;
        }
        if v.phase == SwordholderPhase::Vote && v.ballots.len() == 1 && !reloaded {
            reloaded = true;
            w = common::restore(&w, fresh());
            assert_eq!(swordholder(&w), v);
        }
        let mut acted = false;
        for id in &ids {
            let Some(GameModeContext::Swordholder(c)) = w.game_mode_context(id) else {
                continue;
            };
            match c.actions.first().map(String::as_str) {
                Some("speak") => mode_act(&mut w, id, "speak", None, Some("我想说几句。")),
                Some("vote") => mode_act(&mut w, id, "vote", Some(2), None),
                _ => continue,
            }
            acted = true;
            break;
        }
        if !acted {
            w.step(5.0);
        }
    }
    let v = swordholder(&w);
    assert!(reloaded);
    assert_eq!(v.phase, SwordholderPhase::Over);
    assert_eq!(v.ending, Some(SwordholderEnding::Peace));
    assert_eq!((v.elected, v.swordholder), (Some(2), 2));
    assert!(v.decision.is_none());
    // 只有两位候选人的演说：现任执剑人的两句、选民表态和三体代表的宣布都没有
    let occasions: Vec<&str> = v.lines.iter().map(|l| l.occasion.as_str()).collect();
    assert_eq!(occasions, ["演说", "演说"]);
    // 11:00 交接完就收场，不用等到 19:00
    assert!(w.minutes() < 12.0 * HOUR, "{}", w.minutes());
    w.set_game_mode(None).unwrap();
    no_trace(&w);
}

fn storyline(w: &World) -> StorylineView {
    match w.game_mode_view() {
        Some(GameModeView::Storyline(v)) => v,
        other => panic!("开着的该是故事模式：{other:?}"),
    }
}

fn story_context(w: &World, id: &str) -> protocol::StorylineContext {
    match w.game_mode_context(id) {
        Some(GameModeContext::Storyline(c)) => c,
        other => panic!("{id} 的模型该看到故事这一栏：{other:?}"),
    }
}

/// 四个角色、三个节拍、四个里程碑的一本剧本：08:00 开场，10:00 在甲地开一场三个人轮流说话的戏，
/// 12:00 乙地的一幕，13:00 收场。
fn lighthouse_script() -> StorylineScript {
    let role = |role: &str, goal: &str, secret: Option<&str>| StorylineRole {
        role: role.into(),
        goal: goal.into(),
        secret: secret.map(str::to_owned),
    };
    let beat =
        |id: &str, title: &str, minute: u32, place: Option<&str>, who: &[u32]| StorylineBeat {
            id: id.into(),
            title: title.into(),
            day: 1,
            minute,
            place: place.map(str::to_owned),
            who: who.to_vec(),
            announce: format!("{title}开始了。"),
            brief: None,
            scene: None,
        };
    let milestone = |id: &str, title: &str, when: StorylineWhen, secret: bool| StorylineMilestone {
        id: id.into(),
        title: title.into(),
        detail: format!("{title}发生了"),
        when,
        secret,
    };
    StorylineScript {
        title: "灯塔之约".into(),
        premise: "灯塔要关了，镇上的人得在今天定下它的去留。".into(),
        cast: vec![
            role("守塔人", "把灯塔留下来", None),
            role("镇长", "把灯塔卖掉还债", Some("债是你自己欠下的")),
            role("渔夫", "弄清谁在说谎", None),
            role("外乡人", "买下灯塔", None),
        ],
        beats: vec![
            beat("morning", "晨雾", 480, None, &[]),
            StorylineBeat {
                brief: Some("轮到你时把立场说清楚。".into()),
                scene: Some(StorylineScene {
                    speakers: vec![1, 2, 3],
                    minutes: 10,
                    chars: 20,
                }),
                ..beat("council", "议事", 600, Some("甲地"), &[1, 2, 3])
            },
            beat("dusk", "码头的灯", 720, Some("乙地"), &[4]),
        ],
        milestones: vec![
            milestone(
                "opened",
                "故事开场",
                StorylineWhen::Beat {
                    beat: "morning".into(),
                },
                false,
            ),
            milestone(
                "met",
                "码头密会",
                StorylineWhen::Gather {
                    place: "乙地".into(),
                    seats: vec![3, 4],
                    count: 2,
                },
                false,
            ),
            milestone(
                "confessed",
                "镇长认账",
                StorylineWhen::Declare { seats: vec![2] },
                true,
            ),
            milestone(
                "saved",
                "灯塔留下",
                StorylineWhen::Declare { seats: vec![1] },
                false,
            ),
        ],
        ends_day: 1,
        ends_minute: 780,
        epilogue: Some("雾散了。".into()),
    }
}

/// 故事模式照剧本走一整局：节拍到点宣布（和它有关的人被叫醒、听到叮嘱），戏里一次一人说话
/// （一个人超时跳过、一段话超长截断），三种里程碑各达成一个、一个到收场也没达成；戏演到一半存档读档。
/// 藏着的里程碑只有能宣布它的人知道；剧本对不上（先后排反、指了不存在的节拍、角色比村民多）开不了。
#[test]
fn storyline_fires_beats_runs_a_scene_tracks_milestones_and_ends() {
    let people = [
        ("r1", "阿塔", "loc_a"),
        ("r2", "老镇", "loc_a"),
        ("r3", "阿渔", "loc_a"),
        ("r4", "远客", "loc_a"),
    ];
    let fresh = || {
        let mut w = common::world_with(common::tiny_topology(), 7.0 * HOUR, &people);
        w.set_physiology(PhysiologySwitches {
            fullness: false,
            energy: false,
            water: false,
            health: false,
            weather: false,
        });
        w
    };
    let open = |w: &mut World, script: StorylineScript| {
        w.set_game_mode(Some(&GameModeChoice::Storyline {
            seed: Some(9),
            script: Box::new(script),
        }))
    };
    let mut w = fresh();
    let mut backwards = lighthouse_script();
    backwards.beats.swap(0, 1);
    assert!(open(&mut w, backwards).is_err());
    let mut dangling = lighthouse_script();
    dangling.milestones[0].when = StorylineWhen::Beat {
        beat: "nowhere".into(),
    };
    assert!(open(&mut w, dangling).is_err());
    let mut crowded = lighthouse_script();
    crowded.cast.push(crowded.cast[0].clone());
    assert!(open(&mut w, crowded).is_err());
    assert!(w.game_mode().is_none());

    open(&mut w, lighthouse_script()).unwrap();
    assert_eq!(w.game_mode(), Some("storyline"));
    assert_eq!(w.game_mode_tempo(), 3.0);
    let v = storyline(&w);
    assert!(v
        .beats
        .iter()
        .all(|b| b.state == StorylineBeatState::Upcoming));
    assert_eq!(v.beats[1].place.as_deref(), Some("甲地"));
    // 目标和秘密只有本人看得到；藏着的里程碑只有能宣布它的人知道
    let mayor = story_context(&w, "r2");
    assert_eq!(mayor.secret.as_deref(), Some("债是你自己欠下的"));
    assert_eq!(mayor.actions, ["declare"]);
    assert!(mayor.milestones.iter().any(|m| m.contains("镇长认账")));
    let fisher = story_context(&w, "r3");
    assert!(fisher.secret.is_none() && fisher.actions.is_empty());
    assert!(!fisher.milestones.iter().any(|m| m.contains("镇长认账")));
    assert!(fisher.cast.contains("老镇（镇长）"));

    // 08:00 开场：所有人听到宣布，指着这个节拍的里程碑达成
    w.step(1.1 * HOUR);
    let v = storyline(&w);
    assert_eq!(v.beats[0].state, StorylineBeatState::Done);
    assert!(v.milestones[0].done);
    assert!(story_context(&w, "r4").now.contains("晨雾"));

    // 10:00 议事：三个人轮流说，叮嘱只有他们听到
    w.step(2.0 * HOUR);
    let v = storyline(&w);
    assert_eq!(
        (v.beats[1].state, v.turn),
        (StorylineBeatState::Current, Some(1))
    );
    assert_eq!(w.game_mode_tempo(), 1.0);
    let told = |w: &World, id: &str, words: &str| {
        w.actor(id)
            .unwrap()
            .journal
            .events_after(0)
            .iter()
            .any(|e| e.text.contains(words))
    };
    assert!(told(&w, "r1", "把立场说清楚"));
    assert!(told(&w, "r4", "议事开始了") && !told(&w, "r4", "把立场说清楚"));
    // 1 号轮到说话，另外还有一个由他宣布的里程碑
    assert_eq!(
        story_context(&w, "r1").actions,
        ["speak", "pass", "declare"]
    );
    // 没轮到的人交不了
    let early = GameModeAction {
        action: "speak".into(),
        target: None,
        text: Some("我先说".into()),
        reason: None,
    };
    assert!(w.game_mode_act("r3", &early).is_err());
    mode_act(&mut w, "r1", "speak", None, Some("灯塔不能卖。"));
    // 2 号不交，到点跳过；轮到 3 号时存档读档
    assert_eq!(storyline(&w).turn, Some(2));
    w.step(11.0);
    let v = storyline(&w);
    assert_eq!((v.turn, v.lines.len()), (Some(3), 1));
    w = common::restore(&w, fresh());
    assert_eq!(storyline(&w), v);
    mode_act(
        &mut w,
        "r3",
        "speak",
        None,
        Some("我只想知道是谁在说谎，这件事今天一定要有个交代才行。"),
    );
    let v = storyline(&w);
    assert_eq!((v.turn, v.beats[1].state), (None, StorylineBeatState::Done));
    assert_eq!(v.lines[1].text.chars().count(), 20);
    assert!(!v.lines[1].away);

    // 2 号宣布藏着的里程碑：别人的经历里没有这一条，旁观的人看得到是谁宣布的
    let mut confess = early.clone();
    confess.action = "declare".into();
    confess.target = Some("saved".into());
    assert!(w.game_mode_act("r2", &confess).is_err());
    mode_act_on(
        &mut w,
        "r2",
        "declare",
        "confessed",
        "我当着渔夫的面认了那笔债。",
    );
    let v = storyline(&w);
    assert_eq!((v.milestones[2].done, v.milestones[2].by), (true, Some(2)));
    assert!(!told(&w, "r3", "镇长认账"));
    assert!(story_context(&w, "r2").actions.is_empty());

    // 3 号和 4 号都到了乙地：密会达成
    assert!(!storyline(&w).milestones[1].done);
    for id in ["r3", "r4"] {
        common::run(
            &mut w,
            id,
            "to-dock",
            json!({"intent": {"verb": "前往", "locale": "loc_b"}}),
        )
        .unwrap();
        common::finish(&mut w, id, "to-dock");
    }
    w.step(6.0);
    assert!(storyline(&w).milestones[1].done);

    // 13:00 收场：没人宣布的那一项算没达成，工具收走
    let logged = w.step(4.0 * HOUR);
    let v = storyline(&w);
    assert!(v.ended);
    assert_eq!(v.epilogue.as_deref(), Some("雾散了。"));
    assert!(v.beats.iter().all(|b| b.state == StorylineBeatState::Done));
    let missed: Vec<&str> = v
        .milestones
        .iter()
        .filter(|m| m.missed)
        .map(|m| m.id.as_str())
        .collect();
    assert_eq!(missed, ["saved"]);
    let closing = logged
        .iter()
        .map(|Event::Log { text, .. }| text.as_str())
        .find(|t| t.contains("故事收场"))
        .expect("收场写进了世界日志");
    assert!(closing.contains("没有达成：灯塔留下") && closing.contains("雾散了"));
    assert!(!closing.contains("镇长认账"));
    assert!(w.game_mode_tool("r1").is_none());
    assert_eq!(w.game_mode_tempo(), 1.0);
    w.set_game_mode(None).unwrap();
    no_trace(&w);
}

/// 交一项带 target 是文字（里程碑编号）的玩法动作
fn mode_act_on(w: &mut World, id: &str, action: &str, target: &str, text: &str) {
    let action = GameModeAction {
        action: action.into(),
        target: Some(target.into()),
        text: Some(text.into()),
        reason: None,
    };
    let result = w.game_mode_act(id, &action);
    assert!(result.is_ok(), "{id} 交 {action:?}：{result:?}");
}

fn wolf_context(w: &World, id: &str) -> protocol::WerewolfContext {
    match w.game_mode_context(id) {
        Some(GameModeContext::Werewolf(c)) => c,
        other => panic!("{id} 的模型该看到狼人杀这一栏：{other:?}"),
    }
}

fn mode_act(w: &mut World, id: &str, action: &str, target: Option<u32>, text: Option<&str>) {
    let action = GameModeAction {
        action: action.into(),
        target: target.map(|t| t.to_string()),
        text: text.map(str::to_owned),
        reason: Some(format!("{id} 心里这么想")),
    };
    let result = w.game_mode_act(id, &action);
    assert!(result.is_ok(), "{id} 交 {action:?}：{result:?}");
}

/// 一局 9 人：用给模型看的那一栏（werewolf.actions）当成模型，按固定打法交动作，走完整局。
/// 好人每天都投座号最小的狼，狼人每夜刀座号最小的平民；女巫第 1 夜救人，猎人不开枪。
/// 途中故意让一个人发言超时、一个人不投票（到点往下走），议事中途存档读档。
#[test]
fn werewolf_judge_runs_a_whole_game_through_turns_deadlines_and_a_reload() {
    let names = [
        "阿树", "小禾", "芬恩", "露娜", "石头", "米拉", "诺亚", "阿滩", "阿岚",
    ];
    let ids: Vec<String> = (1..=9).map(|n| format!("r{n}")).collect();
    let people: Vec<(&str, &str, &str)> = ids
        .iter()
        .zip(names)
        .map(|(id, name)| (id.as_str(), name, "loc_a"))
        .collect();
    // 吃喝睡关着（预制小镇的默认），四天里没人饿死渴死
    let fresh = || {
        let mut w = common::world_with(common::tiny_topology(), 7.0 * HOUR, &people);
        w.set_physiology(PhysiologySwitches {
            fullness: false,
            energy: false,
            water: false,
            health: false,
            weather: false,
        });
        w
    };
    let mut w = fresh();
    // 人不够开不了，原来的世界不受影响
    let mut few = common::world_with(common::tiny_topology(), 7.0 * HOUR, &people[..5]);
    assert!(few
        .set_game_mode(Some(&GameModeChoice::Werewolf {
            seed: Some(3),
            day_hours: None,
        }))
        .is_err());
    assert!(few.game_mode().is_none());

    w.set_game_mode(Some(&GameModeChoice::Werewolf {
        seed: Some(11),
        day_hours: Some(6),
    }))
    .unwrap();
    assert_eq!(w.game_mode(), Some("werewolf"));
    let v = werewolf(&w);
    assert_eq!(
        (v.phase, v.day, v.seats.len()),
        (WerewolfPhase::Waiting, 0, 9)
    );
    let count = |role| v.seats.iter().filter(|s| s.role == role).count();
    assert_eq!(
        (count(WerewolfRole::Wolf), count(WerewolfRole::Villager)),
        (3, 3)
    );
    let role_of = |seat: u32| v.seats[seat as usize - 1].role;
    let seat_of = |role| v.seats.iter().find(|s| s.role == role).unwrap().seat;
    let (seer, witch, hunter) = (
        seat_of(WerewolfRole::Seer),
        seat_of(WerewolfRole::Witch),
        seat_of(WerewolfRole::Hunter),
    );
    let villagers: Vec<u32> = v
        .seats
        .iter()
        .filter(|s| s.role == WerewolfRole::Villager)
        .map(|s| s.seat)
        .collect();
    let wolves: Vec<u32> = v
        .seats
        .iter()
        .filter(|s| s.role == WerewolfRole::Wolf)
        .map(|s| s.seat)
        .collect();
    // 开局前就有玩法动作工具（整局工具表不变，请求前缀能留在缓存里），但没有要交的
    assert!(w.game_mode_tool("r1").unwrap().contains("vote"));
    assert!(wolf_context(&w, "r1").actions.is_empty());

    // 没开玩法的世界按正常速度走；开局前是空档，走快一点
    assert_eq!(few.game_mode_tempo(), 1.0);
    assert_eq!(w.game_mode_tempo(), 2.0);
    // 第 1 天 10:30 天亮：每人单独收到自己的身份，狼人还知道同伴
    w.step(3.6 * HOUR);
    assert_eq!(werewolf(&w).phase, WerewolfPhase::Day);
    for (k, id) in ids.iter().enumerate() {
        let a = w.actor(id).unwrap();
        let notice = a
            .journal
            .events_after(0)
            .iter()
            .find(|e| e.kind == FactKind::Notice && e.text.contains("身份是"))
            .unwrap_or_else(|| panic!("{id} 没收到身份"))
            .text
            .clone();
        let seat = k as u32 + 1;
        assert_eq!(
            notice.contains("狼人同伴"),
            role_of(seat) == WerewolfRole::Wolf
        );
    }

    let id_of = |seat: u32| ids[seat as usize - 1].clone();
    let alive = |w: &World, seat: u32| werewolf(w).seats[seat as usize - 1].out.is_none();
    let mut skipped = false;
    let mut abstained = false;
    let mut reloaded = false;
    let mut told_to_stay = false;
    let mut died_at_night = false;
    let mut heard_reason = false;
    // 世界日志：中途读档换了世界，边走边收
    let mut logged = vec![];
    for _ in 0..5000 {
        logged.extend(w.take_events());
        let v = werewolf(&w);
        if v.phase == WerewolfPhase::Over {
            break;
        }
        // 发言、投票这些有期限的回合照常速；发言记下了交的时候写的打算（只给旁观的人看）
        if v.phase == WerewolfPhase::Speech {
            assert_eq!(w.game_mode_tempo(), 1.0);
            if let Some(line) = v.lines.last() {
                heard_reason |= line
                    .reason
                    .as_deref()
                    .is_some_and(|r| r.ends_with("心里这么想"));
            }
        }
        // 第 2 夜的刀口没人救：夜里的选择交齐就结算，人已经倒下，天亮前座位上还不公布
        if let Some(n) = v.night.as_ref().filter(|n| n.night == 2 && !died_at_night) {
            if v.phase == WerewolfPhase::Night && w.minutes() % (24.0 * HOUR) >= 20.0 * HOUR {
                let k = n.kill.expect("第 2 夜狼人选了刀口");
                assert!(w.actor(&id_of(k)).unwrap().needs.body.dead);
                assert_eq!(v.seats[k as usize - 1].out, None);
                assert_eq!(w.game_mode_tempo(), 2.0);
                died_at_night = true;
            }
        }
        // 黄昏回家：停留只认时长，法官把离天亮（10:30）还差几分钟算好
        if v.phase == WerewolfPhase::Dusk && !told_to_stay {
            told_to_stay = true;
            let now = wolf_context(&w, "r1").now;
            let left = (24.0 * HOUR + 10.5 * HOUR - (w.minutes() % (24.0 * HOUR))).ceil();
            assert!(
                now.contains(&format!("duration_minutes 填 {left:.0}")),
                "{now}"
            );
        }
        // 第 2 天第三个发言的人不交，到点跳过
        if v.phase == WerewolfPhase::Speech && v.day == 2 && v.lines.len() == 2 && !skipped {
            skipped = true;
            let silent = v.turn.unwrap();
            w.step(16.0);
            let after = werewolf(&w);
            assert_ne!(after.turn, Some(silent));
            assert_eq!(after.lines.len(), 2);
            continue;
        }
        // 第 3 天议事说到一半存档，读进一个新世界接着走
        if v.phase == WerewolfPhase::Speech && v.day == 3 && v.lines.len() == 2 && !reloaded {
            reloaded = true;
            w = common::restore(&w, fresh());
            assert_eq!(werewolf(&w), v);
        }
        let mut acted = false;
        for seat in 1..=9u32 {
            let id = id_of(seat);
            let Some(GameModeContext::Werewolf(c)) = w.game_mode_context(&id) else {
                continue;
            };
            let Some(first) = c.actions.first().cloned() else {
                continue;
            };
            let v = werewolf(&w);
            let lowest = |pred: &dyn Fn(u32) -> bool| (1..=9u32).find(|&s| alive(&w, s) && pred(s));
            match first.as_str() {
                "speak" => mode_act(&mut w, &id, "speak", None, Some("我是好人，大家听我说。")),
                "vote" | "abstain" => {
                    // 第 2 天最后一个人不投，到点算弃票
                    let voted = v.ballots.len();
                    let living = v.seats.iter().filter(|s| s.out.is_none()).count();
                    if v.day == 2 && voted + 1 == living && !abstained {
                        abstained = true;
                        w.step(16.0);
                        assert!(werewolf(&w).revealed);
                        acted = true;
                        break;
                    }
                    let target = if role_of(seat) == WerewolfRole::Wolf {
                        lowest(&|s| !wolves.contains(&s))
                    } else {
                        lowest(&|s| wolves.contains(&s))
                    };
                    mode_act(&mut w, &id, "vote", target, None);
                }
                "kill" => {
                    let target = lowest(&|s| villagers.contains(&s));
                    mode_act(&mut w, &id, "kill", target, Some("刀他。"));
                }
                "check" => {
                    let target = lowest(&|s| s != seat);
                    mode_act(&mut w, &id, "check", target, None);
                }
                "poison" | "save" | "pass" if seat == witch => {
                    let night = v.night.as_ref().map_or(0, |n| n.night);
                    let save = night == 1 && c.actions.iter().any(|a| a == "save");
                    mode_act(&mut w, &id, if save { "save" } else { "pass" }, None, None);
                }
                _ => mode_act(&mut w, &id, "pass", None, None),
            }
            acted = true;
            break;
        }
        if !acted {
            w.step(5.0);
        }
    }
    assert!(skipped && abstained && reloaded && told_to_stay && died_at_night && heard_reason);
    let v = werewolf(&w);
    // 议事、遗言、狼人频道写进日志时都带着「一段话」：发言人、座号、场合和原话，正文还是原来那一行
    logged.extend(w.take_events());
    let mut occasions = std::collections::BTreeSet::new();
    for Event::Log { text, info, .. } in &logged {
        let Some(said) = &info.speech else {
            continue;
        };
        let seat = said.seat.expect("狼人杀的发言带座号");
        let actor = info.actor.as_ref().expect("发言带发言人");
        assert_eq!(actor.id, id_of(seat));
        let name = &v.seats[seat as usize - 1].name;
        assert_eq!(&actor.name, name);
        assert!(text.contains(&format!("{seat} 号 {name}")), "{text}");
        assert!(text.ends_with(&format!("：{}", said.text)), "{text}");
        occasions.insert(said.occasion.clone());
    }
    for occasion in ["议事", "遗言", "狼人频道"] {
        assert!(
            occasions.contains(occasion),
            "日志里没有{occasion}：{occasions:?}"
        );
    }
    assert_eq!(
        (v.phase, v.winner),
        (WerewolfPhase::Over, Some(WerewolfSide::Good))
    );
    for &wolf in &wolves {
        assert_eq!(v.seats[wolf as usize - 1].out, Some(WerewolfOut::Exiled));
    }
    // 第 1 夜刀的平民被女巫救下，第 2 夜又被刀倒下，第 3 夜倒下第二个；最后一个平民和神职都活着
    assert_eq!(v.seats[villagers[2] as usize - 1].out, None);
    for &k in &villagers[..2] {
        assert_eq!(v.seats[k as usize - 1].out, Some(WerewolfOut::Killed));
        assert!(w.actor(&id_of(k)).unwrap().needs.body.dead);
    }
    for seat in [seer, witch, hunter] {
        assert_eq!(v.seats[seat as usize - 1].out, None);
    }
    let history = wolf_context(&w, &id_of(seer)).history;
    assert!(
        history.iter().any(|h| h == "第 1 夜：平安夜"),
        "{history:?}"
    );
    assert!(history.iter().any(|h| h.contains("弃票")), "{history:?}");
    // 被放逐的人说完遗言走出村子；出局的人和结束后都没有玩法动作工具
    w.step(30.0);
    for &wolf in &wolves {
        assert!(!w.actor(&id_of(wolf)).unwrap().present());
        assert!(w.game_mode_tool(&id_of(wolf)).is_none());
    }
    assert!(w.game_mode_tool(&id_of(seer)).is_none());

    w.set_game_mode(None).unwrap();
    no_trace(&w);
}

fn swordholder(w: &World) -> SwordholderView {
    match w.game_mode_view() {
        Some(GameModeView::Swordholder(v)) => v,
        other => panic!("开着的该是执剑人之夜：{other:?}"),
    }
}

/// 执剑人之夜走一整局：用给模型看的那一栏（swordholder.actions）当成模型，按固定打法交动作。
/// press 时：一个选民表态超时、投票 2 比 2 平票由现任执剑人指定 3 号，交接后执剑人按下开关，三体代表说最后一段话，
/// 投票中途存档读档；不按时：票多的 2 号当选，水滴来袭后执剑人一直不交（到点算没按），三体代表也不开口，法官替他宣布。
fn play_swordholder(press: bool) {
    let names = [
        "罗辑", "程心", "维德", "智子", "艾AA", "老秦", "林朵", "白杨", "小庄",
    ];
    let ids: Vec<String> = (1..=9).map(|n| format!("r{n}")).collect();
    let people: Vec<(&str, &str, &str)> = ids
        .iter()
        .zip(names)
        .map(|(id, name)| (id.as_str(), name, "loc_a"))
        .collect();
    let fresh = || {
        let mut w = common::world_with(common::tiny_topology(), 7.0 * HOUR, &people);
        w.set_physiology(PhysiologySwitches {
            fullness: false,
            energy: false,
            water: false,
            health: false,
            weather: false,
        });
        w
    };
    let mut w = fresh();
    // 人不够开不了，原来的世界不受影响
    let mut few = common::world_with(common::tiny_topology(), 7.0 * HOUR, &people[..5]);
    assert!(few
        .set_game_mode(Some(&GameModeChoice::Swordholder {
            seed: Some(3),
            day_hours: None,
            rules: None,
        }))
        .is_err());
    assert!(few.game_mode().is_none());

    w.set_game_mode(Some(&GameModeChoice::Swordholder {
        seed: Some(5),
        day_hours: Some(8),
        rules: None,
    }))
    .unwrap();
    assert_eq!(w.game_mode(), Some("swordholder"));
    // 没改规则：存档里没有规则这一项，和加规则之前的存档一字不差
    let saved = serde_json::to_value(w.snapshot()).unwrap();
    assert!(saved["game_mode"]["swordholder"].get("rules").is_none());
    let v = swordholder(&w);
    let roles: Vec<SwordholderRole> = v.seats.iter().map(|s| s.role).collect();
    use SwordholderRole::*;
    assert_eq!(
        roles,
        [Holder, Candidate, Candidate, Envoy, Voter, Voter, Voter, Voter, Voter]
    );
    assert_eq!((v.phase, v.swordholder), (SwordholderPhase::Waiting, 1));
    // 开局前就有玩法动作工具（整局工具表不变），但没有要交的；空档走快一点
    assert!(w.game_mode_tool("r5").unwrap().contains("press"));
    assert_eq!(w.game_mode_tempo(), 3.0);
    // 08:30 天亮：每人单独收到自己的座号和身份
    w.step(1.6 * HOUR);
    assert_eq!(swordholder(&w).phase, SwordholderPhase::Day);
    for id in &ids {
        let told = w
            .actor(id)
            .unwrap()
            .journal
            .events_after(0)
            .iter()
            .any(|e| e.kind == FactKind::Notice && e.text.contains("身份是"));
        assert!(told, "{id} 没收到身份");
    }

    let id_of = |seat: u32| ids[seat as usize - 1].clone();
    let mut skipped = false;
    let mut reloaded = false;
    let mut logged = vec![];
    for _ in 0..3000 {
        logged.extend(w.take_events());
        let v = swordholder(&w);
        if v.phase == SwordholderPhase::Over {
            break;
        }
        // 有期限的回合照常速
        if v.phase == SwordholderPhase::Speech {
            assert_eq!(w.game_mode_tempo(), 1.0);
        }
        // 第三个表态的选民不交，到点跳过
        if press && v.phase == SwordholderPhase::Speech && v.lines.len() == 5 && !skipped {
            skipped = true;
            let silent = v.turn.unwrap();
            assert_eq!(v.seats[silent as usize - 1].role, Voter);
            w.step(16.0);
            let after = swordholder(&w);
            assert_ne!(after.turn, Some(silent));
            assert_eq!(after.lines.len(), 5);
            continue;
        }
        // 投票交了两张时存档，读进一个新世界接着走；亮票前看不到票投给了谁
        if press && v.phase == SwordholderPhase::Vote && v.ballots.len() == 2 && !reloaded {
            reloaded = true;
            assert!(v.ballots.iter().all(|b| b.target.is_none()));
            w = common::restore(&w, fresh());
            assert_eq!(swordholder(&w), v);
        }
        let mut acted = false;
        for seat in 1..=9u32 {
            let id = id_of(seat);
            let Some(GameModeContext::Swordholder(c)) = w.game_mode_context(&id) else {
                continue;
            };
            let Some(first) = c.actions.first().cloned() else {
                continue;
            };
            match first.as_str() {
                "speak" if press || v.phase != SwordholderPhase::Aftermath => {
                    mode_act(&mut w, &id, "speak", None, Some("我想说几句。"))
                }
                "vote" => {
                    // press 时 5、6 号投 2 号，7、8 号投 3 号，9 号弃票：平票；不按时都投 2 号
                    let target = match seat {
                        _ if !press => Some(2),
                        5 | 6 => Some(2),
                        7 | 8 => Some(3),
                        _ => None,
                    };
                    let action = if target.is_some() { "vote" } else { "abstain" };
                    mode_act(&mut w, &id, action, target, None);
                }
                "appoint" => mode_act(&mut w, &id, "appoint", Some(3), None),
                "press" if press => mode_act(&mut w, &id, "press", None, Some("按下。")),
                // 执剑人一直不交，到点算没按；三体代表也不开口
                _ => {
                    w.step(5.0);
                }
            }
            acted = true;
            break;
        }
        if !acted {
            w.step(5.0);
        }
    }
    logged.extend(w.take_events());
    let v = swordholder(&w);
    assert_eq!(v.phase, SwordholderPhase::Over);
    let history = match w.game_mode_context("r5") {
        Some(GameModeContext::Swordholder(c)) => c.history,
        other => panic!("{other:?}"),
    };
    let occasions: std::collections::BTreeSet<String> =
        v.lines.iter().map(|l| l.occasion.clone()).collect();
    if press {
        assert!(skipped && reloaded);
        assert_eq!((v.elected, v.appointed, v.swordholder), (Some(3), true, 3));
        assert!(history.iter().any(|h| h.contains("平票")), "{history:?}");
        let d = v.decision.as_ref().expect("执剑人交了决定");
        assert_eq!((d.seat, d.press), (3, true));
        assert!(d
            .reason
            .as_deref()
            .is_some_and(|r| r.ends_with("心里这么想")));
        assert_eq!(v.ending, Some(SwordholderEnding::Broadcast));
        for occasion in ["一句话", "演说", "表态", "交接", "宣布"] {
            assert!(occasions.contains(occasion), "{occasions:?}");
        }
        // 亮票后票型和理由都给旁观的人看
        assert_eq!(v.ballots.len(), 5);
        assert!(v.ballots.iter().any(|b| b.target == Some(3)));
    } else {
        assert_eq!((v.elected, v.appointed, v.swordholder), (Some(2), false, 2));
        assert!(v.decision.is_none());
        assert_eq!(v.ending, Some(SwordholderEnding::Failed));
        assert!(!occasions.contains("宣布"));
        assert!(
            history.iter().any(|h| h.contains("迁往澳大利亚")),
            "{history:?}"
        );
    }
    // 说过的话写进日志时都带着「一段话」：发言人、座号、场合和原话
    let spoken = logged
        .iter()
        .filter(|Event::Log { info, .. }| info.speech.is_some())
        .count();
    assert_eq!(spoken, v.lines.len());
    // 一局结束：没有玩法动作工具了，世界照常走；关掉后存档和名册里没有玩法的痕迹
    assert!(w.game_mode_tool("r3").is_none());
    w.set_game_mode(None).unwrap();
    no_trace(&w);
}

#[test]
fn swordholder_night_elects_hands_over_and_ends_when_the_switch_is_pressed() {
    play_swordholder(true);
}

#[test]
fn swordholder_night_fails_when_the_swordholder_never_presses() {
    play_swordholder(false);
}
