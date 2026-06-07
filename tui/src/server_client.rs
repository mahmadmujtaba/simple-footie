//! Server communication module.
//!
//! Connects to the game server via HTTP (for handshake info) and UDP (for
//! real-time command/event exchange).  Events arrive length-prefixed over UDP:
//! 2 bytes of length followed by the raw `EventPacket` bytes.

use protocol::{CommandPacket, CommandType, EventPacket};
use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc;
use std::thread;

/// Client-side handle to a live match on the game server.
pub struct ServerClient {
    pub match_id: u32,
    pub token: [u8; 16],
    pub socket: UdpSocket,
    pub server_addr: SocketAddr,
    sequence: u16,
    pub event_rx: mpsc::Receiver<EventPacket>,
}

impl ServerClient {
    /// Connect to the server via HTTP to get match info, then establish a UDP
    /// channel.  Spawns a background thread that reads length-prefixed events
    /// and pushes them onto `event_rx`.
    pub fn connect(server_host: &str) -> Result<Self, String> {
        let http_url = format!("http://{}:8080/api/match", server_host);

        // ---- HTTP handshake ----
        let response = ureq::get(&http_url)
            .call()
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        let body = {
            response
                .into_body()
                .read_to_string()
                .map_err(|e| format!("Failed to read response: {}", e))?
        };

        let match_id = parse_json_value(&body, "match_id").unwrap_or(1);
        let token_hex = parse_json_string(&body, "token").unwrap_or_default();
        let token = hex_to_bytes(&token_hex);
        let server_addr: SocketAddr = format!("{}:9001", server_host)
            .parse()
            .map_err(|e| format!("Invalid address: {}", e))?;

        // ---- UDP socket ----
        let socket = UdpSocket::bind("0.0.0.0:0")
            .map_err(|e| format!("Failed to bind UDP socket: {}", e))?;
        socket
            .set_nonblocking(true)
            .map_err(|e| format!("Failed to set nonblocking: {}", e))?;

        let (event_tx, event_rx) = mpsc::channel();

        // Receiver thread – reads length-prefixed event batches from UDP.
        let recv_socket = socket
            .try_clone()
            .map_err(|e| format!("Failed to clone socket: {}", e))?;

        thread::spawn(move || {
            let mut buf = [0u8; 65535];
            loop {
                match recv_socket.recv_from(&mut buf) {
                    Ok((len, _src)) => {
                        let mut offset = 0;
                        while offset + 2 <= len {
                            let event_len =
                                u16::from_le_bytes([buf[offset], buf[offset + 1]]) as usize;
                            offset += 2;
                            if offset + event_len > len || event_len < 12 {
                                break;
                            }
                            if let Some(event) =
                                parse_event_packet(&buf[offset..offset + event_len])
                            {
                                let _ = event_tx.send(event);
                            }
                            offset += event_len;
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(_) => {
                        thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
            }
        });

        // ---- Handshake packet (26 bytes: 10-byte CommandPacket + 16-byte token) ----
        let mut handshake = Vec::with_capacity(26);
        let handshake_cmd = CommandPacket {
            match_id,
            sequence: 0,
            command_type: CommandType::Mentality,
            arg1: 0,
            arg2: 1, // default attack
            arg3: 0,
        };
        let cmd_bytes = unsafe {
            std::slice::from_raw_parts((&handshake_cmd as *const CommandPacket) as *const u8, 10)
        };
        handshake.extend_from_slice(cmd_bytes);
        handshake.extend_from_slice(&token);

        socket
            .send_to(&handshake, server_addr)
            .map_err(|e| format!("Failed to send handshake: {}", e))?;

        Ok(Self {
            match_id,
            token,
            socket,
            server_addr,
            sequence: 1,
            event_rx,
        })
    }

    /// Send a tactical command (10 bytes, steady state).
    pub fn send_command(
        &mut self,
        cmd_type: CommandType,
        team: u8,
        value: u8,
    ) -> Result<(), String> {
        let cmd = CommandPacket {
            match_id: self.match_id,
            sequence: self.sequence,
            command_type: cmd_type,
            arg1: team,
            arg2: value,
            arg3: 0,
        };
        self.sequence += 1;

        let cmd_bytes =
            unsafe { std::slice::from_raw_parts((&cmd as *const CommandPacket) as *const u8, 10) };

        self.socket
            .send_to(cmd_bytes, self.server_addr)
            .map_err(|e| format!("Failed to send command: {}", e))?;

        Ok(())
    }
}

// ── JSON helpers (no serde dependency) ──────────────────────────────

/// Parse an integer value from a JSON object by key.
fn parse_json_value(body: &str, key: &str) -> Option<u32> {
    let search = format!("\"{}\"", key);
    if let Some(start) = body.find(&search) {
        let after_key = &body[start + search.len()..];
        let after_colon = after_key.trim_start();
        if let Some(stripped) = after_colon.strip_prefix(':') {
            let after_colon = stripped.trim_start();
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

/// Parse a quoted string value from a JSON object by key.
fn parse_json_string(body: &str, key: &str) -> Option<String> {
    let search = format!("\"{}\"", key);
    if let Some(start) = body.find(&search) {
        let after_key = &body[start + search.len()..];
        let after_colon = after_key.trim_start();
        if let Some(stripped) = after_colon.strip_prefix(':') {
            let after_colon = stripped.trim_start();
            if let Some(quoted) = after_colon.strip_prefix('"') {
                let mut val = String::new();
                for c in quoted.chars() {
                    if c == '"' {
                        break;
                    }
                    val.push(c);
                }
                return Some(val);
            }
        }
    }
    None
}

/// Convert a hex string (with or without `0x` prefix) into a 16-byte array.
fn hex_to_bytes(hex: &str) -> [u8; 16] {
    let mut bytes = [0u8; 16];
    let hex = hex.trim_start_matches("0x");
    for i in 0..16.min(hex.len() / 2) {
        if let Ok(b) = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16) {
            bytes[i] = b;
        }
    }
    bytes
}

// ── Wire-format parser ─────────────────────────────────────────────

/// Deserialize a single `EventPacket` from raw bytes (at least 12 bytes).
///
/// Wire layout (little-endian):
///   [0..4]   match_id       (u32)
///   [4]      event_type     (u8)
///   [5]      team           (u8)
///   [6..8]   player_index   (u16)
///   [8..12]  value          (f32)
fn parse_event_packet(data: &[u8]) -> Option<EventPacket> {
    if data.len() < 12 {
        return None;
    }
    let match_id = u32::from_le_bytes(data[0..4].try_into().ok()?);
    let event_type = event_type_from_u8(data[4])?;
    let team = if data[5] == 0 {
        protocol::Team::Home
    } else {
        protocol::Team::Away
    };
    let player_index = u16::from_le_bytes(data[6..8].try_into().ok()?);
    let value = f32::from_le_bytes(data[8..12].try_into().ok()?);

    Some(EventPacket {
        match_id,
        event_type,
        team,
        player_index,
        value,
    })
}

fn event_type_from_u8(val: u8) -> Option<protocol::EventType> {
    match val {
        0 => Some(protocol::EventType::Kickoff),
        1 => Some(protocol::EventType::Goal),
        2 => Some(protocol::EventType::Shot),
        3 => Some(protocol::EventType::ShotOnTarget),
        4 => Some(protocol::EventType::Save),
        5 => Some(protocol::EventType::Corner),
        6 => Some(protocol::EventType::FreeKick),
        7 => Some(protocol::EventType::Foul),
        8 => Some(protocol::EventType::YellowCard),
        9 => Some(protocol::EventType::RedCard),
        10 => Some(protocol::EventType::Substitution),
        11 => Some(protocol::EventType::HalfTime),
        12 => Some(protocol::EventType::FullTime),
        13 => Some(protocol::EventType::Injury),
        14 => Some(protocol::EventType::Offside),
        15 => Some(protocol::EventType::Miss),
        _ => None,
    }
}
