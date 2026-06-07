//! Simulation core — runs on Core 3, drains command queue, applies commands,
//! runs batch simulation, generates events.

#![allow(dead_code)]

use crossbeam::channel::Receiver;
use engine::commands::apply_command;
use engine::player::generate_synthetic_squad;
use engine::simulation::simulate_minutes;
use protocol::{EventPacket, EventType, MatchState, Team};
use std::collections::HashMap;
use std::thread;
use std::time::{Duration, Instant};

use crate::network::InboundCommand;
use crate::token::TokenManager;

/// A match in the simulation loop: state + squads + client address.
struct ActiveMatch {
    state: MatchState,
    home_squad: engine::player::PlayerAttributesArray,
    away_squad: engine::player::PlayerAttributesArray,
    client_addr: Option<std::net::SocketAddr>,
    last_sim_time: Instant,
}

/// Convenience alias for an 11-player squad.
type PlayerAttributesArray = [engine::player::PlayerAttributes; 11];

/// The simulation core loop.
///
/// Runs on Core 3:
/// 1. Drain the command channel from Core 0
/// 2. Validate tokens and rate limits
/// 3. Apply commands to match states
/// 4. Periodic batch simulation (every tick)
/// 5. Generate events
pub struct SimulationCore {
    cmd_rx: Receiver<InboundCommand>,
    tokens: TokenManager,
    matches: HashMap<u32, ActiveMatch>,
    /// Seconds between simulation ticks (idle matches simulate 1 minute per tick)
    tick_interval: Duration,
    events_out: Vec<EventPacket>,
}

impl SimulationCore {
    pub fn new(cmd_rx: Receiver<InboundCommand>, tokens: TokenManager) -> Self {
        Self {
            cmd_rx,
            tokens,
            matches: HashMap::new(),
            tick_interval: Duration::from_secs(1),
            events_out: Vec::new(),
        }
    }

    /// Create a new match and start simulating.
    pub fn create_match(
        &mut self,
        match_id: u32,
        home_strength: u8,
        away_strength: u8,
    ) -> [u8; 16] {
        let token = self.tokens.create_token(match_id);

        let state = MatchState {
            match_id,
            token,
            last_seq: 0,
            score: [0, 0],
            minute: 0,
            possession: 0.5,
            stamina: [1.0, 1.0],
            tactic: [
                protocol::TacticState::default(),
                protocol::TacticState::default(),
            ],
            rng_seed: match_id as u64,
        };

        let home_squad = generate_synthetic_squad(Team::Home, home_strength);
        let away_squad = generate_synthetic_squad(Team::Away, away_strength);

        self.matches.insert(
            match_id,
            ActiveMatch {
                state,
                home_squad,
                away_squad,
                client_addr: None,
                last_sim_time: Instant::now(),
            },
        );

        token
    }

    /// Run the main simulation loop (blocks forever).
    pub fn run(&mut self) {
        let mut last_tick = Instant::now();
        let mut rate_limit_tick = Instant::now();

        loop {
            // 1. Drain all pending commands
            self.drain_commands();

            // 2. Tick rate limiter every second
            if rate_limit_tick.elapsed() >= Duration::from_secs(1) {
                self.tokens.tick();
                rate_limit_tick = Instant::now();
            }

            // 3. Batch simulation: every tick, simulate active matches
            if last_tick.elapsed() >= self.tick_interval {
                self.simulate_tick();
                last_tick = Instant::now();
            }

            // Brief sleep to avoid busy-waiting
            thread::sleep(Duration::from_micros(500));
        }
    }

    /// Drain all available commands from the channel.
    fn drain_commands(&mut self) {
        while let Ok(cmd) = self.cmd_rx.try_recv() {
            self.process_command(cmd);
        }
    }

    /// Process a single inbound command.
    fn process_command(&mut self, cmd: InboundCommand) {
        let match_id = cmd.packet.match_id;

        // Validate token
        let is_handshake = cmd.token != [0u8; 16];
        let token_valid = if is_handshake {
            self.tokens.validate_token(match_id, &cmd.token)
        } else {
            // Steady state: check against cached token
            if let Some(am) = self.matches.get(&match_id) {
                self.tokens.validate_token(match_id, &am.state.token)
            } else {
                false
            }
        };

        if !token_valid {
            return; // Invalid token, drop
        }

        // Rate limit
        if !self.tokens.check_rate_limit(match_id) {
            return; // Rate limited, drop
        }

        // Look up or register match
        let match_ref = match self.matches.get_mut(&match_id) {
            Some(m) => {
                // Update client address for event routing
                if is_handshake {
                    m.client_addr = Some(cmd.src_addr);
                }
                m
            }
            None => return, // Unknown match
        };

        // Apply the command
        let _ = apply_command(&mut match_ref.state, &cmd.packet);
        // Errors (stale sequence, invalid args) are silently dropped per spec
    }

    /// Simulate one minute for all active matches.
    fn simulate_tick(&mut self) {
        let mut finished: Vec<u32> = Vec::new();

        for (&match_id, am) in self.matches.iter_mut() {
            if am.state.minute >= 90 {
                // Match already finished, optionally clean up
                if !finished.contains(&match_id) {
                    finished.push(match_id);
                }
                continue;
            }

            // Calculate elapsed real time and convert to match minutes
            let elapsed = am.last_sim_time.elapsed();
            let match_minutes = (elapsed.as_secs_f32() * 2.0) as u8; // 2 match min per real sec
            let minutes = match_minutes.max(1).min(90 - am.state.minute);

            // Run batch simulation
            let result = simulate_minutes(am.state.clone(), am.home_squad, am.away_squad, minutes);

            // Update state
            am.state = result.state;
            am.last_sim_time = Instant::now();

            // Convert SimEvents to EventPackets for network
            for ev in &result.events {
                self.events_out.push(EventPacket {
                    match_id,
                    event_type: ev.event_type,
                    team: ev.team,
                    player_index: ev.player_index,
                    value: ev.value,
                });
            }

            if am.state.minute >= 90 {
                finished.push(match_id);
                // Emit full time event if not already emitted
                if !self
                    .events_out
                    .iter()
                    .any(|e| e.match_id == match_id && e.event_type == EventType::FullTime)
                {
                    self.events_out.push(EventPacket {
                        match_id,
                        event_type: EventType::FullTime,
                        team: Team::Home,
                        player_index: 0,
                        value: 0.0,
                    });
                }
            }
        }

        // Clean up finished matches
        for id in finished {
            self.matches.remove(&id);
            self.tokens.remove(id);
        }

        // Send events to clients (in production, this goes via EventSender)
        // For now, we just drain the buffer
        self.events_out.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_simulate_match() {
        let (_tx, rx) = crossbeam::channel::bounded(64);
        let tokens = TokenManager::new();
        let mut core = SimulationCore::new(rx, tokens);

        let token = core.create_match(1, 80, 75);
        assert!(core.matches.contains_key(&1));
        assert!(token != [0u8; 16]);
    }

    #[test]
    fn test_token_caching() {
        let (_tx, rx) = crossbeam::channel::bounded(64);
        let tokens = TokenManager::new();
        let mut core = SimulationCore::new(rx, tokens);

        let token = core.create_match(1, 78, 78);
        let state = &core.matches.get(&1).unwrap().state;
        assert_eq!(&state.token, &token);
    }
}
