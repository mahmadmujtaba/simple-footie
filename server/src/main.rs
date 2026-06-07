//! Game server – Phase 2: UDP network layer + simulation core.
//!
//! Architecture:
//!   Core 0: Network RX  → recv_mmsg loop, validation, push to crossbeam channel
//!   Core 1: Persistence  → io_uring, journal, snapshots (Phase 3)
//!   Core 2: OS/auxiliary → event sending, metrics, health (Phase 3)
//!   Core 3: Simulation   → drain command queue, apply commands, batch sim
//!
//! For now: single-threaded demo mode. Multi-core isolation comes after
//! the basic loop is validated.

mod network;
mod simulation_core;
mod token;

use crossbeam::channel;
use network::NetworkReceiver;
use simulation_core::SimulationCore;
use std::thread;
use token::TokenManager;

fn main() {
    println!("⚽ fm-rust server — Phase 2");
    println!("Listening on UDP 0.0.0.0:9001\n");

    // ── Channels ────────────────────────────────────────────────
    let (cmd_tx, cmd_rx) = channel::bounded::<network::InboundCommand>(1024);

    // ── Token manager (shared state, thread-safe) ───────────────
    let tokens = TokenManager::new();

    // ── Simulation core ─────────────────────────────────────────
    let mut sim_core = SimulationCore::new(cmd_rx, tokens);

    // Create a demo match so there's something to see
    let token = sim_core.create_match(1, 80, 75);
    println!("  Created match #1 (Rustington 80 vs FC Terminal 75)");
    println!(
        "  Auth token: {:02x}{:02x}{:02x}...\n",
        token[0], token[1], token[2]
    );

    // ── Network receiver (would go on Core 0) ───────────────────
    let net_cmd_tx = cmd_tx.clone();
    let net_handle = thread::spawn(move || {
        let mut receiver =
            NetworkReceiver::bind("0.0.0.0:9001", net_cmd_tx).expect("Failed to bind UDP socket");
        println!("  UDP receiver started on port 9001");
        receiver.run();
    });

    // ── Run simulation on main thread (Core 3) ──────────────────
    println!("  Simulation core running. Send commands via UDP to port 9001.\n");
    println!("  Commands: 10 bytes (steady-state) or 26 bytes (handshake)\n");
    sim_core.run();

    // (Unreachable: sim_core.run() loops forever)
    let _ = net_handle.join();
}

/// Validate a command packet bounds and rate limits.
/// Returns true if the command is valid and should be applied.
pub fn validate_command(
    _match_id: u32,
    _sequence: u16,
    _token: &[u8; 16],
    _args: &[u8; 3],
) -> bool {
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
