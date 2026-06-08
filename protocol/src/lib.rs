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

/// An event emitted from server to client (14 bytes).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(C, packed)]
pub struct EventPacket {
    pub match_id: u32,
    pub event_type: EventType,
    pub team: Team,
    pub player_index: u16,
    pub minute: u8,
    pub unused: u8,
    pub value: f32,
}

impl EventPacket {
    /// Safely copy fields out of a packed struct to avoid unaligned access UB.
    pub fn unpack(&self) -> (u32, EventType, Team, u16, u8, f32) {
        let ptr = self as *const Self as *const u8;
        let match_id = unsafe { read_unaligned(ptr as *const u32) };
        // event_type at offset 4, team at offset 5 — these are u8, always aligned
        let player_index = unsafe { read_unaligned(ptr.add(6) as *const u16) };
        let minute = unsafe { *ptr.add(8) };
        let value = unsafe { read_unaligned(ptr.add(10) as *const f32) };
        (match_id, self.event_type, self.team, player_index, minute, value)
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
    PenaltyGoal = 16,
    PenaltyMiss = 17,
    PenaltySave = 18,
    ExtraTimeStart = 19,
    ExtraTimeHalfTime = 20,
    PenaltyShootoutStart = 21,
    Pass = 22,
    Tackle = 23,
    Dribble = 24,
    Interception = 25,
    Block = 26,
}

impl EventType {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(EventType::Kickoff),
            1 => Some(EventType::Goal),
            2 => Some(EventType::Shot),
            3 => Some(EventType::ShotOnTarget),
            4 => Some(EventType::Save),
            5 => Some(EventType::Corner),
            6 => Some(EventType::FreeKick),
            7 => Some(EventType::Foul),
            8 => Some(EventType::YellowCard),
            9 => Some(EventType::RedCard),
            10 => Some(EventType::Substitution),
            11 => Some(EventType::HalfTime),
            12 => Some(EventType::FullTime),
            13 => Some(EventType::Injury),
            14 => Some(EventType::Offside),
            15 => Some(EventType::Miss),
            16 => Some(EventType::PenaltyGoal),
            17 => Some(EventType::PenaltyMiss),
            18 => Some(EventType::PenaltySave),
            19 => Some(EventType::ExtraTimeStart),
            20 => Some(EventType::ExtraTimeHalfTime),
            21 => Some(EventType::PenaltyShootoutStart),
            22 => Some(EventType::Pass),
            23 => Some(EventType::Tackle),
            24 => Some(EventType::Dribble),
            25 => Some(EventType::Interception),
            26 => Some(EventType::Block),
            _ => None,
        }
    }
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

/// Format the match minute (0-135) into a standard football display string.
pub fn format_match_minute(minute: u8) -> String {
    if minute == 0 {
        "0'".into()
    } else if minute <= 45 {
        format!("{}'", minute)
    } else if minute <= 50 {
        format!("45+{}'", minute - 45)
    } else if minute <= 95 {
        format!("{}'", minute - 5)
    } else if minute <= 100 {
        format!("90+{}'", minute - 95)
    } else if minute <= 115 {
        format!("{}'", 90 + (minute - 100))
    } else if minute <= 117 {
        format!("105+{}'", minute - 115)
    } else if minute <= 132 {
        format!("{}'", 105 + (minute - 117))
    } else if minute <= 135 {
        format!("120+{}'", minute - 132)
    } else {
        "120'".into()
    }
}

pub fn get_player_name(team: Team, index: u16) -> String {
    match team {
        Team::Home => {
            let home_names = [
                "Rusty McSave",
                "Lex Byte",
                "Corey Heap",
                "Sean Stack",
                "Max Alloc",
                "Tommy Mutex",
                "Rusty Channel",
                "Eddie Promise",
                "Ray Arc",
                "Ferris Enum",
                "Will Crash",
                "Ben Chmark",
                "Ollie Fset",
                "Pat Ter",
                "Sid Effect",
                "Niles Serde",
                "Jamie Macro",
            ];
            if (index as usize) < home_names.len() {
                home_names[index as usize].to_string()
            } else {
                format!("Home Player {}", index)
            }
        }
        Team::Away => {
            let away_names = [
                "Null Pointer",      // 0 - GK
                "Stack Overflow",    // 1 - LB
                "Buffer Overflow",   // 2 - CB
                "Race Condition",    // 3 - CB
                "Memory Leak",       // 4 - RB
                "Garbage Collector", // 5 - DM
                "Syntax Error",      // 6 - CM
                "Merge Conflict",    // 7 - LM
                "Infinite Loop",     // 8 - RM
                "Segmentation Fault",// 9 - ST
                "Out of Memory",     // 10 - ST
                "Deadlock",          // 11
                "Kernel Panic",      // 12
                "Null Reference",    // 13
                "Thread Block",      // 14
                "Dirty Read",        // 15
                "Write Hazard",      // 16
            ];
            if (index as usize) < away_names.len() {
                away_names[index as usize].to_string()
            } else {
                format!("Away Player {}", index)
            }
        }
    }
}

pub fn generate_exciting_commentary(
    event_type: EventType,
    team: Team,
    player_index: u16,
    minute: u8,
    _value: f32,
) -> String {
    let player_name = get_player_name(team, player_index);
    let team_name = match team {
        Team::Home => "Rustington United",
        Team::Away => "FC Terminal",
    };

    // Simple deterministic hash to pick a variation
    let hash = (minute as usize)
        .wrapping_mul(31)
        .wrapping_add(player_index as usize)
        .wrapping_mul(17)
        .wrapping_add(event_type as usize);

    let pick_str = |options: &[&str]| -> String {
        let idx = hash % options.len();
        options[idx].to_string()
    };

    let pick_string = |options: &[String]| -> String {
        let idx = hash % options.len();
        options[idx].clone()
    };

    match event_type {
        EventType::Kickoff => {
            let options = [
                "And we are underway! The referee blows the whistle and the battle begins!",
                "KICKOFF! The crowd roars as the ball is kicked into motion!",
                "The wait is over! The match kicks off under an electric atmosphere!",
                "We have kickoff! 90 minutes of pure drama and passion starts now!",
            ];
            pick_str(&options)
        }
        EventType::Goal => {
            let options = [
                format!("GOOOOOOOOAAAAAAAL!!! {} HAS SHATTERED THE NET! ABSOLUTE SCENES!", player_name),
                format!("UNBELIEVABLE! {} scores a sensational goal! The stadium is erupting!", player_name),
                format!("GOAL! GOAL! GOAL! {} buries it with clinical precision! What a finish!", player_name),
                format!("IT'S IN! {} scores! A moment of pure magic that will go down in history!", player_name),
                format!("GOOOOOAL! {} unleashes a thunderbolt into the top corner! Unstoppable!", player_name),
                format!("HE'S DONE IT! {} finds the back of the net! {} fans are going wild!", player_name, team_name),
            ];
            pick_string(&options)
        }
        EventType::Shot => {
            let options = [
                format!("HE UNLEASHES A ROCKET! {} lets fly from distance!", player_name),
                format!("{} goes for glory! A venomous strike from the edge of the box!", player_name),
                format!("{} takes it on the volley! A spectacular effort!", player_name),
                format!("{} curls one towards the far post! Everyone holds their breath!", player_name),
                format!("{} drills it hard and low! A speculative effort!", player_name),
            ];
            pick_string(&options)
        }
        EventType::ShotOnTarget => {
            let options = [
                format!("IT'S ON TARGET! {} forces a desperate action from the keeper!", player_name),
                format!("A stinging effort from {}! That was bound for the top corner!", player_name),
                format!("{} hits it clean! A brilliant shot that tests the goalkeeper's reflexes!", player_name),
                format!("{} directs a powerful header towards goal! It's on target!", player_name),
            ];
            pick_string(&options)
        }
        EventType::Save => {
            let options = [
                format!("OH MY GOD! WHAT A SAVE! {} pulls off an absolute miracle!", player_name),
                format!("SENSATIONAL REFLEXES! {} tips the rocket over the crossbar!", player_name),
                format!("{} flies through the air to deny a certain goal! Unbelievable GK play!", player_name),
                format!("How did he keep that out?! {} is an absolute wall today!", player_name),
                format!("Stunning double-save reflexes! {} stands tall to deny the attackers!", player_name),
            ];
            pick_string(&options)
        }
        EventType::Miss => {
            let options = [
                format!("OH, HE'S MISSED IT! {} sends it miles over the bar!", player_name),
                format!("{} drags it wide! What a golden opportunity wasted!", player_name),
                format!("So close! {}'s shot grazes the outside of the post!", player_name),
                format!("{} can't believe it! He had the whole net to aim at!", player_name),
                format!("A wild attempt! {} sends the ball flying into the stands!", player_name),
            ];
            pick_string(&options)
        }
        EventType::Foul => {
            let options = [
                format!("CRUNCH! {} commits a nasty foul! The referee is running over!", player_name),
                format!("{} gets caught late! A cynical challenge that stops the counter!", player_name),
                format!("A heavy collision! {} is penalized for a clumsy challenge.", player_name),
                format!("{} slides in aggressively and takes down the attacker! Foul!", player_name),
            ];
            pick_string(&options)
        }
        EventType::Corner => {
            let options = [
                format!("Corner kick for {}! The big defenders are heading up into the box!", team_name),
                format!("{} wins a corner! This is a massive opportunity to put pressure on!", team_name),
                format!("A curling corner is swung in! Chaos in the penalty area!"),
            ];
            pick_string(&options)
        }
        EventType::FreeKick => {
            let options = [
                format!("Free kick awarded to {}! {} steps up to take it...", team_name, player_name),
                format!("{} wins a free kick in a dangerous position! {} is lining up the shot!", team_name, player_name),
                format!("{} floats a delicate free kick into the box!", player_name),
            ];
            pick_string(&options)
        }
        EventType::YellowCard => {
            let options = [
                format!("🟨 THE REF HAS THE CARD OUT! {} gets a yellow card for that reckless challenge!", player_name),
                format!("🟨 Yellow card! {} is walking a tightrope now!", player_name),
                format!("🟨 No complaints there. {} goes into the referee's book!", player_name),
            ];
            pick_string(&options)
        }
        EventType::RedCard => {
            let options = [
                format!("🟥 RED CARD!!! UNBELIEVABLE! {} is sent off! The referee shows no mercy!", player_name),
                format!("🟥 OFF! OFF! OFF! A straight red for {}! Absolute disaster for {}!", player_name, team_name),
                format!("🟥 RED CARD! {} is walking down the tunnel! A moment of madness!", player_name),
            ];
            pick_string(&options)
        }
        EventType::Substitution => {
            let options = [
                format!("Tactical change for {}! The manager is trying to shake things up!", team_name),
                format!("Substitution made by {}! Fresh legs coming onto the pitch!", team_name),
            ];
            pick_string(&options)
        }
        EventType::HalfTime => {
            let options = [
                "🏁 HALF TIME! The referee blows the whistle. Players head to the dressing room to regroup!",
                "🏁 Half Time! A breathless first half comes to an end. Time for some tactical adjustments!",
            ];
            pick_str(&options)
        }
        EventType::FullTime => {
            let options = [
                "🏆 FULL TIME! The final whistle blows! What an absolute classic of a match!",
                "🏆 Full Time! The referee brings this epic encounter to an end! SENSATIONAL football!",
            ];
            pick_str(&options)
        }
        EventType::Injury => {
            let options = [
                format!("⚠️ Oh no! {} goes down clutching his leg! The medical staff is rushing on!", player_name),
                format!("⚠️ Play is stopped as {} is receiving medical attention. Hopefully it's not serious!", player_name),
            ];
            pick_string(&options)
        }
        EventType::Offside => {
            let options = [
                format!("Offside! {} timed his run just a fraction too early!", player_name),
                format!("The linesman raises the flag! {} is caught in an offside position!", player_name),
            ];
            pick_string(&options)
        }
        EventType::PenaltyGoal => {
            let options = [
                format!("⚽ GOAL! CLINICAL! {} coolly steps up and sends the keeper the wrong way!", player_name),
                format!("⚽ GOAL! {} smashes the penalty into the top corner! Absolute ice in his veins!", player_name),
                format!("⚽ PENALTY GOAL! {} buries it with supreme confidence!", player_name),
            ];
            pick_string(&options)
        }
        EventType::PenaltyMiss => {
            let options = [
                format!("❌ HE'S MISSED THE PENALTY! {} sends it wide! The crowd is in shock!", player_name),
                format!("❌ OVER THE BAR! {} chokes under immense pressure! Unbelievable!", player_name),
            ];
            pick_string(&options)
        }
        EventType::PenaltySave => {
            let options = [
                format!("🧤 SAVED!!! {} GK GUESSES RIGHT AND DENIES THE PENALTY! HEROIC!", player_name),
                format!("🧤 UNBELIEVABLE! {} GK pulls off a stunning penalty save! The crowd goes wild!", player_name),
            ];
            pick_string(&options)
        }
        EventType::ExtraTimeStart => {
            let options = [
                "⏰ EXTRA TIME STARTS! 30 more minutes of excruciating drama will be played!",
                "⏰ Extra Time begins! The players are exhausted but they must dig deep now!",
            ];
            pick_str(&options)
        }
        EventType::ExtraTimeHalfTime => {
            let options = [
                "⏰ Extra Time Half Time! 15 minutes down, 15 minutes to go. The tension is unbearable!",
            ];
            pick_str(&options)
        }
        EventType::PenaltyShootoutStart => {
            let options = [
                "🧤 PENALTY SHOOTOUT STARTS! It all comes down to this! Absolute nerves of steel required!",
            ];
            pick_str(&options)
        }
        EventType::Pass => {
            let options = [
                format!("WHAT A BALL! {} slices the defense wide open with a pinpoint pass!", player_name),
                format!("{} plays a gorgeous, curling pass into space!", player_name),
                format!("Exquisite vision! {} threads the needle perfectly!", player_name),
                format!("{} keeps the tempo high with a crisp, first-time pass!", player_name),
                format!("A cheeky backheel pass from {}! Pure class!", player_name),
                format!("{} sprays a beautiful diagonal ball across the pitch!", player_name),
            ];
            pick_string(&options)
        }
        EventType::Tackle => {
            let options = [
                format!("BOOM! A thunderous tackle by {} wins the ball back cleanly!", player_name),
                format!("{} puts his body on the line with a spectacular sliding tackle!", player_name),
                format!("Perfect timing! {} dispossesses the attacker with surgical precision!", player_name),
                format!("{} says 'NOT TODAY!' and absolutely robs the attacker!", player_name),
                format!("An aggressive, crunching challenge from {}! Play on!", player_name),
            ];
            pick_string(&options)
        }
        EventType::Dribble => {
            let options = [
                format!("OH MY WORD! {} turns the defender inside out with a jaw-dropping stepover!", player_name),
                format!("{} dances past one, past two! Absolutely mesmerizing dribbling!", player_name),
                format!("{} bursts forward with electric pace, leaving defenders in the dust!", player_name),
                format!("Pure magic! {} nutmegs the defender! The crowd is on its feet!", player_name),
                format!("{} controls the ball like it's glued to his boot, gliding forward!", player_name),
            ];
            pick_string(&options)
        }
        EventType::Interception => {
            let options = [
                format!("SENSATIONAL! {} reads the play like a book and cuts out the pass!", player_name),
                format!("{} intercepts! He was three steps ahead of the attacker!", player_name),
                format!("A vital interception by {}! That was a dangerous attack building!", player_name),
                format!("{} snatches the ball out of thin air! Brilliant defensive awareness!", player_name),
            ];
            pick_string(&options)
        }
        EventType::Block => {
            let options = [
                format!("HEROIC! {} throws himself in front of the shot! What a block!", player_name),
                format!("{} stands tall and blocks the rocket! Absolutely fearless!", player_name),
                format!("A crucial block by {}! That was heading straight for the corner!", player_name),
                format!("{} denies the shot with a desperate, last-ditch block!", player_name),
            ];
            pick_string(&options)
        }
    }
}
