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

/// The 7 Capability Card markers, index = marker − 1, in MODEL_PROTOCOL.md §6 order:
/// `[0]` 1 Memory, `[1]` 2 Continual, `[2]` 3 Self-knowledge, `[3]` 4 Self-model,
/// `[4]` 5 Curiosity, `[5]` 6 Goal transfer, `[6]` 7 Others. Each score is 0.0-1.0
/// (Brier/accuracy/recall per §6's scoring column). `None` = not measured yet, shown
/// plainly, never a fake number; `Some(0.0)` = measured and scored 0 (§6: "a model that
/// skips them scores 0 on the marker") — these two states are never conflated.
pub type Card = [Option<f32>; 7];

/// One resident's position/state for a batched `world_tick`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldResidentTick {
    pub resident_id: String,
    pub x: f32,
    pub y: f32,
    pub awake: bool,
    /// Mandatory, like `arena_action`'s `intent` (MODEL_PROTOCOL.md §3).
    pub intent: String,
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
    /// THE WORLD (world/DESIGN.md §4): a resident appears on the map, gets a viewer card.
    WorldJoin {
        resident_id: String,
        name: String,
        /// owning model name (models/<name>/)
        model: String,
        /// agent id of whoever trained/owns it
        owner: String,
        #[serde(default)]
        card: Card,
    },
    /// Resident removed (benched/returned to registry, not deleted from `models/`).
    WorldLeave {
        resident_id: String,
    },
    /// Batched per-tick position/state diff for the viewer (2-4/s, not one event per resident).
    WorldTick {
        tick: u64,
        residents: Vec<WorldResidentTick>,
    },
    /// Persistent object placed or removed (`object: None` = remove).
    WorldBuild {
        resident_id: String,
        x: i32,
        y: i32,
        #[serde(default)]
        object: Option<PartSpec>,
    },
    /// Drawn as a "zzz" marker; consolidation in progress.
    WorldSleep {
        resident_id: String,
        sleeping: bool,
    },
    /// Capability Card score update, shown beside the resident.
    WorldCard {
        resident_id: String,
        card: Card,
    },
    /// Global kill switch fired; viewer shows the room halted.
    WorldKill {
        reason: String,
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
