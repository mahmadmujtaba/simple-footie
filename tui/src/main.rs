use color_eyre::eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use engine::player::generate_synthetic_squad;
use engine::simulation::simulate_minutes;
use protocol::{CommandType, MatchState, TacticState, Team};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, List, ListItem, Paragraph, Row, Table, Tabs},
    Frame,
};
mod server_client;

use server_client::ServerClient;
use std::time::Duration;

fn main() -> Result<()> {
    color_eyre::install()?;
    let mut terminal = ratatui::init();
    let mut app = App::new();
    let res = app.run(&mut terminal);
    ratatui::restore();
    res
}

fn get_big_digit(digit: u8) -> [&'static str; 3] {
    match digit {
        0 => [
            "█▀▀█",
            "█  █",
            "▀▀▀▀"
        ],
        1 => [
            " ▄█ ",
            "  █ ",
            " ▄█▄"
        ],
        2 => [
            "█▀▀█",
            "  ▄▀",
            "█▄▄█"
        ],
        3 => [
            "█▀▀█",
            "  ▀▄",
            "█▄▄█"
        ],
        4 => [
            "█  █",
            "█▀▀█",
            "   █"
        ],
        5 => [
            "█▀▀▀",
            "▀▀▀█",
            "▀▀▀▀"
        ],
        6 => [
            "█▀▀▀",
            "█▀▀█",
            "▀▀▀▀"
        ],
        7 => [
            "█▀▀█",
            "  █ ",
            " ▐█ "
        ],
        8 => [
            "█▀▀█",
            "█▀▀█",
            "▀▀▀▀"
        ],
        9 => [
            "█▀▀█",
            " ▀▀█",
            "▀▀▀▀"
        ],
        _ => [
            "    ",
            "    ",
            "    "
        ]
    }
}

fn get_big_score(home: u8, away: u8) -> [String; 3] {
    let h_digit = get_big_digit(home);
    let a_digit = get_big_digit(away);
    [
        format!("{}  ▄▄  {}", h_digit[0], a_digit[0]),
        format!("{}  ▀▀  {}", h_digit[1], a_digit[1]),
        format!("{}      {}", h_digit[2], a_digit[2]),
    ]
}

#[derive(Clone)]
struct Player {
    name: String,
    age: u8,
    pos: String,
    ovr: u8,
    pot: u8,
    nation: String,
    contract: String,
    value: String,
}

#[derive(Clone)]
struct LeagueStanding {
    pos: u8,
    team: String,
    played: u8,
    won: u8,
    drawn: u8,
    lost: u8,
    gf: u8,
    ga: u8,
    gd: i8,
    pts: u8,
}

#[derive(Clone)]
struct UpcomingMatch {
    date: String,
    competition: String,
    opponent: String,
    venue: String,
}

#[derive(Clone)]
struct TransferTarget {
    name: String,
    pos: String,
    age: u8,
    club: String,
    value: String,
    interest: u8,
}

#[derive(Clone, Default)]
struct TeamStats {
    shots: u32,
    shots_on_target: u32,
    saves: u32,
    passes: u32,
    tackles: u32,
    dribbles: u32,
    interceptions: u32,
    blocks: u32,
    fouls: u32,
    yellow_cards: u32,
    red_cards: u32,
}

#[derive(Clone, Default)]
struct MatchStats {
    home: TeamStats,
    away: TeamStats,
}

#[derive(Clone)]
struct MatchLogEntry {
    minute_str: String,
    team: Option<Team>,
    text: String,
    event_type: Option<protocol::EventType>,
}

fn get_player_name(team: Team, index: u16, home_players: &[Player]) -> String {
    match team {
        Team::Home => {
            if (index as usize) < home_players.len() {
                home_players[index as usize].name.clone()
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

fn get_ball_target_pos(last_event: Option<(protocol::EventType, protocol::Team, u16)>) -> (f32, f32) {
    if let Some((ev_type, team, player_index)) = last_event {
        match ev_type {
            protocol::EventType::Goal | protocol::EventType::PenaltyGoal => {
                // Ball is in the net!
                match team {
                    Team::Home => (0.95, 0.5), // Away goal
                    Team::Away => (0.05, 0.5), // Home goal
                }
            }
            protocol::EventType::Shot | protocol::EventType::ShotOnTarget => {
                // Shot flying towards goal
                match team {
                    Team::Home => (0.90, 0.45 + (player_index as f32 % 3.0) * 0.05),
                    Team::Away => (0.10, 0.45 + (player_index as f32 % 3.0) * 0.05),
                }
            }
            protocol::EventType::Save | protocol::EventType::PenaltySave => {
                // GK has the ball
                match team {
                    Team::Home => (0.05, 0.5), // Home GK
                    Team::Away => (0.95, 0.5), // Away GK
                }
            }
            protocol::EventType::Corner => {
                // Corner flag
                match team {
                    Team::Home => (0.98, 0.05),
                    Team::Away => (0.02, 0.95),
                }
            }
            protocol::EventType::Miss => {
                // Shot went wide
                match team {
                    Team::Home => (0.98, 0.2),
                    Team::Away => (0.02, 0.8),
                }
            }
            _ => {
                // Ball is with the active player
                get_player_nominal_pos(team, player_index)
            }
        }
    } else {
        (0.5, 0.5) // Kickoff / Center
    }
}

fn get_player_nominal_pos(team: Team, index: u16) -> (f32, f32) {
    let idx = index as usize % 11;
    match team {
        Team::Home => {
            match idx {
                0 => (0.08, 0.50),  // GK
                1 => (0.28, 0.18),  // LB
                2 => (0.26, 0.38),  // CB
                3 => (0.26, 0.62),  // CB
                4 => (0.28, 0.82),  // RB
                5 => (0.48, 0.50),  // DM
                6 => (0.50, 0.32),  // CM
                7 => (0.52, 0.12),  // LM
                8 => (0.52, 0.88),  // RM
                9 => (0.76, 0.35),  // ST
                _ => (0.76, 0.65),  // ST
            }
        }
        Team::Away => {
            match idx {
                0 => (0.92, 0.50),  // GK
                1 => (0.72, 0.82),  // LB
                2 => (0.74, 0.62),  // CB
                3 => (0.74, 0.38),  // CB
                4 => (0.72, 0.18),  // RB
                5 => (0.52, 0.50),  // DM
                6 => (0.50, 0.68),  // CM
                7 => (0.48, 0.88),  // LM
                8 => (0.48, 0.12),  // RM
                9 => (0.24, 0.65),  // ST
                _ => (0.24, 0.35),  // ST
            }
        }
    }
}

fn play_sound(file_path: &str) {
    let path = file_path.to_string();
    std::thread::spawn(move || {
        let _ = std::process::Command::new("paplay")
            .arg(path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    });
}

fn play_bgm(file_path: &str) -> Option<std::process::Child> {
    std::process::Command::new("cvlc")
        .args(&["-I", "dummy", "--loop", file_path])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()
}

fn centered_rect(width: u16, height: u16, r: Rect) -> Rect {
    let padding_y = r.height.saturating_sub(height) / 2;
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(padding_y),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(r);

    let padding_x = r.width.saturating_sub(width) / 2;
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(padding_x),
            Constraint::Length(width),
            Constraint::Min(0),
        ])
        .split(popup_layout[1])[1]
}

fn update_stats_from_event(stats: &mut MatchStats, ev_type: protocol::EventType, ev_team: protocol::Team) {
    let (own_stats, _opp_stats) = match ev_team {
        protocol::Team::Home => (&mut stats.home, &mut stats.away),
        protocol::Team::Away => (&mut stats.away, &mut stats.home),
    };

    match ev_type {
        protocol::EventType::Goal | protocol::EventType::PenaltyGoal => {
            own_stats.shots += 1;
            own_stats.shots_on_target += 1;
        }
        protocol::EventType::Shot => {
            own_stats.shots += 1;
        }
        protocol::EventType::ShotOnTarget => {
            own_stats.shots += 1;
            own_stats.shots_on_target += 1;
        }
        protocol::EventType::Save | protocol::EventType::PenaltySave => {
            own_stats.saves += 1;
        }
        protocol::EventType::Miss | protocol::EventType::PenaltyMiss => {
            own_stats.shots += 1;
        }
        protocol::EventType::Pass => {
            own_stats.passes += 1;
        }
        protocol::EventType::Tackle => {
            own_stats.tackles += 1;
        }
        protocol::EventType::Dribble => {
            own_stats.dribbles += 1;
        }
        protocol::EventType::Interception => {
            own_stats.interceptions += 1;
        }
        protocol::EventType::Block => {
            own_stats.blocks += 1;
        }
        protocol::EventType::Foul => {
            own_stats.fouls += 1;
        }
        protocol::EventType::YellowCard => {
            own_stats.yellow_cards += 1;
        }
        protocol::EventType::RedCard => {
            own_stats.red_cards += 1;
        }
        _ => {}
    }
}

#[derive(PartialEq, Eq)]
enum Screen {
    Dashboard,
    Squad,
    Tactics,
    League,
    Transfers,
    Scouting,
    Match,
    History,
}

impl Screen {
    fn all() -> &'static [Screen; 8] {
        &[
            Screen::Dashboard,
            Screen::Squad,
            Screen::Tactics,
            Screen::League,
            Screen::Transfers,
            Screen::Scouting,
            Screen::Match,
            Screen::History,
        ]
    }

    fn label(&self) -> &'static str {
        match self {
            Screen::Dashboard => " Dashboard ",
            Screen::Squad => " Squad ",
            Screen::Tactics => " Tactics ",
            Screen::League => " League ",
            Screen::Transfers => " Transfers ",
            Screen::Scouting => " Scouting ",
            Screen::Match => " Match ",
            Screen::History => " History & Replays ",
        }
    }
}

struct App {
    tab_index: usize,
    scroll_offset: usize,
    season: u16,
    club: String,
    manager: String,
    players: Vec<Player>,
    standings: Vec<LeagueStanding>,
    fixtures: Vec<UpcomingMatch>,
    transfers: Vec<TransferTarget>,
    messages: Vec<String>,
    match_log: Vec<MatchLogEntry>,
    match_score: [u8; 2],
    match_minute: u8,
    connected: bool,
    server_client: Option<ServerClient>,
    seq: u16,
    match_stats: MatchStats,
    flash_event: Option<(protocol::EventType, String, u8)>,
    last_event: Option<(protocol::EventType, protocol::Team, u16)>,
    ball_progress: f32, // 0.0 to 1.0
    ball_prev_pos: (f32, f32), // (fx, fy)
    bgm_child: Option<std::process::Child>,
    match_started: bool,
    match_finished: bool,
    match_highlights: Vec<MatchLogEntry>,
    replay_events: Vec<engine::database::DbMatchEvent>,
    replay_index: Option<usize>,
    played_matches: Vec<engine::database::DbPlayedMatch>,
    selected_history_index: usize,
}

impl Drop for App {
    fn drop(&mut self) {
        if let Some(mut child) = self.bgm_child.take() {
            let _ = child.kill();
        }
    }
}

impl App {
    fn new() -> Self {
        let bgm = play_bgm("tui/sounds/music.mp3");

        // Initialize SQLite database
        let _ = engine::database::init_db("simple_footie.db");

        let db_players = engine::database::load_squad_players("simple_footie.db", "Home")
            .unwrap_or_default();
        let players = if db_players.is_empty() {
            generate_squad()
        } else {
            db_players.into_iter().map(|p| Player {
                name: p.name,
                age: p.age,
                pos: p.pos,
                ovr: p.ovr,
                pot: p.pot,
                nation: p.nation,
                contract: p.contract,
                value: p.value,
            }).collect()
        };

        let mut app = Self {
            tab_index: 0,
            scroll_offset: 0,
            season: 2026,
            club: "Rustington United".into(),
            manager: "Player".into(),
            players,
            standings: generate_table(),
            fixtures: generate_fixtures(),
            transfers: generate_transfer_targets(),
            messages: vec![
                "Press 'q' to quit  |  Tab/Arrows to navigate  |  's' to simulate match".into(),
                "Transfers: Scout identified 3 targets for CM position".into(),
                "Match Report: Rustington Utd 2-1 FC Terminal".into(),
                "Injury update: L. Byte (knee) out for 2 weeks".into(),
            ],
            match_log: vec![],
            match_score: [0, 0],
            match_minute: 0,
            connected: false,
            server_client: None,
            seq: 0,
            match_stats: MatchStats::default(),
            flash_event: None,
            last_event: None,
            ball_progress: 1.0,
            ball_prev_pos: (0.5, 0.5),
            bgm_child: bgm,
            match_started: false,
            match_finished: false,
            match_highlights: vec![],
            replay_events: vec![],
            replay_index: None,
            played_matches: vec![],
            selected_history_index: 0,
        };

        // Try to connect to the game server
        match ServerClient::connect("127.0.0.1") {
            Ok(client) => {
                app.connected = true;
                app.server_client = Some(client);
                app.messages
                    .insert(0, "✅ Connected to game server (127.0.0.1)".into());
            }
            Err(e) => {
                app.messages.insert(
                    0,
                    format!("⚠️ Server not available: {}. Running offline.", e),
                );
            }
        }

        app
    }

    fn process_replay_event(&mut self, ev: engine::database::DbMatchEvent) {
        let ev_type = protocol::EventType::from_u8(ev.event_type).unwrap_or(protocol::EventType::Pass);
        let ev_team = if ev.team == "Home" { protocol::Team::Home } else { protocol::Team::Away };
        let ev_player_index = ev.player_index;
        let minute = ev.minute;
        let _val = ev.value;
        let text = ev.text;

        // Update match minute
        self.match_minute = minute;
        let formatted_min = protocol::format_match_minute(self.match_minute);

        // Update score for goals
        if ev_type == protocol::EventType::Goal {
            if ev_team == protocol::Team::Home {
                self.match_score[0] = self.match_score[0].wrapping_add(1);
            } else {
                self.match_score[1] = self.match_score[1].wrapping_add(1);
            }
        }

        // Update stats
        update_stats_from_event(&mut self.match_stats, ev_type, ev_team);

        // Flash event banner
        match ev_type {
            protocol::EventType::Goal | protocol::EventType::PenaltyGoal => {
                self.flash_event = Some((ev_type, format!("⚽ GOAL! {}", text), 15));
            }
            protocol::EventType::Save | protocol::EventType::PenaltySave => {
                self.flash_event = Some((ev_type, format!("🧤 SAVE! {}", text), 10));
            }
            protocol::EventType::YellowCard => {
                self.flash_event = Some((ev_type, format!("🟨 YELLOW CARD! {}", text), 10));
            }
            protocol::EventType::RedCard => {
                self.flash_event = Some((ev_type, format!("🟥 RED CARD! {}", text), 12));
            }
            _ => {}
        }

        // Play sound effects
        match ev_type {
            protocol::EventType::Goal | protocol::EventType::PenaltyGoal => {
                play_sound("tui/sounds/cheer.wav");
            }
            protocol::EventType::Save | protocol::EventType::PenaltySave => {
                play_sound("tui/sounds/cheer.wav");
            }
            protocol::EventType::YellowCard | protocol::EventType::RedCard | protocol::EventType::Foul => {
                play_sound("tui/sounds/whistle.wav");
            }
            protocol::EventType::HalfTime | protocol::EventType::FullTime => {
                play_sound("tui/sounds/whistle.wav");
            }
            protocol::EventType::Pass | protocol::EventType::Tackle | protocol::EventType::Dribble | protocol::EventType::Interception | protocol::EventType::Block => {
                play_sound("tui/sounds/clap.wav");
            }
            _ => {}
        }

        // Update 2D pitch tracker ball animation
        self.ball_prev_pos = get_ball_target_pos(self.last_event);
        self.last_event = Some((ev_type, ev_team, ev_player_index));
        self.ball_progress = 0.0;

        // Add to match log & highlights
        let is_highlight = match ev_type {
            protocol::EventType::Goal |
            protocol::EventType::PenaltyGoal |
            protocol::EventType::Save |
            protocol::EventType::PenaltySave |
            protocol::EventType::YellowCard |
            protocol::EventType::RedCard |
            protocol::EventType::PenaltyMiss |
            protocol::EventType::HalfTime |
            protocol::EventType::FullTime => true,
            _ => false,
        };

        if is_highlight {
            self.match_highlights.push(MatchLogEntry {
                minute_str: formatted_min.clone(),
                team: Some(ev_team),
                text: text.clone(),
                event_type: Some(ev_type),
            });
        }

        self.match_log.push(MatchLogEntry {
            minute_str: formatted_min,
            team: Some(ev_team),
            text,
            event_type: Some(ev_type),
        });

        if self.match_log.len() > 100 {
            self.match_log.remove(0);
        }
    }

    fn simulate_demo_match(&mut self) {
        self.match_started = true;
        let state = MatchState {
            match_id: 1,
            token: [0u8; 16],
            last_seq: 0,
            score: [0, 0],
            minute: 0,
            possession: 0.5,
            stamina: [1.0, 1.0],
            tactic: [TacticState::default(), TacticState::default()],
            rng_seed: 42,
        };

        let home = generate_synthetic_squad(Team::Home, 78);
        let away = generate_synthetic_squad(Team::Away, 75);

        let result = simulate_minutes(state, home, away, 135); // Simulate full potential match including extra time/penalties

        self.match_score = result.state.score;
        self.match_minute = result.state.minute;

        self.match_log.clear();
        self.match_stats = MatchStats::default();

        self.match_log.push(MatchLogEntry {
            minute_str: "".into(),
            team: None,
            text: "=== Match Report ===".into(),
            event_type: None,
        });
        self.match_log.push(MatchLogEntry {
            minute_str: "".into(),
            team: None,
            text: format!(
                "Rustington United {} - {} FC Terminal",
                result.state.score[0], result.state.score[1]
            ),
            event_type: None,
        });
        self.match_log.push(MatchLogEntry {
            minute_str: "".into(),
            team: None,
            text: "".into(),
            event_type: None,
        });
        self.match_log.push(MatchLogEntry {
            minute_str: "".into(),
            team: None,
            text: format!(
                "Possession: {:.0}%",
                result.state.possession * 100.0
            ),
            event_type: None,
        });
        self.match_log.push(MatchLogEntry {
            minute_str: "".into(),
            team: None,
            text: "".into(),
            event_type: None,
        });
        self.match_log.push(MatchLogEntry {
            minute_str: "".into(),
            team: None,
            text: "--- Events ---".into(),
            event_type: None,
        });

        for ev in &result.events {
            let formatted_min = protocol::format_match_minute(ev.minute);
            let team_name = match ev.team {
                Team::Home => "Rustington",
                Team::Away => "FC Terminal",
            };

            // Update stats
            update_stats_from_event(&mut self.match_stats, ev.event_type, ev.team);

            let player_name = get_player_name(ev.team, ev.player_index, &self.players);

            let text = match ev.event_type {
                protocol::EventType::Kickoff => "Kickoff!".into(),
                protocol::EventType::Goal => {
                    format!("⚽ GOAL! {} scores for {}!", player_name, team_name)
                }
                protocol::EventType::Shot => format!("Shot by {}", player_name),
                protocol::EventType::ShotOnTarget => format!("Shot on target by {}!", player_name),
                protocol::EventType::Save => {
                    let gk_name = get_player_name(ev.team, ev.player_index, &self.players);
                    format!("Great save by {}!", gk_name)
                }
                protocol::EventType::Miss => format!("Shot wide by {}", player_name),
                protocol::EventType::Foul => format!("Foul committed by {}", player_name),
                protocol::EventType::Corner => "Corner kick".into(),
                protocol::EventType::FreeKick => format!("Free kick taken by {}", player_name),
                protocol::EventType::YellowCard => format!("Yellow card for {}", player_name),
                protocol::EventType::RedCard => format!("🔴 RED CARD for {}", player_name),
                protocol::EventType::Substitution => format!("Substitution for {}", team_name),
                protocol::EventType::Injury => format!("Injury to {}", player_name),
                protocol::EventType::Offside => format!("Offside for {}", player_name),
                protocol::EventType::HalfTime => "Half Time!".into(),
                protocol::EventType::FullTime => "Full Time!".into(),
                protocol::EventType::PenaltyGoal => format!("⚽ PENALTY GOAL! {} scores for {}! Shootout score: {}", player_name, team_name, ev.value as u32),
                protocol::EventType::PenaltyMiss => format!("❌ Penalty missed by {}!", player_name),
                protocol::EventType::PenaltySave => {
                    let gk_name = get_player_name(ev.team, ev.player_index, &self.players);
                    format!("🧤 PENALTY SAVED by {} GK!", gk_name)
                }
                protocol::EventType::ExtraTimeStart => "⏰ EXTRA TIME STARTS! 30 more minutes will be played.".into(),
                protocol::EventType::ExtraTimeHalfTime => "⏰ Extra Time Half Time!".into(),
                protocol::EventType::PenaltyShootoutStart => "🧤 PENALTY SHOOTOUT STARTS!".into(),
                protocol::EventType::Pass => format!("Pass completed by {}", player_name),
                protocol::EventType::Tackle => format!("Tackle won by {}", player_name),
                protocol::EventType::Dribble => format!("Dribble completed by {}", player_name),
                protocol::EventType::Interception => format!("Interception by {}", player_name),
                protocol::EventType::Block => format!("Shot blocked by {}", player_name),
            };

            let event_team = match ev.event_type {
                protocol::EventType::Kickoff |
                protocol::EventType::HalfTime |
                protocol::EventType::FullTime |
                protocol::EventType::ExtraTimeStart |
                protocol::EventType::ExtraTimeHalfTime |
                protocol::EventType::PenaltyShootoutStart => None,
                _ => Some(ev.team),
            };

            let is_highlight = match ev.event_type {
                protocol::EventType::Goal |
                protocol::EventType::PenaltyGoal |
                protocol::EventType::Save |
                protocol::EventType::PenaltySave |
                protocol::EventType::YellowCard |
                protocol::EventType::RedCard |
                protocol::EventType::PenaltyMiss |
                protocol::EventType::HalfTime |
                protocol::EventType::FullTime => true,
                _ => false,
            };

            if is_highlight {
                self.match_highlights.push(MatchLogEntry {
                    minute_str: formatted_min.clone(),
                    team: event_team,
                    text: text.clone(),
                    event_type: Some(ev.event_type),
                });
            }

            self.match_log.push(MatchLogEntry {
                minute_str: formatted_min,
                team: event_team,
                text,
                event_type: Some(ev.event_type),
            });
        }

        // Save simulated demo match to SQLite database
        let date_str = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let mut db_events = Vec::new();
        for ev in &result.events {
            let text = protocol::generate_exciting_commentary(ev.event_type, ev.team, ev.player_index, ev.minute, ev.value);
            db_events.push(engine::database::DbMatchEvent {
                minute: ev.minute,
                event_type: ev.event_type as u8,
                team: match ev.team {
                    Team::Home => "Home".to_string(),
                    Team::Away => "Away".to_string(),
                },
                player_index: ev.player_index,
                value: ev.value,
                text,
            });
        }
        if let Err(e) = engine::database::save_match(
            "simple_footie.db",
            &date_str,
            "Rustington United",
            "FC Terminal",
            result.state.score[0],
            result.state.score[1],
            &db_events,
        ) {
            self.messages.insert(0, format!("⚠️ Failed to save match to DB: {}", e));
        } else {
            self.messages.insert(0, "💾 Match saved to SQLite database successfully!".into());
        }

        self.match_finished = true;
    }

    fn run(&mut self, terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
        loop {
            if let Some((_, _, ref mut ticks)) = self.flash_event {
                if *ticks > 0 {
                    *ticks -= 1;
                } else {
                    self.flash_event = None;
                }
            }
            if self.ball_progress < 1.0 {
                self.ball_progress = (self.ball_progress + 0.1).min(1.0);
            }

            // Advance replay if active
            if let Some(idx) = self.replay_index {
                if idx < self.replay_events.len() {
                    let ev = self.replay_events[idx].clone();
                    self.process_replay_event(ev);
                    self.replay_index = Some(idx + 1);
                    // Sleep to make replay watchable
                    std::thread::sleep(Duration::from_millis(150));
                } else {
                    self.match_finished = true;
                    self.replay_index = None;
                }
            }

            terminal.draw(|frame| self.render(frame))?;
            if !self.handle_events()? {
                break Ok(());
            }
        }
    }

    fn set_tab_index(&mut self, idx: usize) {
        self.tab_index = idx;
        self.scroll_offset = 0;
        if Screen::all()[self.tab_index] == Screen::History {
            if let Ok(matches) = engine::database::load_played_matches("simple_footie.db") {
                self.played_matches = matches;
                self.selected_history_index = 0;
            }
        }
    }

    fn handle_events(&mut self) -> Result<bool> {
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    // If we are on the Match tab and the match has finished, handle highlights popup keys
                    if Screen::all()[self.tab_index] == Screen::Match && self.match_finished {
                        match key.code {
                            KeyCode::Enter | KeyCode::Esc | KeyCode::Char('f') => {
                                // Finish match and reset
                                self.match_started = false;
                                self.match_finished = false;
                                self.match_log.clear();
                                self.match_score = [0, 0];
                                self.match_minute = 0;
                                self.match_stats = MatchStats::default();
                                self.match_highlights.clear();
                                self.set_tab_index(0); // Go back to Dashboard
                            }
                            _ => {}
                        }
                        return Ok(true);
                    }

                    // If we are on the Match tab and the match has not started, handle popup keys
                    if Screen::all()[self.tab_index] == Screen::Match && !self.match_started {
                        match key.code {
                            KeyCode::Enter | KeyCode::Char('s') => {
                                if !self.connected {
                                    self.simulate_demo_match();
                                } else {
                                    self.match_started = true;
                                }
                                play_sound("tui/sounds/whistle.wav");
                            }
                            KeyCode::Esc => {
                                // Go back to Dashboard
                                self.set_tab_index(0);
                            }
                            KeyCode::Tab | KeyCode::Right => {
                                let new_idx = (self.tab_index + 1) % Screen::all().len();
                                self.set_tab_index(new_idx);
                            }
                            KeyCode::BackTab | KeyCode::Left => {
                                let new_idx = if self.tab_index == 0 {
                                    Screen::all().len() - 1
                                } else {
                                    self.tab_index - 1
                                };
                                self.set_tab_index(new_idx);
                            }
                            _ => {}
                        }
                        return Ok(true);
                    }

                    // If we are on the History tab, handle navigation and replay initiation
                    if Screen::all()[self.tab_index] == Screen::History {
                        match key.code {
                            KeyCode::Up | KeyCode::Char('k') => {
                                if self.selected_history_index > 0 {
                                    self.selected_history_index -= 1;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if !self.played_matches.is_empty() && self.selected_history_index < self.played_matches.len() - 1 {
                                    self.selected_history_index += 1;
                                }
                            }
                            KeyCode::Enter => {
                                if self.selected_history_index < self.played_matches.len() {
                                    let m = &self.played_matches[self.selected_history_index];
                                    if let Ok(events) = engine::database::load_match_events("simple_footie.db", m.id) {
                                        self.replay_events = events;
                                        self.replay_index = Some(0);
                                        self.match_started = true;
                                        self.match_finished = false;
                                        self.match_log.clear();
                                        self.match_score = [0, 0];
                                        self.match_minute = 0;
                                        self.match_stats = MatchStats::default();
                                        self.match_highlights.clear();
                                        self.set_tab_index(6); // Switch to Match tab
                                        play_sound("tui/sounds/whistle.wav");
                                    }
                                }
                            }
                            KeyCode::Tab | KeyCode::Right => {
                                let new_idx = (self.tab_index + 1) % Screen::all().len();
                                self.set_tab_index(new_idx);
                            }
                            KeyCode::BackTab | KeyCode::Left => {
                                let new_idx = if self.tab_index == 0 {
                                    Screen::all().len() - 1
                                } else {
                                    self.tab_index - 1
                                };
                                self.set_tab_index(new_idx);
                            }
                            KeyCode::Esc | KeyCode::Char('q') => return Ok(false),
                            _ => {}
                        }
                        return Ok(true);
                    }

                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(false),
                        KeyCode::Char('s') => {
                            if !self.connected {
                                self.simulate_demo_match();
                            }
                        }
                        // Tactical commands (only when connected)
                        KeyCode::Char('1') => self.send_tactical(CommandType::Mentality, 0, 0), // Normal
                        KeyCode::Char('2') => self.send_tactical(CommandType::Mentality, 0, 1), // Attack
                        KeyCode::Char('3') => self.send_tactical(CommandType::Mentality, 0, 2), // Defend
                        KeyCode::Char('4') => self.send_tactical(CommandType::Press, 0, 0), // Low press
                        KeyCode::Char('5') => self.send_tactical(CommandType::Press, 0, 1), // Medium press
                        KeyCode::Char('6') => self.send_tactical(CommandType::Press, 0, 2), // High press
                        KeyCode::Char('7') => self.send_tactical(CommandType::Tempo, 0, 0), // Slow tempo
                        KeyCode::Char('8') => self.send_tactical(CommandType::Tempo, 0, 1), // Normal tempo
                        KeyCode::Char('9') => self.send_tactical(CommandType::Tempo, 0, 2), // Fast tempo
                        KeyCode::Tab | KeyCode::Right => {
                            let new_idx = (self.tab_index + 1) % Screen::all().len();
                            self.set_tab_index(new_idx);
                        }
                        KeyCode::BackTab | KeyCode::Left => {
                            let new_idx = if self.tab_index == 0 {
                                Screen::all().len() - 1
                            } else {
                                self.tab_index - 1
                            };
                            self.set_tab_index(new_idx);
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            self.scroll_offset = self.scroll_offset.saturating_sub(1)
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            self.scroll_offset = self.scroll_offset.saturating_add(1)
                        }
                        KeyCode::Home => self.scroll_offset = usize::MAX,
                        KeyCode::End => self.scroll_offset = 0,
                        _ => {}
                    }
                }
            }
        }
        // Poll for incoming UDP events (drain into local vec to avoid borrow conflict)
        let mut events: Vec<protocol::EventPacket> = Vec::new();
        if let Some(ref client) = self.server_client {
            while let Ok(event) = client.event_rx.try_recv() {
                events.push(event);
            }
        }
        for event in events {
            self.process_server_event(event);
        }

        Ok(true)
    }

    fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        // Check for minimum screen resolution
        const MIN_WIDTH: u16 = 110;
        const MIN_HEIGHT: u16 = 32;

        if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
            let warning_block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                .title(" ⚠️ SCREEN RESOLUTION TOO LOW ⚠️ ")
                .title_alignment(Alignment::Center)
                .style(Style::default().bg(Color::Rgb(20, 10, 10)));

            let warning_text = vec![
                Line::from(""),
                Line::from(Span::styled("Your terminal window is too small to render the game correctly!", Style::default().bold().fg(Color::White))).alignment(Alignment::Center),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Required Minimum: ", Style::default().fg(Color::Gray)),
                    Span::styled(format!("{} x {}", MIN_WIDTH, MIN_HEIGHT), Style::default().bold().fg(Color::Green)),
                    Span::styled(" characters", Style::default().fg(Color::Gray)),
                ]).alignment(Alignment::Center),
                Line::from(vec![
                    Span::styled("Current Size:     ", Style::default().fg(Color::Gray)),
                    Span::styled(format!("{} x {}", area.width, area.height), Style::default().bold().fg(Color::Red)),
                    Span::styled(" characters", Style::default().fg(Color::Gray)),
                ]).alignment(Alignment::Center),
                Line::from(""),
                Line::from(Span::styled("Please resize or zoom out your terminal window to continue.", Style::default().italic().fg(Color::Yellow))).alignment(Alignment::Center),
                Line::from(""),
                Line::from(Span::styled("⚽ Simple Footie TUI ⚽", Style::default().bold().fg(Color::Green))).alignment(Alignment::Center),
            ];

            let warning_paragraph = Paragraph::new(warning_text).block(warning_block);
            frame.render_widget(warning_paragraph, area);
            return;
        }

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(area);

        self.render_title_bar(frame, layout[0]);
        self.render_tabs(frame, layout[1]);
        self.render_content(frame, layout[2]);
        self.render_status_bar(frame, layout[3]);
    }

    fn render_title_bar(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25), // Brand
                Constraint::Percentage(55), // Info
                Constraint::Percentage(20), // Connection Status
            ])
            .split(area);

        // 1. Brand Block
        let brand_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green))
            .style(Style::default().bg(Color::Rgb(10, 35, 10)));
        let brand_text = Line::from(vec![
            Span::styled(" ⚽ SIMPLE FOOTIE ", Style::default().bold().fg(Color::White)),
            Span::styled("v1.0", Style::default().italic().fg(Color::Green)),
        ]).alignment(Alignment::Center);
        frame.render_widget(Paragraph::new(brand_text).block(brand_block), chunks[0]);

        // 2. Info Block
        let info_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .style(Style::default().bg(Color::Rgb(20, 20, 20)));
        let info_text = Line::from(vec![
            Span::styled("🛡️  Club: ", Style::default().fg(Color::Gray)),
            Span::styled(&self.club, Style::default().bold().fg(Color::Green)),
            Span::styled("  |  ⏱️ Season: ", Style::default().fg(Color::Gray)),
            Span::styled(self.season.to_string(), Style::default().bold().fg(Color::Yellow)),
            Span::styled("  |  👤 Manager: ", Style::default().fg(Color::Gray)),
            Span::styled(&self.manager, Style::default().bold().fg(Color::Cyan)),
        ]).alignment(Alignment::Center);
        frame.render_widget(Paragraph::new(info_text).block(info_block), chunks[1]);

        // 3. Connection Status Block
        let (status_text, border_color, bg_color) = if self.connected {
            (
                Line::from(Span::styled(" 🟢 ONLINE ", Style::default().bold().fg(Color::White))),
                Color::Green,
                Color::Rgb(10, 45, 10),
            )
        } else {
            (
                Line::from(Span::styled(" 🔴 OFFLINE ", Style::default().bold().fg(Color::White))),
                Color::Red,
                Color::Rgb(45, 10, 10),
            )
        };
        let status_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .style(Style::default().bg(bg_color));
        frame.render_widget(Paragraph::new(status_text.alignment(Alignment::Center)).block(status_block), chunks[2]);
    }

    fn render_tabs(&self, frame: &mut Frame, area: Rect) {
        let labels: Vec<String> = Screen::all()
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let lbl = s.label().trim();
                if i == self.tab_index {
                    format!(" 🌟 {} ", lbl)
                } else {
                    format!("  {}  ", lbl)
                }
            })
            .collect();
        let tabs = Tabs::new(labels)
            .select(self.tab_index)
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .style(Style::default().fg(Color::Gray));
        frame.render_widget(tabs, area);
    }

    fn render_content(&self, frame: &mut Frame, area: Rect) {
        match Screen::all()[self.tab_index] {
            Screen::Dashboard => self.render_dashboard(frame, area),
            Screen::Squad => self.render_squad(frame, area),
            Screen::Tactics => self.render_tactics(frame, area),
            Screen::League => self.render_league(frame, area),
            Screen::Transfers => self.render_transfers(frame, area),
            Screen::Scouting => self.render_scouting(frame, area),
            Screen::Match => {
                self.render_match(frame, area);
                if !self.match_started {
                    self.render_kickoff_popup(frame, area);
                } else if self.match_finished {
                    self.render_highlights_popup(frame, area);
                }
            }
            Screen::History => self.render_history(frame, area),
        }
    }

    fn render_highlights_popup(&self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(75, 22, area);
        
        // Clear the background of the popup area
        frame.render_widget(ratatui::widgets::Clear, popup_area);

        let popup_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .title(" 🏆 MATCH HIGHLIGHTS & REPORT 🏆 ")
            .title_alignment(Alignment::Center)
            .style(Style::default().bg(Color::Rgb(15, 15, 25))); // Dark blue-ish background

        let popup_inner = popup_block.inner(popup_area);
        frame.render_widget(popup_block, popup_area);

        let popup_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Scoreboard / Title
                Constraint::Min(1),    // List of highlights
                Constraint::Length(3), // Action Button
            ])
            .split(popup_inner);

        // 1. Scoreboard / Title
        let score_line = Line::from(vec![
            Span::styled("🛡️  Rustington United ", Style::default().bold().fg(Color::Green)),
            Span::styled(format!(" {} - {} ", self.match_score[0], self.match_score[1]), Style::default().bold().fg(Color::Yellow)),
            Span::styled(" FC Terminal  💻", Style::default().bold().fg(Color::Yellow)),
        ]).alignment(Alignment::Center);
        frame.render_widget(Paragraph::new(score_line), popup_chunks[0]);

        // 2. List of highlights
        let mut highlight_items = Vec::new();
        if self.match_highlights.is_empty() {
            highlight_items.push(ListItem::new(Line::from("No major highlights recorded in this match.").alignment(Alignment::Center)));
        } else {
            for entry in &self.match_highlights {
                let (style, icon) = if let Some(ev_type) = entry.event_type {
                    match ev_type {
                        protocol::EventType::Goal | protocol::EventType::PenaltyGoal => {
                            (Style::default().bold().fg(Color::Green), "⚽ GOAL! ")
                        }
                        protocol::EventType::YellowCard => {
                            (Style::default().bold().fg(Color::Yellow), "🟨 CARD ")
                        }
                        protocol::EventType::RedCard => {
                            (Style::default().bold().fg(Color::Red), "🟥 SENT OFF! ")
                        }
                        protocol::EventType::Save | protocol::EventType::PenaltySave => {
                            (Style::default().bold().fg(Color::Cyan), "🧤 SAVE! ")
                        }
                        protocol::EventType::PenaltyMiss => {
                            (Style::default().bold().fg(Color::Red), "❌ PENALTY MISSED ")
                        }
                        protocol::EventType::HalfTime => {
                            (Style::default().bold().fg(Color::Blue), "🏁 HALF TIME ")
                        }
                        protocol::EventType::FullTime => {
                            (Style::default().bold().fg(Color::Yellow), "🏆 FULL TIME ")
                        }
                        _ => (Style::default().fg(Color::White), ""),
                    }
                } else {
                    (Style::default().fg(Color::White), "")
                };

                let text_line = Line::from(vec![
                    Span::styled(format!("⏱️ {:>4}  ", entry.minute_str), Style::default().fg(Color::Gray)),
                    Span::styled(icon, style),
                    Span::styled(&entry.text, Style::default().fg(Color::White)),
                ]);
                highlight_items.push(ListItem::new(text_line));
            }
        }

        let highlights_list = List::new(highlight_items)
            .block(Block::default().borders(Borders::ALL).title(" 📜 Match Timeline ").border_style(Style::default().fg(Color::DarkGray)))
            .style(Style::default().bg(Color::Rgb(10, 10, 15)));
        frame.render_widget(highlights_list, popup_chunks[1]);

        // 3. Action Button
        let action_text = Line::from(vec![
            Span::styled(" [ Enter / Esc ] ", Style::default().bold().fg(Color::Yellow).bg(Color::Rgb(40, 40, 20))),
            Span::styled(" Finish Match & Return to Dashboard ", Style::default().bold().fg(Color::White)),
        ]).alignment(Alignment::Center);
        frame.render_widget(Paragraph::new(action_text), popup_chunks[2]);
    }

    fn render_kickoff_popup(&self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(50, 11, area);
        
        // Clear the background of the popup area
        frame.render_widget(ratatui::widgets::Clear, popup_area);

        let popup_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
            .title(" ⚽ MATCH KICK-OFF ⚽ ")
            .title_alignment(Alignment::Center)
            .style(Style::default().bg(Color::Rgb(15, 23, 15)));

        let popup_text = vec![
            Line::from(""),
            Line::from(Span::styled("Are you ready to start the match?", Style::default().bold().fg(Color::White))).alignment(Alignment::Center),
            Line::from(Span::styled(format!("{} vs FC Terminal", self.club), Style::default().bold().fg(Color::Yellow))).alignment(Alignment::Center),
            Line::from(""),
            Line::from(vec![
                Span::styled(" [ Enter ] ", Style::default().bold().fg(Color::Green).bg(Color::Rgb(30, 50, 30))),
                Span::styled(" Kick Off! ", Style::default().bold().fg(Color::White)),
            ]).alignment(Alignment::Center),
            Line::from(""),
            Line::from(Span::styled(" [ Esc ] Go Back ", Style::default().fg(Color::Gray))).alignment(Alignment::Center),
        ];

        let popup_paragraph = Paragraph::new(popup_text).block(popup_block);
        frame.render_widget(popup_paragraph, popup_area);
    }

    fn render_history(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area);

        // Left block: List of played matches
        let mut match_items = Vec::new();
        for (i, m) in self.played_matches.iter().enumerate() {
            let is_selected = i == self.selected_history_index;
            let style = if is_selected {
                Style::default().bold().fg(Color::Yellow).bg(Color::Rgb(30, 30, 40))
            } else {
                Style::default().fg(Color::White)
            };
            let prefix = if is_selected { "▶ " } else { "  " };
            let text_line = Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(format!("{}  ", m.date_played), Style::default().fg(Color::Gray)),
                Span::styled(format!("{} {}-{} {}", m.home_team, m.home_score, m.away_score, m.away_team), style),
            ]);
            match_items.push(ListItem::new(text_line));
        }

        let match_list = List::new(match_items)
            .block(Block::default().borders(Borders::ALL).title(" 🏆 Played Matches History "))
            .style(Style::default());
        frame.render_widget(match_list, chunks[0]);

        // Right block: Instructions and details of selected match
        let detail_block = Block::default()
            .borders(Borders::ALL)
            .title(" 🔍 Match Replay Details ");
        let detail_inner = detail_block.inner(chunks[1]);
        frame.render_widget(detail_block, chunks[1]);

        if self.played_matches.is_empty() {
            let empty_text = vec![
                Line::from(""),
                Line::from(Span::styled("No played matches found in SQLite database.", Style::default().bold().fg(Color::Gray))).alignment(Alignment::Center),
                Line::from(""),
                Line::from(Span::styled("Simulate a match in the 'Match' tab first!", Style::default().fg(Color::DarkGray))).alignment(Alignment::Center),
            ];
            frame.render_widget(Paragraph::new(empty_text), detail_inner);
        } else if self.selected_history_index < self.played_matches.len() {
            let m = &self.played_matches[self.selected_history_index];
            let mut details = vec![
                Line::from(""),
                Line::from(Span::styled("🏆 MATCH REPLAY CENTER 🏆", Style::default().bold().fg(Color::Yellow))).alignment(Alignment::Center),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Home: ", Style::default().fg(Color::Gray)),
                    Span::styled(&m.home_team, Style::default().bold().fg(Color::Green)),
                ]).alignment(Alignment::Center),
                Line::from(vec![
                    Span::styled("Away: ", Style::default().fg(Color::Gray)),
                    Span::styled(&m.away_team, Style::default().bold().fg(Color::Yellow)),
                ]).alignment(Alignment::Center),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Final Score: ", Style::default().fg(Color::Gray)),
                    Span::styled(format!("{} - {}", m.home_score, m.away_score), Style::default().bold().fg(Color::White).bg(Color::Rgb(40, 40, 40))),
                ]).alignment(Alignment::Center),
                Line::from(""),
                Line::from(Span::styled("──────────────────────────────────────────────────", Style::default().fg(Color::DarkGray))).alignment(Alignment::Center),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Press ", Style::default().fg(Color::Gray)),
                    Span::styled("[ Enter ]", Style::default().bold().fg(Color::Cyan)),
                    Span::styled(" to load and replay this match", Style::default().fg(Color::Gray)),
                ]).alignment(Alignment::Center),
                Line::from(Span::styled("The replay will start and visualize player/ball movements", Style::default().italic().fg(Color::DarkGray))).alignment(Alignment::Center),
                Line::from(Span::styled("exactly as it occurred in the live match!", Style::default().italic().fg(Color::DarkGray))).alignment(Alignment::Center),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Use ", Style::default().fg(Color::Gray)),
                    Span::styled("[ Up / Down ]", Style::default().bold().fg(Color::Yellow)),
                    Span::styled(" to navigate the matches history list.", Style::default().fg(Color::Gray)),
                ]).alignment(Alignment::Center),
            ];

            // Load highlights/events count
            if let Ok(events) = engine::database::load_match_events("simple_footie.db", m.id) {
                let goals = events.iter().filter(|e| e.event_type == protocol::EventType::Goal as u8 || e.event_type == protocol::EventType::PenaltyGoal as u8).count();
                let saves = events.iter().filter(|e| e.event_type == protocol::EventType::Save as u8 || e.event_type == protocol::EventType::PenaltySave as u8).count();
                let cards = events.iter().filter(|e| e.event_type == protocol::EventType::YellowCard as u8 || e.event_type == protocol::EventType::RedCard as u8).count();
                
                details.push(Line::from(""));
                details.push(Line::from(Span::styled("📊 Match Statistics Summary:", Style::default().bold().fg(Color::White))).alignment(Alignment::Center));
                details.push(Line::from(format!("⚽ Goals: {}  |  🧤 Saves: {}  |  🟨🟥 Cards: {}", goals, saves, cards)).alignment(Alignment::Center));
            }

            frame.render_widget(Paragraph::new(details), detail_inner);
        }
    }

    fn render_dashboard(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        let items: Vec<ListItem> = self
            .fixtures
            .iter()
            .map(|m| {
                ListItem::new(format!(
                    " {}  {}  {}  {}",
                    m.date, m.competition, m.opponent, m.venue
                ))
            })
            .collect();
        let fixtures_list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" 📅 Upcoming Fixtures "),
            )
            .style(Style::default());
        frame.render_widget(fixtures_list, chunks[0]);

        let items: Vec<ListItem> = self
            .messages
            .iter()
            .map(|m| ListItem::new(m.as_str()))
            .collect();
        let msg_list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(" 📰 News "))
            .style(Style::default());
        frame.render_widget(msg_list, chunks[1]);
    }

    fn render_squad(&self, frame: &mut Frame, area: Rect) {
        let header_cells = [
            "Name", "Age", "Pos", "OVR", "POT", "Nation", "Contract", "Value",
        ]
        .iter()
        .map(|h| Cell::from(Text::from(*h).style(Style::default().add_modifier(Modifier::BOLD))));
        let header = Row::new(header_cells).style(Style::default().bg(Color::DarkGray));

        let rows: Vec<Row> = self
            .players
            .iter()
            .map(|p| {
                let cells = vec![
                    Cell::from(p.name.as_str()),
                    Cell::from(format!("{}", p.age)),
                    Cell::from(p.pos.as_str()),
                    Cell::from(format!("{}", p.ovr)).style(if p.ovr >= 80 {
                        Style::default().fg(Color::Green)
                    } else if p.ovr >= 70 {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::Red)
                    }),
                    Cell::from(format!("{}", p.pot)),
                    Cell::from(p.nation.as_str()),
                    Cell::from(p.contract.as_str()),
                    Cell::from(p.value.as_str()),
                ];
                Row::new(cells)
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(18),
                Constraint::Length(4),
                Constraint::Length(5),
                Constraint::Length(5),
                Constraint::Length(5),
                Constraint::Length(8),
                Constraint::Length(10),
                Constraint::Length(10),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" 🧑‍🤝‍🧑 First Team Squad "),
        );
        frame.render_widget(table, area);
    }

    fn render_tactics(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(30), Constraint::Min(1)])
            .split(area);

        let formation_lines = vec![
            Line::from(Span::styled(
                "       ⚽  Formation  ⚽",
                Style::default().bold(),
            )),
            Line::from(""),
            Line::from("    GK    Rusty McSave"),
            Line::from("  LB  CB  CB  RB"),
            Line::from("  L.Byte  K.Heap  S.Tack  M.Alloc"),
            Line::from("       CM  CM"),
            Line::from("       ST  ST"),
            Line::from(""),
            Line::from(Span::styled(
                "  Possession: 4-4-2",
                Style::default().fg(Color::Cyan),
            )),
            Line::from(Span::styled(
                "  Out of Poss: 4-4-1-1",
                Style::default().fg(Color::Cyan),
            )),
            Line::from(""),
            Line::from("  Mentality:  Balanced"),
            Line::from("  Tempo:      Normal"),
            Line::from("  Press:      High"),
            Line::from("  Width:      Normal"),
        ];
        let formation = Paragraph::new(Text::from(formation_lines))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" 📋 Current Setup "),
            )
            .style(Style::default());
        frame.render_widget(formation, chunks[0]);

        let items: Vec<ListItem> = self
            .players
            .iter()
            .take(11)
            .map(|p| {
                ListItem::new(format!(
                    "{}  {}  {}  {}",
                    p.pos,
                    p.name,
                    match p.pos.as_str() {
                        "GK" => "Goalkeeper",
                        "LB" | "RB" => "Full Back",
                        "CB" => "Centre Back",
                        "CM" => "Box-to-Box Mid",
                        "LM" | "RM" => "Wide Midfielder",
                        "ST" => "Advanced Forward",
                        _ => "Default Role",
                    },
                    p.ovr
                ))
            })
            .collect();
        let roles_list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" 🎯 Player Roles "),
            )
            .style(Style::default());
        frame.render_widget(roles_list, chunks[1]);
    }

    fn render_league(&self, frame: &mut Frame, area: Rect) {
        let header_cells = ["#", "Team", "P", "W", "D", "L", "GF", "GA", "GD", "Pts"]
            .iter()
            .map(|h| {
                Cell::from(Text::from(*h).style(Style::default().add_modifier(Modifier::BOLD)))
            });
        let header = Row::new(header_cells).style(Style::default().bg(Color::DarkGray));

        let rows: Vec<Row> = self
            .standings
            .iter()
            .map(|s| {
                let color = if s.pos <= 2 {
                    Style::default().fg(Color::Green)
                } else if s.pos <= 4 {
                    Style::default().fg(Color::Yellow)
                } else if s.pos >= self.standings.len() as u8 - 2 {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default()
                };
                let cells = vec![
                    Cell::from(format!("{}", s.pos)).style(color),
                    Cell::from(s.team.as_str()),
                    Cell::from(format!("{}", s.played)),
                    Cell::from(format!("{}", s.won)),
                    Cell::from(format!("{}", s.drawn)),
                    Cell::from(format!("{}", s.lost)),
                    Cell::from(format!("{}", s.gf)),
                    Cell::from(format!("{}", s.ga)),
                    Cell::from(format!("{}", s.gd)),
                    Cell::from(format!("{}", s.pts))
                        .style(Style::default().add_modifier(Modifier::BOLD)),
                ];
                Row::new(cells)
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(3),
                Constraint::Length(22),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(4),
                Constraint::Length(4),
                Constraint::Length(4),
                Constraint::Length(4),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" 🏆 Premier League Table "),
        );
        frame.render_widget(table, area);
    }

    fn render_transfers(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(10), Constraint::Min(1)])
            .split(area);

        let budget = Paragraph::new(Text::from(vec![
            Line::from(Span::styled(
                "💰 Transfer Budget:  £12,500,000",
                Style::default().bold().fg(Color::Green),
            )),
            Line::from(Span::styled(
                "💵 Wage Budget:      £250,000 / week",
                Style::default().bold().fg(Color::Green),
            )),
            Line::from(Span::from("")),
            Line::from(Span::styled(
                "  Transfer Targets — prioritized by scout recommendation:",
                Style::default().fg(Color::Cyan),
            )),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" 💼 Transfer Hub "),
        )
        .style(Style::default());
        frame.render_widget(budget, chunks[0]);

        let header_cells = ["Name", "Pos", "Age", "Club", "Value", "Interest"]
            .iter()
            .map(|h| {
                Cell::from(Text::from(*h).style(Style::default().add_modifier(Modifier::BOLD)))
            });
        let header = Row::new(header_cells).style(Style::default().bg(Color::DarkGray));

        let rows: Vec<Row> = self
            .transfers
            .iter()
            .map(|t| {
                let cells = vec![
                    Cell::from(t.name.as_str()),
                    Cell::from(t.pos.as_str()),
                    Cell::from(format!("{}", t.age)),
                    Cell::from(t.club.as_str()),
                    Cell::from(t.value.as_str()),
                    Cell::from(format!("{}%", t.interest)).style(if t.interest >= 70 {
                        Style::default().fg(Color::Green)
                    } else if t.interest >= 40 {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::Red)
                    }),
                ];
                Row::new(cells)
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(18),
                Constraint::Length(5),
                Constraint::Length(4),
                Constraint::Length(22),
                Constraint::Length(10),
                Constraint::Length(10),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" 🎯 Transfer Targets "),
        );
        frame.render_widget(table, chunks[1]);
    }

    fn render_scouting(&self, frame: &mut Frame, area: Rect) {
        let regions = [
            ("Europe", 5),
            ("South America", 3),
            ("Africa", 2),
            ("Asia", 1),
            ("North America", 2),
        ];

        let mut lines: Vec<Line> = regions
            .iter()
            .map(|(region, scouts)| {
                let progress = std::iter::repeat_n("▓", *scouts)
                    .chain(std::iter::repeat_n("░", 5 - scouts))
                    .collect::<String>();
                Line::from(Span::styled(
                    format!(
                        "  {}: [{}] {} / 5 scouts assigned",
                        region, progress, scouts
                    ),
                    Style::default(),
                ))
            })
            .collect();

        let mut content = vec![
            Line::from(Span::styled(
                "  🌍 Scouting Network",
                Style::default().bold().fg(Color::Cyan),
            )),
            Line::from(""),
        ];
        content.append(&mut lines);
        content.push(Line::from(""));
        content.push(Line::from(Span::from(
            "  Recent reports: 2 new wonderkids identified in Brazil",
        )));
        content.push(Line::from(Span::styled(
            "  Next assignment: South American U20 Championship (2 weeks)",
            Style::default().fg(Color::Yellow),
        )));

        let scouting = Paragraph::new(Text::from(content))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" 🔍 Scouting Overview "),
            )
            .style(Style::default());
        frame.render_widget(scouting, area);
    }

    fn render_match(&self, frame: &mut Frame, area: Rect) {
        let chunks = if self.flash_event.is_some() {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(6), // Scoreboard (increased from 5 to 6)
                    Constraint::Length(3), // Flash banner!
                    Constraint::Min(1),    // Match content
                ])
                .split(area)
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(6), // Scoreboard (increased from 5 to 6)
                    Constraint::Min(1),    // Match content
                ])
                .split(area)
        };

        let formatted_minute = protocol::format_match_minute(self.match_minute);
        let live_dot = if self.match_minute > 0 && self.match_minute < 135 {
            "● LIVE"
        } else {
            "● FT"
        };
        
        let title_str = format!(
            " ⏱️ {} | {} | 📊 Match Centre ",
            formatted_minute,
            live_dot
        );

        // Scoreboard Outer Block
        let scoreboard_block = Block::default()
            .borders(Borders::ALL)
            .title(title_str);
        let scoreboard_inner = scoreboard_block.inner(chunks[0]);
        frame.render_widget(scoreboard_block, chunks[0]);

        // Scoreboard 3-Column Layout
        let scoreboard_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(40), // Home Team
                Constraint::Percentage(20), // Score
                Constraint::Percentage(40), // Away Team
            ])
            .split(scoreboard_inner);

        // Home Team Column
        let home_text = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("🛡️  ", Style::default().fg(Color::Green)),
                Span::styled(
                    "RUSTINGTON UNITED",
                    Style::default().bold().fg(Color::Green)
                ),
            ]).alignment(Alignment::Right),
            Line::from(Span::styled(
                "Home Team",
                Style::default().fg(Color::DarkGray)
            )).alignment(Alignment::Right),
        ];
        frame.render_widget(Paragraph::new(home_text), scoreboard_chunks[0]);

        // Center Score Column (using big block score)
        let big_score = get_big_score(self.match_score[0], self.match_score[1]);
        let score_text = vec![
            Line::from(Span::styled(&big_score[0], Style::default().bold().fg(Color::Yellow))).alignment(Alignment::Center),
            Line::from(Span::styled(&big_score[1], Style::default().bold().fg(Color::Yellow))).alignment(Alignment::Center),
            Line::from(Span::styled(&big_score[2], Style::default().bold().fg(Color::Yellow))).alignment(Alignment::Center),
        ];
        frame.render_widget(Paragraph::new(score_text), scoreboard_chunks[1]);

        // Away Team Column
        let away_text = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "FC TERMINAL",
                    Style::default().bold().fg(Color::Yellow)
                ),
                Span::styled("  💻", Style::default().fg(Color::Yellow)),
            ]).alignment(Alignment::Left),
            Line::from(Span::styled(
                "Away Team",
                Style::default().fg(Color::DarkGray)
            )).alignment(Alignment::Left),
        ];
        frame.render_widget(Paragraph::new(away_text), scoreboard_chunks[2]);

        // Render flash banner if active
        if let Some((ev_type, ref msg, ticks)) = self.flash_event {
            let is_flash_on = ticks % 2 == 0;
            let style = match ev_type {
                protocol::EventType::Goal | protocol::EventType::PenaltyGoal => {
                    if is_flash_on {
                        Style::default().fg(Color::Black).bg(Color::Green).bold()
                    } else {
                        Style::default().fg(Color::Green).bg(Color::Black).bold()
                    }
                }
                protocol::EventType::YellowCard => {
                    if is_flash_on {
                        Style::default().fg(Color::Black).bg(Color::Yellow).bold()
                    } else {
                        Style::default().fg(Color::Yellow).bg(Color::Black).bold()
                    }
                }
                protocol::EventType::RedCard => {
                    if is_flash_on {
                        Style::default().fg(Color::White).bg(Color::Red).bold()
                    } else {
                        Style::default().fg(Color::Red).bg(Color::Black).bold()
                    }
                }
                protocol::EventType::Save | protocol::EventType::PenaltySave => {
                    if is_flash_on {
                        Style::default().fg(Color::Black).bg(Color::Cyan).bold()
                    } else {
                        Style::default().fg(Color::Cyan).bg(Color::Black).bold()
                    }
                }
                _ => {
                    if is_flash_on {
                        Style::default().fg(Color::Black).bg(Color::Magenta).bold()
                    } else {
                        Style::default().fg(Color::Magenta).bg(Color::Black).bold()
                    }
                }
            };

            let flash_banner = Paragraph::new(Line::from(msg.as_str()).alignment(Alignment::Center))
                .block(Block::default().borders(Borders::ALL).style(style))
                .style(style);

            frame.render_widget(flash_banner, chunks[1]);
        }

        let match_content_area = if self.flash_event.is_some() { chunks[2] } else { chunks[1] };

        // Split match content horizontally (70% Commentary, 30% Stats & Graphs)
        let match_panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(match_content_area);

        // Match log
        let total_entries = self.match_log.len();
        let visible_rows = (match_panes[0].height as usize).saturating_sub(4); // subtract borders and header

        // Clamp scroll_offset to the maximum possible scroll
        let max_scroll = total_entries.saturating_sub(visible_rows);
        let scroll_offset = self.scroll_offset.min(max_scroll);

        let start_idx = total_entries.saturating_sub(visible_rows + scroll_offset);
        let end_idx = (start_idx + visible_rows).min(total_entries);

        let visible_entries = &self.match_log[start_idx..end_idx];

        let mut rows = Vec::new();
        for entry in visible_entries {
            // Determine styling based on event type
            let (style, bg_color, icon) = if let Some(ev_type) = entry.event_type {
                match ev_type {
                    protocol::EventType::Goal | protocol::EventType::PenaltyGoal => {
                        (Style::default().bold().fg(Color::Rgb(255, 255, 255)), Color::Rgb(30, 80, 30), "⚽ GOAL! ")
                    }
                    protocol::EventType::YellowCard => {
                        (Style::default().bold().fg(Color::Rgb(0, 0, 0)), Color::Rgb(220, 180, 0), "🟨 CARD ")
                    }
                    protocol::EventType::RedCard => {
                        (Style::default().bold().fg(Color::Rgb(255, 255, 255)), Color::Rgb(180, 20, 20), "🟥 SENT OFF! ")
                    }
                    protocol::EventType::Save | protocol::EventType::PenaltySave => {
                        (Style::default().bold().fg(Color::Rgb(255, 255, 255)), Color::Rgb(20, 80, 100), "🧤 SAVE! ")
                    }
                    protocol::EventType::Shot | protocol::EventType::ShotOnTarget => {
                        (Style::default().bold().fg(Color::Rgb(255, 220, 100)), Color::Reset, "🎯 SHOT ")
                    }
                    protocol::EventType::Foul => {
                        (Style::default().fg(Color::Rgb(255, 140, 50)), Color::Reset, "⚠️ FOUL ")
                    }
                    protocol::EventType::Corner => {
                        (Style::default().bold().fg(Color::Rgb(100, 200, 255)), Color::Reset, "🚩 CORNER ")
                    }
                    protocol::EventType::FreeKick => {
                        (Style::default().bold().fg(Color::Rgb(100, 200, 255)), Color::Reset, "📐 FREEKICK ")
                    }
                    protocol::EventType::HalfTime | protocol::EventType::FullTime | protocol::EventType::ExtraTimeStart | protocol::EventType::PenaltyShootoutStart => {
                        (Style::default().bold().fg(Color::Rgb(255, 255, 255)), Color::Rgb(50, 50, 120), "🏁 ")
                    }
                    protocol::EventType::Pass => {
                        (Style::default().fg(Color::Rgb(140, 200, 140)), Color::Reset, "👟 ")
                    }
                    protocol::EventType::Tackle => {
                        (Style::default().fg(Color::Rgb(220, 180, 140)), Color::Reset, "🛑 ")
                    }
                    protocol::EventType::Dribble => {
                        (Style::default().fg(Color::Rgb(200, 140, 220)), Color::Reset, "🏃 ")
                    }
                    protocol::EventType::Interception => {
                        (Style::default().fg(Color::Rgb(140, 220, 200)), Color::Reset, "⚡ ")
                    }
                    protocol::EventType::Block => {
                        (Style::default().fg(Color::Rgb(220, 140, 140)), Color::Reset, "🧱 ")
                    }
                    _ => {
                        match entry.team {
                            Some(Team::Home) => (Style::default().fg(Color::Rgb(140, 220, 140)), Color::Reset, "• ") ,
                            Some(Team::Away) => (Style::default().fg(Color::Rgb(240, 240, 140)), Color::Reset, "• "),
                            None => (Style::default().fg(Color::Rgb(180, 240, 240)), Color::Reset, "• "),
                        }
                    }
                }
            } else {
                (Style::default().fg(Color::Gray), Color::Reset, "")
            };

            let formatted_text = format!("{}{}", icon, entry.text);

            let row = match entry.team {
                Some(Team::Home) => {
                    Row::new(vec![
                        Cell::from(Line::from(Span::styled(formatted_text, style)).alignment(Alignment::Right)).style(Style::default().bg(bg_color)),
                        Cell::from(Line::from(Span::styled(&entry.minute_str, Style::default().bold().fg(Color::White))).alignment(Alignment::Center)),
                        Cell::from(""),
                    ])
                }
                Some(Team::Away) => {
                    Row::new(vec![
                        Cell::from(""),
                        Cell::from(Line::from(Span::styled(&entry.minute_str, Style::default().bold().fg(Color::White))).alignment(Alignment::Center)),
                        Cell::from(Line::from(Span::styled(formatted_text, style)).alignment(Alignment::Left)).style(Style::default().bg(bg_color)),
                    ])
                }
                None => {
                    Row::new(vec![
                        Cell::from(Line::from(Span::styled(formatted_text.clone(), style)).alignment(Alignment::Right)).style(Style::default().bg(bg_color)),
                        Cell::from(Line::from(Span::styled(&entry.minute_str, Style::default().bold().fg(Color::White))).alignment(Alignment::Center)),
                        Cell::from(Line::from(Span::styled(formatted_text, style)).alignment(Alignment::Left)).style(Style::default().bg(bg_color)),
                    ])
                }
            };
            rows.push(row);
        }

        let widths = [
            Constraint::Percentage(45),
            Constraint::Length(10),
            Constraint::Percentage(45),
        ];

        let header = Row::new(vec![
            Cell::from(Line::from("🛡️  RUSTINGTON UNITED (HOME)").alignment(Alignment::Right).bold().fg(Color::Green)),
            Cell::from(Line::from("TIME").alignment(Alignment::Center).bold().fg(Color::Cyan)),
            Cell::from(Line::from("FC TERMINAL (AWAY)  💻").alignment(Alignment::Left).bold().fg(Color::Yellow)),
        ]).style(Style::default().bg(Color::Rgb(20, 20, 30)).add_modifier(Modifier::UNDERLINED));

        let scroll_title = if max_scroll > 0 {
            format!(" 📜 Commentary (Scroll: {}/{} - Use Up/Down to scroll) ", scroll_offset, max_scroll)
        } else {
            " 📜 Commentary ".to_string()
        };

        let table = Table::new(rows, widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(scroll_title),
            )
            .style(Style::default());

        frame.render_widget(table, match_panes[0]);

        // Render Stats & Graphs in match_panes[1]
        let stats_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(13), // ⚽ Live Pitch (increased height to fit players and GKs!)
                Constraint::Length(12), // Graphs
                Constraint::Min(1),     // Table
            ])
            .split(match_panes[1]);

        // Render Live Pitch Tracker
        let pitch_block = Block::default()
            .borders(Borders::ALL)
            .title(" ⚽ Live 2D Pitch Tracker ");
        let pitch_inner = pitch_block.inner(stats_chunks[0]);
        frame.render_widget(pitch_block, stats_chunks[0]);

        let pitch_w = pitch_inner.width as usize;
        let pitch_h = pitch_inner.height as usize;

        // Calculate Possession
        let home_passes = self.match_stats.home.passes;
        let away_passes = self.match_stats.away.passes;
        let total_passes = home_passes + away_passes;
        let (home_pos, away_pos) = if total_passes > 0 {
            let hp = (home_passes as f32 / total_passes as f32 * 100.0).round() as u32;
            (hp, 100 - hp)
        } else {
            (50, 50)
        };

        if pitch_w >= 24 && pitch_h >= 9 {
            let field_w = pitch_w.saturating_sub(4);
            let field_h = pitch_h.saturating_sub(1); // leave 1 row for labels

            // Interpolate ball position
            let (prev_bx, prev_by) = self.ball_prev_pos;
            let (target_bx, target_by) = get_ball_target_pos(self.last_event);
            let ball_fx = prev_bx + (target_bx - prev_bx) * self.ball_progress;
            let ball_fy = prev_by + (target_by - prev_by) * self.ball_progress;

            // Map ball to character grid
            let ball_col = ((ball_fx * (field_w as f32 - 1.0)).round() as usize).min(field_w - 1);
            let ball_row = ((ball_fy * (field_h as f32 - 1.0)).round() as usize).min(field_h - 1);

            // Map referee position (follows ball slightly offset towards center)
            let ref_fx = ball_fx * 0.4 + 0.3; // stays closer to center midfield
            let ref_fy = ball_fy * 0.4 + 0.3;
            let ref_col = ((ref_fx * (field_w as f32 - 1.0)).round() as usize).min(field_w - 1);
            let ref_row = ((ref_fy * (field_h as f32 - 1.0)).round() as usize).min(field_h - 1);

            // Build grid character buffer
            let mut grid = vec![vec![(' ', Style::default()); field_w]; field_h];

            // Draw field markings (boundaries, center line, center circle, goals)
            let mid_col = field_w / 2;
            for r in 0..field_h {
                for c in 0..field_w {
                    let mut style = Style::default().fg(Color::DarkGray);
                    let mut ch = ' ';

                    // Center line
                    if c == mid_col {
                        ch = '│';
                    }

                    // Center circle markings
                    let dy = (r as f32 - field_h as f32 / 2.0) * 1.8;
                    let dx = c as f32 - mid_col as f32;
                    let dist = (dx*dx + dy*dy).sqrt();
                    if dist >= 2.8 && dist <= 3.8 {
                        ch = 'o';
                    }

                    // Boundaries
                    if r == 0 {
                        ch = '─';
                    } else if r == field_h - 1 {
                        ch = '─';
                    }

                    if c == 0 {
                        if r >= field_h / 3 && r <= (2 * field_h) / 3 {
                            ch = '║'; // Home goal
                            style = style.fg(Color::Green);
                        } else {
                            ch = '│';
                        }
                    } else if c == field_w - 1 {
                        if r >= field_h / 3 && r <= (2 * field_h) / 3 {
                            ch = '║'; // Away goal
                            style = style.fg(Color::Yellow);
                        } else {
                            ch = '│';
                        }
                    }

                    grid[r][c] = (ch, style);
                }
            }

            // Draw Home Players (Green shapes)
            // Shapes: Triangles ▲ for ST, Circles ● for Midfielders, Squares ■ for Defenders
            for idx in 0..11u16 {
                let (fx, fy) = get_player_nominal_pos(Team::Home, idx);
                let col = ((fx * (field_w as f32 - 1.0)).round() as usize).min(field_w - 1);
                let row = ((fy * (field_h as f32 - 1.0)).round() as usize).min(field_h - 1);

                let (ch, style) = if idx == 0 {
                    ('◆', Style::default().bold().fg(Color::Green)) // GK: Diamond ◆
                } else if idx >= 9 {
                    ('▲', Style::default().bold().fg(Color::Green)) // ST: Triangle ▲
                } else if idx >= 5 {
                    ('●', Style::default().fg(Color::Green))       // MID: Circle ●
                } else {
                    ('■', Style::default().fg(Color::Green))       // DEF: Square ■
                };

                grid[row][col] = (ch, style);
            }

            // Draw Away Players (Yellow shapes)
            for idx in 0..11u16 {
                let (fx, fy) = get_player_nominal_pos(Team::Away, idx);
                let col = ((fx * (field_w as f32 - 1.0)).round() as usize).min(field_w - 1);
                let row = ((fy * (field_h as f32 - 1.0)).round() as usize).min(field_h - 1);

                let (ch, style) = if idx == 0 {
                    ('◆', Style::default().bold().fg(Color::Yellow)) // GK: Diamond ◆
                } else if idx >= 9 {
                    ('▲', Style::default().bold().fg(Color::Yellow)) // ST: Triangle ▲
                } else if idx >= 5 {
                    ('●', Style::default().fg(Color::Yellow))       // MID: Circle ●
                } else {
                    ('■', Style::default().fg(Color::Yellow))       // DEF: Square ■
                };

                grid[row][col] = (ch, style);
            }

            // Draw Referee (White Star ★)
            grid[ref_row][ref_col] = ('★', Style::default().bold().fg(Color::White));

            // Draw Ball (⚽) - overwrites player/ref if overlapping
            grid[ball_row][ball_col] = ('⚽', Style::default().bold().fg(Color::Red));

            // Convert character buffer to Paragraph Lines
            let mut pitch_lines = Vec::new();
            for r in 0..field_h {
                let mut spans = Vec::new();
                spans.push(Span::raw("  "));
                for c in 0..field_w {
                    let (ch, style) = grid[r][c];
                    spans.push(Span::styled(ch.to_string(), style));
                }
                pitch_lines.push(Line::from(spans));
            }

            // Legend / Labels line
            let mut legend = Vec::new();
            legend.push(Span::raw("  "));
            legend.push(Span::styled("■ Def ", Style::default().fg(Color::DarkGray)));
            legend.push(Span::styled("● Mid ", Style::default().fg(Color::DarkGray)));
            legend.push(Span::styled("▲ Att ", Style::default().fg(Color::DarkGray)));
            legend.push(Span::styled("◆ GK ", Style::default().fg(Color::DarkGray)));
            legend.push(Span::styled("★ Ref ", Style::default().fg(Color::White)));
            legend.push(Span::styled("⚽ Ball", Style::default().fg(Color::Red)));

            let spaces_needed = pitch_w.saturating_sub(44);
            legend.push(Span::raw(" ".repeat(spaces_needed)));
            legend.push(Span::styled("Home 🛡️", Style::default().bold().fg(Color::Green)));
            legend.push(Span::raw(" vs "));
            legend.push(Span::styled("💻 Away", Style::default().bold().fg(Color::Yellow)));

            pitch_lines.push(Line::from(legend));

            frame.render_widget(Paragraph::new(pitch_lines), pitch_inner);
        } else {
            let fallback = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled("⚽ Match in Progress", Style::default().bold().fg(Color::Green))).alignment(Alignment::Center),
            ]);
            frame.render_widget(fallback, pitch_inner);
        }

        let make_fancy_bar = |label: &str, home_val: u32, away_val: u32| -> Vec<Line> {
            let total = home_val + away_val;

            let bar_width = 10;
            let home_chars = if total > 0 {
                ((home_val as f32 / total as f32) * bar_width as f32).round() as usize
            } else {
                bar_width / 2
            };
            let away_chars = bar_width - home_chars;

            let home_bar = "█".repeat(home_chars);
            let away_bar = "░".repeat(away_chars);

            vec![
                Line::from(vec![
                    Span::styled(format!(" {:<10}", label), Style::default().bold().fg(Color::White)),
                    Span::styled(format!(" {:>3}", home_val), Style::default().fg(Color::Green)),
                    Span::styled(" [", Style::default().fg(Color::DarkGray)),
                    Span::styled(home_bar, Style::default().fg(Color::Green)),
                    Span::styled(away_bar, Style::default().fg(Color::Yellow)),
                    Span::styled("] ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{:<3}", away_val), Style::default().fg(Color::Yellow)),
                ])
            ]
        };

        let mut graph_lines = Vec::new();
        graph_lines.push(Line::from(""));
        graph_lines.extend(make_fancy_bar("Possession", home_pos, away_pos));
        graph_lines.push(Line::from(""));
        graph_lines.extend(make_fancy_bar("Shots", self.match_stats.home.shots, self.match_stats.away.shots));
        graph_lines.push(Line::from(""));
        graph_lines.extend(make_fancy_bar("On Target", self.match_stats.home.shots_on_target, self.match_stats.away.shots_on_target));
        graph_lines.push(Line::from(""));
        graph_lines.extend(make_fancy_bar("Passes", self.match_stats.home.passes, self.match_stats.away.passes));
        graph_lines.push(Line::from(""));
        graph_lines.extend(make_fancy_bar("Tackles", self.match_stats.home.tackles, self.match_stats.away.tackles));

        let graphs_widget = Paragraph::new(graph_lines)
            .block(Block::default().borders(Borders::ALL).title(" 📈 Match Graphs "));
        frame.render_widget(graphs_widget, stats_chunks[1]);

        let mut stats_rows = Vec::new();
        let raw_stats = vec![
            ("⚽ Goals", self.match_score[0].to_string(), self.match_score[1].to_string(), true),
            ("🎯 Shots", self.match_stats.home.shots.to_string(), self.match_stats.away.shots.to_string(), false),
            ("On Target", self.match_stats.home.shots_on_target.to_string(), self.match_stats.away.shots_on_target.to_string(), false),
            ("🧤 Saves", self.match_stats.home.saves.to_string(), self.match_stats.away.saves.to_string(), false),
            ("👟 Passes", self.match_stats.home.passes.to_string(), self.match_stats.away.passes.to_string(), false),
            ("🛑 Tackles", self.match_stats.home.tackles.to_string(), self.match_stats.away.tackles.to_string(), false),
            ("🏃 Dribbles", self.match_stats.home.dribbles.to_string(), self.match_stats.away.dribbles.to_string(), false),
            ("⚡ Interceptions", self.match_stats.home.interceptions.to_string(), self.match_stats.away.interceptions.to_string(), false),
            ("🧱 Blocks", self.match_stats.home.blocks.to_string(), self.match_stats.away.blocks.to_string(), false),
            ("⚠️ Fouls", self.match_stats.home.fouls.to_string(), self.match_stats.away.fouls.to_string(), false),
            ("🟨 Yellow Cards", self.match_stats.home.yellow_cards.to_string(), self.match_stats.away.yellow_cards.to_string(), false),
            ("🟥 Red Cards", self.match_stats.home.red_cards.to_string(), self.match_stats.away.red_cards.to_string(), false),
        ];

        for (i, (label, home_val, away_val, is_bold)) in raw_stats.into_iter().enumerate() {
            let bg_color = if i % 2 == 0 {
                Color::Rgb(24, 24, 24)
            } else {
                Color::Reset
            };

            let label_style = if is_bold {
                Style::default().bold().fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
            };

            let row = Row::new(vec![
                Cell::from(label).style(label_style),
                Cell::from(home_val).style(Style::default().bold().fg(Color::Green)),
                Cell::from(away_val).style(Style::default().bold().fg(Color::Yellow)),
            ])
            .style(Style::default().bg(bg_color));
            stats_rows.push(row);
        }

        let stats_table = Table::new(
            stats_rows,
            [
                Constraint::Percentage(50),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ],
        )
        .header(
            Row::new(vec![
                Cell::from("Stat").bold().fg(Color::White),
                Cell::from("Home").bold().fg(Color::Green),
                Cell::from("Away").bold().fg(Color::Yellow),
            ])
            .style(Style::default().add_modifier(Modifier::UNDERLINED)),
        )
        .block(Block::default().borders(Borders::ALL).title(" 📊 Detailed Stats "));

        frame.render_widget(stats_table, stats_chunks[2]);
    }

    fn render_status_bar(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(40), // Navigation
                Constraint::Percentage(60), // Tactical Controls
            ])
            .split(area);

        // 1. Navigation Block
        let nav_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .style(Style::default().bg(Color::Rgb(10, 20, 30)));
        let nav_text = Line::from(vec![
            Span::styled(" 🎮 NAVIGATE: ", Style::default().bold().fg(Color::Cyan)),
            Span::styled("Tab/Arrows", Style::default().bold().fg(Color::White)),
            Span::styled(" to switch tabs  |  ", Style::default().fg(Color::Gray)),
            Span::styled("q", Style::default().bold().fg(Color::Red)),
            Span::styled(" to quit", Style::default().fg(Color::Gray)),
        ]).alignment(Alignment::Center);
        frame.render_widget(Paragraph::new(nav_text).block(nav_block), chunks[0]);

        // 2. Tactical Controls Block
        let tact_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Magenta))
            .style(Style::default().bg(Color::Rgb(30, 10, 30)));
        let tact_text = Line::from(vec![
            Span::styled(" 📋 TACTICS: ", Style::default().bold().fg(Color::Magenta)),
            Span::styled("1-3", Style::default().bold().fg(Color::Yellow)),
            Span::styled(" Mentality  |  ", Style::default().fg(Color::Gray)),
            Span::styled("4-6", Style::default().bold().fg(Color::Yellow)),
            Span::styled(" Pressing  |  ", Style::default().fg(Color::Gray)),
            Span::styled("7-9", Style::default().bold().fg(Color::Yellow)),
            Span::styled(" Tempo", Style::default().fg(Color::Gray)),
        ]).alignment(Alignment::Center);
        frame.render_widget(Paragraph::new(tact_text).block(tact_block), chunks[1]);
    }

    fn send_tactical(&mut self, cmd_type: CommandType, team: u8, value: u8) {
        if let Some(ref mut client) = self.server_client {
            let label = format!("{:?}={}", cmd_type, value);
            match client.send_command(cmd_type, team, value) {
                Ok(_) => {
                    self.messages.insert(0, format!("📤 Sent: {}", label));
                    self.messages.truncate(20);
                }
                Err(e) => {
                    self.messages.insert(0, format!("⚠️ Command failed: {}", e));
                }
            }
        }
    }

    fn process_server_event(&mut self, event: protocol::EventPacket) {
        if !self.match_started {
            return;
        }
        // Safely unpack fields from the packed struct to avoid UB on u32/u16/f32
        let (_mid, ev_type, ev_team, ev_player_index, minute, val) = event.unpack();

        let team_name = match ev_team {
            protocol::Team::Home => "Rustington",
            protocol::Team::Away => "FC Terminal",
        };

        // Update match minute
        self.match_minute = minute;
        let formatted_min = protocol::format_match_minute(self.match_minute);

        // Update score for goals
        if ev_type == protocol::EventType::Goal {
            if ev_team == protocol::Team::Home {
                self.match_score[0] = self.match_score[0].wrapping_add(1);
            } else {
                self.match_score[1] = self.match_score[1].wrapping_add(1);
            }
        }

        // Update stats
        update_stats_from_event(&mut self.match_stats, ev_type, ev_team);

        let player_name = get_player_name(ev_team, ev_player_index, &self.players);

        // Trigger flashes for major events
        let flash_msg = match ev_type {
            protocol::EventType::Goal => {
                Some(format!("⚽ GOAL!!! {} scores for {}! ⚽", player_name, team_name))
            }
            protocol::EventType::PenaltyGoal => {
                Some(format!("⚽ PENALTY GOAL!!! {} converts! ⚽", player_name))
            }
            protocol::EventType::Save => {
                Some(format!("🧤 OUTSTANDING SAVE by {}! 🧤", player_name))
            }
            protocol::EventType::PenaltySave => {
                Some(format!("🧤 PENALTY SAVED!!! {} makes a heroic save! 🧤", player_name))
            }
            protocol::EventType::YellowCard => {
                Some(format!("🟨 YELLOW CARD shown to {}! 🟨", player_name))
            }
            protocol::EventType::RedCard => {
                Some(format!("🟥 RED CARD!!! {} is sent off! 🟥", player_name))
            }
            protocol::EventType::PenaltyMiss => {
                Some(format!("❌ PENALTY MISSED by {}! ❌", team_name))
            }
            protocol::EventType::HalfTime => {
                Some("🏁 HALF TIME! Teams head to the dressing room. 🏁".to_string())
            }
            protocol::EventType::FullTime => {
                Some("🏆 FULL TIME! The final whistle blows! 🏆".to_string())
            }
            protocol::EventType::ExtraTimeStart => {
                Some("⏰ EXTRA TIME STARTS! ⏰".to_string())
            }
            protocol::EventType::PenaltyShootoutStart => {
                Some("🧤 PENALTY SHOOTOUT STARTS! 🧤".to_string())
            }
            _ => None,
        };

        if let Some(msg) = flash_msg {
            self.flash_event = Some((ev_type, msg, 25)); // flash for 25 frames (approx 2.5 seconds)
        }

        // Update ball animation and last event
        let (old_bx, old_by) = get_ball_target_pos(self.last_event);
        self.ball_prev_pos = (old_bx, old_by);
        self.last_event = Some((ev_type, ev_team, ev_player_index));
        self.ball_progress = 0.0;

        // Play sound effects based on event type
        match ev_type {
            protocol::EventType::Goal | protocol::EventType::PenaltyGoal => {
                play_sound("tui/sounds/cheer.wav");
            }
            protocol::EventType::Kickoff |
            protocol::EventType::HalfTime |
            protocol::EventType::FullTime |
            protocol::EventType::ExtraTimeStart |
            protocol::EventType::PenaltyShootoutStart |
            protocol::EventType::Foul |
            protocol::EventType::YellowCard |
            protocol::EventType::RedCard => {
                play_sound("tui/sounds/whistle.wav");
            }
            protocol::EventType::Pass |
            protocol::EventType::Tackle |
            protocol::EventType::Dribble |
            protocol::EventType::Interception |
            protocol::EventType::Block => {
                play_sound("tui/sounds/clap.wav");
            }
            protocol::EventType::Save | protocol::EventType::PenaltySave => {
                play_sound("tui/sounds/cheer.wav");
            }
            _ => {}
        }

        // Add to match log
        let text = protocol::generate_exciting_commentary(ev_type, ev_team, ev_player_index, minute, val);

        let event_team = match ev_type {
            protocol::EventType::Kickoff |
            protocol::EventType::HalfTime |
            protocol::EventType::FullTime |
            protocol::EventType::ExtraTimeStart |
            protocol::EventType::ExtraTimeHalfTime |
            protocol::EventType::PenaltyShootoutStart => None,
            _ => Some(ev_team),
        };

        let is_highlight = match ev_type {
            protocol::EventType::Goal |
            protocol::EventType::PenaltyGoal |
            protocol::EventType::Save |
            protocol::EventType::PenaltySave |
            protocol::EventType::YellowCard |
            protocol::EventType::RedCard |
            protocol::EventType::PenaltyMiss |
            protocol::EventType::HalfTime |
            protocol::EventType::FullTime => true,
            _ => false,
        };

        if is_highlight {
            self.match_highlights.push(MatchLogEntry {
                minute_str: formatted_min.clone(),
                team: event_team,
                text: text.clone(),
                event_type: Some(ev_type),
            });
        }

        self.match_log.push(MatchLogEntry {
            minute_str: formatted_min,
            team: event_team,
            text,
            event_type: Some(ev_type),
        });

        if ev_type == protocol::EventType::FullTime {
            self.match_finished = true;
        }

        if self.match_log.len() > 100 {
            self.match_log.remove(0);
        }
    }
}

// ── Data Generators ─────────────────────────────────────────────

fn generate_squad() -> Vec<Player> {
    vec![
        Player {
            name: "Rusty McSave".into(),
            age: 32,
            pos: "GK".into(),
            ovr: 82,
            pot: 82,
            nation: "Scotland".into(),
            contract: "2027".into(),
            value: "£2.5M".into(),
        },
        Player {
            name: "Lex Byte".into(),
            age: 26,
            pos: "LB".into(),
            ovr: 76,
            pot: 78,
            nation: "Germany".into(),
            contract: "2028".into(),
            value: "£4.2M".into(),
        },
        Player {
            name: "Corey Heap".into(),
            age: 24,
            pos: "CB".into(),
            ovr: 80,
            pot: 86,
            nation: "England".into(),
            contract: "2030".into(),
            value: "£8.5M".into(),
        },
        Player {
            name: "Sean Stack".into(),
            age: 29,
            pos: "CB".into(),
            ovr: 78,
            pot: 78,
            nation: "Ireland".into(),
            contract: "2027".into(),
            value: "£3.1M".into(),
        },
        Player {
            name: "Max Alloc".into(),
            age: 23,
            pos: "RB".into(),
            ovr: 75,
            pot: 84,
            nation: "France".into(),
            contract: "2029".into(),
            value: "£5.8M".into(),
        },
        Player {
            name: "Tommy Mutex".into(),
            age: 27,
            pos: "CM".into(),
            ovr: 81,
            pot: 83,
            nation: "England".into(),
            contract: "2028".into(),
            value: "£7.2M".into(),
        },
        Player {
            name: "Rusty Channel".into(),
            age: 22,
            pos: "CM".into(),
            ovr: 77,
            pot: 88,
            nation: "Spain".into(),
            contract: "2030".into(),
            value: "£10.0M".into(),
        },
        Player {
            name: "Eddie Promise".into(),
            age: 30,
            pos: "LM".into(),
            ovr: 79,
            pot: 79,
            nation: "Portugal".into(),
            contract: "2026".into(),
            value: "£3.8M".into(),
        },
        Player {
            name: "Ray Arc".into(),
            age: 28,
            pos: "RM".into(),
            ovr: 80,
            pot: 80,
            nation: "Brazil".into(),
            contract: "2027".into(),
            value: "£6.0M".into(),
        },
        Player {
            name: "Ferris Enum".into(),
            age: 31,
            pos: "ST".into(),
            ovr: 85,
            pot: 85,
            nation: "Argentina".into(),
            contract: "2027".into(),
            value: "£15.0M".into(),
        },
        Player {
            name: "Will Crash".into(),
            age: 20,
            pos: "ST".into(),
            ovr: 70,
            pot: 92,
            nation: "England".into(),
            contract: "2031".into(),
            value: "£12.0M".into(),
        },
        Player {
            name: "Ben Chmark".into(),
            age: 25,
            pos: "CM".into(),
            ovr: 74,
            pot: 80,
            nation: "Netherlands".into(),
            contract: "2028".into(),
            value: "£4.0M".into(),
        },
        Player {
            name: "Ollie Fset".into(),
            age: 33,
            pos: "CB".into(),
            ovr: 73,
            pot: 73,
            nation: "Wales".into(),
            contract: "2026".into(),
            value: "£1.2M".into(),
        },
        Player {
            name: "Pat Ter".into(),
            age: 21,
            pos: "LB".into(),
            ovr: 68,
            pot: 83,
            nation: "Belgium".into(),
            contract: "2030".into(),
            value: "£3.5M".into(),
        },
        Player {
            name: "Sid Effect".into(),
            age: 27,
            pos: "ST".into(),
            ovr: 76,
            pot: 78,
            nation: "Nigeria".into(),
            contract: "2028".into(),
            value: "£5.0M".into(),
        },
        Player {
            name: "Niles Serde".into(),
            age: 24,
            pos: "GK".into(),
            ovr: 72,
            pot: 85,
            nation: "Canada".into(),
            contract: "2029".into(),
            value: "£6.0M".into(),
        },
        Player {
            name: "Jamie Macro".into(),
            age: 26,
            pos: "RM".into(),
            ovr: 75,
            pot: 76,
            nation: "Scotland".into(),
            contract: "2027".into(),
            value: "£3.0M".into(),
        },
        Player {
            name: "Tess Data".into(),
            age: 22,
            pos: "LM".into(),
            ovr: 71,
            pot: 86,
            nation: "Sweden".into(),
            contract: "2031".into(),
            value: "£7.5M".into(),
        },
    ]
}

fn generate_table() -> Vec<LeagueStanding> {
    vec![
        LeagueStanding {
            pos: 1,
            team: "Manchester red".into(),
            played: 14,
            won: 10,
            drawn: 3,
            lost: 1,
            gf: 32,
            ga: 12,
            gd: 20,
            pts: 33,
        },
        LeagueStanding {
            pos: 2,
            team: "Rustington United".into(),
            played: 14,
            won: 9,
            drawn: 4,
            lost: 1,
            gf: 28,
            ga: 14,
            gd: 14,
            pts: 31,
        },
        LeagueStanding {
            pos: 3,
            team: "FC Terminal".into(),
            played: 14,
            won: 9,
            drawn: 2,
            lost: 3,
            gf: 26,
            ga: 15,
            gd: 11,
            pts: 29,
        },
        LeagueStanding {
            pos: 4,
            team: "Arsenal XI".into(),
            played: 14,
            won: 8,
            drawn: 4,
            lost: 2,
            gf: 24,
            ga: 13,
            gd: 11,
            pts: 28,
        },
        LeagueStanding {
            pos: 5,
            team: "Chelsea Blues".into(),
            played: 14,
            won: 7,
            drawn: 3,
            lost: 4,
            gf: 22,
            ga: 16,
            gd: 6,
            pts: 24,
        },
        LeagueStanding {
            pos: 6,
            team: "Tottenham Hotspur".into(),
            played: 14,
            won: 6,
            drawn: 5,
            lost: 3,
            gf: 20,
            ga: 17,
            gd: 3,
            pts: 23,
        },
        LeagueStanding {
            pos: 7,
            team: "Liverpool Reds".into(),
            played: 14,
            won: 6,
            drawn: 3,
            lost: 5,
            gf: 21,
            ga: 19,
            gd: 2,
            pts: 21,
        },
        LeagueStanding {
            pos: 8,
            team: "Newcastle Magpies".into(),
            played: 14,
            won: 5,
            drawn: 4,
            lost: 5,
            gf: 18,
            ga: 18,
            gd: 0,
            pts: 19,
        },
        LeagueStanding {
            pos: 9,
            team: "Aston Villains".into(),
            played: 14,
            won: 5,
            drawn: 3,
            lost: 6,
            gf: 19,
            ga: 22,
            gd: -3,
            pts: 18,
        },
        LeagueStanding {
            pos: 10,
            team: "West Ham Irons".into(),
            played: 14,
            won: 4,
            drawn: 5,
            lost: 5,
            gf: 16,
            ga: 20,
            gd: -4,
            pts: 17,
        },
        LeagueStanding {
            pos: 11,
            team: "Brighton Seagulls".into(),
            played: 14,
            won: 4,
            drawn: 4,
            lost: 6,
            gf: 15,
            ga: 21,
            gd: -6,
            pts: 16,
        },
        LeagueStanding {
            pos: 12,
            team: "Wolverhampton".into(),
            played: 14,
            won: 4,
            drawn: 3,
            lost: 7,
            gf: 14,
            ga: 23,
            gd: -9,
            pts: 15,
        },
        LeagueStanding {
            pos: 13,
            team: "Forest Town".into(),
            played: 14,
            won: 3,
            drawn: 5,
            lost: 6,
            gf: 13,
            ga: 22,
            gd: -9,
            pts: 14,
        },
        LeagueStanding {
            pos: 14,
            team: "Crystal Palace".into(),
            played: 14,
            won: 3,
            drawn: 4,
            lost: 7,
            gf: 12,
            ga: 24,
            gd: -12,
            pts: 13,
        },
        LeagueStanding {
            pos: 15,
            team: "Everton Toffees".into(),
            played: 14,
            won: 2,
            drawn: 6,
            lost: 6,
            gf: 11,
            ga: 23,
            gd: -12,
            pts: 12,
        },
        LeagueStanding {
            pos: 16,
            team: "Ipswich Town".into(),
            played: 14,
            won: 2,
            drawn: 4,
            lost: 8,
            gf: 10,
            ga: 27,
            gd: -17,
            pts: 10,
        },
    ]
}

fn generate_fixtures() -> Vec<UpcomingMatch> {
    vec![
        UpcomingMatch {
            date: "15 Jun 2026".into(),
            competition: "Premier League".into(),
            opponent: "Chelsea Blues".into(),
            venue: "Home".into(),
        },
        UpcomingMatch {
            date: "22 Jun 2026".into(),
            competition: "Premier League".into(),
            opponent: "FC Terminal".into(),
            venue: "Away".into(),
        },
        UpcomingMatch {
            date: "29 Jun 2026".into(),
            competition: "FA Cup".into(),
            opponent: "Arsenal XI".into(),
            venue: "Home".into(),
        },
        UpcomingMatch {
            date: "6 Jul 2026".into(),
            competition: "Premier League".into(),
            opponent: "Liverpool Reds".into(),
            venue: "Away".into(),
        },
        UpcomingMatch {
            date: "13 Jul 2026".into(),
            competition: "Premier League".into(),
            opponent: "Manchester red".into(),
            venue: "Home".into(),
        },
    ]
}

fn generate_transfer_targets() -> Vec<TransferTarget> {
    vec![
        TransferTarget {
            name: "Marco Reus Jr".into(),
            pos: "CM".into(),
            age: 23,
            club: "Borussia Dot".into(),
            value: "£8.5M".into(),
            interest: 85,
        },
        TransferTarget {
            name: "Pedro Flow".into(),
            pos: "ST".into(),
            age: 21,
            club: "Benfica Byte".into(),
            value: "£6.2M".into(),
            interest: 72,
        },
        TransferTarget {
            name: "Lars Binden".into(),
            pos: "CB".into(),
            age: 25,
            club: "Ajax Async".into(),
            value: "£4.8M".into(),
            interest: 65,
        },
        TransferTarget {
            name: "Kai Fetch".into(),
            pos: "LM".into(),
            age: 19,
            club: "Sporting CP".into(),
            value: "£3.5M".into(),
            interest: 55,
        },
        TransferTarget {
            name: "Diego Alloc-o".into(),
            pos: "RB".into(),
            age: 27,
            club: "Sevilla".into(),
            value: "£5.0M".into(),
            interest: 45,
        },
        TransferTarget {
            name: "Yuki Monad".into(),
            pos: "CM".into(),
            age: 24,
            club: "PSV".into(),
            value: "£2.8M".into(),
            interest: 30,
        },
    ]
}
