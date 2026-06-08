//! Simulation core — runs on Core 3, drains command queue, applies commands,
//! runs batch simulation, generates events.

#![allow(dead_code)]

use crossbeam::channel::Receiver;
use engine::commands::apply_command;
use engine::player::generate_synthetic_squad;
use engine::simulation::simulate_minutes;
use protocol::{EventPacket, MatchState, Team};
use std::collections::HashMap;
use std::thread;
use std::time::{Duration, Instant};

use crate::network::{EventSender, InboundCommand};
use crate::token::TokenManager;
use crate::http_bridge::HttpState;
use std::sync::Arc;

/// A match in the simulation loop: state + squads + client address.
struct ActiveMatch {
    state: MatchState,
    home_squad: engine::player::PlayerAttributesArray,
    away_squad: engine::player::PlayerAttributesArray,
    client_addr: Option<std::net::SocketAddr>,
    last_sim_time: Instant,
    events: Vec<engine::database::DbMatchEvent>,
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
    event_sender: Option<EventSender>,
    http_state: Option<Arc<HttpState>>,
}

impl SimulationCore {
    pub fn new(
        cmd_rx: Receiver<InboundCommand>,
        tokens: TokenManager,
        event_sender: Option<EventSender>,
        http_state: Option<Arc<HttpState>>,
    ) -> Self {
        Self {
            cmd_rx,
            tokens,
            matches: HashMap::new(),
            tick_interval: Duration::from_secs(1),
            events_out: Vec::new(),
            event_sender,
            http_state,
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

        let home_squad = engine::database::load_squad_attributes("simple_footie.db", "Home")
            .unwrap_or_else(|_| generate_synthetic_squad(Team::Home, home_strength));
        let away_squad = engine::database::load_squad_attributes("simple_footie.db", "Away")
            .unwrap_or_else(|_| generate_synthetic_squad(Team::Away, away_strength));

        self.matches.insert(
            match_id,
            ActiveMatch {
                state,
                home_squad,
                away_squad,
                client_addr: None,
                last_sim_time: Instant::now(),
                events: Vec::new(),
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
        if apply_command(&mut match_ref.state, &cmd.packet).is_ok() {
            if cmd.packet.command_type == protocol::CommandType::Substitution {
                let team_idx = cmd.packet.arg1 as usize;
                let player_out_idx = cmd.packet.arg2 as usize;
                if player_out_idx < 11 {
                    let team = if team_idx == 0 {
                        Team::Home
                    } else {
                        Team::Away
                    };
                    let mut sub = engine::player::PlayerAttributes::default();
                    sub.index = player_out_idx as u16;
                    sub.team = team;

                    let old_overall = if team_idx == 0 {
                        match_ref.home_squad[player_out_idx].overall
                    } else {
                        match_ref.away_squad[player_out_idx].overall
                    };

                    sub.overall = old_overall;
                    sub.finishing = old_overall.saturating_add(5).min(100);
                    sub.passing = old_overall.saturating_add(5).min(100);
                    sub.defending = old_overall.saturating_add(5).min(100);
                    sub.stamina = 100; // Fresh substitute gets 100 stamina!

                    sub.position = if team_idx == 0 {
                        match_ref.home_squad[player_out_idx].position
                    } else {
                        match_ref.away_squad[player_out_idx].position
                    };

                    if team_idx == 0 {
                        match_ref.home_squad[player_out_idx] = sub;
                    } else {
                        match_ref.away_squad[player_out_idx] = sub;
                    }
                }
            }
        }
    }

    /// Simulate one minute for all active matches.
    fn simulate_tick(&mut self) {
        let mut finished: Vec<u32> = Vec::new();

        for (&match_id, am) in self.matches.iter_mut() {
            // Check if match is already finished
            let is_finished = engine::simulation::get_next_minute(am.state.minute, am.state.rng_seed, true, am.state.score).is_none();
            if is_finished {
                if !finished.contains(&match_id) {
                    finished.push(match_id);
                }
                continue;
            }

            // Calculate elapsed real time and convert to match minutes
            let elapsed = am.last_sim_time.elapsed();
            let match_minutes = (elapsed.as_secs_f32() * 1.0) as u8; // 1 match min per real sec
            let minutes = match_minutes.max(1);

            // Run batch simulation
            let result = simulate_minutes(am.state.clone(), am.home_squad, am.away_squad, minutes);

            // Update state
            am.state = result.state;
            am.last_sim_time = Instant::now();

            // Convert SimEvents to EventPackets for network
            for ev in &result.events {
                let text = protocol::generate_exciting_commentary(ev.event_type, ev.team, ev.player_index, ev.minute, ev.value);
                am.events.push(engine::database::DbMatchEvent {
                    minute: ev.minute,
                    event_type: ev.event_type as u8,
                    team: match ev.team {
                        Team::Home => "Home".to_string(),
                        Team::Away => "Away".to_string(),
                    },
                    player_index: ev.player_index,
                    value: ev.value,
                    text: text.clone(),
                });

                self.events_out.push(EventPacket {
                    match_id,
                    event_type: ev.event_type,
                    team: ev.team,
                    player_index: ev.player_index,
                    minute: ev.minute,
                    unused: 0,
                    value: ev.value,
                });
            }

            // Update HTTP state if this is match #1
            if match_id == 1 {
                if let Some(ref http_state) = self.http_state {
                    http_state.match_minute.store(am.state.minute as u32, std::sync::atomic::Ordering::Relaxed);
                    *http_state.match_score.lock().unwrap() = am.state.score;
                    
                    // Add new events to http_state.last_events
                    let mut last_events = http_state.last_events.lock().unwrap();
                    for ev in &result.events {
                        let formatted_min = protocol::format_match_minute(ev.minute);
                        let text = protocol::generate_exciting_commentary(ev.event_type, ev.team, ev.player_index, ev.minute, ev.value);
                        last_events.push(format!("{}' - {}", formatted_min, text));
                    }
                }
            }

            // Check if match has just finished
            let is_finished_now = engine::simulation::get_next_minute(am.state.minute, am.state.rng_seed, true, am.state.score).is_none();
            if is_finished_now {
                if !finished.contains(&match_id) {
                    finished.push(match_id);
                }
            }
        }

        // Send events to clients
        if let Some(ref sender) = self.event_sender {
            // Group events by match_id
            let mut events_by_match: std::collections::HashMap<u32, Vec<EventPacket>> =
                std::collections::HashMap::new();
            for ev in &self.events_out {
                events_by_match.entry(ev.match_id).or_default().push(*ev);
            }
            for (match_id, events) in &events_by_match {
                if let Some(am) = self.matches.get(match_id) {
                    if let Some(ref client_addr) = am.client_addr {
                        let _ = sender.send_batch(events, *client_addr);
                    }
                }
            }
        }
        self.events_out.clear();

        // Clean up finished matches
        for id in finished {
            if let Some(am) = self.matches.get(&id) {
                let date_str = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                if let Err(e) = engine::database::save_match(
                    "simple_footie.db",
                    &date_str,
                    "Rustington United",
                    "FC Terminal",
                    am.state.score[0],
                    am.state.score[1],
                    &am.events,
                ) {
                    eprintln!("  ⚠  Failed to save match to database: {e}");
                } else {
                    println!("  💾 Match #{} saved to database successfully!", id);
                }
            }
            self.matches.remove(&id);
            self.tokens.remove(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_simulate_match() {
        let (_tx, rx) = crossbeam::channel::bounded(64);
        let tokens = TokenManager::new();
        let mut core = SimulationCore::new(rx, tokens, None, None);

        let token = core.create_match(1, 80, 75);
        assert!(core.matches.contains_key(&1));
        assert!(token != [0u8; 16]);
    }

    #[test]
    fn test_token_caching() {
        let (_tx, rx) = crossbeam::channel::bounded(64);
        let tokens = TokenManager::new();
        let mut core = SimulationCore::new(rx, tokens, None, None);

        let token = core.create_match(1, 78, 78);
        let state = &core.matches.get(&1).unwrap().state;
        assert_eq!(&state.token, &token);
    }
}
