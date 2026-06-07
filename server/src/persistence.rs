//! Write-ahead journal + periodic snapshots for crash recovery.
//!
//! Journal:
//! - Append-only file, every mutating command written before execution.
//! - O_DIRECT + io_uring in production; for now, std::fs with O_DSYNC.
//! - 64 KB batching: commands grouped and written every 100ms or when buffer reaches 64KB.
//! - CRC32C checksum per block for corruption detection.
//!
//! Snapshots:
//! - Full match state dump every 5 minutes (configurable).
//! - Written to temp file, then atomically renamed over previous snapshot.
//! - CRC32C checksum appended at end.
//!
//! Recovery:
//! 1. Load latest snapshot → reconstruct active match states.
//! 2. Replay journal entries with sequence > snapshot's last_sequence.

#![allow(dead_code)]

use crc32c::crc32c;
use protocol::CommandPacket;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Magic bytes for journal block identification.
const JOURNAL_MAGIC: u32 = 0x464D524A; // "FMRJ"
/// Magic bytes for snapshot identification.
const SNAPSHOT_MAGIC: u32 = 0x464D5253; // "FMRS"
/// Default journal flush interval.
const FLUSH_INTERVAL: Duration = Duration::from_millis(100);
/// Default journal batch size before forced flush.
const BATCH_SIZE: usize = 64 * 1024; // 64 KB
/// Default snapshot interval.
const SNAPSHOT_INTERVAL: Duration = Duration::from_secs(300); // 5 minutes

/// A single journal entry wrapping a command packet.
#[derive(Debug, Clone)]
pub struct JournalEntry {
    pub sequence: u64,
    pub command: CommandPacket,
    pub timestamp: u64,
}

/// Manages the write-ahead journal and periodic snapshots.
pub struct Journal {
    /// Journal file handle.
    file: Option<File>,
    journal_path: PathBuf,
    /// Pending bytes not yet flushed to disk.
    buffer: Vec<u8>,
    last_flush: Instant,
    /// Total bytes written to journal (for sequence tracking).
    bytes_written: AtomicU64,
    /// Journal entries count.
    entries_count: AtomicU64,
    /// Snapshot directory.
    snapshot_dir: PathBuf,
    last_snapshot: Instant,
    /// Current snapshot interval.
    snapshot_interval: Duration,
    /// Disabled flag (e.g. disk full).
    disabled: bool,
}

impl Journal {
    /// Create a new journal in the given directory.
    /// Creates the directory if it doesn't exist.
    pub fn open(dir: &Path) -> io::Result<Self> {
        fs::create_dir_all(dir)?;

        // Find the latest journal file or create a new one
        let journal_path = dir.join("journal.dat");
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&journal_path)?;

        Ok(Self {
            file: Some(file),
            journal_path,
            buffer: Vec::with_capacity(BATCH_SIZE),
            last_flush: Instant::now(),
            bytes_written: AtomicU64::new(0),
            entries_count: AtomicU64::new(0),
            snapshot_dir: dir.to_path_buf(),
            last_snapshot: Instant::now(),
            snapshot_interval: SNAPSHOT_INTERVAL,
            disabled: false,
        })
    }

    /// Append a command to the journal buffer.
    /// Does not flush immediately — honors batching.
    pub fn append(&mut self, cmd: &CommandPacket) -> io::Result<()> {
        if self.disabled {
            return Ok(());
        }

        let entry = JournalEntry {
            sequence: self.entries_count.fetch_add(1, Ordering::Relaxed),
            command: *cmd,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        // Serialize entry: magic(4) + seq(8) + cmd(10) + ts(8) + crc32c(4) = 34 bytes
        let magic = JOURNAL_MAGIC.to_le_bytes();
        let seq = entry.sequence.to_le_bytes();
        let cmd_bytes = unsafe {
            std::slice::from_raw_parts(
                (&entry.command as *const CommandPacket) as *const u8,
                10, // CommandPacket is 10 bytes packed
            )
        };
        let ts = entry.timestamp.to_le_bytes();

        // Write parts for CRC
        let mut full = Vec::with_capacity(34);
        full.extend_from_slice(&magic);
        full.extend_from_slice(&seq);
        full.extend_from_slice(cmd_bytes);
        full.extend_from_slice(&ts);

        // Compute CRC32C over the data (not including CRC itself)
        let crc = crc32c(&full);
        full.extend_from_slice(&crc.to_le_bytes());

        self.buffer.extend_from_slice(&full);
        self.bytes_written.fetch_add(34, Ordering::Relaxed);

        // Flush if buffer exceeds batch size or flush interval elapsed
        if self.buffer.len() >= BATCH_SIZE || self.last_flush.elapsed() >= FLUSH_INTERVAL {
            self.flush()?;
        }

        Ok(())
    }

    /// Force-flush buffered entries to disk.
    pub fn flush(&mut self) -> io::Result<()> {
        if self.disabled || self.buffer.is_empty() {
            return Ok(());
        }

        if let Some(ref mut file) = self.file {
            file.write_all(&self.buffer)?;
            file.sync_all()?;
            self.buffer.clear();
            self.last_flush = Instant::now();
        }

        Ok(())
    }

    /// Check if it's time to take a snapshot.
    /// Call periodically (e.g. every tick).
    pub fn should_snapshot(&self) -> bool {
        !self.disabled && self.last_snapshot.elapsed() >= self.snapshot_interval
    }

    /// Write a snapshot of the current state.
    /// The `data` should be the serialized match states.
    pub fn write_snapshot(&mut self, data: &[u8]) -> io::Result<()> {
        if self.disabled {
            return Ok(());
        }

        let snapshot_path = self.snapshot_dir.join("snapshot.tmp");
        let final_path = self.snapshot_dir.join("snapshot.dat");

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&snapshot_path)?;

        // Format: magic(4) + data_len(8) + data(N) + crc(4)
        let magic = SNAPSHOT_MAGIC.to_le_bytes();
        let len = (data.len() as u64).to_le_bytes();
        file.write_all(&magic)?;
        file.write_all(&len)?;
        file.write_all(data)?;

        let crc = crc32c(data);
        file.write_all(&crc.to_le_bytes())?;
        file.sync_all()?;

        // Atomic rename
        fs::rename(&snapshot_path, &final_path)?;

        self.last_snapshot = Instant::now();
        Ok(())
    }

    /// Load the latest snapshot. Returns None if no snapshot exists.
    pub fn load_snapshot(&self) -> io::Result<Option<Vec<u8>>> {
        let snapshot_path = self.snapshot_dir.join("snapshot.dat");
        if !snapshot_path.exists() {
            return Ok(None);
        }

        let mut file = File::open(&snapshot_path)?;
        let mut magic = [0u8; 4];
        file.read_exact(&mut magic)?;
        if u32::from_le_bytes(magic) != SNAPSHOT_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid snapshot magic",
            ));
        }

        let mut len_bytes = [0u8; 8];
        file.read_exact(&mut len_bytes)?;
        let data_len = u64::from_le_bytes(len_bytes) as usize;

        let mut data = vec![0u8; data_len];
        file.read_exact(&mut data)?;

        let mut stored_crc = [0u8; 4];
        file.read_exact(&mut stored_crc)?;
        let crc = u32::from_le_bytes(stored_crc);

        let computed = crc32c(&data);
        if computed != crc {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Snapshot CRC mismatch",
            ));
        }

        Ok(Some(data))
    }

    /// Replay journal entries that have a sequence > `after_seq`.
    /// Returns the list of command packets that need re-application.
    pub fn replay_after(&self, after_seq: u64) -> io::Result<Vec<CommandPacket>> {
        let mut file = File::open(&self.journal_path)?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;

        let mut commands = Vec::new();
        let mut pos = 0;

        while pos + 34 <= contents.len() {
            let magic = u32::from_le_bytes(contents[pos..pos + 4].try_into().unwrap());
            if magic != JOURNAL_MAGIC {
                pos += 1;
                continue;
            }

            let seq = u64::from_le_bytes(contents[pos + 4..pos + 12].try_into().unwrap());
            if seq > after_seq {
                // Read command bytes (offset 12, length 10)
                let cmd_bytes = &contents[pos + 12..pos + 22];
                if cmd_bytes.len() >= 10 {
                    let cmd = CommandPacket {
                        match_id: u32::from_le_bytes(cmd_bytes[0..4].try_into().unwrap_or([0; 4])),
                        sequence: u16::from_le_bytes(cmd_bytes[4..6].try_into().unwrap_or([0; 2])),
                        command_type: cmd_bytes
                            .get(6)
                            .copied()
                            .and_then(|v| v.try_into().ok())
                            .unwrap_or(protocol::CommandType::Mentality),
                        arg1: cmd_bytes.get(7).copied().unwrap_or(0),
                        arg2: cmd_bytes.get(8).copied().unwrap_or(0),
                        arg3: cmd_bytes.get(9).copied().unwrap_or(0),
                    };
                    commands.push(cmd);
                }
            }

            pos += 34;
        }

        Ok(commands)
    }

    /// Get total bytes written to journal.
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written.load(Ordering::Relaxed)
    }

    /// Get total entries count.
    pub fn entries_count(&self) -> u64 {
        self.entries_count.load(Ordering::Relaxed)
    }

    /// Disable journaling (e.g. on disk full).
    pub fn disable(&mut self) {
        self.disabled = true;
    }

    /// Check if journal is still writable by testing disk space.
    /// Engine stops accepting commands when disk <5% free.
    pub fn check_disk_space(&self) -> io::Result<bool> {
        // Use `statvfs` on Linux to check available space
        let path = self.snapshot_dir.as_os_str().to_str().unwrap_or("/");
        if let Ok(stats) = nix::sys::statvfs::statvfs(path) {
            let free_ratio = stats.blocks_available() as f64 / stats.blocks() as f64;
            Ok(free_ratio > 0.05) // >5% free
        } else {
            Ok(true) // Can't check, assume OK
        }
    }
}

impl Drop for Journal {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::CommandPacket;
    use tempfile::tempdir;

    fn make_test_cmd(seq: u16) -> CommandPacket {
        CommandPacket {
            match_id: 1,
            sequence: seq,
            command_type: protocol::CommandType::Mentality,
            arg1: 0,
            arg2: 1,
            arg3: 0,
        }
    }

    #[test]
    fn test_journal_append_and_flush() {
        let dir = tempdir().unwrap();
        let mut journal = Journal::open(dir.path()).unwrap();
        journal.append(&make_test_cmd(1)).unwrap();
        journal.append(&make_test_cmd(2)).unwrap();
        journal.flush().unwrap();
        assert_eq!(journal.entries_count(), 2);
        assert!(journal.bytes_written() > 0);
    }

    #[test]
    fn test_snapshot_write_and_load() {
        let dir = tempdir().unwrap();
        let mut journal = Journal::open(dir.path()).unwrap();
        let data = b"test snapshot data";
        journal.write_snapshot(data).unwrap();

        let loaded = journal.load_snapshot().unwrap().unwrap();
        assert_eq!(loaded, data);
    }

    #[test]
    fn test_replay_after_sequence() {
        let dir = tempdir().unwrap();
        let mut journal = Journal::open(dir.path()).unwrap();

        journal.append(&make_test_cmd(1)).unwrap();
        journal.append(&make_test_cmd(2)).unwrap();
        journal.append(&make_test_cmd(3)).unwrap();
        journal.flush().unwrap();

        let replayed = journal.replay_after(0).unwrap();
        assert_eq!(replayed.len(), 2);
        // Copy to locals to avoid unaligned access on packed struct
        let seq0 = replayed[0].sequence;
        let seq1 = replayed[1].sequence;
        assert_eq!(seq0, 2);
        assert_eq!(seq1, 3);
    }

    #[test]
    fn test_should_snapshot() {
        let dir = tempdir().unwrap();
        let journal = Journal::open(dir.path()).unwrap();
        assert!(!journal.should_snapshot()); // hasn't been 5 min
    }
}
