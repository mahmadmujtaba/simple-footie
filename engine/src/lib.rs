//! Algebraic football simulation engine.
//!
//! Core philosophy:
//! - Football outcomes (possession, shots, goals) computed by probability formulas, not physics.
//! - Event-driven: matches advance only when commands arrive.
//! - Deterministic: seeded with (match_id, token), replayable.
//! - Minimal state: ~80 bytes per match.

pub mod commands;
pub mod database;
pub mod match_store;
pub mod player;
pub mod simulation;

pub use commands::{apply_command, CommandError};
pub use match_store::MatchStore;
pub use player::PlayerAttributes;
pub use simulation::{simulate_minutes, MatchResult, SimEvent};
