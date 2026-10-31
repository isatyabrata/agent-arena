use lab_core::*;

#[test]
fn demo_feed_runs_full_cycles_and_finishes_matches() {
    let mut w = World::new();
    let mut d = DemoFeed::new(42);
    let mut matches_ended = 0;
    for _ in 0..(20 * 400) {
        for e in d.step(&w, 0.05) {
            if matches!(e, Event::ArenaEnd { .. }) {
                matches_ended += 1;
            }
            // every event must round-trip through JSON (the plug-in protocol)
            let s = serde_json::to_string(&e).unwrap();
            let back: Event = serde_json::from_str(&s).unwrap();
            w.apply(&back);
        }
        w.tick(0.05);
    }
    assert!(w.agents.len() >= 23);
    assert!(matches_ended >= 2, "only {matches_ended} matches ended");
    // snapshot round-trip
    let snap = serde_json::to_string(&w).unwrap();
    let _: World = serde_json::from_str(&snap).unwrap();
}

#[test]
fn external_agents_are_left_alone_by_demo() {
    let mut w = World::new();
    w.apply(&Event::Join { id: "r1".into(), name: "Real".into(), role: Role::Researcher, model: None, harness: None, bio: None, color: None });
    w.agents[0].external = true;
    let mut d = DemoFeed::new(1);
    for _ in 0..5000 {
        for e in d.step(&w, 0.05) {
            assert_ne!(e.agent_id(), Some("r1"));
            w.apply(&e);
        }
        w.tick(0.05);
    }
}

#[test]
fn arena_actions_build_blueprint() {
    let mut w = World::new();
    w.apply(&Event::ArenaStart {
        title: "T".into(), description: String::new(), goal: String::new(), cols: 2, rows: 1, parts: vec![], target: vec![1, 2],
        robots: vec![RobotSpec { name: "m".into(), color: 0, profile: String::new(), owner: String::new() }],
    });
    for (c, color) in [(0, 1u8), (1, 2u8)] {
        w.apply(&Event::ArenaAction { robot: 0, action: RobotAction::Pick { color }, intent: None });
        w.apply(&Event::ArenaAction { robot: 0, action: RobotAction::Place { col: c, row: 0 }, intent: None });
    }
    assert!(w.arena.complete(0));
    assert!(w.arena.robots[0].finished_at.is_some());
}

fn join(w: &mut World, id: &str, role: Role) {
    w.apply(&Event::Join { id: id.into(), name: id.into(), role, model: Some("m".into()), harness: None, bio: None, color: None });
}

#[test]
fn rooms_grow_to_fit_any_number_of_agents() {
    let mut w = World::new();
    let h0 = w.layout.h;
    for i in 0..40 {
        join(&mut w, &format!("e{i}"), Role::Engineer);
    }
    assert_eq!(w.agents.len(), 40);
    assert!(w.layout.h > h0, "world should grow");
    // every engineer has its own desk and sits at it
    let mut seats: Vec<_> = w.agents.iter().map(|a| a.seat().unwrap()).collect();
    seats.dedup();
    assert_eq!(seats.len(), 40);
    assert!(w.agents.iter().all(|a| a.seated()));
}

#[test]
fn roster_from_config_starts_offline_then_comes_online() {
    let mut cfg = Config::default();
    cfg.agents.push(AgentCfg { id: "ada".into(), name: "Ada".into(), role: Some(Role::Researcher), model: "claude".into(), ..Default::default() });
    let mut w = World::with_config(cfg);
    assert!(!w.agent("ada").unwrap().online);
    w.apply(&Event::Output { id: "ada".into(), text: "hello\nworld".into() });
    let a = w.agent("ada").unwrap();
    assert!(a.online);
    assert_eq!(a.terminal.len(), 2);
}

#[test]
fn runtime_defined_open_task_with_custom_parts() {
    let mut w = World::new();
    let ev: Event = serde_json::from_str(r##"{"type":"arena_start","title":"CROSSWORD","goal":"most words","cols":4,"rows":4,
        "parts":[{"name":"letter A","color":"#ffffff","glyph":"A"},{"name":"letter B","color":"#ffffff","glyph":"B"}],
        "robots":[{"name":"m1"},{"name":"m2"}]}"##).unwrap();
    w.apply(&ev);
    assert!(!w.arena.has_target());
    let a: Event = serde_json::from_str(r#"{"type":"arena_action","robot":1,"intent":"fill row 1","action":{"do":"set_cell","col":0,"row":0,"part":2}}"#).unwrap();
    w.apply(&a);
    let p: Event = serde_json::from_str(r#"{"type":"arena_action","robot":1,"action":{"do":"progress","value":0.5}}"#).unwrap();
    w.apply(&p);
    w.apply(&Event::ArenaScore { robot: 1, score: 9.0, note: "4 words".into() });
    assert_eq!(w.arena.robots[1].grid[0], 2);
    assert_eq!(w.arena.progress(1), 0.5);
    assert_eq!(w.arena.robots[1].intent, "fill row 1");
    let j = w.panel_json(None);
    assert!(j.contains("CROSSWORD"));
}

#[test]
fn shipped_lab_toml_parses() {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../lab.toml")).unwrap();
    let cfg: Config = toml::from_str(&text).unwrap();
    assert!(cfg.arena.tasks.len() >= 3);
    assert_eq!(cfg.rooms.len(), 6);
    // and survives a round trip, as POST /api/config writes it back
    let back: Config = toml::from_str(&toml::to_string_pretty(&cfg).unwrap()).unwrap();
    assert_eq!(back, cfg);
}

#[test]
fn reported_status_say_and_notes_show_in_the_terminal() {
    let mut w = World::new();
    w.apply(&Event::Join { id: "e9".into(), name: "Real".into(), role: Role::Engineer, model: None, harness: None, bio: None, color: None });
    let st = |task: &str| Event::Status { id: "e9".into(), activity: Activity::Typing, task: Some(task.into()), screen: Some(vec!["main.rs".into()]) };
    w.apply(&st("writing loader"));
    w.apply(&st("writing loader")); // repeat is not duplicated
    w.apply(&Event::Say { id: "e9".into(), text: "found a bug".into() });
    w.apply(&Event::Note { kind: NoteKind::Works, text: "loader ok".into(), by: "e9".into() });
    let t: Vec<&str> = w.agents[0].terminal.iter().map(|s| s.as_str()).collect();
    assert_eq!(t, ["[status] typing: writing loader", "[screen] main.rs", "[say] found a bug", "[works] loader ok"]);
}

#[test]
fn busy_office_working_at_desk_idle_in_lounge_and_visits() {
    let mut w = World::new();
    for id in ["a", "b", "c"] {
        w.apply(&Event::Join { id: id.into(), name: id.to_uppercase(), role: Role::Engineer, model: None, harness: None, bio: None, color: None });
    }
    w.apply(&Event::Status { id: "a".into(), activity: Activity::Typing, task: Some("coding".into()), screen: None });
    for _ in 0..1200 {
        w.tick(0.05);
    }
    let by = |w: &World, id: &str| w.agent(id).unwrap().clone();
    assert!(by(&w, "a").working(w.t) && by(&w, "a").seated(), "worker stays at desk");
    assert!(!by(&w, "b").working(w.t) && !by(&w, "b").seated(), "idle agent left the desk");
    assert_eq!(w.agents.iter().filter(|a| a.working(w.t)).count(), 1);
    // a speaks to c: walks over next to c
    w.apply(&Event::Say { id: "a".into(), text: "@c please review".into() });
    assert!(!by(&w, "a").path.is_empty());
    for _ in 0..1200 {
        w.tick(0.05);
        if w.agent("a").unwrap().path.is_empty() {
            break;
        }
    }
    let (a, c) = (by(&w, "a"), by(&w, "c"));
    assert!((a.x - c.x).abs() < 14.0 && (a.y - c.y).abs() < 14.0, "a stands next to c");
    // back to the desk after the bubble fades
    for _ in 0..1200 {
        w.tick(0.05);
    }
    assert!(by(&w, "a").seated());
    // stale workers are no longer counted
    for _ in 0..5000 {
        w.tick(0.25);
    }
    assert_eq!(w.agents.iter().filter(|a| a.working(w.t)).count(), 0);
}
