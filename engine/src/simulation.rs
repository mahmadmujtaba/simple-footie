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

/// Helper to determine the next minute of the match deterministically.
pub fn get_next_minute(current_minute: u8, rng_seed: u64, is_cup: bool, score: [u8; 2]) -> Option<u8> {
    let mut rng = StdRng::seed_from_u64(rng_seed);
    
    // Determine stoppage times deterministically
    let t1 = 1 + (rng.gen::<u32>() % 4) as u8; // 1 to 4 minutes
    
    let mut rng2 = StdRng::seed_from_u64(rng_seed + 90);
    let t2 = 1 + (rng2.gen::<u32>() % 5) as u8; // 1 to 5 minutes
    
    let mut rng3 = StdRng::seed_from_u64(rng_seed + 105);
    let t3 = (rng3.gen::<u32>() % 2) as u8; // 0 to 1 minute
    
    let mut rng4 = StdRng::seed_from_u64(rng_seed + 120);
    let t4 = 1 + (rng4.gen::<u32>() % 3) as u8; // 1 to 3 minutes

    if current_minute < 45 {
        Some(current_minute + 1)
    } else if current_minute < 45 + t1 {
        Some(current_minute + 1)
    } else if current_minute == 45 + t1 {
        Some(51)
    } else if current_minute < 95 {
        Some(current_minute + 1)
    } else if current_minute < 95 + t2 {
        Some(current_minute + 1)
    } else if current_minute == 95 + t2 {
        if is_cup && score[0] == score[1] {
            Some(101)
        } else {
            None
        }
    } else if current_minute < 115 {
        Some(current_minute + 1)
    } else if current_minute < 115 + t3 {
        Some(current_minute + 1)
    } else if current_minute == 115 + t3 {
        Some(118)
    } else if current_minute < 132 {
        Some(current_minute + 1)
    } else if current_minute < 132 + t4 {
        Some(current_minute + 1)
    } else {
        None
    }
}

/// Helper to get the sequence of minutes to simulate forward.
pub fn get_minutes_sequence(start_minute: u8, count: u8, rng_seed: u64, is_cup: bool, score: [u8; 2]) -> Vec<u8> {
    let mut seq = Vec::new();
    let mut curr = start_minute;
    for _ in 0..count {
        if let Some(next) = get_next_minute(curr, rng_seed, is_cup, score) {
            seq.push(next);
            curr = next;
        } else {
            break;
        }
    }
    seq
}

/// Check if a penalty shootout is decided.
fn is_shootout_decided(home_score: i32, away_score: i32, home_kicks: i32, away_kicks: i32, sudden_death: bool) -> bool {
    if sudden_death {
        if home_kicks == away_kicks {
            home_score != away_score
        } else {
            false
        }
    } else {
        let home_remaining = 5 - home_kicks;
        let away_remaining = 5 - away_kicks;
        if home_score > away_score + away_remaining {
            true
        } else if away_score > home_score + home_remaining {
            true
        } else {
            false
        }
    }
}

/// Simulate a penalty shootout and return the events.
pub fn simulate_penalty_shootout(
    rng: &mut StdRng,
    home_squad: &[PlayerAttributes; 11],
    away_squad: &[PlayerAttributes; 11],
    minute: u8,
) -> Vec<SimEvent> {
    let mut events = Vec::new();
    
    events.push(SimEvent {
        minute,
        event_type: EventType::PenaltyShootoutStart,
        team: Team::Home,
        player_index: 0,
        value: 0.0,
    });

    let mut home_score = 0;
    let mut away_score = 0;
    let mut home_kicks = 0;
    let mut away_kicks = 0;

    let mut round = 1;
    let mut sudden_death = false;

    loop {
        // 1. Home kick
        if !sudden_death || home_kicks == away_kicks {
            home_kicks += 1;
            let shooter_idx = rng.gen_range(1..11) as u16;
            let shooter = &home_squad[shooter_idx as usize];
            let gk = &away_squad[0];

            let mut score_prob = 0.75 + (shooter.finishing as f32 - gk.overall as f32) * 0.002;
            score_prob = score_prob.clamp(0.5, 0.95);

            if rng.gen::<f32>() < score_prob {
                home_score += 1;
                events.push(SimEvent {
                    minute,
                    event_type: EventType::PenaltyGoal,
                    team: Team::Home,
                    player_index: shooter_idx,
                    value: home_score as f32,
                });
            } else {
                if rng.gen::<f32>() < 0.6 {
                    events.push(SimEvent {
                        minute,
                        event_type: EventType::PenaltySave,
                        team: Team::Away,
                        player_index: 0,
                        value: 0.0,
                    });
                } else {
                    events.push(SimEvent {
                        minute,
                        event_type: EventType::PenaltyMiss,
                        team: Team::Home,
                        player_index: shooter_idx,
                        value: 0.0,
                    });
                }
            }

            if is_shootout_decided(home_score, away_score, home_kicks, away_kicks, sudden_death) {
                break;
            }
        }

        // 2. Away kick
        if !sudden_death || away_kicks < home_kicks {
            away_kicks += 1;
            let shooter_idx = rng.gen_range(1..11) as u16;
            let shooter = &away_squad[shooter_idx as usize];
            let gk = &home_squad[0];

            let mut score_prob = 0.75 + (shooter.finishing as f32 - gk.overall as f32) * 0.002;
            score_prob = score_prob.clamp(0.5, 0.95);

            if rng.gen::<f32>() < score_prob {
                away_score += 1;
                events.push(SimEvent {
                    minute,
                    event_type: EventType::PenaltyGoal,
                    team: Team::Away,
                    player_index: shooter_idx,
                    value: away_score as f32,
                });
            } else {
                if rng.gen::<f32>() < 0.6 {
                    events.push(SimEvent {
                        minute,
                        event_type: EventType::PenaltySave,
                        team: Team::Home,
                        player_index: 0,
                        value: 0.0,
                    });
                } else {
                    events.push(SimEvent {
                        minute,
                        event_type: EventType::PenaltyMiss,
                        team: Team::Away,
                        player_index: shooter_idx,
                        value: 0.0,
                    });
                }
            }

            if is_shootout_decided(home_score, away_score, home_kicks, away_kicks, sudden_death) {
                break;
            }
        }

        if home_kicks == 5 && away_kicks == 5 {
            sudden_death = true;
        }
        
        round += 1;
        if round > 30 {
            break;
        }
    }

    events
}

/// Safely calculate a player's effective overall performance based on stamina.
fn get_effective_overall(p: &PlayerAttributes, team_stamina: f32) -> f32 {
    let individual_factor = p.stamina as f32 / 100.0;
    // Combine team-level stamina ratio (60%) and individual player stamina (40%)
    let stamina_factor = 0.6 * team_stamina + 0.4 * individual_factor;
    // Fatigue can drop effective performance by up to 25%
    let factor = 0.75 + 0.25 * stamina_factor;
    p.overall as f32 * factor
}

/// Safely calculate a player's effective finishing rating based on stamina.
fn get_effective_finishing(p: &PlayerAttributes, team_stamina: f32) -> f32 {
    let individual_factor = p.stamina as f32 / 100.0;
    let stamina_factor = 0.6 * team_stamina + 0.4 * individual_factor;
    let factor = 0.75 + 0.25 * stamina_factor;
    p.finishing as f32 * factor
}

/// Simulate a match for a specified number of minutes.
///
/// Supports batch catch-up: pass `minutes` = 90 for a full match,
/// or fewer to simulate forward from the current state. The simulation
/// uses the RNG seeded from state.rng_seed, so results are deterministic.
pub fn simulate_minutes(
    mut state: MatchState,
    mut home_squad: [PlayerAttributes; 11],
    mut away_squad: [PlayerAttributes; 11],
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

    // Determine stoppage times deterministically
    let mut rng_stoppage = StdRng::seed_from_u64(state.rng_seed);
    let t1 = 1 + (rng_stoppage.gen::<u32>() % 4) as u8; // 1 to 4 minutes
    
    let mut rng_stoppage2 = StdRng::seed_from_u64(state.rng_seed + 90);
    let t2 = 1 + (rng_stoppage2.gen::<u32>() % 5) as u8; // 1 to 5 minutes
    
    let mut rng_stoppage3 = StdRng::seed_from_u64(state.rng_seed + 105);
    let t3 = (rng_stoppage3.gen::<u32>() % 2) as u8; // 0 to 1 minute
    
    let mut rng_stoppage4 = StdRng::seed_from_u64(state.rng_seed + 120);
    let t4 = 1 + (rng_stoppage4.gen::<u32>() % 3) as u8; // 1 to 3 minutes

    // Generate the sequence of minutes to simulate
    // We assume is_cup = true to support extra time if needed
    let minutes_seq = get_minutes_sequence(state.minute, minutes, state.rng_seed, true, state.score);

    for &minute in &minutes_seq {
        state.minute = minute;

        // ── 1. Stamina Decay per Minute ──────────────────────────────
        for team_idx in 0..2 {
            let tempo = state.tactic[team_idx].tempo;
            let press = state.tactic[team_idx].press;

            let tempo_decay_factor = match tempo {
                protocol::Tempo::Slow => 0.8,
                protocol::Tempo::Normal => 1.0,
                protocol::Tempo::Fast => 1.3,
            };

            let press_decay_factor = match press {
                protocol::Press::Low => 0.8,
                protocol::Press::Medium => 1.0,
                protocol::Press::High => 1.45,
            };

            // Base decay rate of team stamina per minute is 0.003
            let decay = 0.003 * tempo_decay_factor * press_decay_factor;
            state.stamina[team_idx] = (state.stamina[team_idx] - decay).max(0.15);
        }

        // Decay individual player stamina ratings
        for p in home_squad.iter_mut() {
            let decay_rate = match state.tactic[0].press {
                protocol::Press::Low => 1,
                protocol::Press::Medium => 2,
                protocol::Press::High => 3,
            };
            p.stamina = p.stamina.saturating_sub(decay_rate);
        }
        for p in away_squad.iter_mut() {
            let decay_rate = match state.tactic[1].press {
                protocol::Press::Low => 1,
                protocol::Press::Medium => 2,
                protocol::Press::High => 3,
            };
            p.stamina = p.stamina.saturating_sub(decay_rate);
        }

        // ── 2. Possession Calculation ───────────────────────────────
        let home_mid = home_squad
            .iter()
            .filter(|p| p.position == crate::player::Position::Midfielder)
            .map(|p| get_effective_overall(p, state.stamina[0]))
            .sum::<f32>();
        let away_mid = away_squad
            .iter()
            .filter(|p| p.position == crate::player::Position::Midfielder)
            .map(|p| get_effective_overall(p, state.stamina[1]))
            .sum::<f32>();

        let possession_chance = home_mid / (home_mid + away_mid.max(1.0));

        // Add tactic influences (mentality and width)
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

        let home_width_mod = match state.tactic[0].width {
            protocol::Width::Wide => 0.03,
            protocol::Width::Narrow => -0.02,
            protocol::Width::Normal => 0.0,
        };
        let away_width_mod = match state.tactic[1].width {
            protocol::Width::Wide => 0.03,
            protocol::Width::Narrow => -0.02,
            protocol::Width::Normal => 0.0,
        };

        // Low press concede possession slightly, high press wins it
        let home_press_mod = match state.tactic[0].press {
            protocol::Press::Low => -0.03,
            protocol::Press::Medium => 0.0,
            protocol::Press::High => 0.04,
        };
        let away_press_mod = match state.tactic[1].press {
            protocol::Press::Low => -0.03,
            protocol::Press::Medium => 0.0,
            protocol::Press::High => 0.04,
        };

        let effective_possession = (possession_chance + home_mentality_mod - away_mentality_mod
            + home_width_mod
            - away_width_mod
            + home_press_mod
            - away_press_mod)
            .clamp(0.15, 0.85);

        state.possession = effective_possession;

        // ── 3. Simulation Events Checks (scaled by Tempo) ────────────
        let home_tempo_factor = match state.tactic[0].tempo {
            protocol::Tempo::Slow => 0.8,
            protocol::Tempo::Normal => 1.0,
            protocol::Tempo::Fast => 1.25,
        };
        let away_tempo_factor = match state.tactic[1].tempo {
            protocol::Tempo::Slow => 0.8,
            protocol::Tempo::Normal => 1.0,
            protocol::Tempo::Fast => 1.25,
        };
        let match_tempo = 0.5 * (home_tempo_factor + away_tempo_factor);

        // Every minute check for potential incidents, typically once per ~3 minutes on average
        let check_chance = 0.33 * match_tempo;
        if rng.gen::<f32>() >= check_chance {
            // Period end checks even if no event happened
            check_period_ends(minute, t1, t2, t3, t4, &mut events, &mut state, &mut rng, &home_squad, &away_squad);
            continue;
        }

        // Determine who has the initiative for this action sequence
        let home_has_ball = rng.gen::<f32>() < effective_possession;
        let (
            attack_team,
            defend_team,
            attack_squad,
            defend_squad,
            attack_stamina,
            defend_stamina,
            attack_tactic,
            defend_tactic,
        ) = if home_has_ball {
            (
                Team::Home,
                Team::Away,
                &home_squad,
                &away_squad,
                state.stamina[0],
                state.stamina[1],
                state.tactic[0],
                state.tactic[1],
            )
        } else {
            (
                Team::Away,
                Team::Home,
                &away_squad,
                &home_squad,
                state.stamina[1],
                state.stamina[0],
                state.tactic[1],
                state.tactic[0],
            )
        };

        // ── 3.1 Offside Check (First barrier of attack) ────────────────
        let offside_prob = 0.08;
        if rng.gen::<f32>() < offside_prob {
            events.push(SimEvent {
                minute,
                event_type: EventType::Offside,
                team: attack_team,
                player_index: rng.gen_range(9..=10), // Forwards get caught offside
                value: 0.0,
            });
            check_period_ends(minute, t1, t2, t3, t4, &mut events, &mut state, &mut rng, &home_squad, &away_squad);
            continue;
        }

        // ── 3.2 Attacking vs Defending Strengths ───────────────────────
        let mut attack_strength: f32 = attack_squad
            .iter()
            .map(|p| get_effective_overall(p, attack_stamina) * p.position.possession_weight())
            .sum();
        let mut defend_strength: f32 = defend_squad
            .iter()
            .map(|p| get_effective_overall(p, defend_stamina) * p.position.possession_weight())
            .sum();

        // High Press increases defense strength and forces mistakes
        if defend_tactic.press == protocol::Press::High {
            defend_strength *= 1.15;
        }
        // Narrow Width narrows spaces making central defending easier
        if defend_tactic.width == protocol::Width::Narrow {
            defend_strength *= 1.05;
        }
        // Wide Width helps stretching the play making attacks more potent
        if attack_tactic.width == protocol::Width::Wide {
            attack_strength *= 1.08;
        }

        // Shot probability
        let shot_prob = 0.12 * (attack_strength / defend_strength.max(1.0)).clamp(0.2, 2.0);

        if rng.gen::<f32>() < shot_prob {
            // Pick a shooter (prefer forwards)
            let shooter_idx = {
                let forwards: Vec<&PlayerAttributes> = attack_squad
                    .iter()
                    .filter(|p| p.position == crate::player::Position::Forward)
                    .collect();
                if !forwards.is_empty() && rng.gen::<f32>() < 0.75 {
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

            // Emit Shot event
            events.push(SimEvent {
                minute,
                event_type: EventType::Shot,
                team: attack_team,
                player_index: shooter_idx,
                value: minute as f32,
            });

            // Shot accuracy (influenced by finishing and opponent pressing intensity)
            let mut accuracy_prob = get_effective_finishing(shooter, attack_stamina) / 140.0;
            if defend_tactic.press == protocol::Press::High {
                accuracy_prob -= 0.08; // High pressing reduces shooting composure
            }
            let shot_on_target = rng.gen::<f32>() < accuracy_prob.clamp(0.1, 0.9);

            if shot_on_target {
                events.push(SimEvent {
                    minute,
                    event_type: EventType::ShotOnTarget,
                    team: attack_team,
                    player_index: shooter_idx,
                    value: minute as f32,
                });

                // Goalkeeper Save vs Goal
                let gk = defend_squad
                    .iter()
                    .find(|p| p.position == crate::player::Position::Goalkeeper)
                    .unwrap();
                let save_chance =
                    (get_effective_overall(gk, defend_stamina) / 120.0).clamp(0.2, 0.85);

                if rng.gen::<f32>() < save_chance {
                    events.push(SimEvent {
                        minute,
                        event_type: EventType::Save,
                        team: defend_team,
                        player_index: gk.index,
                        value: minute as f32,
                    });

                    // Goalkeeper saves often go out for a Corner (35% chance)
                    if rng.gen::<f32>() < 0.35 {
                        events.push(SimEvent {
                            minute,
                            event_type: EventType::Corner,
                            team: attack_team,
                            player_index: rng.gen_range(5..=8), // Midfielder takes the corner
                            value: minute as f32,
                        });
                    }
                } else {
                    // GOAL!
                    if attack_team == Team::Home {
                        state.score[0] = state.score[0].saturating_add(1);
                    } else {
                        state.score[1] = state.score[1].saturating_add(1);
                    }
                    events.push(SimEvent {
                        minute,
                        event_type: EventType::Goal,
                        team: attack_team,
                        player_index: shooter_idx,
                        value: 1.0,
                    });
                }
            } else {
                events.push(SimEvent {
                    minute,
                    event_type: EventType::Miss,
                    team: attack_team,
                    player_index: shooter_idx,
                    value: minute as f32,
                });
            }
            check_period_ends(minute, t1, t2, t3, t4, &mut events, &mut state, &mut rng, &home_squad, &away_squad);
            continue;
        }

        // ── 3.3 Foul Checks and Discipline ───────────────────────────
        let base_foul_chance = 0.08;
        let press_foul_factor = match defend_tactic.press {
            protocol::Press::Low => 0.7,
            protocol::Press::Medium => 1.0,
            protocol::Press::High => 1.6, // High pressing causes more fouls
        };

        if rng.gen::<f32>() < (base_foul_chance * press_foul_factor) {
            let fouler_idx = rng.gen_range(1..9u16); // Midfielders and Defenders foul
            events.push(SimEvent {
                minute,
                event_type: EventType::Foul,
                team: defend_team,
                player_index: fouler_idx,
                value: minute as f32,
            });

            // Yellow / Red Card checks
            let card_roll = rng.gen::<f32>();
            if card_roll < 0.22 {
                // direct Red Card check (extremely rare, 1%)
                if card_roll < 0.015 {
                    events.push(SimEvent {
                        minute,
                        event_type: EventType::RedCard,
                        team: defend_team,
                        player_index: fouler_idx,
                        value: minute as f32,
                    });
                } else {
                    // Yellow card
                    events.push(SimEvent {
                        minute,
                        event_type: EventType::YellowCard,
                        team: defend_team,
                        player_index: fouler_idx,
                        value: minute as f32,
                    });
                }
            } else if rng.gen::<f32>() < 0.25 {
                // Award a Free Kick event after a foul
                events.push(SimEvent {
                    minute,
                    event_type: EventType::FreeKick,
                    team: attack_team,
                    player_index: rng.gen_range(5..=10),
                    value: minute as f32,
                });
            }
            check_period_ends(minute, t1, t2, t3, t4, &mut events, &mut state, &mut rng, &home_squad, &away_squad);
            continue;
        }

        // ── 3.4 Random Injury Checks ─────────────────────────────────
        if minute % 15 == 0 && rng.gen::<f32>() < 0.012 {
            let injured_idx = rng.gen_range(0..11u16);
            events.push(SimEvent {
                minute,
                event_type: EventType::Injury,
                team: attack_team,
                player_index: injured_idx,
                value: minute as f32,
            });

            // Trigger an automatic substitution to simulate the replacement of the injured player
            events.push(SimEvent {
                minute,
                event_type: EventType::Substitution,
                team: attack_team,
                player_index: injured_idx,
                value: minute as f32,
            });
        }

        check_period_ends(minute, t1, t2, t3, t4, &mut events, &mut state, &mut rng, &home_squad, &away_squad);
    }

    MatchResult {
        state,
        events,
        home_squad,
        away_squad,
    }
}

/// Helper to check and emit period end events.
fn check_period_ends(
    minute: u8,
    t1: u8,
    t2: u8,
    t3: u8,
    t4: u8,
    events: &mut Vec<SimEvent>,
    state: &mut MatchState,
    rng: &mut StdRng,
    home_squad: &[PlayerAttributes; 11],
    away_squad: &[PlayerAttributes; 11],
) {
    if minute == 45 + t1 {
        events.push(SimEvent {
            minute,
            event_type: EventType::HalfTime,
            team: Team::Home,
            player_index: 0,
            value: 0.0,
        });
    } else if minute == 95 + t2 {
        if state.score[0] == state.score[1] {
            events.push(SimEvent {
                minute,
                event_type: EventType::ExtraTimeStart,
                team: Team::Home,
                player_index: 0,
                value: 0.0,
            });
        } else {
            events.push(SimEvent {
                minute,
                event_type: EventType::FullTime,
                team: Team::Home,
                player_index: 0,
                value: 0.0,
            });
        }
    } else if minute == 115 + t3 {
        events.push(SimEvent {
            minute,
            event_type: EventType::ExtraTimeHalfTime,
            team: Team::Home,
            player_index: 0,
            value: 0.0,
        });
    } else if minute == 132 + t4 {
        if state.score[0] == state.score[1] {
            // Simulate penalty shootout!
            let mut shootout_events = simulate_penalty_shootout(rng, home_squad, away_squad, minute);
            events.append(&mut shootout_events);
        }
        events.push(SimEvent {
            minute,
            event_type: EventType::FullTime,
            team: Team::Home,
            player_index: 0,
            value: 0.0,
        });
    }
}

    MatchResult {
        state,
        events,
        home_squad,
        away_squad,
    }
}
