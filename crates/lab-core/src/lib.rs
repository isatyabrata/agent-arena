//! lab-core: the harness-agnostic virtual world.
//!
//! The world is a pure state machine. Anything that can emit JSON events
//! (Claude Code hooks, Codex, a Python script, a freshly trained Rust model)
//! drives it through [`Event`]s. The same code runs inside the server and
//! inside the browser (compiled to WASM), so every viewer sees the same world.

pub mod config;
pub mod demo;
pub mod event;
pub mod layout;
pub mod rng;
pub mod world;

pub use config::*;
pub use demo::DemoFeed;
pub use event::*;
pub use world::*;
