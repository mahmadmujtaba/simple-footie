//! Command application — translates binary commands into match state mutations.

use protocol::{CommandPacket, CommandType, MatchState, Mentality, Press, Tempo, Width};
use thiserror::Error;

/// Errors that can occur during command application.
#[derive(Debug, Clone, Copy, Error)]
pub enum CommandError {
    #[error("invalid mentality value: {0}")]
    InvalidMentality(u8),
    #[error("invalid press value: {0}")]
    InvalidPress(u8),
    #[error("invalid tempo value: {0}")]
    InvalidTempo(u8),
    #[error("invalid width value: {0}")]
    InvalidWidth(u8),
    #[error("substitution player index out of range: {0}")]
    InvalidPlayerIndex(u8),
    #[error("command sequence {0} is stale (last applied: {1})")]
    StaleSequence(u16, u16),
}

/// Convert a raw u8 to a Mentality, returning an error on invalid values.
fn parse_mentality(val: u8) -> Result<Mentality, CommandError> {
    match val {
        0 => Ok(Mentality::Normal),
        1 => Ok(Mentality::Attack),
        2 => Ok(Mentality::Defend),
        _ => Err(CommandError::InvalidMentality(val)),
    }
}

/// Convert a raw u8 to a Press, returning an error on invalid values.
fn parse_press(val: u8) -> Result<Press, CommandError> {
    match val {
        0 => Ok(Press::Low),
        1 => Ok(Press::Medium),
        2 => Ok(Press::High),
        _ => Err(CommandError::InvalidPress(val)),
    }
}

/// Convert a raw u8 to a Tempo, returning an error on invalid values.
fn parse_tempo(val: u8) -> Result<Tempo, CommandError> {
    match val {
        0 => Ok(Tempo::Slow),
        1 => Ok(Tempo::Normal),
        2 => Ok(Tempo::Fast),
        _ => Err(CommandError::InvalidTempo(val)),
    }
}

/// Convert a raw u8 to a Width, returning an error on invalid values.
fn parse_width(val: u8) -> Result<Width, CommandError> {
    match val {
        0 => Ok(Width::Narrow),
        1 => Ok(Width::Normal),
        2 => Ok(Width::Wide),
        _ => Err(CommandError::InvalidWidth(val)),
    }
}

/// Apply a validated command to a match state.
///
/// The command's sequence is checked for idempotency — if the sequence
/// is not greater than `state.last_seq`, a `StaleSequence` error is returned.
/// Otherwise the tactic is updated and `last_seq` is bumped.
///
/// Returns the number of minutes to simulate forward after this command.
/// In event-driven mode this is 0 — the command just changes state and
/// simulation continues at the next tick. For catch-up scenarios the
/// caller computes elapsed minutes separately.
pub fn apply_command(state: &mut MatchState, cmd: &CommandPacket) -> Result<(), CommandError> {
    // Idempotency check
    if cmd.sequence <= state.last_seq {
        return Err(CommandError::StaleSequence(cmd.sequence, state.last_seq));
    }
    state.last_seq = cmd.sequence;

    // Determine which team the command targets.
    // arg1 = team (0 = home, 1 = away)
    let team_idx = (cmd.arg1.min(1)) as usize;

    match cmd.command_type {
        CommandType::Mentality => {
            state.tactic[team_idx].mentality = parse_mentality(cmd.arg2)?;
        }
        CommandType::Press => {
            state.tactic[team_idx].press = parse_press(cmd.arg2)?;
        }
        CommandType::Tempo => {
            state.tactic[team_idx].tempo = parse_tempo(cmd.arg2)?;
        }
        CommandType::Width => {
            state.tactic[team_idx].width = parse_width(cmd.arg2)?;
        }
        CommandType::Substitution => {
            // arg2 = player_out, arg3 = player_in
            if cmd.arg2 >= 11 || cmd.arg3 >= 11 {
                return Err(CommandError::InvalidPlayerIndex(cmd.arg2.max(cmd.arg3)));
            }
            // Substitution is noted but doesn't change the simulation state
            // in the simple algebraic model (all players are abstracted).
            // In a full implementation this would swap player attributes.
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::{CommandPacket, CommandType, TacticState};

    fn make_state() -> MatchState {
        MatchState {
            match_id: 1,
            token: [0u8; 16],
            last_seq: 0,
            score: [0, 0],
            minute: 30,
            possession: 0.5,
            stamina: [1.0, 1.0],
            tactic: [TacticState::default(), TacticState::default()],
            rng_seed: 42,
        }
    }

    fn make_cmd(seq: u16, cmd_type: CommandType, team: u8, arg: u8) -> CommandPacket {
        CommandPacket {
            match_id: 1,
            sequence: seq,
            command_type: cmd_type,
            arg1: team,
            arg2: arg,
            arg3: 0,
        }
    }

    #[test]
    fn test_apply_mentality() {
        let mut state = make_state();
        let cmd = make_cmd(1, CommandType::Mentality, 0, 1); // Home, Attack
        assert!(apply_command(&mut state, &cmd).is_ok());
        assert_eq!(state.tactic[0].mentality, Mentality::Attack);
        assert_eq!(state.last_seq, 1);
    }

    #[test]
    fn test_apply_press() {
        let mut state = make_state();
        let cmd = make_cmd(1, CommandType::Press, 1, 2); // Away, High
        assert!(apply_command(&mut state, &cmd).is_ok());
        assert_eq!(state.tactic[1].press, Press::High);
    }

    #[test]
    fn test_stale_sequence_rejected() {
        let mut state = make_state();
        state.last_seq = 5;
        let cmd = make_cmd(3, CommandType::Mentality, 0, 2);
        assert!(apply_command(&mut state, &cmd).is_err());
    }

    #[test]
    fn test_invalid_mentality_rejected() {
        let mut state = make_state();
        let cmd = make_cmd(1, CommandType::Mentality, 0, 99);
        assert!(apply_command(&mut state, &cmd).is_err());
    }

    #[test]
    fn test_substitution_out_of_range() {
        let mut state = make_state();
        let cmd = CommandPacket {
            match_id: 1,
            sequence: 1,
            command_type: CommandType::Substitution,
            arg1: 0,
            arg2: 99,
            arg3: 5,
        };
        assert!(apply_command(&mut state, &cmd).is_err());
    }
}
