//! Game server – receives UDP commands, runs simulation, sends events.
//!
//! Core 0: Network RX (recv_mmsg, validation, push to queue)
//! Core 1: Persistence (io_uring, journal, snapshots) — Phase 2
//! Core 2: OS & auxiliary (event sending, metrics, health) — Phase 2
//! Core 3: Simulation (drain queue, apply commands, algebraic sim)

fn main() {
    println!("fm-rust server — Phase 1 scaffolding");
    println!("Server logic starts in Phase 2 (UDP loop + simulation core)");
}

/// Validate a command packet bounds and rate limits.
/// Returns true if the command is valid and should be applied.
pub fn validate_command(
    _match_id: u32,
    _sequence: u16,
    _token: &[u8; 16],
    _args: &[u8; 3],
) -> bool {
    // TODO: Phase 2 — actual validation
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_stub() {
        assert!(validate_command(1, 0, &[0u8; 16], &[0u8; 3]));
    }
}
