//! Algebraic match simulation.
//!
//! Computes match outcomes (possession, shots, goals, events) using simple
//! probability formulas seeded with a deterministic RNG. No physics.

use protocol::{EventType, MatchState, Team};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::player::PlayerAttributes;

/// Full result of a simulated match.
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub state: MatchState,
    pub events: Vec<SimEvent>,
    pub home_squad: [PlayerAttributes; 11],
    pub away_squad: [PlayerAttributes; 11],
}

/// A single event during match simulation.
#[derive(Debug, Clone)]
pub struct SimEvent {
    pub minute: u8,
    pub event_type: EventType,
    pub team: Team,
    pub player_index: u16,
    /// Contextual value (e.g. shot xG, goal scorer index).
    pub value: f32,
}

/// Simulate a match for a specified number of minutes.
///
/// Supports batch catch-up: pass `minutes` = 90 for a full match,
/// or fewer to simulate forward from the current state. The simulation
/// uses the RNG seeded from state.rng_seed, so results are deterministic.
///
/// In event-driven mode, the caller invokes this with `minutes = 0` on each
/// tick (nothing happens until a command arrives). For catch-up after a
/// user returns from idle, pass the number of elapsed minutes.
pub fn simulate_minutes(
    mut state: MatchState,
    home_squad: [PlayerAttributes; 11],
    away_squad: [PlayerAttributes; 11],
    minutes: u8,
) -> MatchResult {
    // Deterministic RNG from match state seed
    let mut rng = StdRng::seed_from_u64(state.rng_seed);
    let mut events: Vec<SimEvent> = Vec::with_capacity(64);

    // Only emit kickoff if match hasn't started yet
    if state.minute == 0 {
        events.push(SimEvent {
            minute: 0,
            event_type: EventType::Kickoff,
            team: Team::Home,
            player_index: 0,
            value: 0.0,
        });
    }

    let end_minute = (state.minute + minutes).min(90);

    // Simulate from current minute to end minute
    for minute in (state.minute + 1)..=end_minute {
        state.minute = minute;

        // --- Possession ---
        let home_mid = home_squad
            .iter()
            .filter(|p| p.position == crate::player::Position::Midfielder)
            .map(|p| p.overall as f32)
            .sum::<f32>();
        let away_mid = away_squad
            .iter()
            .filter(|p| p.position == crate::player::Position::Midfielder)
            .map(|p| p.overall as f32)
            .sum::<f32>();
        let possession_chance = home_mid / (home_mid + away_mid.max(1.0));
        // Add tactic influence
        let home_mentality_mod = match state.tactic[0].mentality {
            protocol::Mentality::Attack => 0.08,
            protocol::Mentality::Defend => -0.05,
            protocol::Mentality::Normal => 0.0,
        };
        let away_mentality_mod = match state.tactic[1].mentality {
            protocol::Mentality::Attack => 0.08,
            protocol::Mentality::Defend => -0.05,
            protocol::Mentality::Normal => 0.0,
        };
        let effective_possession =
            (possession_chance + home_mentality_mod - away_mentality_mod).clamp(0.2, 0.8);
        state.possession = effective_possession;

        // --- Events every ~3 minutes (reduce noise) ---
        if minute % 3 != 0 {
            continue;
        }

        let home_has_ball = rng.gen::<f32>() < effective_possession;

        let (attack_team, _defend_team, attack_squad, defend_squad) = if home_has_ball {
            (Team::Home, Team::Away, &home_squad, &away_squad)
        } else {
            (Team::Away, Team::Home, &away_squad, &home_squad)
        };

        // --- Shot probability ---
        let attack_strength: f32 = attack_squad
            .iter()
            .map(|p| p.overall as f32 * p.position.possession_weight())
            .sum();
        let defend_strength: f32 = defend_squad
            .iter()
            .map(|p| p.overall as f32 * p.position.possession_weight())
            .sum();
        let shot_prob = 0.08 * (attack_strength / defend_strength.max(1.0)).clamp(0.3, 2.0);

        if rng.gen::<f32>() < shot_prob {
            // Pick a shooter (prefer forwards)
            let shooter_idx = {
                let forwards: Vec<&PlayerAttributes> = attack_squad
                    .iter()
                    .filter(|p| p.position == crate::player::Position::Forward)
                    .collect();
                if !forwards.is_empty() && rng.gen::<f32>() < 0.7 {
                    let pick = rng.gen_range(0..forwards.len());
                    forwards[pick].index
                } else {
                    let pick = rng.gen_range(0..attack_squad.len());
                    attack_squad[pick].index
                }
            };

            let shooter = attack_squad
                .iter()
                .find(|p| p.index == shooter_idx)
                .unwrap();

            // Shot on target?
            let shot_on_target = rng.gen::<f32>() < (shooter.finishing as f32 / 150.0);

            let event_type = if shot_on_target {
                // Save or goal?
                let gk = defend_squad
                    .iter()
                    .find(|p| p.position == crate::player::Position::Goalkeeper)
                    .unwrap();
                let save_chance = gk.overall as f32 / 130.0;
                if rng.gen::<f32>() < save_chance {
                    EventType::Save
                } else {
                    // GOAL!
                    if attack_team == Team::Home {
                        state.score[0] += 1;
                    } else {
                        state.score[1] += 1;
                    }
                    EventType::Goal
                }
            } else {
                EventType::Miss
            };

            events.push(SimEvent {
                minute,
                event_type,
                team: attack_team,
                player_index: shooter_idx,
                value: if event_type == EventType::Goal {
                    1.0
                } else {
                    0.0
                },
            });
        }

        // --- Fouls (rare) ---
        if minute % 10 == 0 && rng.gen::<f32>() < 0.15 {
            let fouler_idx = rng.gen_range(0..11u16);
            events.push(SimEvent {
                minute,
                event_type: EventType::Foul,
                team: if home_has_ball {
                    Team::Away
                } else {
                    Team::Home
                },
                player_index: fouler_idx,
                value: 0.0,
            });
        }
    }

    // Full time — only emit if the match actually reached 90 minutes
    if end_minute >= 90 {
        events.push(SimEvent {
            minute: 90,
            event_type: EventType::FullTime,
            team: Team::Home,
            player_index: 0,
            value: 0.0,
        });
    }

    MatchResult {
        state,
        events,
        home_squad,
        away_squad,
    }
}
