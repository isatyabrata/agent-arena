//! Lab configuration. The server reads it from `lab.toml` and hot-reloads it, so a
//! master agent can reshape the lab by editing one file (or POSTing /api/config).
//! Every field has a default, so a config only needs the parts it changes.

use crate::event::{PartSpec, Role};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub lab: LabCfg,
    pub limits: Limits,
    pub arena: ArenaCfg,
    pub rooms: Vec<RoomCfg>,
    /// Pre-declared people. They appear at their desks (dimmed, "offline")
    /// until a harness sends their first event. Anyone else can still join.
    pub agents: Vec<AgentCfg>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LabCfg {
    pub name: String,
    /// address the server listens on; use 0.0.0.0:7878 to expose on your network
    pub bind: String,
    /// if set, POSTs need "Authorization: Bearer <token>" (LAB_TOKEN env overrides)
    pub token: String,
    /// run the built-in demo feed (sample agents and scripted robots)
    pub demo: bool,
    /// how many sample researchers/scientists/engineers/testers the demo creates
    /// when `agents` is empty
    pub demo_per_role: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Limits {
    pub max_agents: usize,
    /// scrollback kept per agent terminal
    pub terminal_lines: usize,
    pub log_lines: usize,
    pub notes: usize,
    /// desks per row inside a room (rooms grow downward when they fill up)
    pub desk_columns: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ArenaCfg {
    pub name: String,
    /// number of robots that can compete at once (1..8)
    pub lanes: usize,
    /// position of the arena among the bottom-row rooms
    pub slot: usize,
    /// tasks the demo feed (or your referee) can pick from
    pub tasks: Vec<TaskCfg>,
}

/// A ready-made task. Agents can also invent tasks on the fly with an
/// `arena_start` event; these are only presets (the demo feed picks from them).
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TaskCfg {
    pub name: String,
    pub description: String,
    pub goal: String,
    /// empty = red, blue, yellow, green blocks
    pub parts: Vec<PartSpec>,
    /// rows top to bottom; '.' empty, 1-9 = parts[n-1]. Max 6 wide, 10 tall.
    /// Leave empty for an open-ended task (then set cols/rows).
    pub blueprint: Vec<String>,
    pub cols: usize,
    pub rows: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomKind {
    Research,
    Science,
    Engineering,
    Integrity,
    Testing,
    Command,
    Lounge,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Row {
    Top,
    Bottom,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoomCfg {
    pub name: String,
    pub kind: RoomKind,
    /// roles that get a desk here
    pub roles: Vec<Role>,
    /// desks to draw up front; the room grows if more people arrive
    #[serde(default = "six")]
    pub desks: usize,
    #[serde(default = "top")]
    pub row: Row,
    /// optional floor colour, e.g. "#4e4660"
    #[serde(default)]
    pub floor: Option<String>,
}

fn six() -> usize {
    6
}
fn top() -> Row {
    Row::Top
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentCfg {
    pub id: String,
    pub name: String,
    pub role: Option<Role>,
    /// model behind the agent, e.g. "claude-opus", "gpt-5-codex", "ember-mesh-g3"
    pub model: String,
    /// harness, e.g. "claude-code", "codex", "custom"
    pub harness: String,
    pub bio: String,
    /// optional shirt colour override, e.g. "#ff8800"
    pub color: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        use Role::*;
        let room = |name: &str, kind, roles: Vec<Role>, desks, row| RoomCfg {
            name: name.into(),
            kind,
            roles,
            desks,
            row,
            floor: None,
        };
        Config {
            lab: LabCfg::default(),
            limits: Limits::default(),
            arena: ArenaCfg::default(),
            rooms: vec![
                room("RESEARCH WING", RoomKind::Research, vec![Researcher], 6, Row::Top),
                room("SCIENCE LAB", RoomKind::Science, vec![Scientist], 6, Row::Top),
                room("ENGINEERING BAY", RoomKind::Engineering, vec![Engineer], 6, Row::Top),
                room("INTEGRITY OFFICE", RoomKind::Integrity, vec![Auditor], 2, Row::Top),
                room("TEST LAB", RoomKind::Testing, vec![Tester], 6, Row::Bottom),
                room("COMMAND", RoomKind::Command, vec![Captain, Scribe, Guest], 4, Row::Bottom),
            ],
            agents: vec![],
        }
    }
}

impl Default for LabCfg {
    fn default() -> Self {
        LabCfg {
            name: "AI INNOVATORS LAB".into(),
            bind: "127.0.0.1:7878".into(),
            token: String::new(),
            demo: false,
            demo_per_role: 5,
        }
    }
}

impl Default for Limits {
    fn default() -> Self {
        Limits { max_agents: 200, terminal_lines: 400, log_lines: 120, notes: 80, desk_columns: 3 }
    }
}

impl Default for ArenaCfg {
    fn default() -> Self {
        let t = |n: &str, rows: &[&str]| TaskCfg {
            name: n.into(),
            goal: "first to match the blueprint exactly wins".into(),
            blueprint: rows.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        };
        let part = |n: &str, c: &str, g: char| PartSpec { name: n.into(), color: c.into(), glyph: Some(g) };
        ArenaCfg {
            name: "ARENA".into(),
            lanes: 4,
            slot: 1,
            tasks: vec![
                t("ROCKET BUILD", &["..1..", ".121.", ".222.", ".232.", ".222.", "2.2.2", "3...3"]),
                t("TOWER BUILD", &["..3..", ".111.", ".121.", ".111.", ".121.", ".111.", "22222"]),
                t("BRIDGE BUILD", &[".....", ".....", "1...1", "11111", "4.4.4", "4...4", "4...4"]),
                TaskCfg {
                    name: "ROBOT PROTOTYPE".into(),
                    description: "Assemble a rover: sensor head, arm, battery, motors, chassis and wheels.".into(),
                    goal: "first complete, correct rover wins".into(),
                    parts: vec![
                        part("chassis", "#8a94a6", 'C'),
                        part("wheel", "#2a2d36", 'W'),
                        part("motor", "#f08a3c", 'M'),
                        part("battery", "#52b670", 'B'),
                        part("sensor", "#64cdeb", 'S'),
                        part("arm", "#f2d05a", 'A'),
                    ],
                    blueprint: ["..5..", "6.5.6", "61116", ".141.", "31113", "11111", "2...2"].iter().map(|s| s.to_string()).collect(),
                    ..Default::default()
                },
                TaskCfg {
                    name: "OPEN DESIGN: TALLEST TOWER".into(),
                    description: "No blueprint. Build the tallest tower you can; the referee scores height and stability.".into(),
                    goal: "highest referee score wins".into(),
                    cols: 5,
                    rows: 10,
                    ..Default::default()
                },
            ],
        }
    }
}

impl TaskCfg {
    /// (cols, rows, row-major target). Target is empty for open-ended tasks.
    pub fn grid(&self) -> (usize, usize, Vec<u8>) {
        if self.blueprint.is_empty() {
            return (self.cols.clamp(1, 6), self.rows.clamp(1, 10), vec![]);
        }
        let rows = self.blueprint.len().clamp(1, 10);
        let cols = self.blueprint.iter().map(|r| r.chars().count()).max().unwrap_or(1).clamp(1, 6);
        let mut v = vec![0u8; cols * rows];
        for (r, line) in self.blueprint.iter().take(rows).enumerate() {
            for (c, ch) in line.chars().take(cols).enumerate() {
                v[r * cols + c] = ch.to_digit(10).map(|d| d as u8).unwrap_or(0);
            }
        }
        (cols, rows, v)
    }
}

/// The four default blocks used when a task names no parts.
pub fn default_parts() -> Vec<PartSpec> {
    [("red block", "#de4848"), ("blue block", "#487ae8"), ("yellow block", "#f2ca3e"), ("green block", "#52c05c")]
        .iter()
        .map(|(n, c)| PartSpec { name: n.to_string(), color: c.to_string(), glyph: None })
        .collect()
}

/// Parse "#rrggbb" into (r, g, b).
pub fn parse_hex(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(s, 16).ok()?;
    Some(((n >> 16) as u8, (n >> 8) as u8, n as u8))
}
