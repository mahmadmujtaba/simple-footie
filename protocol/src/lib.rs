//! Binary protocol types for the football game engine.
//!
//! Matches the architecture doc:
//! - Command: 10 bytes (steady state, token cached server-side after handshake)
//! - Event:   12 bytes

use serde::{Deserialize, Serialize};

use std::ptr::read_unaligned;

// ── Command Types ───────────────────────────────────────────────

/// A command from client to server (10 bytes steady state).
///
/// On first command per match, the client includes the 16-byte token
/// alongside this packet. After the server caches the token, subsequent
/// commands are just this 10-byte packet.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(C, packed)]
pub struct CommandPacket {
    pub match_id: u32,
    pub sequence: u16,
    pub command_type: CommandType,
    pub arg1: u8,
    pub arg2: u8,
    pub arg3: u8,
}

/// All possible command types a player can send.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum CommandType {
    Mentality = 0,
    Substitution = 1,
    Press = 2,
    Tempo = 3,
    Width = 4,
}

impl TryFrom<u8> for CommandType {
    type Error = ();

    fn try_from(val: u8) -> Result<Self, Self::Error> {
        match val {
            0 => Ok(CommandType::Mentality),
            1 => Ok(CommandType::Substitution),
            2 => Ok(CommandType::Press),
            3 => Ok(CommandType::Tempo),
            4 => Ok(CommandType::Width),
            _ => Err(()),
        }
    }
}

/// Mentality values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Mentality {
    Normal = 0,
    Attack = 1,
    Defend = 2,
}

/// Press intensity values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Press {
    Low = 0,
    Medium = 1,
    High = 2,
}

/// Tempo values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Tempo {
    Slow = 0,
    Normal = 1,
    Fast = 2,
}

/// Width values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Width {
    Narrow = 0,
    Normal = 1,
    Wide = 2,
}

// ── Event Types ─────────────────────────────────────────────────

/// An event emitted from server to client (12 bytes).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(C, packed)]
pub struct EventPacket {
    pub match_id: u32,
    pub event_type: EventType,
    pub team: Team,
    pub player_index: u16,
    pub value: f32,
}

impl EventPacket {
    /// Safely copy fields out of a packed struct to avoid unaligned access UB.
    pub fn unpack(&self) -> (u32, EventType, Team, u16, f32) {
        let ptr = self as *const Self as *const u8;
        let match_id = unsafe { read_unaligned(ptr as *const u32) };
        // event_type at offset 4, team at offset 5 — these are u8, always aligned
        let player_index = unsafe { read_unaligned(ptr.add(6) as *const u16) };
        let value = unsafe { read_unaligned(ptr.add(8) as *const f32) };
        (match_id, self.event_type, self.team, player_index, value)
    }
}

/// All possible match event types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum EventType {
    Kickoff = 0,
    Goal = 1,
    Shot = 2,
    ShotOnTarget = 3,
    Save = 4,
    Corner = 5,
    FreeKick = 6,
    Foul = 7,
    YellowCard = 8,
    RedCard = 9,
    Substitution = 10,
    HalfTime = 11,
    FullTime = 12,
    Injury = 13,
    Offside = 14,
    Miss = 15,
}

/// Team identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Team {
    Home = 0,
    Away = 1,
}

// ── Tactic State ────────────────────────────────────────────────

/// Packed tactic settings for one team.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[repr(C, packed)]
pub struct TacticState {
    pub mentality: Mentality,
    pub press: Press,
    pub tempo: Tempo,
    pub width: Width,
    /// Formation: packed as 4 bytes (e.g. 4-4-2 -> [4, 4, 2, 0])
    pub formation: [u8; 4],
}

impl Default for TacticState {
    fn default() -> Self {
        Self {
            mentality: Mentality::Normal,
            press: Press::Medium,
            tempo: Tempo::Normal,
            width: Width::Normal,
            formation: [4, 4, 2, 0],
        }
    }
}

// ── Match State (shared between engine and server) ──────────────

/// Full ephemeral state for a single match (target: ~80 bytes).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(C)]
pub struct MatchState {
    pub match_id: u32,
    /// 16-byte authentication token.
    pub token: [u8; 16],
    /// Last applied command sequence (for idempotency).
    pub last_seq: u16,
    /// Current score [home, away].
    pub score: [u8; 2],
    /// Current minute (0-90).
    pub minute: u8,
    /// Possession as fraction (0.0 – 1.0).
    pub possession: f32,
    /// Stamina ratio per team [0.0 – 1.0].
    pub stamina: [f32; 2],
    /// Tactic for each team.
    pub tactic: [TacticState; 2],
    /// RNG seed for deterministic replay.
    pub rng_seed: u64,
}

impl MatchState {
    pub const SIZE: usize = 4 + 16 + 2 + 2 + 1 + 4 + 8 + 4 + 8;
}
