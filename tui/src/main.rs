use color_eyre::eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use engine::player::generate_synthetic_squad;
use engine::simulation::simulate_minutes;
use protocol::{CommandType, MatchState, TacticState, Team};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
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

enum Screen {
    Dashboard,
    Squad,
    Tactics,
    League,
    Transfers,
    Scouting,
    Match,
}

impl Screen {
    fn all() -> &'static [Screen; 7] {
        &[
            Screen::Dashboard,
            Screen::Squad,
            Screen::Tactics,
            Screen::League,
            Screen::Transfers,
            Screen::Scouting,
            Screen::Match,
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
    match_log: Vec<String>,
    match_score: [u8; 2],
    match_minute: u8,
    connected: bool,
    server_client: Option<ServerClient>,
    seq: u16,
}

impl App {
    fn new() -> Self {
        let mut app = Self {
            tab_index: 0,
            scroll_offset: 0,
            season: 2026,
            club: "Rustington United".into(),
            manager: "Player".into(),
            players: generate_squad(),
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
                // Fall back to local simulation
                app.simulate_demo_match();
            }
        }

        app
    }

    fn simulate_demo_match(&mut self) {
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
        self.match_log.push("=== Match Report ===".into());
        self.match_log.push(format!(
            "Rustington United {} - {} FC Terminal",
            result.state.score[0], result.state.score[1]
        ));
        self.match_log.push("".into());
        self.match_log.push(format!(
            "Possession: {:.0}%",
            result.state.possession * 100.0
        ));
        self.match_log.push("".into());
        self.match_log.push("--- Events ---".into());

        for ev in &result.events {
            let formatted_min = protocol::format_match_minute(ev.minute);
            let team_name = match ev.team {
                Team::Home => "Rustington",
                Team::Away => "FC Terminal",
            };
            match ev.event_type {
                protocol::EventType::Kickoff => self.match_log.push(format!("{} - Kickoff!", formatted_min)),
                protocol::EventType::Goal => {
                    self.match_log.push(format!(
                        "{} - ⚽ GOAL! {} scores for {}!",
                        formatted_min, ev.player_index, team_name
                    ));
                }
                protocol::EventType::Shot => self.match_log.push(format!(
                    "{} - Shot by player {}",
                    formatted_min, ev.player_index
                )),
                protocol::EventType::ShotOnTarget => self.match_log.push(format!(
                    "{} - Shot on target!",
                    formatted_min
                )),
                protocol::EventType::Save => {
                    self.match_log.push(format!("{} - Great save!", formatted_min))
                }
                protocol::EventType::Miss => {
                    self.match_log.push(format!("{} - Shot wide", formatted_min))
                }
                protocol::EventType::Foul => self
                    .match_log
                    .push(format!("{} - Foul committed", formatted_min)),
                protocol::EventType::Corner => self.match_log.push(format!("{} - Corner kick", formatted_min)),
                protocol::EventType::FreeKick => self.match_log.push(format!("{} - Free kick", formatted_min)),
                protocol::EventType::YellowCard => self.match_log.push(format!("{} - Yellow card for {}", formatted_min, team_name)),
                protocol::EventType::RedCard => self.match_log.push(format!("{} - 🔴 RED CARD for {}", formatted_min, team_name)),
                protocol::EventType::Substitution => self.match_log.push(format!("{} - Substitution for {}", formatted_min, team_name)),
                protocol::EventType::Injury => self.match_log.push(format!("{} - Injury to player {}", formatted_min, ev.player_index)),
                protocol::EventType::Offside => self.match_log.push(format!("{} - Offside!", formatted_min)),
                protocol::EventType::HalfTime => self.match_log.push(format!("{} - Half Time!", formatted_min)),
                protocol::EventType::FullTime => self.match_log.push(format!("{} - Full Time!", formatted_min)),
                protocol::EventType::PenaltyGoal => self.match_log.push(format!(
                    "{} - ⚽ PENALTY GOAL! {} scores for {}! Shootout score: {}",
                    formatted_min, ev.player_index, team_name, ev.value as u32
                )),
                protocol::EventType::PenaltyMiss => self.match_log.push(format!(
                    "{} - ❌ Penalty missed by {}!",
                    formatted_min, team_name
                )),
                protocol::EventType::PenaltySave => self.match_log.push(format!(
                    "{} - 🧤 PENALTY SAVED by {} GK!",
                    formatted_min, team_name
                )),
                protocol::EventType::ExtraTimeStart => self.match_log.push(format!(
                    "{} - ⏰ EXTRA TIME STARTS! 30 more minutes will be played.",
                    formatted_min
                )),
                protocol::EventType::ExtraTimeHalfTime => self.match_log.push(format!(
                    "{} - ⏰ Extra Time Half Time!",
                    formatted_min
                )),
                protocol::EventType::PenaltyShootoutStart => self.match_log.push(format!(
                    "{} - 🧤 PENALTY SHOOTOUT STARTS!",
                    formatted_min
                )),
            }
        }
    }

    fn run(&mut self, terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
        loop {
            terminal.draw(|frame| self.render(frame))?;
            if !self.handle_events()? {
                break Ok(());
            }
        }
    }

    fn handle_events(&mut self) -> Result<bool> {
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
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
                            self.tab_index = (self.tab_index + 1) % Screen::all().len();
                            self.scroll_offset = 0;
                        }
                        KeyCode::BackTab | KeyCode::Left => {
                            self.tab_index = if self.tab_index == 0 {
                                Screen::all().len() - 1
                            } else {
                                self.tab_index - 1
                            };
                            self.scroll_offset = 0;
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            self.scroll_offset = self.scroll_offset.saturating_add(1)
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            self.scroll_offset = self.scroll_offset.saturating_sub(1)
                        }
                        KeyCode::Home => self.scroll_offset = 0,
                        KeyCode::End => self.scroll_offset = usize::MAX,
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
        let title = format!(
            " ⚽ {}  |  Season {}  |  Manager: {}",
            self.club, self.season, self.manager
        );
        let block = Block::default().style(Style::default().fg(Color::White).bg(Color::DarkGray));
        frame.render_widget(Paragraph::new(title).block(block).bold(), area);
    }

    fn render_tabs(&self, frame: &mut Frame, area: Rect) {
        let labels: Vec<&str> = Screen::all().iter().map(|s| s.label()).collect();
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
            Screen::Match => self.render_match(frame, area),
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
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(5), Constraint::Min(1)])
            .split(area);

        // Scoreboard
        let formatted_minute = protocol::format_match_minute(self.match_minute);
        let scoreboard = Paragraph::new(Text::from(vec![
            Line::from(Span::styled(
                " ⚽ LIVE MATCH",
                Style::default().bold().fg(Color::Cyan),
            )),
            Line::from(Span::styled(
                format!(
                    "   Rustington United {} - {} FC Terminal  (Minute: {})",
                    self.match_score[0], self.match_score[1], formatted_minute
                ),
                Style::default()
                    .bold()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" 📊 Match Centre "),
        )
        .style(Style::default());
        frame.render_widget(scoreboard, chunks[0]);

        // Match log
        let items: Vec<ListItem> = self
            .match_log
            .iter()
            .map(|l| ListItem::new(l.as_str()))
            .collect();
        let log = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" 📜 Commentary "),
            )
            .style(Style::default());
        frame.render_widget(log, chunks[1]);
    }

    fn render_status_bar(&self, frame: &mut Frame, area: Rect) {
        let conn_label = if self.connected {
            "Connected"
        } else {
            "Offline"
        };
        let status = format!(
            " [Tab/Arrows: Navigate]  |  Screen: {}  |  Players: {}  |  1-3:Mentality 4-6:Press 7-9:Tempo  |  {}  |  'q': Quit ",
            Screen::all()[self.tab_index].label().trim(),
            self.players.len(),
            conn_label,
        );
        let block = Block::default().style(Style::default().fg(Color::Black).bg(Color::Gray));
        frame.render_widget(Paragraph::new(status).block(block), area);
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
        // Safely unpack fields from the packed struct to avoid UB on u32/u16/f32
        let (_mid, ev_type, ev_team, ev_player_index, val) = event.unpack();

        let team_name = match ev_team {
            protocol::Team::Home => "Rustington",
            protocol::Team::Away => "FC Terminal",
        };

        // Update match minute
        self.match_minute = val as u8;
        let formatted_min = protocol::format_match_minute(self.match_minute);

        // Update score for goals
        if ev_type == protocol::EventType::Goal {
            if ev_team == protocol::Team::Home {
                self.match_score[0] = self.match_score[0].wrapping_add(1);
            } else {
                self.match_score[1] = self.match_score[1].wrapping_add(1);
            }
        }

        // Add to match log
        let msg = match ev_type {
            protocol::EventType::Kickoff => format!("{} - Kickoff!", formatted_min),
            protocol::EventType::Goal => {
                format!("{} - ⚽ GOAL! {} scores for {}!", formatted_min, ev_player_index, team_name)
            }
            protocol::EventType::Shot => {
                format!("{} - Shot by player {}", formatted_min, ev_player_index)
            }
            protocol::EventType::ShotOnTarget => {
                format!("{} - Shot on target!", formatted_min)
            }
            protocol::EventType::Save => format!("{} - Great save!", formatted_min),
            protocol::EventType::Miss => format!("{} - Shot wide", formatted_min),
            protocol::EventType::Foul => format!("{} - Foul committed", formatted_min),
            protocol::EventType::HalfTime => format!("{} - Half Time!", formatted_min),
            protocol::EventType::FullTime => format!("{} - Full Time!", formatted_min),
            protocol::EventType::Corner => format!("{} - Corner kick", formatted_min),
            protocol::EventType::YellowCard => {
                format!("{} - Yellow card for {}", formatted_min, team_name)
            }
            protocol::EventType::RedCard => {
                format!("{} - 🔴 RED CARD for {}", formatted_min, team_name)
            }
            protocol::EventType::Substitution => {
                format!("{} - Substitution for {}", formatted_min, team_name)
            }
            protocol::EventType::PenaltyGoal => {
                format!("{} - ⚽ PENALTY GOAL! {} scores for {}! Shootout score: {}", formatted_min, ev_player_index, team_name, val as u32)
            }
            protocol::EventType::PenaltyMiss => {
                format!("{} - ❌ Penalty missed by {}!", formatted_min, team_name)
            }
            protocol::EventType::PenaltySave => {
                format!("{} - 🧤 PENALTY SAVED by {} GK!", formatted_min, team_name)
            }
            protocol::EventType::ExtraTimeStart => {
                format!("{} - ⏰ EXTRA TIME STARTS! 30 more minutes will be played.", formatted_min)
            }
            protocol::EventType::ExtraTimeHalfTime => {
                format!("{} - ⏰ Extra Time Half Time!", formatted_min)
            }
            protocol::EventType::PenaltyShootoutStart => {
                format!("{} - 🧤 PENALTY SHOOTOUT STARTS!", formatted_min)
            }
            _ => return, // Skip unknown events
        };

        self.match_log.push(msg);
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
