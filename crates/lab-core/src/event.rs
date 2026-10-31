//! The plug-in protocol. Every change to the world is one of these JSON events.
//! See docs/PROTOCOL.md for examples.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Captain,
    Researcher,
    Scientist,
    Engineer,
    Tester,
    Auditor,
    Scribe,
    Guest,
}

impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Role::Captain => "CAPTAIN",
            Role::Researcher => "RESEARCHER",
            Role::Scientist => "SCIENTIST",
            Role::Engineer => "ENGINEER",
            Role::Tester => "TESTER",
            Role::Auditor => "INTEGRITY AUDITOR",
            Role::Scribe => "SCRIBE",
            Role::Guest => "GUEST",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Activity {
    #[default]
    Idle,
    Typing,
    Reading,
    Thinking,
    Talking,
    Training,
    Testing,
    Auditing,
    Grading,
    Waiting,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Pretrain,
    Finetune,
    Eval,
    Done,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteKind {
    Works,
    Fails,
    Info,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PartSpec {
    /// e.g. "wheel", "motor", "red block", "letter A"
    pub name: String,
    /// "#rrggbb"
    #[serde(default)]
    pub color: String,
    /// one character drawn on the part
    #[serde(default)]
    pub glyph: Option<char>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RobotSpec {
    /// model identifier shown above the robot
    pub name: String,
    /// 0..7 palette slot for the robot body
    #[serde(default)]
    pub color: u8,
    /// e.g. "arch ember-mesh, 1.2M params, finetuned on blueprints-v2"
    #[serde(default)]
    pub profile: String,
    /// agent id of whoever trained it
    #[serde(default)]
    pub owner: String,
}

/// What a robot does in the arena. `row: -1` targets the block pile, with `col` = block color - 1.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "do", rename_all = "snake_case")]
pub enum RobotAction {
    MoveTo { col: i32, row: i32 },
    /// pick a part (1-based index into the task's parts)
    Pick {
        #[serde(alias = "part")]
        color: u8,
    },
    Place { col: i32, row: i32 },
    Remove { col: i32, row: i32 },
    Think { text: String },
    /// put a part straight into a cell (no walking), for fast or abstract tasks
    SetCell { col: i32, row: i32, part: u8 },
    /// report progress 0..1 yourself (open-ended tasks without a target)
    Progress { value: f32 },
    /// record any metric, e.g. {"name":"speed","value":"1.4 m/s"}
    Metric { name: String, value: String },
    /// hand in a text answer or design (crossword words, a plan, code...)
    Submit { text: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// A new agent appears (any harness, any model). It takes a free desk in its role's room.
    Join {
        id: String,
        name: String,
        role: Role,
        /// model behind the agent, e.g. "claude-opus"
        #[serde(default)]
        model: Option<String>,
        /// harness, e.g. "claude-code", "codex"
        #[serde(default)]
        harness: Option<String>,
        #[serde(default)]
        bio: Option<String>,
        /// shirt colour override "#rrggbb"
        #[serde(default)]
        color: Option<String>,
    },
    Leave {
        id: String,
    },
    /// Walk somewhere: desk, whiteboard, bench, cluster, registry, notes, coffee,
    /// office, arena, test, or "agent:<id>" / "desk:<id>".
    Goto {
        id: String,
        place: String,
    },
    /// What the agent is doing and what its computer screen shows.
    Status {
        id: String,
        activity: Activity,
        #[serde(default)]
        task: Option<String>,
        #[serde(default)]
        screen: Option<Vec<String>>,
    },
    Say {
        id: String,
        text: String,
    },
    /// Append to the agent's terminal (shown when you click them). Newlines split lines.
    Output {
        id: String,
        text: String,
    },
    /// Training telemetry; drawn as a loss curve on the owner's screen.
    ModelProgress {
        model: String,
        owner: String,
        stage: Stage,
        progress: f32,
        loss: f32,
    },
    /// Start a live competition. The task is defined right here, at runtime:
    /// any parts, any grid, an optional target, and free-text goal/description.
    ArenaStart {
        title: String,
        #[serde(default)]
        description: String,
        /// what counts as winning, shown to viewers
        #[serde(default)]
        goal: String,
        #[serde(default = "default_cols")]
        cols: usize,
        #[serde(default = "default_rows")]
        rows: usize,
        /// parts robots can use; empty = red, blue, yellow, green blocks
        #[serde(default)]
        parts: Vec<PartSpec>,
        /// optional target, row-major, 0 = empty, n = parts[n-1].
        /// Empty = open-ended task; robots/referee report progress and scores.
        #[serde(default, alias = "blueprint")]
        target: Vec<u8>,
        robots: Vec<RobotSpec>,
    },
    ArenaAction {
        robot: usize,
        action: RobotAction,
        /// what the model is trying to do right now, shown live
        #[serde(default)]
        intent: Option<String>,
    },
    /// Referee (or judge model) score for one robot.
    ArenaScore {
        robot: usize,
        score: f32,
        #[serde(default)]
        note: String,
    },
    ArenaEnd {
        /// robot indices, best first
        ranking: Vec<usize>,
        #[serde(default)]
        note: String,
    },
    /// Shared knowledge board: what works, what does not.
    Note {
        kind: NoteKind,
        text: String,
        by: String,
    },
    /// Integrity auditor flag (cheating, reused known architecture, leakage...).
    Flag {
        by: String,
        target: String,
        reason: String,
    },
    Phase {
        name: String,
    },
    Log {
        text: String,
    },
}

impl Event {
    /// The agent an event acts on, if any.
    pub fn agent_id(&self) -> Option<&str> {
        match self {
            Event::Join { id, .. }
            | Event::Leave { id }
            | Event::Goto { id, .. }
            | Event::Status { id, .. }
            | Event::Say { id, .. }
            | Event::Output { id, .. } => Some(id),
            Event::ModelProgress { owner, .. } => Some(owner),
            _ => None,
        }
    }
}

fn default_cols() -> usize {
    5
}
fn default_rows() -> usize {
    7
}
