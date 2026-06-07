//! Player attribute models.
//!
//! Each player has a fixed set of attributes that influence simulation outcomes.
//! In production these come from the global database; for now we generate test data.

use protocol::Team;

/// Core football attributes for a single player.
#[derive(Debug, Clone, Copy)]
pub struct PlayerAttributes {
    /// Player index in the match squad (0-21).
    pub index: u16,
    /// Team this player belongs to.
    pub team: Team,
    /// Position group.
    pub position: Position,
    /// Overall ability (0-100).
    pub overall: u8,
    /// Finishing ability (0-100).
    pub finishing: u8,
    /// Passing ability (0-100).
    pub passing: u8,
    /// Dribbling ability (0-100).
    pub dribbling: u8,
    /// Defending ability (0-100).
    pub defending: u8,
    /// Pace (0-100).
    pub pace: u8,
    /// Stamina (0-100).
    pub stamina: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Goalkeeper,
    Defender,
    Midfielder,
    Forward,
}

impl Position {
    /// Approximate weight of this position in possession calculations.
    pub fn possession_weight(self) -> f32 {
        match self {
            Position::Goalkeeper => 0.05,
            Position::Defender => 0.25,
            Position::Midfielder => 0.45,
            Position::Forward => 0.25,
        }
    }
}

/// Generate a synthetic squad of 11 players for testing.
pub fn generate_synthetic_squad(team: Team, base_overall: u8) -> [PlayerAttributes; 11] {
    let positions = [
        (Position::Goalkeeper, 0),
        (Position::Defender, 1),
        (Position::Defender, 2),
        (Position::Defender, 3),
        (Position::Defender, 4),
        (Position::Midfielder, 5),
        (Position::Midfielder, 6),
        (Position::Midfielder, 7),
        (Position::Midfielder, 8),
        (Position::Forward, 9),
        (Position::Forward, 10),
    ];

    let mut squad = [PlayerAttributes::default(); 11];
    for (i, (pos, idx)) in positions.iter().enumerate() {
        let variance = (i as u8).wrapping_mul(3) % 15;
        let ovr = base_overall.saturating_sub(variance);
        squad[i] = PlayerAttributes {
            index: *idx as u16,
            team,
            position: *pos,
            overall: ovr,
            finishing: if *pos == Position::Forward {
                ovr.saturating_add(5).min(100)
            } else {
                ovr.saturating_sub(10)
            },
            passing: if *pos == Position::Midfielder {
                ovr.saturating_add(5).min(100)
            } else {
                ovr.saturating_sub(5)
            },
            dribbling: ovr.saturating_sub(3),
            defending: if *pos == Position::Defender {
                ovr.saturating_add(5).min(100)
            } else {
                ovr.saturating_sub(15)
            },
            pace: ovr.saturating_sub(2),
            stamina: 85,
        };
    }
    squad
}

impl Default for PlayerAttributes {
    fn default() -> Self {
        Self {
            index: 0,
            team: Team::Home,
            position: Position::Midfielder,
            overall: 70,
            finishing: 60,
            passing: 60,
            dribbling: 60,
            defending: 60,
            pace: 60,
            stamina: 80,
        }
    }
}
