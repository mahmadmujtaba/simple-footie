//! UDP network layer — receives commands, sends events.
//!
//! Core 0: `recv_mmsg` loop, packet validation, push to lock-free command queue.
//! Core 2: batched event sending via UDP.

use crossbeam::channel::Sender;
use protocol::{CommandPacket, EventPacket};
use std::net::UdpSocket;
use std::thread;
use std::time::Duration;

/// Parsed inbound command with source address for reply routing.
#[derive(Debug, Clone)]
pub struct InboundCommand {
    pub packet: CommandPacket,
    pub token: [u8; 16],
    pub src_addr: std::net::SocketAddr,
}

/// The UDP network receiver.
///
/// Runs on Core 0: polls the socket with `recv_mmsg`, validates packet
/// structure, extracts (cmd, token, src), and pushes to the command queue.
pub struct NetworkReceiver {
    socket: UdpSocket,
    cmd_tx: Sender<InboundCommand>,
    buffer: Vec<u8>,
    batch_size: usize,
}

impl NetworkReceiver {
    /// Create a new receiver bound to `addr`.
    /// `cmd_tx` is the crossbeam channel to the simulation core.
    pub fn bind(addr: &str, cmd_tx: Sender<InboundCommand>) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            cmd_tx,
            buffer: vec![0u8; 65535], // max UDP datagram
            batch_size: 64,
        })
    }

    /// Run the receive loop (call on Core 0).
    ///
    /// Reads packets, validates them, and pushes valid commands onto the
    /// crossbeam channel for the simulation core to process.
    pub fn run(&mut self) {
        loop {
            match self.socket.recv_from(&mut self.buffer) {
                Ok((len, src_addr)) => {
                    let data = &self.buffer[..len];
                    if let Some(cmd) = self.parse_packet(data, src_addr) {
                        // Ignore send errors (receiver disconnected)
                        let _ = self.cmd_tx.send(cmd);
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_micros(100));
                }
                Err(e) => {
                    eprintln!("UDP recv error: {e}");
                }
            }
        }
    }

    /// Parse a UDP datagram into an InboundCommand.
    ///
    /// Supports two formats:
    /// - **Handshake** (first command): 26 bytes: 10-byte CommandPacket + 16-byte token
    /// - **Steady state** (cached token): 10 bytes: just CommandPacket
    ///
    /// Token validation is done by the caller (simulation core) against the
    /// TokenManager.
    fn parse_packet(&self, data: &[u8], src_addr: std::net::SocketAddr) -> Option<InboundCommand> {
        if data.len() < 10 {
            return None; // too short
        }

        let cmd = CommandPacket {
            match_id: u32::from_le_bytes(data[0..4].try_into().ok()?),
            sequence: u16::from_le_bytes(data[4..6].try_into().ok()?),
            command_type: data[6].try_into().ok()?,
            arg1: data[7],
            arg2: data[8],
            arg3: data.get(9).copied().unwrap_or(0),
        };

        let token = if data.len() >= 26 {
            // Handshake: includes 16-byte token after the command
            let mut t = [0u8; 16];
            t.copy_from_slice(&data[10..26]);
            t
        } else {
            // Steady state: token cached server-side, use placeholder
            // The simulation core will look up the real token.
            [0u8; 16]
        };

        Some(InboundCommand {
            packet: cmd,
            token,
            src_addr,
        })
    }
}

/// Sends events back to clients via UDP.
pub struct EventSender {
    socket: UdpSocket,
}

impl EventSender {
    pub fn bind(addr: &str) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        Ok(Self { socket })
    }

    /// Send a single event to a client.
    pub fn send(&self, event: &EventPacket, dest: std::net::SocketAddr) -> std::io::Result<usize> {
        let bytes = unsafe {
            std::slice::from_raw_parts(
                (event as *const EventPacket) as *const u8,
                std::mem::size_of::<EventPacket>(),
            )
        };
        self.socket.send_to(bytes, dest)
    }

    /// Send multiple events batched into a single datagram.
    /// Each event is prefixed with a 2-byte length.
    pub fn send_batch(
        &self,
        events: &[EventPacket],
        dest: std::net::SocketAddr,
    ) -> std::io::Result<usize> {
        let mut buf = Vec::with_capacity(events.len() * (2 + std::mem::size_of::<EventPacket>()));
        for event in events {
            let len = std::mem::size_of::<EventPacket>() as u16;
            buf.extend_from_slice(&len.to_le_bytes());
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    (event as *const EventPacket) as *const u8,
                    std::mem::size_of::<EventPacket>(),
                )
            };
            buf.extend_from_slice(bytes);
        }
        self.socket.send_to(&buf, dest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::CommandType;

    #[test]
    fn test_parse_handshake_packet() {
        let (tx, _rx) = crossbeam::channel::bounded(64);
        let receiver = NetworkReceiver::bind("127.0.0.1:0", tx).unwrap();

        let mut data = vec![0u8; 26];
        // match_id = 1
        data[0..4].copy_from_slice(&1u32.to_le_bytes());
        // sequence = 5
        data[4..6].copy_from_slice(&5u16.to_le_bytes());
        // command_type = Mentality (0)
        data[6] = 0;
        // arg1 = team home (0)
        data[7] = 0;
        // arg2 = Attack (1)
        data[8] = 1;
        // arg3 = 0
        data[9] = 0;
        // token = [1u8; 16]
        data[10..26].copy_from_slice(&[1u8; 16]);

        let addr = "127.0.0.1:9999".parse().unwrap();
        let cmd = receiver.parse_packet(&data, addr).unwrap();
        // Copy fields to locals to avoid unaligned reference on packed struct
        let match_id = cmd.packet.match_id;
        let seq = cmd.packet.sequence;
        let ct = cmd.packet.command_type;
        assert_eq!(match_id, 1);
        assert_eq!(seq, 5);
        assert_eq!(ct, CommandType::Mentality);
        assert_eq!(cmd.token, [1u8; 16]);
    }

    #[test]
    fn test_parse_steady_state_packet() {
        let (tx, _rx) = crossbeam::channel::bounded(64);
        let receiver = NetworkReceiver::bind("127.0.0.1:0", tx).unwrap();

        let data = vec![0u8; 10]; // just the command, no token
        let addr = "127.0.0.1:9999".parse().unwrap();
        let cmd = receiver.parse_packet(&data, addr).unwrap();
        assert_eq!(cmd.token, [0u8; 16]); // placeholder
    }

    #[test]
    fn test_short_packet_rejected() {
        let (tx, _rx) = crossbeam::channel::bounded(64);
        let receiver = NetworkReceiver::bind("127.0.0.1:0", tx).unwrap();
        let data = vec![0u8; 5]; // too short
        let addr = "127.0.0.1:9999".parse().unwrap();
        assert!(receiver.parse_packet(&data, addr).is_none());
    }
}
