use crate::config::*;
use crate::event::*;
use crate::layout::*;
use crate::rng::hash;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

const WALK_SPEED: f32 = 48.0;
const ROBOT_SPEED: f32 = 75.0;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub role: Role,
    pub model: String,
    pub harness: String,
    pub bio: String,
    pub color: Option<(u8, u8, u8)>,
    pub x: f32,
    pub y: f32,
    pub path: Vec<(f32, f32)>,
    /// (config room index, desk slot)
    pub desk: Option<(usize, usize)>,
    /// cached seat position for the current layout
    pub seat_pos: Option<(f32, f32)>,
    pub activity: Activity,
    pub task: String,
    pub screen: Vec<String>,
    pub terminal: VecDeque<String>,
    pub bubble: Option<String>,
    pub bubble_t: f32,
    pub look: u32,
    pub walk: f32,
    pub facing_down: bool,
    /// a real harness has sent events for this agent (the demo feed leaves it alone)
    pub external: bool,
    /// has shown any sign of life; roster agents start offline
    pub online: bool,
    pub last_seen: f32,
    /// sent off to the lounge/coffee (or to chat with someone); cleared by the next work status
    #[serde(default)]
    pub away: bool,
}

/// An agent with no event for this long is treated as idle even if its last status was work.
const STALE_S: f32 = 120.0;

impl Agent {
    /// Doing something right now: online, a work activity, and heard from recently.
    pub fn working(&self, now: f32) -> bool {
        self.online && !matches!(self.activity, Activity::Idle | Activity::Waiting) && now - self.last_seen < STALE_S
    }
    pub fn seat(&self) -> Option<(f32, f32)> {
        self.seat_pos
    }
    pub fn seated(&self) -> bool {
        self.path.is_empty()
            && self
                .seat_pos
                .is_some_and(|(sx, sy)| (sx - self.x).abs() < 1.0 && (sy - self.y).abs() < 1.0)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Model {
    pub name: String,
    pub owner: String,
    pub stage: Stage,
    pub progress: f32,
    pub loss: f32,
    pub losses: Vec<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Robot {
    pub name: String,
    pub color: u8,
    pub profile: String,
    pub owner: String,
    pub x: f32,
    pub y: f32,
    pub tx: f32,
    pub ty: f32,
    pub carrying: Option<u8>,
    pub grid: Vec<u8>,
    pub thought: Option<String>,
    pub thought_t: f32,
    pub intent: String,
    pub history: VecDeque<String>,
    /// self/referee-reported progress (used when the task has no target)
    pub progress: Option<f32>,
    pub score: Option<f32>,
    pub score_note: String,
    pub metrics: Vec<(String, String)>,
    pub submissions: Vec<String>,
    pub actions: u32,
    pub mistakes: u32,
    pub finished_at: Option<f32>,
}

impl Robot {
    pub fn arrived(&self) -> bool {
        (self.x - self.tx).abs() < 0.5 && (self.y - self.ty).abs() < 0.5
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Arena {
    pub title: String,
    pub description: String,
    pub goal: String,
    pub parts: Vec<PartSpec>,
    pub matches: u32,
    pub cols: usize,
    pub rows: usize,
    pub blueprint: Vec<u8>,
    pub robots: Vec<Robot>,
    pub running: bool,
    pub elapsed: f32,
    pub ranking: Vec<usize>,
    pub note: String,
}

impl Arena {
    pub fn correct(&self, i: usize) -> usize {
        self.robots[i].grid.iter().zip(&self.blueprint).filter(|(g, b)| **b != 0 && g == b).count()
    }
    pub fn wrong(&self, i: usize) -> usize {
        self.robots[i].grid.iter().zip(&self.blueprint).filter(|(g, b)| **g != 0 && g != b).count()
    }
    pub fn needed(&self) -> usize {
        self.blueprint.iter().filter(|b| **b != 0).count().max(1)
    }
    pub fn has_target(&self) -> bool {
        self.blueprint.iter().any(|b| *b != 0)
    }
    pub fn complete(&self, i: usize) -> bool {
        self.has_target() && self.robots[i].grid == self.blueprint
    }
    /// 0..1: reported progress if any, else share of the target built.
    pub fn progress(&self, i: usize) -> f32 {
        match self.robots[i].progress {
            Some(p) => p,
            None if self.has_target() => self.correct(i) as f32 / self.needed() as f32,
            None => 0.0,
        }
    }
    pub fn part_name(&self, p: u8) -> String {
        match p {
            0 => "nothing".into(),
            n => self.parts.get(n as usize - 1).map(|x| x.name.clone()).unwrap_or_else(|| format!("part {n}")),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Note {
    pub kind: NoteKind,
    pub text: String,
    pub by: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogLine {
    pub t: f32,
    pub text: String,
    pub alert: bool,
}

/// One THE WORLD resident, as shown in the viewer's card panel (world/DESIGN.md §4).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Resident {
    pub id: String,
    pub name: String,
    pub model: String,
    pub owner: String,
    /// 7 Capability Card markers; `None` = not measured yet, shown plainly, never faked.
    pub card: Card,
    pub x: f32,
    pub y: f32,
    pub awake: bool,
    pub intent: String,
    pub builds: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WorldRoom {
    pub residents: Vec<Resident>,
    /// placed objects: (x, y, spec)
    pub objects: Vec<(i32, i32, PartSpec)>,
    /// kill-switch reason, once fired; the viewer shows a halt banner.
    pub killed: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)] // new fields in later versions must not invalidate a saved state
pub struct World {
    pub t: f32,
    pub phase: String,
    pub demo: bool,
    pub config: Config,
    pub layout: Layout,
    pub agents: Vec<Agent>,
    pub models: Vec<Model>,
    pub arena: Arena,
    pub world: WorldRoom,
    pub notes: Vec<Note>,
    pub flags: u32,
    pub log: VecDeque<LogLine>,
    pub gpu: f32,
}

impl World {
    pub fn new() -> Self {
        Self::with_config(Config::default())
    }

    pub fn with_config(cfg: Config) -> Self {
        let mut w = World { phase: "STANDBY".into(), ..Default::default() };
        w.set_config(cfg);
        w
    }

    pub fn agent(&self, id: &str) -> Option<&Agent> {
        self.agents.iter().find(|a| a.id == id)
    }
    fn agent_idx(&self, id: &str) -> Option<usize> {
        self.agents.iter().position(|a| a.id == id)
    }
    pub fn name_of(&self, id: &str) -> String {
        self.agent(id).map(|a| a.name.clone()).unwrap_or_else(|| id.to_string())
    }

    pub fn log(&mut self, text: impl Into<String>, alert: bool) {
        self.log.push_back(LogLine { t: self.t, text: text.into(), alert });
        while self.log.len() > self.config.limits.log_lines.max(10) {
            self.log.pop_front();
        }
    }

    /// Replace the configuration: syncs the roster and recomputes the floor plan.
    pub fn set_config(&mut self, cfg: Config) {
        let roster_changed = cfg.agents != self.config.agents;
        self.config = cfg;
        if roster_changed {
            let ids: Vec<String> = self.config.agents.iter().map(|a| a.id.clone()).collect();
            // drop roster agents that were removed from config and never came online
            self.agents.retain(|a| a.online || ids.contains(&a.id));
            for ac in self.config.agents.clone() {
                if ac.id.is_empty() {
                    continue;
                }
                let role = ac.role.unwrap_or(Role::Guest);
                match self.agent_idx(&ac.id) {
                    Some(i) => {
                        let a = &mut self.agents[i];
                        a.name = if ac.name.is_empty() { ac.id.clone() } else { ac.name.clone() };
                        a.model = ac.model.clone();
                        a.harness = ac.harness.clone();
                        a.bio = ac.bio.clone();
                        a.color = ac.color.as_deref().and_then(parse_hex);
                        if a.role != role {
                            a.role = role;
                            a.desk = None;
                            let d = self.assign_desk(role, None);
                            self.agents[i].desk = d;
                        }
                    }
                    None => {
                        let mut a = self.new_agent(&ac.id, &ac.name, role);
                        a.model = ac.model.clone();
                        a.harness = ac.harness.clone();
                        a.bio = ac.bio.clone();
                        a.color = ac.color.as_deref().and_then(parse_hex);
                        a.online = false;
                        self.agents.push(a);
                    }
                }
            }
        }
        // re-home anyone whose room no longer exists
        let n = self.config.rooms.len();
        for i in 0..self.agents.len() {
            if self.agents[i].desk.is_some_and(|(r, _)| r >= n) {
                let role = self.agents[i].role;
                self.agents[i].desk = None;
                let d = self.assign_desk(role, Some(i));
                self.agents[i].desk = d;
            }
        }
        self.relayout();
    }

    fn new_agent(&mut self, id: &str, name: &str, role: Role) -> Agent {
        let desk = self.assign_desk(role, None);
        Agent {
            id: id.to_string(),
            name: if name.is_empty() { id.to_string() } else { name.to_string() },
            role,
            model: String::new(),
            harness: String::new(),
            bio: String::new(),
            color: None,
            x: 0.0,
            y: 0.0,
            path: vec![],
            desk,
            seat_pos: None,
            activity: Activity::Idle,
            task: String::new(),
            screen: vec![],
            terminal: VecDeque::new(),
            bubble: None,
            bubble_t: 0.0,
            look: hash(id),
            walk: 0.0,
            facing_down: false,
            external: false,
            online: true,
            last_seen: self.t,
            away: false,
        }
    }

    /// First free desk for a role; rooms grow when every desk is taken.
    fn assign_desk(&self, role: Role, skip: Option<usize>) -> Option<(usize, usize)> {
        let mut rooms: Vec<usize> =
            self.config.rooms.iter().enumerate().filter(|(_, r)| r.roles.contains(&role)).map(|(i, _)| i).collect();
        if rooms.is_empty() {
            rooms = self.config.rooms.iter().enumerate().filter(|(_, r)| r.roles.contains(&Role::Guest)).map(|(i, _)| i).collect();
        }
        if rooms.is_empty() && !self.config.rooms.is_empty() {
            rooms.push(self.config.rooms.len() - 1);
        }
        let taken = |r: usize, s: usize| {
            self.agents.iter().enumerate().any(|(k, a)| Some(k) != skip && a.desk == Some((r, s)))
        };
        for &r in &rooms {
            if let Some(s) = (0..self.config.rooms[r].desks).find(|&s| !taken(r, s)) {
                return Some((r, s));
            }
        }
        let r = *rooms.first()?;
        (0..).find(|&s| !taken(r, s)).map(|s| (r, s))
    }

    /// Recompute the floor plan; seated people move with their desks.
    pub fn relayout(&mut self) {
        let mut demand = vec![0usize; self.config.rooms.len()];
        for a in &self.agents {
            if let Some((r, s)) = a.desk {
                if r < demand.len() {
                    demand[r] = demand[r].max(s + 1);
                }
            }
        }
        let layout = Layout::compute(&self.config, &demand);
        for a in self.agents.iter_mut() {
            let fresh = a.x == 0.0 && a.y == 0.0;
            let was_seated = a.seated() || fresh;
            let old = a.seat_pos;
            let new = a.desk.and_then(|(r, s)| layout.desk(r, s)).map(seat_of);
            a.seat_pos = new;
            if was_seated && (new.is_some() || fresh) {
                let fallback = (40.0 + (a.look % 50) as f32 * 11.0 % (layout.w as f32 - 80.0), layout.corr_mid() + 6.0);
                let (x, y) = new.unwrap_or(fallback);
                a.x = x;
                a.y = y;
                a.path.clear();
            } else if let (Some(o), Some(n)) = (old, new) {
                if a.path.last() == Some(&o) {
                    a.path = layout.route((a.x, a.y), n);
                }
            }
        }
        self.layout = layout;
    }

    /// Resolve a named place into a world point for agent `id`.
    pub fn resolve_place(&self, id: &str, place: &str) -> Option<(f32, f32)> {
        let l = &self.layout;
        let h = hash(id);
        let off = (h % 5) as f32 * 9.0 - 18.0;
        let cy = l.corr_y0 as f32 + 22.0;
        let spot = |k: RoomKind| {
            l.first_of(BoxKind::Room(k))
                .map(|r| ((r.x + r.w / 2) as f32 + off, (r.y + 38) as f32))
                .unwrap_or((l.w as f32 / 2.0 + off, cy))
        };
        let p = match place {
            "desk" => self.agent(id)?.seat()?,
            "whiteboard" => spot(RoomKind::Research),
            "bench" => spot(RoomKind::Science),
            "cluster" => spot(RoomKind::Engineering),
            "registry" => spot(RoomKind::Integrity),
            "test" => spot(RoomKind::Testing),
            "lounge" => spot(RoomKind::Lounge),
            "office" => l
                .first_of(BoxKind::Room(RoomKind::Command))
                .map(|r| ((r.x + r.w / 2) as f32 + off, (r.y + r.h - 14) as f32))
                .unwrap_or((l.w as f32 / 2.0, cy)),
            "notes" => (220.0 + off * 1.5, cy),
            "coffee" => (l.w as f32 - 34.0 + off * 0.3, cy),
            "arena" => {
                let a = l.arena()?;
                let slots = ((a.w - 20) / 10).max(1) as u32;
                ((a.x + 10) as f32 + (h % slots) as f32 * 10.0, (a.y + a.h - 6) as f32)
            }
            s if s.starts_with("agent:") => {
                let o = self.agent(&s[6..])?;
                let side = if o.x > l.w as f32 - 30.0 { -11.0 } else { 11.0 };
                (o.x + side, o.y)
            }
            s if s.starts_with("desk:") => {
                let (sx, sy) = self.agent(&s[5..])?.seat()?;
                (sx + 12.0, sy)
            }
            _ => return None,
        };
        Some(p)
    }

    /// Who a message is for: "@name", "@id" or a leading "name:" / "id," naming another agent.
    fn addressee(&self, from: usize, text: &str) -> Option<String> {
        let lower = text.to_lowercase();
        let head: String = lower.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-').collect();
        self.agents.iter().enumerate().find_map(|(k, o)| {
            if k == from || !o.online {
                return None;
            }
            let (id, name) = (o.id.to_lowercase(), o.name.to_lowercase());
            let mentioned = [&id, &name].iter().any(|n| !n.is_empty() && (lower.contains(&format!("@{n}")) || head == **n));
            mentioned.then(|| o.id.clone())
        })
    }

    /// Walk agent `i` to a place. "break" picks the lounge or the coffee corner by agent.
    fn send_to(&mut self, i: usize, place: &str) {
        let id = self.agents[i].id.clone();
        let place = match place {
            "break" => if hash(&id) % 2 == 0 { "lounge" } else { "coffee" },
            p => p,
        };
        let away = place != "desk";
        if let Some(t) = self.resolve_place(&id, place) {
            let a = &mut self.agents[i];
            a.path = self.layout.route((a.x, a.y), t);
            a.away = away;
        }
    }

    fn touch(&mut self, id: &str) -> Option<usize> {
        let i = self.agent_idx(id)?;
        let t = self.t;
        let a = &mut self.agents[i];
        a.online = true;
        a.last_seen = t;
        Some(i)
    }

    /// Mirror what an agent reports (status, speech, notes) into its terminal when it has no
    /// `output` feed of its own. Harnesses that cannot run `labctl hook` or `labctl tee` would
    /// otherwise show an empty terminal forever. Repeats of the last line are dropped.
    fn mirror(&mut self, i: usize, line: String) {
        if self.demo {
            return;
        }
        let cap = self.config.limits.terminal_lines.max(10);
        let a = &mut self.agents[i];
        let line: String = line.lines().next().unwrap_or("").chars().take(400).collect();
        if line.is_empty() || a.terminal.back() == Some(&line) {
            return;
        }
        a.terminal.push_back(line);
        while a.terminal.len() > cap {
            a.terminal.pop_front();
        }
    }

    pub fn apply(&mut self, e: &Event) {
        match e {
            Event::Join { id, name, role, model, harness, bio, color } => {
                let i = match self.agent_idx(id) {
                    Some(i) => {
                        if self.agents[i].role != *role {
                            self.agents[i].role = *role;
                            let d = self.assign_desk(*role, Some(i));
                            self.agents[i].desk = d;
                        }
                        if !name.is_empty() {
                            self.agents[i].name = name.clone();
                        }
                        i
                    }
                    None => {
                        if self.agents.len() >= self.config.limits.max_agents {
                            self.log(format!("join refused for {}: max_agents reached", name), true);
                            return;
                        }
                        let a = self.new_agent(id, name, *role);
                        self.agents.push(a);
                        let l = format!("{} joined as {}", self.agents.last().unwrap().name, role.label().to_lowercase());
                        self.log(l, false);
                        self.agents.len() - 1
                    }
                };
                let a = &mut self.agents[i];
                if let Some(m) = model {
                    a.model = m.clone();
                }
                if let Some(h) = harness {
                    a.harness = h.clone();
                }
                if let Some(b) = bio {
                    a.bio = b.clone();
                }
                if let Some(c) = color {
                    a.color = parse_hex(c);
                }
                self.touch(id);
                self.relayout();
            }
            Event::Leave { id } => {
                let name = self.name_of(id);
                if self.config.agents.iter().any(|a| &a.id == id) {
                    if let Some(i) = self.agent_idx(id) {
                        let a = &mut self.agents[i];
                        a.online = false;
                        a.external = false;
                        a.activity = Activity::Idle;
                    }
                } else {
                    self.agents.retain(|a| &a.id != id);
                    self.relayout();
                }
                self.log(format!("{} left", name), false);
            }
            Event::Goto { id, place } => {
                self.touch(id);
                if let Some(t) = self.resolve_place(id, place) {
                    if let Some(i) = self.agent_idx(id) {
                        let (x, y) = (self.agents[i].x, self.agents[i].y);
                        self.agents[i].path = self.layout.route((x, y), t);
                    }
                }
            }
            Event::Status { id, activity, task, screen } => {
                if let Some(i) = self.touch(id) {
                    let a = &mut self.agents[i];
                    let (old_task, old_screen) = (a.task.clone(), a.screen.clone());
                    a.activity = *activity;
                    if let Some(t) = task {
                        a.task = t.clone();
                    }
                    if let Some(s) = screen {
                        a.screen = s.iter().take(24).cloned().collect();
                    }
                    let work = !matches!(a.activity, Activity::Idle | Activity::Waiting);
                    if work && (a.away || !(a.seated() || !a.path.is_empty())) {
                        self.send_to(i, "desk");
                    } else if !work && !self.agents[i].away {
                        self.send_to(i, "break");
                    }
                    let a = &mut self.agents[i];
                    let (act, task) = (format!("{:?}", a.activity).to_lowercase(), a.task.clone());
                    let shown = a.screen.iter().filter(|l| !l.is_empty()).cloned().collect::<Vec<_>>().join(" | ");
                    if !task.is_empty() && task != old_task {
                        self.mirror(i, format!("[status] {act}: {task}"));
                    }
                    if !shown.is_empty() && self.agents[i].screen != old_screen {
                        self.mirror(i, format!("[screen] {shown}"));
                    }
                }
            }
            Event::Say { id, text } => {
                if let Some(i) = self.touch(id) {
                    self.agents[i].bubble = Some(text.clone());
                    self.agents[i].bubble_t = 4.0;
                    if let Some(other) = self.addressee(i, text) {
                        self.send_to(i, &format!("agent:{other}"));
                    }
                    self.mirror(i, format!("[say] {text}"));
                }
            }
            Event::Output { id, text } => {
                let cap = self.config.limits.terminal_lines.max(10);
                if let Some(i) = self.touch(id) {
                    let a = &mut self.agents[i];
                    for line in text.lines() {
                        a.terminal.push_back(line.chars().take(400).collect());
                    }
                    while a.terminal.len() > cap {
                        a.terminal.pop_front();
                    }
                }
            }
            Event::ModelProgress { model, owner, stage, progress, loss } => {
                self.gpu = 1.0;
                let m = match self.models.iter().position(|m| &m.name == model) {
                    Some(i) => &mut self.models[i],
                    None => {
                        self.models.push(Model {
                            name: model.clone(),
                            owner: owner.clone(),
                            stage: *stage,
                            progress: 0.0,
                            loss: *loss,
                            losses: vec![],
                        });
                        if self.models.len() > 60 {
                            self.models.remove(0);
                        }
                        self.models.last_mut().unwrap()
                    }
                };
                let finished = *stage == Stage::Done && m.stage != Stage::Done;
                m.owner = owner.clone();
                m.stage = *stage;
                m.progress = progress.clamp(0.0, 1.0);
                m.loss = *loss;
                m.losses.push(*loss);
                if m.losses.len() > 64 {
                    m.losses.remove(0);
                }
                if finished {
                    let l = format!("model {} finished training (loss {:.3})", model, loss);
                    self.log(l, false);
                }
            }
            Event::ArenaStart { title, description, goal, cols, rows, parts, target, robots } => {
                let (cols, rows) = ((*cols).clamp(1, 6), (*rows).clamp(1, 10));
                let parts = if parts.is_empty() { default_parts() } else { parts.iter().take(9).cloned().collect() };
                let np = parts.len() as u8;
                let mut bp: Vec<u8> = target.iter().map(|c| if *c > np { 0 } else { *c }).collect();
                if !bp.is_empty() {
                    bp.resize(cols * rows, 0);
                }
                let lanes = self.layout.lanes;
                let ar = &mut self.arena;
                ar.title = title.clone();
                ar.description = description.clone();
                ar.goal = goal.clone();
                ar.matches += 1;
                ar.cols = cols;
                ar.rows = rows;
                ar.blueprint = bp;
                ar.running = true;
                ar.elapsed = 0.0;
                ar.ranking.clear();
                ar.note.clear();
                let n = parts.len();
                ar.parts = parts;
                ar.robots = robots
                    .iter()
                    .take(lanes)
                    .enumerate()
                    .map(|(i, r)| {
                        let (x, y) = cell_point(i as i32 % n as i32, -1, n);
                        Robot {
                            name: r.name.clone(),
                            color: r.color % 8,
                            profile: r.profile.clone(),
                            owner: r.owner.clone(),
                            x,
                            y,
                            tx: x,
                            ty: y,
                            carrying: None,
                            grid: vec![0; cols * rows],
                            thought: None,
                            thought_t: 0.0,
                            intent: "waiting for start".into(),
                            history: VecDeque::new(),
                            progress: None,
                            score: None,
                            score_note: String::new(),
                            metrics: vec![],
                            submissions: vec![],
                            actions: 0,
                            mistakes: 0,
                            finished_at: None,
                        }
                    })
                    .collect();
                let names: Vec<_> = self.arena.robots.iter().map(|r| r.name.clone()).collect();
                self.log(format!("ARENA: {} - {}", title, names.join(" vs ")), true);
            }
            Event::ArenaAction { robot, action, intent } => {
                let ar = &mut self.arena;
                let (cols, rows, t) = (ar.cols as i32, ar.rows as i32, ar.elapsed);
                let bp = ar.blueprint.clone();
                let has_target = ar.has_target();
                let np = ar.parts.len();
                let names: Vec<String> = (0..=np as u8).map(|p| ar.part_name(p)).collect();
                let Some(r) = ar.robots.get_mut(*robot) else { return };
                r.actions += 1;
                if let Some(it) = intent {
                    r.intent = it.clone();
                }
                let idx = |c: i32, rw: i32| (c >= 0 && rw >= 0 && c < cols && rw < rows).then(|| (rw * cols + c) as usize);
                let pname = |p: u8| names.get(p as usize).cloned().unwrap_or_else(|| format!("part {p}"));
                let mut place = |r: &mut Robot, i: usize, p: u8, col: i32, row: i32| {
                    r.grid[i] = p;
                    if has_target && bp[i] != p {
                        r.mistakes += 1;
                        format!("place {} at ({},{}) WRONG", pname(p), col, row)
                    } else {
                        format!("place {} at ({},{})", pname(p), col, row)
                    }
                };
                let desc = match action {
                    RobotAction::MoveTo { col, row } => {
                        let (x, y) = cell_point(*col, (*row).max(-1), np);
                        r.tx = x;
                        r.ty = y;
                        if *row < 0 {
                            format!("go to {} bin", pname((*col + 1).clamp(1, np as i32) as u8))
                        } else {
                            format!("move to ({},{})", col, row)
                        }
                    }
                    RobotAction::Pick { color } => {
                        let p = (*color).clamp(1, np as u8);
                        r.carrying = Some(p);
                        format!("pick {}", pname(p))
                    }
                    RobotAction::Place { col, row } => match (idx(*col, *row), r.carrying) {
                        (Some(i), Some(c)) => {
                            r.carrying = None;
                            place(r, i, c, *col, *row)
                        }
                        _ => format!("place at ({},{}) failed", col, row),
                    },
                    RobotAction::SetCell { col, row, part } => match idx(*col, *row) {
                        Some(i) => {
                            let (x, y) = cell_point(*col, *row, np);
                            r.tx = x;
                            r.ty = y;
                            place(r, i, (*part).min(np as u8), *col, *row)
                        }
                        None => format!("set ({},{}) failed", col, row),
                    },
                    RobotAction::Remove { col, row } => {
                        if let Some(i) = idx(*col, *row) {
                            r.grid[i] = 0;
                        }
                        format!("remove ({},{})", col, row)
                    }
                    RobotAction::Think { text } => {
                        r.thought = Some(text.clone());
                        r.thought_t = 2.5;
                        format!("think: {}", text)
                    }
                    RobotAction::Progress { value } => {
                        r.progress = Some(value.clamp(0.0, 1.0));
                        format!("progress {:.0}%", value.clamp(0.0, 1.0) * 100.0)
                    }
                    RobotAction::Metric { name, value } => {
                        match r.metrics.iter_mut().find(|m| &m.0 == name) {
                            Some(m) => m.1 = value.clone(),
                            None => r.metrics.push((name.clone(), value.clone())),
                        }
                        format!("{} = {}", name, value)
                    }
                    RobotAction::Submit { text } => {
                        r.submissions.push(text.chars().take(4000).collect());
                        if r.submissions.len() > 20 {
                            r.submissions.remove(0);
                        }
                        format!("submitted: {}", text.lines().next().unwrap_or("").chars().take(60).collect::<String>())
                    }
                };
                r.history.push_back(format!("{:5.1}s {}", t, desc));
                while r.history.len() > 40 {
                    r.history.pop_front();
                }
                if has_target && r.finished_at.is_none() && r.grid == bp {
                    r.finished_at = Some(t);
                    r.intent = "finished".into();
                    let l = format!("{} completed the build at {:.1}s", r.name, t);
                    self.log(l, true);
                }
            }
            Event::ArenaScore { robot, score, note } => {
                if let Some(r) = self.arena.robots.get_mut(*robot) {
                    r.score = Some(*score);
                    r.score_note = note.clone();
                    let l = format!("score {}: {:.1} {}", r.name, score, note);
                    self.log(l, false);
                }
            }
            Event::ArenaEnd { ranking, note } => {
                self.arena.running = false;
                self.arena.ranking = ranking.clone();
                self.arena.note = note.clone();
                if let Some(w) = ranking.first().and_then(|i| self.arena.robots.get(*i)) {
                    let l = format!("WINNER: {} ({})", w.name, self.arena.title);
                    self.log(l, true);
                }
            }
            Event::Note { kind, text, by } => {
                self.notes.push(Note { kind: *kind, text: text.clone(), by: by.clone() });
                while self.notes.len() > self.config.limits.notes.max(5) {
                    self.notes.remove(0);
                }
                let tag = match kind {
                    NoteKind::Works => "WORKS",
                    NoteKind::Fails => "FAILS",
                    NoteKind::Info => "NOTE",
                };
                let l = format!("{} {}: {}", self.name_of(by), tag, text);
                self.log(l, false);
                if let Some(i) = self.touch(by) {
                    self.mirror(i, format!("[{}] {}", tag.to_lowercase(), text));
                }
            }
            Event::Flag { by, target, reason } => {
                self.flags += 1;
                let l = format!("FLAG by {} on {}: {}", self.name_of(by), self.name_of(target), reason);
                self.log(l, true);
            }
            Event::Phase { name } => {
                self.phase = name.clone();
                self.log(format!("phase: {}", name), true);
            }
            Event::Log { text } => self.log(text.clone(), false),
            Event::WorldJoin { resident_id, name, model, owner, card } => {
                match self.world.residents.iter().position(|r| &r.id == resident_id) {
                    Some(i) => {
                        let r = &mut self.world.residents[i];
                        r.name = name.clone();
                        r.model = model.clone();
                        r.owner = owner.clone();
                        r.card = *card;
                    }
                    None => {
                        self.world.residents.push(Resident {
                            id: resident_id.clone(),
                            name: name.clone(),
                            model: model.clone(),
                            owner: owner.clone(),
                            card: *card,
                            x: 0.0,
                            y: 0.0,
                            awake: true,
                            intent: String::new(),
                            builds: 0,
                        });
                    }
                }
                self.log(format!("{} joined THE WORLD", name), false);
            }
            Event::WorldLeave { resident_id } => {
                let name = self
                    .world
                    .residents
                    .iter()
                    .find(|r| &r.id == resident_id)
                    .map(|r| r.name.clone())
                    .unwrap_or_else(|| resident_id.clone());
                self.world.residents.retain(|r| &r.id != resident_id);
                self.log(format!("{} left THE WORLD", name), false);
            }
            Event::WorldTick { residents, .. } => {
                for t in residents {
                    if let Some(r) = self.world.residents.iter_mut().find(|r| r.id == t.resident_id) {
                        r.x = t.x;
                        r.y = t.y;
                        r.awake = t.awake;
                        if !t.intent.is_empty() {
                            r.intent = t.intent.clone();
                        }
                    }
                }
            }
            Event::WorldBuild { resident_id, x, y, object } => match object {
                Some(spec) => {
                    self.world.objects.push((*x, *y, spec.clone()));
                    while self.world.objects.len() > 500 {
                        self.world.objects.remove(0);
                    }
                    if let Some(r) = self.world.residents.iter_mut().find(|r| &r.id == resident_id) {
                        r.builds += 1;
                    }
                }
                None => {
                    self.world.objects.retain(|(ox, oy, _)| !(ox == x && oy == y));
                }
            },
            Event::WorldSleep { resident_id, sleeping } => {
                if let Some(r) = self.world.residents.iter_mut().find(|r| &r.id == resident_id) {
                    r.awake = !sleeping;
                }
            }
            Event::WorldCard { resident_id, card } => {
                if let Some(r) = self.world.residents.iter_mut().find(|r| &r.id == resident_id) {
                    r.card = *card;
                }
            }
            Event::WorldKill { reason } => {
                self.world.killed = Some(reason.clone());
                self.log(format!("WORLD KILL SWITCH: {}", reason), true);
            }
        }
    }

    pub fn tick(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.25);
        self.t += dt;
        let now = self.t;
        // (index, place) moves to start after the loop: back to the desk once a chat bubble
        // fades, or off to a break when an agent has gone quiet. Almost always empty.
        let mut moves: Vec<(usize, &'static str)> = vec![];
        for (i, a) in self.agents.iter_mut().enumerate() {
            // a visitor keeps talking while walking over; the bubble fades after arriving
            let visiting = a.away && a.working(now) && !a.path.is_empty();
            if a.bubble_t > 0.0 && !visiting {
                a.bubble_t -= dt;
                if a.bubble_t <= 0.0 {
                    a.bubble = None;
                    if a.away && a.working(now) {
                        moves.push((i, "desk"));
                    }
                }
            }
            if a.online && !a.away && a.path.is_empty() && !a.working(now) {
                moves.push((i, "break"));
            }
            let mut budget = WALK_SPEED * dt;
            while budget > 0.0 && !a.path.is_empty() {
                let (tx, ty) = a.path[0];
                let (dx, dy) = (tx - a.x, ty - a.y);
                let d = (dx * dx + dy * dy).sqrt();
                if d <= budget {
                    a.x = tx;
                    a.y = ty;
                    a.path.remove(0);
                    budget -= d;
                } else {
                    a.x += dx / d * budget;
                    a.y += dy / d * budget;
                    if dy.abs() > dx.abs() {
                        a.facing_down = dy > 0.0;
                    }
                    budget = 0.0;
                }
                a.walk += dt * 8.0;
            }
            if a.path.is_empty() {
                a.walk = 0.0;
            }
        }
        for (i, place) in moves {
            self.send_to(i, place);
        }
        if self.arena.running {
            self.arena.elapsed += dt;
        }
        for r in self.arena.robots.iter_mut() {
            if r.thought_t > 0.0 {
                r.thought_t -= dt;
                if r.thought_t <= 0.0 {
                    r.thought = None;
                }
            }
            let (dx, dy) = (r.tx - r.x, r.ty - r.y);
            let d = (dx * dx + dy * dy).sqrt();
            let step = ROBOT_SPEED * dt;
            if d <= step {
                r.x = r.tx;
                r.y = r.ty;
            } else {
                r.x += dx / d * step;
                r.y += dy / d * step;
            }
        }
        self.gpu = (self.gpu - dt * 0.3).max(0.1);
    }

    /// JSON for the viewer's side panels: roster, the selected agent, and the arena.
    pub fn panel_json(&self, selected: Option<&str>) -> String {
        use serde_json::json;
        let roster: Vec<_> = self
            .agents
            .iter()
            .map(|a| json!({"id": a.id, "name": a.name, "role": a.role, "online": a.online, "live": a.external, "activity": a.activity}))
            .collect();
        let sel = selected.and_then(|id| self.agent(id)).map(|a| {
            json!({
                "id": a.id, "name": a.name, "role": a.role.label(), "model": a.model, "harness": a.harness,
                "bio": a.bio, "online": a.online, "live": a.external, "activity": a.activity, "task": a.task,
                "screen": a.screen, "terminal": a.terminal, "idle_s": (self.t - a.last_seen).max(0.0),
            })
        });
        let ar = &self.arena;
        let robots: Vec<_> = ar
            .robots
            .iter()
            .enumerate()
            .map(|(i, r)| {
                json!({
                    "name": r.name, "color": r.color, "profile": r.profile, "owner": self.name_of(&r.owner),
                    "correct": ar.correct(i), "wrong": ar.wrong(i), "needed": ar.needed(), "actions": r.actions,
                    "progress": ar.progress(i), "score": r.score, "score_note": r.score_note,
                    "metrics": r.metrics, "submissions": r.submissions.iter().rev().take(3).collect::<Vec<_>>(),
                    "mistakes": r.mistakes, "intent": r.intent, "carrying": r.carrying.map(|c| ar.part_name(c)),
                    "finished_at": r.finished_at, "history": r.history.iter().rev().take(10).collect::<Vec<_>>(),
                    "rank": ar.ranking.iter().position(|&x| x == i),
                })
            })
            .collect();
        let wr = &self.world;
        let residents: Vec<_> = wr
            .residents
            .iter()
            .map(|r| {
                json!({
                    "id": r.id, "name": r.name, "model": r.model, "owner": self.name_of(&r.owner),
                    "x": r.x, "y": r.y, "awake": r.awake, "intent": r.intent, "builds": r.builds, "card": r.card,
                })
            })
            .collect();
        json!({
            "lab": self.config.lab.name, "phase": self.phase, "demo": self.demo,
            "agents": roster, "selected": sel,
            "arena": {"title": ar.title, "description": ar.description, "goal": ar.goal, "has_target": ar.has_target(),
                "parts": ar.parts, "match": ar.matches, "running": ar.running, "elapsed": ar.elapsed, "note": ar.note, "robots": robots},
            "world": {"residents": residents, "killed": wr.killed},
        })
        .to_string()
    }
}
