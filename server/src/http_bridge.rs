//! HTTP bridge — serves HTML control panel + forwards commands to the simulation core.
//!
//! Provides:
//! - `GET /` → HTML page with tactical controls
//! - `POST /api/command` → send a command to the game engine
//! - `GET /api/match` → current match state (score, minute, events)

use crossbeam::channel::Sender;
use protocol::{CommandPacket, CommandType};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::network::InboundCommand;

/// Shared match state readable from the HTTP API.
pub struct HttpState {
    pub match_score: Mutex<[u8; 2]>,
    pub match_minute: AtomicU32,
    pub last_events: Mutex<Vec<String>>,
    pub match_active: AtomicU32,
}

impl HttpState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            match_score: Mutex::new([0, 0]),
            match_minute: AtomicU32::new(0),
            last_events: Mutex::new(Vec::new()),
            match_active: AtomicU32::new(1),
        })
    }
}

/// Start the HTTP bridge server on port 8080.
///
/// Serves the HTML control panel and forwards commands to the engine.
pub fn start_http_bridge(
    cmd_tx: Sender<InboundCommand>,
    state: Arc<HttpState>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let listener = match TcpListener::bind("127.0.0.1:8080") {
            Ok(l) => l,
            Err(e) => {
                eprintln!("⚠  HTTP bridge failed to bind: {e}");
                return;
            }
        };

        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    let mut buf = [0u8; 4096];
                    if let Ok(n) = stream.read(&mut buf) {
                        let request = String::from_utf8_lossy(&buf[..n]);
                        let response = handle_request(&request, &cmd_tx, &state);
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.shutdown(std::net::Shutdown::Both);
                    }
                }
                Err(e) => eprintln!("HTTP connection error: {e}"),
            }
        }
    })
}

/// Route a single HTTP request.
fn handle_request(request: &str, cmd_tx: &Sender<InboundCommand>, state: &HttpState) -> String {
    let request_line = request.lines().next().unwrap_or("GET / HTTP/1.0");

    if request_line.starts_with("GET /api/match") {
        handle_api_match(state)
    } else if request_line.starts_with("POST /api/command") {
        handle_api_command(request, cmd_tx, state)
    } else {
        handle_index()
    }
}

/// Serve the HTML control panel.
fn handle_index() -> String {
    let html = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>⚽ fm-rust — Match Control</title>
<style>
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body {
    font-family: 'Segoe UI', system-ui, -apple-system, sans-serif;
    background: #1a1a2e; color: #e0e0e0;
    display: flex; justify-content: center; align-items: center;
    min-height: 100vh; padding: 20px;
  }
  .container { max-width: 800px; width: 100%; }
  h1 { text-align: center; font-size: 1.8rem; margin-bottom: 8px; }
  h1 span { color: #f1c40f; }
  .subtitle { text-align: center; color: #888; margin-bottom: 24px; font-size: 0.9rem; }
  .scoreboard {
    background: #16213e; border-radius: 12px; padding: 20px;
    text-align: center; margin-bottom: 20px;
    border: 1px solid #0f3460;
  }
  .scoreboard .score { font-size: 3rem; font-weight: bold; letter-spacing: 4px; }
  .scoreboard .score .home { color: #e74c3c; }
  .scoreboard .score .away { color: #3498db; }
  .scoreboard .info { color: #888; margin-top: 8px; font-size: 0.85rem; }
  .panels { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; margin-bottom: 20px; }
  .panel {
    background: #16213e; border-radius: 12px; padding: 16px;
    border: 1px solid #0f3460;
  }
  .panel h3 { font-size: 0.85rem; color: #888; text-transform: uppercase; margin-bottom: 12px; }
  .btn-group { display: flex; flex-direction: column; gap: 6px; }
  .btn {
    padding: 8px 16px; border: 1px solid #0f3460; border-radius: 8px;
    background: #1a1a4e; color: #e0e0e0; cursor: pointer;
    font-size: 0.85rem; transition: all 0.15s;
  }
  .btn:hover { background: #0f3460; border-color: #1a5276; }
  .btn.active { background: #0f3460; border-color: #3498db; }
  .btn.primary { background: #27ae60; border-color: #2ecc71; color: #fff; }
  .btn.primary:hover { background: #2ecc71; }
  .btn.danger { background: #c0392b; border-color: #e74c3c; color: #fff; }
  .btn.danger:hover { background: #e74c3c; }
  .events {
    background: #16213e; border-radius: 12px; padding: 16px;
    border: 1px solid #0f3460; max-height: 300px; overflow-y: auto;
  }
  .events h3 { font-size: 0.85rem; color: #888; text-transform: uppercase; margin-bottom: 8px; }
  .events ul { list-style: none; }
  .events li { padding: 4px 0; font-size: 0.85rem; color: #aaa; border-bottom: 1px solid #0f3460; }
  .events .goal { color: #f1c40f; font-weight: bold; }
  .events .shot { color: #3498db; }
  .events .save { color: #2ecc71; }
  .events .miss { color: #e67e22; }
  .events .foul { color: #e74c3c; }
  .toast {
    position: fixed; bottom: 20px; right: 20px;
    background: #27ae60; color: #fff; padding: 12px 24px;
    border-radius: 8px; font-size: 0.85rem;
    opacity: 0; transition: opacity 0.3s;
  }
  .toast.show { opacity: 1; }
  @media (max-width: 600px) { .panels { grid-template-columns: 1fr; } }
</style>
</head>
<body>
<div class="container">
  <h1>⚽ <span>fm-rust</span> Match Control</h1>
  <p class="subtitle">Rustington United vs FC Terminal — Live tactical commands</p>

  <div class="scoreboard">
    <div class="score">
      <span class="home" id="homeScore">0</span>
      <span style="color:#555"> - </span>
      <span class="away" id="awayScore">0</span>
    </div>
    <div class="info">
      <span id="matchMinute">0</span>' &middot;
      Match <span id="matchActive">active</span> &middot;
      <span id="possession">50</span>% possession
    </div>
  </div>

  <div class="panels">
    <div class="panel">
      <h3>Mentality</h3>
      <div class="btn-group">
        <button class="btn" onclick="sendCommand(0, 0, 0)">🧘 Normal</button>
        <button class="btn" onclick="sendCommand(0, 0, 1)">⚔️ Attack</button>
        <button class="btn" onclick="sendCommand(0, 0, 2)">🛡️ Defend</button>
      </div>
    </div>
    <div class="panel">
      <h3>Press Intensity</h3>
      <div class="btn-group">
        <button class="btn" onclick="sendCommand(2, 0, 0)">🔽 Low</button>
        <button class="btn" onclick="sendCommand(2, 0, 1)">➡️ Medium</button>
        <button class="btn" onclick="sendCommand(2, 0, 2)">🔼 High</button>
      </div>
    </div>
    <div class="panel">
      <h3>Tempo</h3>
      <div class="btn-group">
        <button class="btn" onclick="sendCommand(3, 0, 0)">🐢 Slow</button>
        <button class="btn" onclick="sendCommand(3, 0, 1)">➡️ Normal</button>
        <button class="btn" onclick="sendCommand(3, 0, 2)">⚡ Fast</button>
      </div>
    </div>
    <div class="panel">
      <h3>Width</h3>
      <div class="btn-group">
        <button class="btn" onclick="sendCommand(4, 0, 0)">↔️ Narrow</button>
        <button class="btn" onclick="sendCommand(4, 0, 1)">➡️ Normal</button>
        <button class="btn" onclick="sendCommand(4, 0, 2)">↔️ Wide</button>
      </div>
    </div>
  </div>

  <div class="panels">
    <div class="panel" style="grid-column: 1 / -1;">
      <h3>Away Team Commands</h3>
      <div class="btn-group" style="flex-direction: row; flex-wrap: wrap;">
        <button class="btn" onclick="sendCommand(0, 1, 0)">Away Normal</button>
        <button class="btn" onclick="sendCommand(0, 1, 1)">Away Attack</button>
        <button class="btn" onclick="sendCommand(0, 1, 2)">Away Defend</button>
        <button class="btn" onclick="sendCommand(2, 1, 2)">Away High Press</button>
        <button class="btn" onclick="sendCommand(3, 1, 2)">Away Fast Tempo</button>
      </div>
    </div>
  </div>

  <div class="events" id="events">
    <h3>📜 Match Events</h3>
    <ul id="eventList">
      <li>Waiting for match events...</li>
    </ul>
  </div>
</div>

<div class="toast" id="toast">Command sent!</div>

<script>
let seq = 1;

function sendCommand(cmdType, team, value) {
  seq++;
  fetch('/api/command', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      match_id: 1,
      sequence: seq,
      command_type: cmdType,
      team: team,
      value: value
    })
  })
  .then(r => r.json())
  .then(data => {
    showToast(data.status || 'Command sent!');
    refreshMatch();
  })
  .catch(() => showToast('Error sending command'));
}

function refreshMatch() {
  fetch('/api/match')
    .then(r => r.json())
    .then(data => {
      document.getElementById('homeScore').textContent = data.score[0];
      document.getElementById('awayScore').textContent = data.score[1];
      document.getElementById('matchMinute').textContent = data.minute;
      document.getElementById('matchActive').textContent =
        data.active ? 'active' : 'finished';
      document.getElementById('possession').textContent =
        Math.round(data.possession * 100);

      const list = document.getElementById('eventList');
      if (data.events && data.events.length > 0) {
        list.innerHTML = data.events.slice(-20).map(e =>
          `<li class="${e.type}">${e.minute}' - ${e.text}</li>`
        ).join('');
      }
    })
    .catch(() => {});
}

function showToast(msg) {
  const t = document.getElementById('toast');
  t.textContent = msg;
  t.classList.add('show');
  setTimeout(() => t.classList.remove('show'), 2000);
}

// Poll match state every 2 seconds
setInterval(refreshMatch, 2000);
refreshMatch();
</script>
</body>
</html>"##;

    let len = html.len();
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
        len, html
    )
}

/// Handle GET /api/match — return current match state as JSON.
fn handle_api_match(state: &HttpState) -> String {
    let score = state.match_score.lock().unwrap();
    let events = state.last_events.lock().unwrap();
    let events_json = events
        .iter()
        .map(|e| {
            format!(
                "{{\"minute\":\"{}\",\"type\":\"{}\",\"text\":\"{}\"}}",
                e.split("'").next().unwrap_or("0"),
                if e.contains("GOAL") {
                    "goal"
                } else if e.contains("Save") {
                    "save"
                } else if e.contains("wide") {
                    "miss"
                } else if e.contains("Foul") {
                    "foul"
                } else {
                    "shot"
                },
                e
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    let body = format!(
        r#"{{"score":[{},{}],"minute":{},"possession":0.5,"active":{},"events":[{}]}}"#,
        score[0],
        score[1],
        state.match_minute.load(Ordering::Relaxed),
        if state.match_active.load(Ordering::Relaxed) > 0 {
            "true"
        } else {
            "false"
        },
        events_json
    );
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    )
}

/// Handle POST /api/command — parse JSON body and forward to engine.
fn handle_api_command(request: &str, cmd_tx: &Sender<InboundCommand>, state: &HttpState) -> String {
    // Extract JSON body after headers
    let body = if let Some(idx) = request.find("\r\n\r\n") {
        &request[idx + 4..]
    } else {
        ""
    };

    // Parse JSON manually (no serde dependency needed)
    let match_id = parse_json_value(body, "match_id").unwrap_or(1);
    let sequence = parse_json_value(body, "sequence").unwrap_or(1) as u16;
    let cmd_type = parse_json_value(body, "command_type").unwrap_or(0);
    let team = parse_json_value(body, "team").unwrap_or(0);
    let value = parse_json_value(body, "value").unwrap_or(0);

    let command_type = match cmd_type {
        0 => CommandType::Mentality,
        1 => CommandType::Substitution,
        2 => CommandType::Press,
        3 => CommandType::Tempo,
        4 => CommandType::Width,
        _ => CommandType::Mentality,
    };

    let cmd = CommandPacket {
        match_id,
        sequence,
        command_type,
        arg1: team as u8,
        arg2: value as u8,
        arg3: 0,
    };

    // Send to simulation core via crossbeam channel
    let inbound = InboundCommand {
        packet: cmd,
        token: [0u8; 16], // steady-state (token cached server-side)
        src_addr: ([127, 0, 0, 1], 0).into(),
    };

    let status = match cmd_tx.send(inbound) {
        Ok(_) => {
            // Also bump the minute counter for the demo
            state.match_minute.fetch_add(1, Ordering::Relaxed);
            "ok"
        }
        Err(_) => "channel full",
    };

    let body = format!(
        r#"{{"status":"{}","sequence":{},"command_type":{},"team":{},"value":{}}}"#,
        status, sequence, cmd_type, team, value
    );

    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    )
}

/// Minimal JSON integer parser (no deps).
fn parse_json_value(body: &str, key: &str) -> Option<u32> {
    let search = format!("\"{}\"", key);
    if let Some(start) = body.find(&search) {
        let after_key = &body[start + search.len()..];
        // Skip colon, whitespace
        let after_colon = after_key.trim_start();
        if after_colon.starts_with(':') {
            let after_colon = after_colon[1..].trim_start();
            let mut num_str = String::new();
            for c in after_colon.chars() {
                if c.is_ascii_digit() || c == '-' {
                    num_str.push(c);
                } else {
                    break;
                }
            }
            return num_str.parse::<i32>().ok().map(|v| v as u32);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam::channel;

    #[test]
    fn test_parse_json_value() {
        let json = r#"{"match_id":1,"sequence":5,"command_type":2,"team":0,"value":1}"#;
        assert_eq!(parse_json_value(json, "match_id"), Some(1));
        assert_eq!(parse_json_value(json, "sequence"), Some(5));
        assert_eq!(parse_json_value(json, "command_type"), Some(2));
        assert_eq!(parse_json_value(json, "team"), Some(0));
        assert_eq!(parse_json_value(json, "value"), Some(1));
    }

    #[test]
    fn test_handle_index_returns_html() {
        let response = handle_index();
        assert!(response.contains("HTTP/1.1 200"));
        assert!(response.contains("fm-rust"));
        assert!(response.contains("sendCommand"));
    }

    #[test]
    fn test_handle_api_match() {
        let state = HttpState::new();
        let response = handle_api_match(&state);
        assert!(response.contains("HTTP/1.1 200"));
        assert!(response.contains(r#""score":[0,0]"#));
    }
}
