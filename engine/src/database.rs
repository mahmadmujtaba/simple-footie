use rusqlite::{params, Connection, Result};
use protocol::Team;
use crate::player::{PlayerAttributes, Position};

#[derive(Debug, Clone)]
pub struct DbPlayer {
    pub name: String,
    pub age: u8,
    pub pos: String,
    pub ovr: u8,
    pub pot: u8,
    pub nation: String,
    pub contract: String,
    pub value: String,
    pub team: String,
    pub player_index: u16,
    pub finishing: u8,
    pub passing: u8,
    pub dribbling: u8,
    pub defending: u8,
    pub pace: u8,
    pub stamina: u8,
}

#[derive(Debug, Clone)]
pub struct DbPlayedMatch {
    pub id: i64,
    pub date_played: String,
    pub home_team: String,
    pub away_team: String,
    pub home_score: u8,
    pub away_score: u8,
}

#[derive(Debug, Clone)]
pub struct DbMatchEvent {
    pub minute: u8,
    pub event_type: u8,
    pub team: String,
    pub player_index: u16,
    pub value: f32,
    pub text: String,
}

/// Initialize the SQLite database and populate default players if empty.
pub fn init_db(db_path: &str) -> Result<()> {
    let conn = Connection::open(db_path)?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS players (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            age INTEGER NOT NULL,
            pos TEXT NOT NULL,
            ovr INTEGER NOT NULL,
            pot INTEGER NOT NULL,
            nation TEXT NOT NULL,
            contract TEXT NOT NULL,
            value TEXT NOT NULL,
            team TEXT NOT NULL,
            player_index INTEGER NOT NULL,
            finishing INTEGER NOT NULL,
            passing INTEGER NOT NULL,
            dribbling INTEGER NOT NULL,
            defending INTEGER NOT NULL,
            pace INTEGER NOT NULL,
            stamina INTEGER NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS matches (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date_played TEXT NOT NULL,
            home_team TEXT NOT NULL,
            away_team TEXT NOT NULL,
            home_score INTEGER NOT NULL,
            away_score INTEGER NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS match_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            match_id INTEGER NOT NULL,
            minute INTEGER NOT NULL,
            event_type INTEGER NOT NULL,
            team TEXT NOT NULL,
            player_index INTEGER NOT NULL,
            value REAL NOT NULL,
            text TEXT NOT NULL,
            FOREIGN KEY(match_id) REFERENCES matches(id)
        )",
        [],
    )?;

    // Check if players table is empty
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM players", [], |row| row.get(0))?;
    if count == 0 {
        populate_default_players(&conn)?;
    }

    Ok(())
}

fn populate_default_players(conn: &Connection) -> Result<()> {
    // 1. Home Players (Rustington United)
    let home_players = vec![
        ("Rusty McSave", 32, "GK", 82, 82, "Scotland", "2027", "£2.5M", 0, 10, 50, 50, 10, 50, 82),
        ("Lex Byte", 26, "LB", 76, 78, "Germany", "2028", "£4.2M", 1, 45, 72, 70, 78, 76, 85),
        ("Corey Heap", 24, "CB", 80, 86, "England", "2030", "£8.5M", 2, 35, 65, 60, 82, 72, 85),
        ("Sean Stack", 29, "CB", 78, 78, "Ireland", "2027", "£3.1M", 3, 30, 60, 55, 80, 68, 85),
        ("Max Alloc", 23, "RB", 75, 84, "France", "2029", "£5.8M", 4, 40, 70, 68, 75, 78, 85),
        ("Tommy Mutex", 27, "CM", 81, 83, "England", "2028", "£7.2M", 5, 65, 84, 78, 72, 70, 85),
        ("Rusty Channel", 22, "CM", 77, 88, "Spain", "2030", "£10.0M", 6, 60, 80, 75, 68, 72, 85),
        ("Eddie Promise", 30, "LM", 79, 79, "Portugal", "2026", "£3.8M", 7, 70, 78, 80, 50, 82, 85),
        ("Ray Arc", 28, "RM", 80, 80, "Brazil", "2027", "£6.0M", 8, 68, 76, 82, 52, 84, 85),
        ("Ferris Enum", 31, "ST", 85, 85, "Argentina", "2027", "£15.0M", 9, 88, 70, 84, 35, 80, 85),
        ("Will Crash", 20, "ST", 70, 92, "England", "2031", "£12.0M", 10, 75, 62, 72, 30, 85, 85),
    ];

    for p in home_players {
        conn.execute(
            "INSERT INTO players (name, age, pos, ovr, pot, nation, contract, value, team, player_index, finishing, passing, dribbling, defending, pace, stamina)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'Home', ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![p.0, p.1, p.2, p.3, p.4, p.5, p.6, p.7, p.8, p.9, p.10, p.11, p.12, p.13, p.14],
        )?;
    }

    // 2. Away Players (FC Terminal)
    let away_players = vec![
        ("Null Pointer", 31, "GK", 80, 80, "USA", "2028", "£2.0M", 0, 10, 45, 45, 10, 45, 80),
        ("Stack Overflow", 28, "LB", 75, 77, "Canada", "2027", "£3.0M", 1, 40, 68, 68, 74, 75, 85),
        ("Buffer Overflow", 27, "CB", 78, 81, "India", "2029", "£5.0M", 2, 30, 58, 55, 79, 68, 85),
        ("Race Condition", 29, "CB", 76, 76, "Australia", "2028", "£3.5M", 3, 30, 55, 50, 77, 70, 85),
        ("Memory Leak", 25, "RB", 74, 78, "Japan", "2030", "£4.0M", 4, 35, 65, 65, 72, 75, 85),
        ("Garbage Collector", 30, "CM", 79, 79, "Netherlands", "2027", "£4.5M", 5, 55, 80, 72, 75, 65, 85),
        ("Syntax Error", 24, "CM", 75, 82, "Ukraine", "2029", "£6.0M", 6, 58, 76, 74, 65, 70, 85),
        ("Merge Conflict", 26, "LM", 77, 80, "Poland", "2028", "£5.5M", 7, 68, 74, 78, 48, 80, 85),
        ("Infinite Loop", 28, "RM", 78, 78, "Sweden", "2027", "£5.0M", 8, 65, 72, 79, 50, 82, 85),
        ("Segmentation Fault", 29, "ST", 82, 82, "Italy", "2028", "£10.0M", 9, 84, 65, 80, 32, 78, 85),
        ("Out of Memory", 23, "ST", 73, 85, "Norway", "2030", "£8.0M", 10, 76, 60, 70, 28, 82, 85),
    ];

    for p in away_players {
        conn.execute(
            "INSERT INTO players (name, age, pos, ovr, pot, nation, contract, value, team, player_index, finishing, passing, dribbling, defending, pace, stamina)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'Away', ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![p.0, p.1, p.2, p.3, p.4, p.5, p.6, p.7, p.8, p.9, p.10, p.11, p.12, p.13, p.14],
        )?;
    }

    Ok(())
}

/// Load player attributes for simulation (11-player array).
pub fn load_squad_attributes(db_path: &str, team: &str) -> Result<[PlayerAttributes; 11]> {
    let conn = Connection::open(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT player_index, pos, ovr, finishing, passing, dribbling, defending, pace, stamina
         FROM players WHERE team = ?1 ORDER BY player_index ASC LIMIT 11",
    )?;

    let team_enum = if team == "Home" { Team::Home } else { Team::Away };

    let rows = stmt.query_map([team], |row| {
        let pos_str: String = row.get(1)?;
        let position = match pos_str.as_str() {
            "GK" => Position::Goalkeeper,
            "LB" | "CB" | "RB" => Position::Defender,
            "LM" | "CM" | "RM" => Position::Midfielder,
            "ST" => Position::Forward,
            _ => Position::Midfielder,
        };

        Ok(PlayerAttributes {
            index: row.get(0)?,
            team: team_enum,
            position,
            overall: row.get(2)?,
            finishing: row.get(3)?,
            passing: row.get(4)?,
            dribbling: row.get(5)?,
            defending: row.get(6)?,
            pace: row.get(7)?,
            stamina: row.get(8)?,
        })
    })?;

    let mut squad = [PlayerAttributes::default(); 11];
    for (i, p) in rows.enumerate() {
        if i < 11 {
            squad[i] = p?;
        }
    }

    Ok(squad)
}

/// Load players for TUI rendering.
pub fn load_squad_players(db_path: &str, team: &str) -> Result<Vec<DbPlayer>> {
    let conn = Connection::open(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT name, age, pos, ovr, pot, nation, contract, value, team, player_index, finishing, passing, dribbling, defending, pace, stamina
         FROM players WHERE team = ?1 ORDER BY player_index ASC",
    )?;

    let rows = stmt.query_map([team], |row| {
        Ok(DbPlayer {
            name: row.get(0)?,
            age: row.get(1)?,
            pos: row.get(2)?,
            ovr: row.get(3)?,
            pot: row.get(4)?,
            nation: row.get(5)?,
            contract: row.get(6)?,
            value: row.get(7)?,
            team: row.get(8)?,
            player_index: row.get(9)?,
            finishing: row.get(10)?,
            passing: row.get(11)?,
            dribbling: row.get(12)?,
            defending: row.get(13)?,
            pace: row.get(14)?,
            stamina: row.get(15)?,
        })
    })?;

    let mut players = Vec::new();
    for p in rows {
        players.push(p?);
    }
    Ok(players)
}

/// Save a played match and all of its commentary events to the database.
pub fn save_match(
    db_path: &str,
    date_played: &str,
    home_team: &str,
    away_team: &str,
    home_score: u8,
    away_score: u8,
    events: &[DbMatchEvent],
) -> Result<i64> {
    let mut conn = Connection::open(db_path)?;
    let tx = conn.transaction()?;

    tx.execute(
        "INSERT INTO matches (date_played, home_team, away_team, home_score, away_score)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![date_played, home_team, away_team, home_score, away_score],
    )?;

    let match_id = tx.last_insert_rowid();

    for ev in events {
        tx.execute(
            "INSERT INTO match_events (match_id, minute, event_type, team, player_index, value, text)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![match_id, ev.minute, ev.event_type, ev.team, ev.player_index, ev.value, ev.text],
        )?;
    }

    tx.commit()?;
    Ok(match_id)
}

/// Load all played matches.
pub fn load_played_matches(db_path: &str) -> Result<Vec<DbPlayedMatch>> {
    let conn = Connection::open(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT id, date_played, home_team, away_team, home_score, away_score
         FROM matches ORDER BY id DESC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(DbPlayedMatch {
            id: row.get(0)?,
            date_played: row.get(1)?,
            home_team: row.get(2)?,
            away_team: row.get(3)?,
            home_score: row.get(4)?,
            away_score: row.get(5)?,
        })
    })?;

    let mut matches = Vec::new();
    for m in rows {
        matches.push(m?);
    }
    Ok(matches)
}

/// Load all events for a played match.
pub fn load_match_events(db_path: &str, match_id: i64) -> Result<Vec<DbMatchEvent>> {
    let conn = Connection::open(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT minute, event_type, team, player_index, value, text
         FROM match_events WHERE match_id = ?1 ORDER BY id ASC",
    )?;

    let rows = stmt.query_map([match_id], |row| {
        Ok(DbMatchEvent {
            minute: row.get(0)?,
            event_type: row.get(1)?,
            team: row.get(2)?,
            player_index: row.get(3)?,
            value: row.get(4)?,
            text: row.get(5)?,
        })
    })?;

    let mut events = Vec::new();
    for ev in rows {
        events.push(ev?);
    }
    Ok(events)
}
