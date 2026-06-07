//! Embedded Prometheus metrics endpoint.
//!
//! Exposes a lightweight HTTP endpoint on localhost:9090 with:
//! - `active_matches` – current number of active (non-finished) matches
//! - `command_rate_total` – total commands received since server start
//! - `command_rate_1m` – commands received in the last 60 seconds
//! - `queue_depth` – commands waiting in the channel
//! - `last_snapshot_time_seconds` – unix timestamp of last snapshot
//! - `disk_free_bytes` – available disk space on the data directory
//! - `journal_bytes_written` – total bytes written to journal
//! - `match_minutes_simulated` – total match-minutes simulated

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

/// Shared metrics accessible across threads.
pub struct Metrics {
    pub active_matches: AtomicU64,
    pub command_rate_total: AtomicU64,
    pub command_rate_1m: AtomicU64,
    pub queue_depth: AtomicU64,
    pub last_snapshot_time: AtomicU64,
    pub disk_free_bytes: AtomicU64,
    pub journal_bytes_written: AtomicU64,
    pub match_minutes_simulated: AtomicU64,
}

impl Metrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            active_matches: AtomicU64::new(0),
            command_rate_total: AtomicU64::new(0),
            command_rate_1m: AtomicU64::new(0),
            queue_depth: AtomicU64::new(0),
            last_snapshot_time: AtomicU64::new(0),
            disk_free_bytes: AtomicU64::new(0),
            journal_bytes_written: AtomicU64::new(0),
            match_minutes_simulated: AtomicU64::new(0),
        })
    }
}

/// Start the Prometheus metrics HTTP server on localhost:9090.
///
/// Runs in its own thread (Core 2 role). Serves `GET /metrics`.
pub fn start_metrics_server(metrics: Arc<Metrics>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let listener = match TcpListener::bind("127.0.0.1:9090") {
            Ok(l) => l,
            Err(e) => {
                eprintln!("⚠  Metrics server failed to bind: {e}");
                return;
            }
        };

        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    let mut buf = [0u8; 1024];
                    if let Ok(n) = stream.read(&mut buf) {
                        let request = String::from_utf8_lossy(&buf[..n]);
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();

                        let body = if request.starts_with("GET /metrics") {
                            format!(
                                "# HELP fm_active_matches Current number of active matches\n\
                                 # TYPE fm_active_matches gauge\n\
                                 fm_active_matches {}\n\
                                 \n\
                                 # HELP fm_command_rate_total Total commands received\n\
                                 # TYPE fm_command_rate_total counter\n\
                                 fm_command_rate_total {}\n\
                                 \n\
                                 # HELP fm_command_rate_1m Commands in last 60 seconds\n\
                                 # TYPE fm_command_rate_1m gauge\n\
                                 fm_command_rate_1m {}\n\
                                 \n\
                                 # HELP fm_queue_depth Commands waiting in channel\n\
                                 # TYPE fm_queue_depth gauge\n\
                                 fm_queue_depth {}\n\
                                 \n\
                                 # HELP fm_last_snapshot_time_seconds Unix timestamp of last snapshot\n\
                                 # TYPE fm_last_snapshot_time_seconds gauge\n\
                                 fm_last_snapshot_time_seconds {}\n\
                                 \n\
                                 # HELP fm_disk_free_bytes Available disk space\n\
                                 # TYPE fm_disk_free_bytes gauge\n\
                                 fm_disk_free_bytes {}\n\
                                 \n\
                                 # HELP fm_journal_bytes_written Total bytes written to journal\n\
                                 # TYPE fm_journal_bytes_written counter\n\
                                 fm_journal_bytes_written {}\n\
                                 \n\
                                 # HELP fm_match_minutes_simulated Total match-minutes simulated\n\
                                 # TYPE fm_match_minutes_simulated counter\n\
                                 fm_match_minutes_simulated {}\n",
                                metrics.active_matches.load(Ordering::Relaxed),
                                metrics.command_rate_total.load(Ordering::Relaxed),
                                metrics.command_rate_1m.load(Ordering::Relaxed),
                                metrics.queue_depth.load(Ordering::Relaxed),
                                metrics.last_snapshot_time.load(Ordering::Relaxed),
                                metrics.disk_free_bytes.load(Ordering::Relaxed),
                                metrics.journal_bytes_written.load(Ordering::Relaxed),
                                metrics.match_minutes_simulated.load(Ordering::Relaxed),
                            )
                        } else {
                            "404 Not Found\n".to_string()
                        };

                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = stream.write_all(response.as_bytes());
                    }
                }
                Err(e) => eprintln!("Metrics connection error: {e}"),
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_new() {
        let metrics = Metrics::new();
        assert_eq!(metrics.active_matches.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.command_rate_total.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_metrics_update() {
        let metrics = Metrics::new();
        metrics.active_matches.store(42, Ordering::Relaxed);
        metrics.command_rate_total.store(100, Ordering::Relaxed);
        assert_eq!(metrics.active_matches.load(Ordering::Relaxed), 42);
        assert_eq!(metrics.command_rate_total.load(Ordering::Relaxed), 100);
    }
}
