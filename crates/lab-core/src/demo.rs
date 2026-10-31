//! Demo feed: a scripted stand-in for real harnesses, so the world is never empty.
//! It emits exactly the events a real agent would. Agents a real harness has taken
//! over (marked `external`) are skipped. Nothing here is real research: the robot
//! "models" are scripted policies with a random skill value.

use crate::config::TaskCfg;
use crate::event::*;
use crate::layout::{WORLD_COLS, WORLD_ROWS};
use crate::rng::Rng;
use crate::world::World;
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Cycle {
    Boot,
    Work,
    Training,
    Testing,
    Review,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum PStep {
    Idle,
    ToPile,
    ToCell,
    ToFix,
    Done,
}

struct Plan {
    skill: f32,
    cool: f32,
    step: PStep,
    target: usize,
}

struct Job {
    model: String,
    owner: String,
    start: f32,
    end: f32,
    profile: String,
}

pub struct DemoFeed {
    rng: Rng,
    cycle: Cycle,
    st: f32,
    entered: bool,
    timers: BTreeMap<String, f32>,
    train_t: f32,
    jobs: Vec<Job>,
    plans: Vec<Plan>,
    first_finish: Option<f32>,
    round: u32,
    world_joined: bool,
    world_tick: u64,
    world_timer: f32,
}

const ARCH: &[&str] = &["ember-mesh", "tidal-loom", "lattice-weave", "quasar-drift", "helix-bloom", "myco-shard", "aurora-kernel", "cinder-forge"];

fn goto(id: &str, place: &str) -> Event {
    Event::Goto { id: id.into(), place: place.into() }
}
fn status(id: &str, a: Activity, task: &str, screen: &[&str]) -> Event {
    Event::Status {
        id: id.into(),
        activity: a,
        task: Some(task.into()),
        screen: Some(screen.iter().map(|s| s.to_string()).collect()),
    }
}
fn say(id: &str, text: &str) -> Event {
    Event::Say { id: id.into(), text: text.into() }
}
fn out(id: &str, text: impl Into<String>) -> Event {
    Event::Output { id: id.into(), text: text.into() }
}

fn ids_of(w: &World, role: Role) -> Vec<String> {
    w.agents.iter().filter(|a| a.role == role && !a.external).map(|a| a.id.clone()).collect()
}

impl DemoFeed {
    pub fn new(seed: u64) -> Self {
        DemoFeed {
            rng: Rng::new(seed),
            cycle: Cycle::Boot,
            st: 0.0,
            entered: false,
            timers: BTreeMap::new(),
            train_t: 0.0,
            jobs: vec![],
            plans: vec![],
            first_finish: None,
            round: 0,
            world_joined: false,
            world_tick: 0,
            world_timer: 0.0,
        }
    }

    fn enter(&mut self, c: Cycle) {
        self.cycle = c;
        self.st = 0.0;
        self.entered = false;
    }

    /// Advance the feed and return the events to apply this frame.
    pub fn step(&mut self, w: &World, dt: f32) -> Vec<Event> {
        let mut ev = Vec::new();
        self.st += dt;
        let first = !self.entered;
        self.entered = true;
        match self.cycle {
            Cycle::Boot => {
                if w.config.agents.is_empty() {
                    let n = w.config.lab.demo_per_role.clamp(1, 30);
                    let mut roster: Vec<(String, String, Role)> = vec![
                        ("captain".into(), "Captain".into(), Role::Captain),
                        ("scribe".into(), "Scribe".into(), Role::Scribe),
                        ("auditor".into(), "Auditor".into(), Role::Auditor),
                    ];
                    for (p, label, role) in [
                        ("r", "Researcher", Role::Researcher),
                        ("s", "Scientist", Role::Scientist),
                        ("e", "Engineer", Role::Engineer),
                        ("t", "Tester", Role::Tester),
                    ] {
                        for i in 1..=n {
                            roster.push((format!("{p}{i}"), format!("{label} {i}"), role));
                        }
                    }
                    for (id, name, role) in roster {
                        if w.agent(&id).is_none() {
                            ev.push(Event::Join {
                                id,
                                name,
                                role,
                                model: Some("sample".into()),
                                harness: Some("demo feed".into()),
                                bio: Some("Scripted sample agent. Replace with a real harness.".into()),
                                color: None,
                            });
                        }
                    }
                }
                ev.push(Event::Log { text: "DEMO FEED: sample agents, scripted robots".into() });
                self.enter(Cycle::Work);
            }
            Cycle::Work => {
                if first {
                    self.round += 1;
                    ev.push(Event::Phase { name: format!("WORK ROUND {}", self.round) });
                    if let Some(c) = ids_of(w, Role::Captain).first() {
                        ev.push(say(c, "Morning team. Plug in when ready."));
                    }
                }
                if self.st > 22.0 {
                    self.enter(Cycle::Training);
                }
            }
            Cycle::Training => {
                if first {
                    ev.push(Event::Phase { name: "TRAINING".into() });
                    if let Some(c) = ids_of(w, Role::Captain).first() {
                        ev.push(goto(c, "cluster"));
                    }
                    self.jobs.clear();
                    let engineers = ids_of(w, Role::Engineer);
                    let n = w.layout.lanes.max(engineers.len()).min(8);
                    for i in 0..n {
                        let owner = engineers.get(i % engineers.len().max(1)).cloned().unwrap_or_default();
                        let arch = ARCH[(self.round as usize + i) % ARCH.len()];
                        let model = format!("{}-r{}", arch, self.round);
                        let params = self.rng.between(0.4, 3.0);
                        let profile = format!("arch {arch}, {params:.1}M params, finetuned on blueprints-v{}", self.round);
                        let (s, e) = (self.rng.between(2.8, 3.4), self.rng.between(0.3, 1.4));
                        if !owner.is_empty() {
                            ev.push(goto(&owner, "desk"));
                            ev.push(out(&owner, format!("$ cargo run --release --bin train -- --arch {arch} --params {params:.1}M")));
                        }
                        self.jobs.push(Job { model, owner, start: s, end: e, profile });
                    }
                    self.train_t = 0.0;
                }
                self.train_t += dt;
                if self.train_t >= 1.0 {
                    self.train_t = 0.0;
                    let p = (self.st / 24.0).min(1.0);
                    for k in 0..self.jobs.len() {
                        let (model, owner, s, f) = {
                            let j = &self.jobs[k];
                            (j.model.clone(), j.owner.clone(), j.start, j.end)
                        };
                        let stage = if p >= 1.0 {
                            Stage::Done
                        } else if p < 0.6 {
                            Stage::Pretrain
                        } else {
                            Stage::Finetune
                        };
                        let bump = if stage == Stage::Finetune { 0.4 * (-(p - 0.6) * 12.0).exp() } else { 0.0 };
                        let loss = f + (s - f) * (-4.0 * p).exp() + bump + self.rng.between(-0.04, 0.04);
                        ev.push(Event::ModelProgress { model: model.clone(), owner: owner.clone(), stage, progress: p, loss });
                        if owner.is_empty() {
                            continue;
                        }
                        let pct = format!("{:?} {:.0}%", stage, p * 100.0).to_uppercase();
                        let l = format!("LOSS {:.3}", loss);
                        let title = format!("TRAIN {}", model);
                        ev.push(status(&owner, Activity::Training, &title, &[&title, &pct, &l]));
                        ev.push(out(&owner, format!("[{}] step {:>5}  {:?}  loss {:.4}", model, (p * 5000.0) as u32, stage, loss)));
                    }
                    if p >= 1.0 {
                        self.enter(Cycle::Testing);
                    }
                }
            }
            Cycle::Testing => {
                if first {
                    self.start_match(w, &mut ev);
                } else if w.arena.running {
                    self.drive_robots(w, dt, &mut ev);
                }
            }
            Cycle::Review => {
                if first {
                    ev.push(Event::Phase { name: "REVIEW".into() });
                    let scientists = ids_of(w, Role::Scientist);
                    let s0 = scientists.first().cloned().unwrap_or_default();
                    if let Some(r) = w.arena.ranking.first().and_then(|i| w.arena.robots.get(*i)) {
                        ev.push(Event::Note { kind: NoteKind::Info, text: format!("sample: {} won {}", r.name, w.arena.title), by: s0.clone() });
                    }
                    ev.push(Event::Note { kind: NoteKind::Works, text: "sample: fixing wrong blocks early saves time".into(), by: s0.clone() });
                    ev.push(Event::Note { kind: NoteKind::Fails, text: "sample: rushing picks causes rework".into(), by: s0.clone() });
                    for id in scientists.iter().take(3) {
                        ev.push(goto(id, "notes"));
                        ev.push(out(id, format!("graded match {}: {}", w.arena.matches, w.arena.title)));
                    }
                    for id in ids_of(w, Role::Tester).iter().chain(ids_of(w, Role::Auditor).iter()) {
                        ev.push(goto(id, "desk"));
                    }
                    if !s0.is_empty() {
                        ev.push(say(&s0, "Posting results to the board"));
                    }
                }
                if self.st > 12.0 {
                    self.enter(Cycle::Work);
                }
            }
        }
        self.world_step(w, dt, &mut ev);
        if self.cycle != Cycle::Boot {
            self.ambient(w, dt, &mut ev);
        }
        // never drive agents a real harness has taken over
        ev.retain(|e| match e.agent_id() {
            Some(id) => !w.agent(id).is_some_and(|a| a.external),
            None => true,
        });
        ev
    }

    fn start_match(&mut self, w: &World, ev: &mut Vec<Event>) {
        ev.push(Event::Phase { name: "TESTING".into() });
        let fallback = crate::config::ArenaCfg::default().tasks;
        let tasks: &Vec<TaskCfg> = if w.config.arena.tasks.is_empty() { &fallback } else { &w.config.arena.tasks };
        let task = self.rng.pick(tasks).clone();
        let (cols, rows, bp) = task.grid();
        self.jobs.sort_by(|a, b| a.end.partial_cmp(&b.end).unwrap());
        let lanes = w.layout.lanes;
        let picked: Vec<&Job> = self.jobs.iter().take(lanes).collect();
        let robots = picked
            .iter()
            .enumerate()
            .map(|(i, j)| RobotSpec { name: j.model.clone(), color: i as u8, profile: j.profile.clone(), owner: j.owner.clone() })
            .collect();
        self.plans = picked
            .iter()
            .map(|j| Plan {
                skill: (1.0 - (j.end - 0.3) / 1.3).clamp(0.1, 0.95) * 0.7 + self.rng.f32() * 0.3,
                cool: 1.0,
                step: PStep::Idle,
                target: 0,
            })
            .collect();
        self.first_finish = None;
        ev.push(Event::ArenaStart {
            title: task.name.clone(),
            description: task.description.clone(),
            goal: task.goal.clone(),
            cols,
            rows,
            parts: task.parts.clone(),
            target: bp,
            robots,
        });
        for (i, id) in ids_of(w, Role::Tester).iter().take(lanes).enumerate() {
            ev.push(goto(id, "arena"));
            let screen = format!("WATCHING LANE {}", i + 1);
            ev.push(status(id, Activity::Testing, "refereeing the arena", &[&screen]));
            ev.push(out(id, format!("referee: lane {} armed for {}", i + 1, task.name)));
        }
        for id in ids_of(w, Role::Captain).iter().chain(ids_of(w, Role::Auditor).iter()) {
            ev.push(goto(id, "arena"));
        }
        for id in ids_of(w, Role::Auditor) {
            ev.push(status(&id, Activity::Auditing, "checking robots see only the blueprint", &["NO HIDDEN STATE", "NO PRESOLVED GRID"]));
            ev.push(out(&id, "audit: robot inputs = blueprint + own lane only. OK"));
        }
    }

    fn drive_robots(&mut self, w: &World, dt: f32, ev: &mut Vec<Event>) {
        if !w.arena.has_target() {
            return self.drive_open(w, dt, ev);
        }
        let a = &w.arena;
        let cols = a.cols as i32;
        let st = self.st;
        for (i, p) in self.plans.iter_mut().enumerate() {
            let Some(r) = a.robots.get(i) else { continue };
            if self.first_finish.is_none() && r.finished_at.is_some() {
                self.first_finish = Some(st);
            }
            p.cool -= dt;
            if p.cool > 0.0 || !r.arrived() || p.step == PStep::Done {
                continue;
            }
            let act = |action, intent: String| Event::ArenaAction { robot: i, action, intent: Some(intent) };
            let cell = |idx: usize| ((idx as i32) % cols, (idx as i32) / cols);
            match p.step {
                PStep::Idle | PStep::Done => {
                    let wrong = (0..a.blueprint.len()).find(|&k| r.grid[k] != 0 && r.grid[k] != a.blueprint[k]);
                    let next = (0..a.rows)
                        .rev()
                        .flat_map(|row| (0..a.cols).map(move |c| row * a.cols + c))
                        .find(|&k| a.blueprint[k] != 0 && r.grid[k] == 0);
                    if let Some(k) = wrong.filter(|_| self.rng.chance(0.3 + 0.6 * p.skill)) {
                        let (c, rw) = cell(k);
                        ev.push(act(RobotAction::Think { text: "wrong block!".into() }, format!("fix wrong block at ({c},{rw})")));
                        ev.push(act(RobotAction::MoveTo { col: c, row: rw }, format!("fix wrong block at ({c},{rw})")));
                        p.target = k;
                        p.step = PStep::ToFix;
                    } else if let Some(k) = next {
                        let (c, rw) = cell(k);
                        let col = a.blueprint[k];
                        let it = format!("fetch {} for ({c},{rw})", a.part_name(col));
                        ev.push(act(RobotAction::MoveTo { col: col as i32 - 1, row: -1 }, it));
                        p.target = k;
                        p.step = PStep::ToPile;
                    } else if wrong.is_none() {
                        ev.push(act(RobotAction::Think { text: "done!".into() }, "finished".into()));
                        p.step = PStep::Done;
                    } else {
                        ev.push(act(RobotAction::Think { text: "checking...".into() }, "inspecting the build".into()));
                    }
                }
                PStep::ToPile => {
                    let want = a.blueprint[p.target];
                    let np = a.parts.len().max(1) as u8;
                    let color = if self.rng.chance(0.6 + 0.4 * p.skill) { want } else { (want % np) + 1 };
                    let (c, rw) = cell(p.target);
                    let it = format!("carry {} to ({c},{rw})", a.part_name(color));
                    ev.push(act(RobotAction::Pick { color }, it.clone()));
                    ev.push(act(RobotAction::MoveTo { col: c, row: rw }, it));
                    p.step = PStep::ToCell;
                }
                PStep::ToCell => {
                    let (c, rw) = cell(p.target);
                    ev.push(act(RobotAction::Place { col: c, row: rw }, format!("place block at ({c},{rw})")));
                    p.step = PStep::Idle;
                }
                PStep::ToFix => {
                    let (c, rw) = cell(p.target);
                    ev.push(act(RobotAction::Remove { col: c, row: rw }, format!("remove wrong block at ({c},{rw})")));
                    p.step = PStep::Idle;
                }
            }
            p.cool = 0.2 + (1.0 - p.skill) * self.rng.between(0.2, 0.9);
        }
        let all_done = self.plans.iter().all(|p| p.step == PStep::Done);
        let timeout = self.first_finish.is_some_and(|f| st > f + 6.0) || st > 90.0;
        if all_done || timeout {
            let mut idx: Vec<usize> = (0..a.robots.len()).collect();
            idx.sort_by(|&x, &y| {
                let fx = a.robots[x].finished_at.unwrap_or(f32::MAX);
                let fy = a.robots[y].finished_at.unwrap_or(f32::MAX);
                fx.partial_cmp(&fy).unwrap().then(a.correct(y).cmp(&a.correct(x)))
            });
            let note = idx
                .first()
                .and_then(|&i| a.robots.get(i))
                .map(|r| match r.finished_at {
                    Some(t) => format!("{} finished first in {:.1}s", r.name, t),
                    None => format!("{} placed the most correct blocks", r.name),
                })
                .unwrap_or_default();
            ev.push(Event::ArenaEnd { ranking: idx, note });
            self.enter(Cycle::Review);
        }
    }

    /// Open-ended task (no target): stack parts as high as possible; a referee scores at the end.
    fn drive_open(&mut self, w: &World, dt: f32, ev: &mut Vec<Event>) {
        let a = &w.arena;
        let (cols, rows) = (a.cols as i32, a.rows as i32);
        let np = a.parts.len().max(1);
        let height = |g: &Vec<u8>| -> i32 {
            (0..rows).find(|&r| (0..cols).any(|c| g[(r * cols + c) as usize] != 0)).map(|r| rows - r).unwrap_or(0)
        };
        for (i, p) in self.plans.iter_mut().enumerate() {
            let Some(r) = a.robots.get(i) else { continue };
            p.cool -= dt;
            if p.cool > 0.0 || !r.arrived() {
                continue;
            }
            let act = |action, intent: String| Event::ArenaAction { robot: i, action, intent: Some(intent) };
            match p.step {
                PStep::Idle | PStep::Done | PStep::ToFix => {
                    let part = 1 + self.rng.range(np) as u8;
                    ev.push(act(RobotAction::MoveTo { col: part as i32 - 1, row: -1 }, format!("fetch {} to build higher", a.part_name(part))));
                    p.target = part as usize;
                    p.step = PStep::ToPile;
                }
                PStep::ToPile => {
                    // stack on the column with the tallest pile (skilled) or a random one
                    let col = if self.rng.chance(p.skill) { cols / 2 } else { self.rng.range(cols as usize) as i32 };
                    let row = (0..rows).rev().find(|&rw| r.grid[(rw * cols + col) as usize] == 0).unwrap_or(0);
                    ev.push(act(RobotAction::Pick { color: p.target as u8 }, format!("stack on column {col}")));
                    ev.push(act(RobotAction::MoveTo { col, row }, format!("stack on column {col}")));
                    p.target = (row * cols + col) as usize;
                    p.step = PStep::ToCell;
                }
                PStep::ToCell => {
                    let (c, rw) = ((p.target as i32) % cols, (p.target as i32) / cols);
                    ev.push(act(RobotAction::Place { col: c, row: rw }, format!("place at ({c},{rw})")));
                    let mut g = r.grid.clone();
                    g[p.target] = 1;
                    let h = height(&g);
                    ev.push(act(RobotAction::Progress { value: h as f32 / rows as f32 }, format!("tower height {h}")));
                    ev.push(act(RobotAction::Metric { name: "height".into(), value: h.to_string() }, format!("tower height {h}")));
                    if h > 3 && self.rng.chance(0.12 * (1.0 - p.skill)) {
                        ev.push(act(RobotAction::Think { text: "it wobbles!".into() }, "re-balancing".into()));
                        ev.push(act(RobotAction::Remove { col: c, row: rw }, "re-balancing".into()));
                    }
                    p.step = PStep::Idle;
                }
            }
            p.cool = 0.2 + (1.0 - p.skill) * self.rng.between(0.2, 0.9);
        }
        if self.st > 45.0 {
            let mut scores: Vec<(usize, f32)> = (0..a.robots.len())
                .map(|i| {
                    let h = height(&a.robots[i].grid) as f32;
                    let filled = a.robots[i].grid.iter().filter(|c| **c != 0).count() as f32;
                    (i, h * 10.0 + filled * 0.5)
                })
                .collect();
            for (i, s) in &scores {
                ev.push(Event::ArenaScore { robot: *i, score: *s, note: "height x10 + parts x0.5".into() });
            }
            scores.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap());
            let note = scores.first().map(|(i, s)| format!("{} scored {:.1}", a.robots[*i].name, s)).unwrap_or_default();
            ev.push(Event::ArenaEnd { ranking: scores.iter().map(|x| x.0).collect(), note });
            self.enter(Cycle::Review);
        }
    }

    /// THE WORLD (world/DESIGN.md §4): 2 stub residents wander the tile grid; real models
    /// arrive at M4. Ticks are batched at ~2.5-4/s, never one event per resident.
    fn world_step(&mut self, w: &World, dt: f32, ev: &mut Vec<Event>) {
        if !self.world_joined {
            self.world_joined = true;
            for (id, name, model) in [("res1", "Pip", "stub-explorer"), ("res2", "Mochi", "stub-builder")] {
                if w.world.residents.iter().any(|r| r.id == id) {
                    continue;
                }
                ev.push(Event::WorldJoin {
                    resident_id: id.into(),
                    name: name.into(),
                    model: model.into(),
                    owner: "demo".into(),
                    card: [None; 7],
                });
            }
        }
        self.world_timer -= dt;
        if self.world_timer > 0.0 {
            return;
        }
        self.world_timer = self.rng.between(0.25, 0.4);
        self.world_tick += 1;
        let intents = ["exploring", "wandering", "resting", "looking around", "thinking"];
        let residents: Vec<WorldResidentTick> = w
            .world
            .residents
            .iter()
            .map(|r| {
                let (mut x, mut y) = (r.x, r.y);
                if x == 0.0 && y == 0.0 {
                    x = self.rng.between(2.0, WORLD_COLS as f32 - 2.0);
                    y = self.rng.between(2.0, WORLD_ROWS as f32 - 2.0);
                }
                x = (x + self.rng.between(-1.2, 1.2)).clamp(1.0, WORLD_COLS as f32 - 1.0);
                y = (y + self.rng.between(-1.2, 1.2)).clamp(1.0, WORLD_ROWS as f32 - 1.0);
                let intent = *self.rng.pick(&intents);
                WorldResidentTick { resident_id: r.id.clone(), x, y, awake: r.awake, intent: intent.into() }
            })
            .collect();
        if !residents.is_empty() {
            ev.push(Event::WorldTick { tick: self.world_tick, residents });
        }
        if self.world_tick % 40 == 0 {
            if let Some(r) = w.world.residents.first() {
                ev.push(Event::WorldBuild {
                    resident_id: r.id.clone(),
                    x: r.x as i32,
                    y: r.y as i32,
                    object: Some(PartSpec { name: "marker".into(), color: "#f2d05a".into(), glyph: None }),
                });
            }
        }
    }

    /// Background life: walking, typing, talking, coffee, terminal output.
    fn ambient(&mut self, w: &World, dt: f32, ev: &mut Vec<Event>) {
        let mut due = vec![];
        for a in w.agents.iter().filter(|a| !a.external) {
            let t = self.timers.entry(a.id.clone()).or_insert(0.5 + (a.look % 40) as f32 / 10.0);
            *t -= dt;
            if *t <= 0.0 {
                due.push(a.id.clone());
            }
        }
        for id in due {
            let next = self.rng.between(6.0, 14.0);
            self.timers.insert(id.clone(), next);
            let Some(a) = w.agent(&id) else { continue };
            let busy = match self.cycle {
                Cycle::Training => a.role == Role::Engineer,
                Cycle::Testing => matches!(a.role, Role::Tester | Role::Auditor | Role::Captain),
                Cycle::Review => matches!(a.role, Role::Scientist | Role::Captain),
                _ => false,
            };
            if busy {
                continue;
            }
            let roll = self.rng.f32();
            if roll < 0.08 {
                ev.push(goto(&id, "coffee"));
                ev.push(status(&id, Activity::Idle, "coffee break", &[]));
                continue;
            }
            if roll < 0.22 {
                let peers: Vec<&str> = w.agents.iter().filter(|o| o.id != id && o.path.is_empty() && o.online).map(|o| o.id.as_str()).collect();
                if !peers.is_empty() {
                    let peer = *self.rng.pick(&peers);
                    ev.push(goto(&id, &format!("agent:{}", peer)));
                    ev.push(status(&id, Activity::Talking, &format!("talking with {}", w.name_of(peer)), &[]));
                    let line = *self.rng.pick(&["Seen the board?", "Try this idea", "What failed?", "Pair on this?", "Notes posted"]);
                    ev.push(say(&id, line));
                    continue;
                }
            }
            let (place, act, task, screen, cmd): (&str, Activity, &str, &[&str], &str) = match a.role {
                Role::Researcher => *self.rng.pick(&[
                    ("whiteboard", Activity::Thinking, "sketching ideas", &["IDEA BOARD"][..], "sketch: new memory block on the whiteboard"),
                    ("desk", Activity::Typing, "writing notes", &["NOTES.MD"][..], "$ vim notes/ideas.md"),
                    ("desk", Activity::Reading, "reading lab notes", &["LAB NOTES"][..], "$ cat notes/board.md"),
                ]),
                Role::Scientist => *self.rng.pick(&[
                    ("bench", Activity::Thinking, "running a bench experiment", &["BENCH RUN"][..], "$ cargo run --bin bench -- --seed 7"),
                    ("desk", Activity::Typing, "writing an eval", &["EVAL.RS"][..], "$ cargo test -p evals"),
                    ("desk", Activity::Grading, "scoring results", &["SCORES"][..], "scores: 0.71 0.64 0.58 0.42"),
                ]),
                Role::Engineer => *self.rng.pick(&[
                    ("cluster", Activity::Thinking, "checking the cluster", &["GPU RACK"][..], "$ nvidia-smi"),
                    ("desk", Activity::Typing, "writing Rust", &["MAIN.RS"][..], "$ cargo build --release"),
                ]),
                Role::Tester => *self.rng.pick(&[
                    ("desk", Activity::Typing, "writing a test task", &["TASK.RS"][..], "$ cargo test -p arena-tasks"),
                    ("test", Activity::Testing, "dry-running a task", &["DRY RUN"][..], "dry run: blueprint parsed, 15 cells"),
                ]),
                Role::Auditor => *self.rng.pick(&[
                    ("registry", Activity::Auditing, "checking the known-architecture registry", &["REGISTRY"][..], "audit: compare designs against registry"),
                    ("desk", Activity::Auditing, "reviewing logs", &["AUDIT LOG"][..], "$ grep -r 'attention' designs/"),
                ]),
                Role::Scribe => ("desk", Activity::Typing, "writing the lab journal", &["JOURNAL"][..], "journal: logged today's work"),
                Role::Captain => *self.rng.pick(&[
                    ("office", Activity::Thinking, "planning", &["PLAN"][..], "plan: next round priorities"),
                    ("desk", Activity::Reading, "reading reports", &["REPORTS"][..], "$ cat reports/latest.md"),
                    ("notes", Activity::Reading, "reading the board", &["BOARD"][..], "reading the knowledge board"),
                ]),
                Role::Guest => ("desk", Activity::Idle, "visiting", &[][..], "hello"),
            };
            ev.push(goto(&id, place));
            ev.push(status(&id, act, task, screen));
            ev.push(out(&id, cmd));
        }
    }
}
