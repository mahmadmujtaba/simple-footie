//! Token generation and validation for match authentication.
//!
//! Uses `getrandom` for 128-bit cryptographically random tokens.
//! Tokens are generated on match creation and sent to the client once.
//! The server caches them and validates every incoming command.

use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Manages match tokens and rate-limiting state.
pub struct TokenManager {
    /// `match_id` -> 16-byte token
    tokens: DashMap<u32, [u8; 16]>,
    /// `match_id` -> tick count for rate limiting (simple counter)
    rate_limit: DashMap<u32, u8>,
    /// Global rate limit ticker
    ticker: AtomicU64,
}

impl TokenManager {
    pub fn new() -> Self {
        Self {
            tokens: DashMap::new(),
            rate_limit: DashMap::new(),
            ticker: AtomicU64::new(0),
        }
    }

    /// Generate a new 128-bit random token for a match.
    pub fn create_token(&self, match_id: u32) -> [u8; 16] {
        let mut token = [0u8; 16];
        getrandom::getrandom(&mut token).expect("getrandom failed");
        self.tokens.insert(match_id, token);
        token
    }

    /// Validate a token for a given match_id.
    /// Returns true if the token matches.
    pub fn validate_token(&self, match_id: u32, token: &[u8; 16]) -> bool {
        self.tokens
            .get(&match_id)
            .map(|stored| *stored == *token)
            .unwrap_or(false)
    }

    /// Check rate limit: max 2 commands per match per tick.
    /// Returns true if the command is allowed.
    pub fn check_rate_limit(&self, match_id: u32) -> bool {
        let mut entry = self.rate_limit.entry(match_id).or_insert(0);
        if *entry >= 2 {
            false
        } else {
            *entry += 1;
            true
        }
    }

    /// Called every second to reset rate limit counters.
    pub fn tick(&self) {
        self.ticker.fetch_add(1, Ordering::Relaxed);
        self.rate_limit.clear();
    }

    /// Remove a match's token (e.g. on match end).
    pub fn remove(&self, match_id: u32) {
        self.tokens.remove(&match_id);
        self.rate_limit.remove(&match_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_validate_token() {
        let mgr = TokenManager::new();
        let token = mgr.create_token(1);
        assert!(mgr.validate_token(1, &token));
        assert!(!mgr.validate_token(1, &[0u8; 16]));
    }

    #[test]
    fn test_rate_limit() {
        let mgr = TokenManager::new();
        assert!(mgr.check_rate_limit(1));
        assert!(mgr.check_rate_limit(1));
        assert!(!mgr.check_rate_limit(1)); // third denied

        mgr.tick(); // reset
        assert!(mgr.check_rate_limit(1)); // allowed again
    }

    #[test]
    fn test_remove_token() {
        let mgr = TokenManager::new();
        let token = mgr.create_token(1);
        assert!(mgr.validate_token(1, &token));
        mgr.remove(1);
        assert!(!mgr.validate_token(1, &token));
    }
}
