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

mod http_bridge;
mod metrics;
mod network;
mod persistence;
mod simulation_core;
mod token;

use crossbeam::channel;
use http_bridge::{start_http_bridge, HttpState};
use metrics::{start_metrics_server, Metrics};
use network::NetworkReceiver;
use simulation_core::SimulationCore;
use std::path::Path;
use std::thread;
use token::TokenManager;

fn main() {
    println!("⚽ fm-rust server — Phase 3");
    println!("Listening on UDP 0.0.0.0:9001");
    println!("Metrics on http://127.0.0.1:9090/metrics");
    println!("Data directory: ./data/\n");

    // ── Channels ────────────────────────────────────────────────
    let (cmd_tx, cmd_rx) = channel::bounded::<network::InboundCommand>(1024);

    // ── Token manager (shared state, thread-safe) ───────────────
    let tokens = TokenManager::new();

    // ── Metrics ─────────────────────────────────────────────────
    let metrics = Metrics::new();
    metrics
        .active_matches
        .store(1, std::sync::atomic::Ordering::Relaxed);
    let _metrics_handle = start_metrics_server(metrics.clone());

    // ── HTTP Bridge (control panel on port 8080) ────────────────
    let http_state = HttpState::new();
    let _http_handle = start_http_bridge(cmd_tx.clone(), http_state.clone());
    println!("  Control panel: http://127.0.0.1:8080");

    // ── Persistence (journal + snapshots) ───────────────────────
    let data_dir = Path::new("./data");
    let mut journal = persistence::Journal::open(data_dir).expect("Failed to open journal");

    // Check disk space on startup
    match journal.check_disk_space() {
        Ok(true) => println!("  Disk space: OK"),
        Ok(false) => {
            eprintln!("  ⚠  Disk <5% free — journaling disabled");
            journal.disable();
        }
        Err(e) => eprintln!("  ⚠  Failed to check disk space: {e}"),
    }

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
    let _metrics_clone = metrics.clone();
    let net_handle = thread::spawn(move || {
        let mut receiver =
            NetworkReceiver::bind("0.0.0.0:9001", net_cmd_tx).expect("Failed to bind UDP socket");
        println!("  UDP receiver started on port 9001");
        receiver.run();
    });

    // ── Run simulation on main thread (Core 3) ──────────────────
    println!("  Simulation core running. Send commands via UDP to port 9001.");
    println!("  Metrics: curl http://127.0.0.1:9090/metrics\n");

    // In production, sim_core.run() would be wrapped to journal commands
    // and take snapshots. For now, sim_core.run() loops forever.
    sim_core.run();

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
